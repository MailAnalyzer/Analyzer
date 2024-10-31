mod auth_checker;
mod link_checker;
mod nlp_checker;

use crate::analysis::auth_checker::AuthAnalyzer;
use crate::analysis::link_checker::LinkAnalyzer;
use crate::analysis::nlp_checker::NLPChecker;
use crate::command::AnalysisCommand;
use crate::email::OwnedEmail;
use mail_parser::{Address, Message};
use rand::random;
use rocket::serde::json::serde_json;
use serde::Serialize;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::OnceCell;

pub static ANALYZERS: OnceCell<Vec<Arc<dyn MailAnalyzer>>> = OnceCell::const_new();

pub fn init_analyzers() {
    let analyzers: Vec<Arc<dyn MailAnalyzer>> = vec![
        Arc::new(LinkAnalyzer),
        Arc::new(AuthAnalyzer),
        Arc::new(NLPChecker),
    ];
    if ANALYZERS.set(analyzers).is_err() {
        panic!("analyzers should not be already initialized")
    };
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    id: usize,
    pub analysis_name: String,
    pub verdict: AnalysisVerdict,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisVerdict {
    kind: String,
    value: serde_json::Value,
}

impl AnalysisVerdict {
    pub fn new<V: Serialize>(kind: &str, value: &V) -> Self {
        let value = serde_json::to_value(value).unwrap();
        Self {
            kind: kind.to_string(),
            value,
        }
    }

    pub fn error<V: Serialize>(value: &V) -> Self {
        Self::new("error", value)
    }
}

pub struct AnalysisSetup {
    pub expected_verdict_count: usize,
}

impl AnalysisResult {
    pub fn new(analysis_name: String, verdict: AnalysisVerdict) -> Self {
        let id = random();
        Self {
            id,
            analysis_name,
            verdict,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum JobEvent {
    Error(String),
    ExpandedResultCount(usize),
    Progress(AnalysisResult),
    AnalysisDone(String),
    JobComplete,
}

pub trait MailAnalyzer: Send + Sync {
    fn name(&self) -> String;
    fn analyze(&self, email: OwnedEmail, command: AnalysisCommand) -> AnalysisSetup;
}

pub async fn start_email_analysis(
    email_string: &str,
    analyzers: Vec<Arc<dyn MailAnalyzer>>,
    events_publisher: Arc<Sender<JobEvent>>,
) {
    let mut total_expected_verdict_count = 0;
    for analyzer in analyzers {
        let email_string = String::from(email_string);

        let command = AnalysisCommand::new(analyzer.name(), events_publisher.clone());
        println!("Launched {}", analyzer.name());
        let setup = analyzer.analyze(OwnedEmail::new(email_string), command);

        total_expected_verdict_count += setup.expected_verdict_count;
    }

    events_publisher
        .send(JobEvent::ExpandedResultCount(total_expected_verdict_count))
        .unwrap();

    //FIXME workaround for app's desynchronisation after a job is submitted
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}

fn extract_sender_address(mail: Message) -> String {
    let Address::List(senders) = mail.sender().expect("email should contain a valid sender") else {
        unreachable!("a sender can only be a person")
    };

    if senders.len() > 1 {
        panic!("email has multiple senders")
    }

    let sender = senders
        .first()
        .expect("email should contain at least one sender");
    let sender_mail_address = sender
        .address()
        .expect("sender address should contain an email address");
    sender_mail_address.to_string()
}

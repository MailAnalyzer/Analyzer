mod auth_checker;
mod link_checker;
mod mock_analyzer;
mod nlp_checker;

use crate::analysis::auth_checker::AuthAnalyzer;
use crate::analysis::link_checker::LinkAnalyzer;
use crate::analysis::mock_analyzer::MockAnalyzer;
use mail_parser::{Address, Message, MessageParser};
use rand::{random, Rng};
use rocket::serde::json::serde_json;
use serde::Serialize;
use std::future::Future;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::OnceCell;

pub static ANALYZERS: OnceCell<Vec<Arc<dyn MailAnalyzer>>> = OnceCell::const_new();

pub fn init_analyzers() {
    let analyzers: Vec<Arc<dyn MailAnalyzer>> = vec![
        Arc::new(LinkAnalyzer),
        Arc::new(AuthAnalyzer),
        Arc::new(MockAnalyzer),
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

pub struct AnalysisCommand {
    analysis_name: String,
    sender: Arc<Sender<JobEvent>>,
    last_expected_result: AtomicUsize,
}

impl Drop for AnalysisCommand {
    fn drop(&mut self) {
        println!("Analysis Command {} dropped !", self.analysis_name);
        self.sender
            .send(JobEvent::AnalysisDone(self.analysis_name.clone()))
            .unwrap();
    }
}

impl AnalysisCommand {
    fn new(name: String, sender: Arc<Sender<JobEvent>>) -> Self {
        Self {
            analysis_name: name,
            sender,
            last_expected_result: AtomicUsize::default(),
        }
    }

    fn get_expected_result_count(&self) -> usize {
        self.last_expected_result.load(Ordering::Acquire)
    }

    fn result(&self, verdict: AnalysisVerdict) {
        let result = AnalysisResult::new(self.analysis_name.clone(), verdict);
        self.sender.send(JobEvent::Progress(result)).unwrap();
    }

    fn set_expected_results(&mut self, new_count: usize) {
        let diff = new_count - self.get_expected_result_count();
        self.last_expected_result.store(new_count, Ordering::Relaxed);
        self.sender
            .send(JobEvent::ExpandedResultCount(diff))
            .unwrap();
    }

    fn spawn(self: &Arc<Self>, task: impl Future<Output=AnalysisVerdict> + Send + 'static) {
        let arc = self.clone();
        tokio::spawn(async move {
            arc.result(task.await)
        });
        self.last_expected_result.fetch_add(1, Ordering::Acquire);
    }

    fn gen_setup(&self) -> AnalysisSetup {
        AnalysisSetup {
            expected_verdict_count: self.last_expected_result.load(Ordering::Acquire)
        }
    }
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
    JobComplete
}

pub trait MailAnalyzer: Send + Sync {
    fn name(&self) -> String;
    fn analyze(&self, email: Message, command: AnalysisCommand) -> AnalysisSetup;
}

pub async fn start_email_analysis(
    email_string: &str,
    analyzers: Vec<Arc<dyn MailAnalyzer>>,
    events_publisher: Arc<Sender<JobEvent>>,
) {
    let mut total_expected_verdict_count = 0;
    for analyzer in analyzers {
        let email_string = String::from(email_string);

        let email = MessageParser::new()
            .parse(&email_string)
            .expect("valid email");

        let command = AnalysisCommand::new(analyzer.name(), events_publisher.clone());
        println!("Launched {}", command.analysis_name);
        let setup = analyzer.analyze(email, command);

        total_expected_verdict_count += setup.expected_verdict_count;
    }

    events_publisher.send(JobEvent::ExpandedResultCount(total_expected_verdict_count)).unwrap();

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

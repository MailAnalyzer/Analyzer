mod auth_checker;
mod entity_checker;
mod link_checker;
mod nlp_checker;

use crate::analysis::auth_checker::AuthAnalyzer;
use crate::analysis::entity_checker::EntityChecker;
use crate::analysis::link_checker::LinkAnalyzer;
use crate::analysis::nlp_checker::NLPChecker;
use crate::command::AnalysisCommand;
use crate::email::OwnedEmail;
use mail_parser::{Address, Message};
use rand::random;
use rocket::serde::json::serde_json;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::OnceCell;
use crate::job::Job;

pub static ANALYZERS: OnceCell<Vec<Arc<dyn MailAnalyzer>>> = OnceCell::const_new();

pub fn init_analyzers() {
    let analyzers: Vec<Arc<dyn MailAnalyzer>> = vec![
        Arc::new(EntityChecker),
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
    pub kind: String,
    pub value: serde_json::Value,
}

impl AnalysisVerdict {
    pub fn new<V: Serialize>(kind: &str, value: V) -> Self {
        let value = serde_json::to_value(value).unwrap();
        Self {
            kind: kind.to_string(),
            value,
        }
    }

    pub fn error<V: Serialize>(value: V) -> Self {
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
    ExpandedResultCount(usize),
    Progress(AnalysisResult),
    AnalysisDone(String),

    //currently unuseds
    Error(String),
    JobComplete,
}

impl JobEvent {
    pub fn is_closing_action(&self) -> bool {
        matches!(self, JobEvent::Error(_) | JobEvent::JobComplete)
    }
}

pub trait MailAnalyzer: Send + Sync {
    fn name(&self) -> String;
    fn analyze(&self, email: OwnedEmail, command: AnalysisCommand) -> AnalysisSetup;
}

pub async fn start_email_analysis(
    analyzers: Vec<Arc<dyn MailAnalyzer>>,
    job: Arc<Job>,
) {
    let mut total_expected_verdict_count = 0;
    for analyzer in analyzers {
        let email_string = job.email.clone();

        let command = AnalysisCommand::new(analyzer.name(), job.clone());
        println!("Launched {}", analyzer.name());
        let setup = analyzer.analyze(OwnedEmail::new(email_string), command);

        total_expected_verdict_count += setup.expected_verdict_count;
    }

    job
        .event_channel
        .send(JobEvent::ExpandedResultCount(total_expected_verdict_count))
        .unwrap();

    //FIXME workaround for app's desynchronisation after a job is submitted
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}


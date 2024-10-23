pub mod email_behavior_checker;
mod link_checker;
mod mock_analyzer;
mod snow_personna_checker;
mod auth_checker;

use crate::analysis::link_checker::LinkAnalyzer;
use crate::analysis::mock_analyzer::MockAnalyzer;
use mail_parser::{Address, Message, MessageParser};
use rand::{random, Rng};
use rocket::serde::json::serde_json;
use serde::Serialize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::OnceCell;
use tokio::task::JoinSet;
use crate::analysis::auth_checker::AuthAnalyzer;

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

pub struct Analysis {
    pub name: String,
    pub verdicts: Vec<Pin<Box<dyn Future<Output = AnalysisVerdict> + Send + Sync>>>,
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
    Done,
}

//TODO use async_trait
pub trait MailAnalyzer: Send + Sync {
    fn analyze(&self, email: Message) -> Analysis;
}

pub async fn handle_email(
    email_string: &str,
    analyzers: Vec<Arc<dyn MailAnalyzer>>,
    events_publisher: Arc<Sender<JobEvent>>,
) {
    let mut verdict_join_set: JoinSet<()> = JoinSet::new();
    let mut verdict_total_count: usize = 0;

    let analysis: Vec<_> = analyzers
        .iter()
        .map(|analyzer| {
            let email_string = String::from(email_string);

            let email = MessageParser::new()
                .parse(&email_string)
                .expect("valid email");
            let analysis = analyzer.analyze(email);

            verdict_total_count += analysis.verdicts.len();
            analysis
        })
        .collect();

    // workaround for desynchronisation after a submitted job
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    events_publisher
        .send(JobEvent::ExpandedResultCount(verdict_total_count))
        .unwrap();

    for analysis in analysis {
        for verdict in analysis.verdicts {
            let analysis_name = analysis.name.clone();
            let channel = events_publisher.clone();

            verdict_join_set.spawn(async move {
                let verdict = verdict.await;

                channel
                    .send(JobEvent::Progress(AnalysisResult::new(
                        analysis_name,
                        verdict,
                    )))
                    .unwrap();
            });
        }
    }

    verdict_join_set.join_all().await;
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

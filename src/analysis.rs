pub mod email_behavior_checker;
mod mock_analyzer;
mod snow_personna_checker;

use crate::analysis::email_behavior_checker::AddressBehaviorMailAnalyzer;
use crate::analysis::mock_analyzer::MockAnalyzer;
use mail_parser::{Address, Message, MessageParser};
use rand::{random, Rng};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::{Mutex, OnceCell};
use tokio::task::JoinSet;

pub static ANALYZERS: OnceCell<Vec<Arc<dyn MailAnalyzer>>> = OnceCell::const_new();

pub fn init_analyzers() {
    let analyzers: Vec<Arc<dyn MailAnalyzer>> = vec![
        Arc::new(AddressBehaviorMailAnalyzer::new(extract_sender_address)), //analyze the sender email
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
        Arc::new(MockAnalyzer),
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
    pub name: String,
    pub description: String,
    pub verdict_description: String,
    pub errors: Vec<String>,
}

impl AnalysisResult {
    pub fn new(
        name: String,
        description: String,
        verdict_description: String,
        errors: Vec<String>,
    ) -> Self {
        let id = random();
        Self {
            id,
            name,
            description,
            verdict_description,
            errors,
        }
    }
    
    pub fn id(&self) -> usize {
        self.id
    }
}



#[derive(Debug, Clone)]
pub enum JobEvent {
    Error(String),
    Progress(AnalysisResult),
    Done,
}


//TODO use async_trait
pub trait MailAnalyzer: Send + Sync {
    fn analyze(&self, email: Message) -> AnalysisResult;
}

pub async fn handle_email(
    email_string: &str,
    analyzers: Vec<Arc<dyn MailAnalyzer>>,
    events_publisher: Arc<Sender<JobEvent>>,
) {
    let mut task_set: JoinSet<()> = JoinSet::new();

    for analyzer in analyzers {
        let email_string = String::from(email_string);

        let publisher = events_publisher.clone();

        task_set.spawn(async move {
            let email = MessageParser::new().parse(&email_string).expect("valid");

            let result = analyzer.analyze(email);
            publisher
                .send(JobEvent::Progress(result))
                .expect("could publish analysis result");
        });
    }

    task_set.join_all().await;
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

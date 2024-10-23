use crate::analysis::{Analysis, MailAnalyzer};
use mail_parser::Message;
use rand::Rng;
use rand_derive::Rand;
use std::time::Duration;

/// From Splunk logs, get emails related to a given email address
/// and try to determine if it behaves like a bot or a legit person
/// (that have discussions with other members)

#[derive(Debug, Rand)]
pub enum BehaviorVerdict {
    Legit,
    Bot,
}

pub async fn check_email_behavior(email_address: &str) -> Result<BehaviorVerdict, String> {
    let mut rng = rand::thread_rng();
    // Simulate high computational task
    tokio::time::sleep(Duration::from_secs(15)).await;
    Ok(rng.gen())
}

pub struct AddressBehaviorMailAnalyzer<F>
where
    F: Fn(Message) -> String + Sync + Send,
{
    address: F,
}

impl<F> MailAnalyzer for AddressBehaviorMailAnalyzer<F>
where
    F: Fn(Message) -> String + Sync + Send,
{
    fn analyze(&self, email: Message) -> Analysis {
        // Analysis {
        //     name: "Email Analyzer".to_string(),
        //     analyzed_elements: vec![],
        // };
        todo!();
    }
}

impl<F> AddressBehaviorMailAnalyzer<F>
where
    F: Fn(Message) -> String + Sync + Send,
{
    pub(crate) fn new(f: F) -> Self {
        Self { address: f }
    }
}

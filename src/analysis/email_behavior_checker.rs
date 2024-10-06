use crate::analysis::{AnalysisResult, MailAnalyzer};
use mail_parser::Message;
use rand::Rng;
use rand_derive::Rand;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::task;

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
    fn analyze(&self, email: Message) -> AnalysisResult {
        let mail_address = (self.address)(email);

        task::block_in_place(|| {
            Handle::current().block_on(async {
                let result = check_email_behavior(&mail_address).await;

                let name = "Email Behavior Analysis".to_string();
                let description = format!("Behavior analysis results of email `{mail_address}`");

                match result {
                    Ok(verdict) => {
                        AnalysisResult::new(name, description, format!("{verdict:?}"), vec![])
                    }
                    Err(err) => {
                        AnalysisResult::new(name, description, "Error".to_string(), vec![err])
                    }
                }
            })
        })
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

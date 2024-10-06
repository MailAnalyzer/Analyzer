use crate::analysis::{AnalysisResult, MailAnalyzer};
use mail_parser::Message;
use rand::{thread_rng, Rng};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::task;

pub struct MockAnalyzer;

impl MailAnalyzer for MockAnalyzer {
    fn analyze(&self, email: Message) -> AnalysisResult {
        task::block_in_place(|| {
            Handle::current().block_on(async {
                let mut rng = thread_rng();

                tokio::time::sleep(Duration::from_secs(rng.gen_range(0..20))).await;

                let nb_errors = rng.gen_range(0..5);

                let mut errors = Vec::with_capacity(nb_errors);
                for i in 0..nb_errors {
                    errors.push(format!("Error {i}"));
                }

                AnalysisResult::new(
                    "Mock analysis result".to_string(),
                    "A generated analysis with no particular meaning. For test purposes.".to_string(),
                    "No verdict".to_string(),
                    errors,
                )
            })
        })
    }
}

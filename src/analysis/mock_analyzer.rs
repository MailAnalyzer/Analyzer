use std::future::Future;
use std::pin::Pin;
use crate::analysis::{Analysis, AnalysisVerdict, MailAnalyzer};
use mail_parser::Message;
use rand::{thread_rng, Rng};
use std::time::Duration;

pub struct MockAnalyzer;

impl MailAnalyzer for MockAnalyzer {
    fn analyze(&self, email: Message) -> Analysis {
        let task: Pin<Box<dyn Future<Output = AnalysisVerdict> + Sync + Send>> = Box::pin(async {
            let range = {
                let mut rng = thread_rng();
                rng.gen_range(0..20)
            };
            tokio::time::sleep(Duration::from_secs(range)).await;

            let mut rng = thread_rng();
            
            let nb_errors = rng.gen_range(0..5);

            let mut errors = Vec::with_capacity(nb_errors);
            for i in 0..nb_errors {
                errors.push(format!("Error {i}"));
            }

            if errors.is_empty() {
                AnalysisVerdict::Completed("Mock".to_string())
            } else {
                AnalysisVerdict::Error(errors)
            }
        });

        Analysis {
            name: "Mock analysis".to_string(),
            verdicts: vec![Box::pin(task)],
        }
    }
}

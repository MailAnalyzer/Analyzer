use std::sync::Arc;
use crate::analysis::{AnalysisCommand, AnalysisSetup, AnalysisVerdict, MailAnalyzer};
use mail_parser::Message;
use rand::{thread_rng, Rng};
use std::time::Duration;

pub struct MockAnalyzer;

impl MailAnalyzer for MockAnalyzer {
    fn name(&self) -> String {
        String::from("Mock")
    }

    fn analyze(&self, email: Message, mut command: AnalysisCommand) -> AnalysisSetup {
        let range = {
            let mut rng = thread_rng();
            rng.gen_range(0..20)
        };

        let command = Arc::new(command);

        command.spawn(async move {
            tokio::time::sleep(Duration::from_secs(range)).await;

            let mut rng = thread_rng();

            let nb_errors = rng.gen_range(0..5);

            let mut errors = Vec::with_capacity(nb_errors);
            for i in 0..nb_errors {
                errors.push(format!("Error {i}"));
            }

            if errors.is_empty() {
                AnalysisVerdict::new("mock", &"Mock")
            } else {
                AnalysisVerdict::error(&errors)
            }

        });

        command.gen_setup()
    }
}

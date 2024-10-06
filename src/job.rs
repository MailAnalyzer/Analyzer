use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::{Mutex};
use tokio::sync::broadcast::{Receiver, Sender};
use mail_parser::{Message, MessageParser};
use rocket::serde::Serialize;
use crate::analysis::{AnalysisResult, JobEvent};

pub struct Job {
    pub email: String,
    pub state: Mutex<JobState>,
    pub results: Mutex<Vec<AnalysisResult>>,
    pub expected_result_count: usize,
    pub id: usize,
    pub(crate) event_channel: Arc<Sender<JobEvent>>,
}

pub enum JobState {
    Analyzing,
    Error(String),
    Analyzed,
}

impl Job {
    pub(crate) fn new(
        email: String,
        id: usize,
        expected_result_count: usize,
        event_channel: Sender<JobEvent>,
    ) -> Self {
        Self {
            email,
            state: Mutex::new(JobState::Analyzing),
            results: Mutex::new(Vec::with_capacity(expected_result_count)),
            event_channel: Arc::new(event_channel),
            expected_result_count,
            id,
        }
    }

    pub fn email(&self) -> Message {
        MessageParser::new().parse(&self.email).unwrap()
    }

    pub fn subscribe_events(&self) -> Receiver<JobEvent> {
        self.event_channel.subscribe()
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JobDescription {
    subject: String,
    target_result_count: usize,
    error: Option<String>,
    id: usize,
    results: Vec<AnalysisResult>,
}

impl JobDescription {
    pub async fn from_job(job: &Job) -> Self {
        let error = if let JobState::Error(s) = job.state.lock().await.deref() {
            Some(s.clone())
        } else {
            None
        };

        let current_results = job.results.lock().await;

        JobDescription {
            subject: job
                .email()
                .subject()
                .map_or(String::default(), ToOwned::to_owned),
            id: job.id,
            error,
            results: current_results.clone(),
            target_result_count: job.expected_result_count,
        }
    }
}
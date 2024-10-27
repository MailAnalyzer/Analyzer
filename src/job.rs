use crate::analysis::{AnalysisResult, JobEvent};
use mail_parser::{Message, MessageParser};
use rocket::serde::Serialize;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::Mutex;

pub enum JobState {
    Analyzing,
    Error(String),
    Analyzed,
}

pub struct Job {
    pub email: String,
    pub state: Mutex<JobState>,
    pub results: Mutex<Vec<AnalysisResult>>,
    pub expected_result_count: AtomicI32,
    pub id: usize,
    pub(crate) event_channel: Arc<Sender<JobEvent>>,
    is_complete: AtomicBool,
}

impl Job {
    pub(crate) fn new(email: String, id: usize, event_channel: Sender<JobEvent>) -> Self {
        Self {
            email,
            state: Mutex::new(JobState::Analyzing),
            results: Mutex::new(Vec::new()),
            event_channel: Arc::new(event_channel),
            expected_result_count: AtomicI32::new(-1),
            is_complete: AtomicBool::new(false),
            id,
        }
    }

    pub fn email(&self) -> Message {
        MessageParser::new().parse(&self.email).unwrap()
    }

    pub fn subscribe_events(&self) -> Receiver<JobEvent> {
        self.event_channel.subscribe()
    }
    
    pub fn is_complete(&self) -> bool {
        self.is_complete.load(Ordering::Acquire)
    }
    
    pub fn mark_as_complete(&self) {
        self.is_complete.store(true, Ordering::Release);
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JobDescription {
    subject: String,
    target_result_count: Option<usize>,
    error: Option<String>,
    id: usize,
    results: Vec<AnalysisResult>,
    is_complete: bool
}

impl JobDescription {
    pub async fn from_job(job: &Job) -> Self {
        let error = if let JobState::Error(s) = job.state.lock().await.deref() {
            Some(s.clone())
        } else {
            None
        };

        let current_results = job.results.lock().await;

        let result_count = job.expected_result_count.load(Ordering::Acquire);

        JobDescription {
            subject: job
                .email()
                .subject()
                .map_or(String::default(), ToOwned::to_owned),
            id: job.id,
            error,
            results: current_results.clone(),
            target_result_count: if result_count == -1 {
                None
            } else {
                Some(result_count as usize)
            },
            is_complete: job.is_complete.load(Ordering::Acquire),
        }
    }
}

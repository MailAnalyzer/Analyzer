use crate::JobDescription;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::Mutex;
use crate::job::Job;

pub struct ServerState {
    pub(crate) jobs: Arc<Mutex<Jobs>>,
}

pub struct Jobs {
    //TODO Arc here might be removable
    jobs: Vec<Arc<Job>>,
    total_jobs_count: usize,
    event_channel: Sender<ServerStateEvent>,
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ServerStateEvent {
    NewJob(JobDescription),
}

impl Jobs {
    pub fn new() -> Self {
        Self {
            jobs: vec![],
            total_jobs_count: 0,
            event_channel: tokio::sync::broadcast::channel::<ServerStateEvent>(100).0,
        }
    }

    pub fn iter_jobs(&self) -> impl Iterator<Item = &Arc<Job>> {
        self.jobs.iter()
    }

    pub async fn add_job(&mut self, email_content: String) -> Arc<Job> {
        self.total_jobs_count += 1;

        let job_id = self.total_jobs_count;

        let (sx, _) = tokio::sync::broadcast::channel(100);

        let job = Arc::new(Job::new(email_content, job_id, sx));

        self.jobs.push(job.clone());
        
        self.event_channel
            .send(ServerStateEvent::NewJob(JobDescription::from_job(&job).await))
            .expect("could not send job add event");

        job
    }

    pub fn complete_job(&mut self, job_id: usize) {
        self.jobs.retain(|j| j.id != job_id)
    }

    pub fn find_job(&self, job_id: usize) -> Option<Arc<Job>> {
        self.jobs.iter().find(|j| j.id == job_id).cloned()
    }

    pub fn subscribe_events(&self) -> Receiver<ServerStateEvent> {
        self.event_channel.subscribe()
    }
}

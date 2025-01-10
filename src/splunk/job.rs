use crate::splunk::{SplunkClient, SplunkError};
use chrono::{DateTime, Utc};
use enum_assoc::Assoc;
use rocket::async_stream::stream;
use serde::Deserialize;
use serde_json::Value;
use std::ops::Range;
use std::sync::Arc;
use tokio_stream::Stream;

pub struct JobDescription {
    pub search: String,
    pub level: SearchLevel,
    pub time_range: Range<DateTime<Utc>>,
}

#[derive(Assoc)]
#[func(pub const fn name(&self) -> &'static str)]
pub enum SearchLevel {
    #[assoc(name = "verbose")]
    Verbose,
    #[assoc(name = "smart")]
    Smart,
    #[assoc(name = "fast")]
    Fast,
}

pub struct Job<C: SplunkClient> {
    pub(crate) client: Arc<C>,
    pub(crate) sid: String,
}

#[derive(Debug, Clone)]
pub struct Event {
    value: String,
}

struct JobProgress {
    is_complete: bool,
    event_count: u32,
}

impl<C: SplunkClient> Job<C> {
    pub async fn poll_events(&self) -> Result<impl Stream<Item=Result<Event, SplunkError>> + '_, SplunkError> {
        let mut event_count = 0;

        let stream = stream! {
            loop {

                let progress = self.get_job_progress().await?;
                
                if progress.event_count != event_count {
                    event_count = progress.event_count;
                    
                    for event in self.poll_job_events().await? {
                        yield Ok(event)
                    }
                }
                
                if progress.is_complete {
                    break
                }
            }
        };

        Ok(stream)
    }

    async fn poll_job_events(&self, offset: u32) -> Result<Vec<Event>, SplunkError> {
        let response = self.client
            .get(&format!("/jobs/{}/events?output_mode=json&offset={offset}&count=0", self.sid))
            .send()
            .await?;
        
        let json: Value = response.json().await?;
    }
    
    async fn get_job_progress(&self) -> Result<JobProgress, SplunkError> {
        let response = self.client
            .get(&format!("/{}?output_mode=json", self.sid))
            .send()
            .await?;


        #[derive(Deserialize)]
        struct JobProgressDAO {
            entry: Vec<JobProgressEntryDAO>,
        }

        #[derive(Deserialize)]
        struct JobProgressEntryDAO {
            content: JobProgressContentDAO,
        }

        #[derive(Deserialize)]
        struct JobProgressContentDAO {
            event_count: u32,
            is_done: bool,
        }

        let dao: JobProgressDAO = response.json().await?;

        let entry_dao = dao.entry.first().unwrap();
        
        Ok(JobProgress {
            event_count: entry_dao.content.event_count,
            is_complete: entry_dao.content.is_done,
        })
    }
}


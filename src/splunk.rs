mod web_client;
mod job;

use crate::splunk::job::{Job, JobDescription};
use reqwest::RequestBuilder;
use std::sync::Arc;
use url::Url;

#[derive(Clone)]
pub struct Splunk<C: SplunkClient> {
    client: Arc<C>,
}


#[derive(Debug)]
pub enum SplunkError {
    Std(Box<dyn std::error::Error>),
    Message(String),
}

impl<E: std::error::Error + 'static> From<E> for SplunkError {
    fn from(value: E) -> Self {
        SplunkError::Std(Box::new(value))
    }
}

impl<C: SplunkClient> Splunk<C> {
    async fn job(&self, jq: JobDescription) -> Result<Job<C>, SplunkError> {
        let response = self.client
            .post("/jobs")
            .form(&vec![
                ("adhoc_search_level", jq.level.name().to_string()),
                ("earliest_time", jq.time_range.start.timestamp().to_string()),
                ("latest_time", jq.time_range.end.timestamp().to_string()),
                ("search", jq.search),
                ("output_mode", "json".to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SplunkError::Message(format!("Received response status {} : {}", response.status(), response.text().await.unwrap_or("<no content>".to_string()))));
        }

        let json: serde_json::Value = response.json().await?;

        if json.get("success").is_some_and(|z| !z.as_bool().unwrap()) {
            return Err(SplunkError::Message(format!("Received error from api: {json}")));
        }

        let sid = json.get("sid")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        Ok(Job {
            client: self.client.clone(),
            sid
        })
    }
}


pub trait SplunkClient {
    fn post(&self, url: &str) -> RequestBuilder;
    fn get(&self, url: &str) -> RequestBuilder;
    
}

pub struct SplunkClientConfig {
    endpoint: Url,
    portal: Url,
}


impl<C: SplunkClient> From<C> for Splunk<C> {
    fn from(client: C) -> Self {
        Self {
            client: Arc::new(client)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::splunk::job::{JobDescription, SearchLevel};
    use crate::splunk::web_client::WebClient;
    use crate::splunk::{Splunk, SplunkClientConfig};
    use chrono::{DateTime, TimeDelta};
    use rocket::async_test;
    use std::ops::Sub;
    use std::time::SystemTime;
    use url::Url;

    #[async_test]
    async fn test_initialisation() {
        let client = WebClient::new_via_web_portal(SplunkClientConfig {
            portal: Url::parse("https://soc-siem.eu.airbus.corp:8000/").unwrap(),
            endpoint: Url::parse("https://soc-siem.eu.airbus.corp:8000/en-US/splunkd/__raw/servicesNS/mbat3wm0/SplunkEnterpriseSecuritySuite/search/v2").unwrap(),
        }).await.unwrap();
        let splunk = Splunk::from(client);

        let job = splunk.job(JobDescription {
            search: String::from("search index=*netskope* dvc=APLF300000511"),
            time_range: DateTime::from(SystemTime::now()).sub(TimeDelta::minutes(10))..DateTime::from(SystemTime::now()),
            level: SearchLevel::Verbose,
        }).await.unwrap();

        println!("{}", job.sid)
    }
}
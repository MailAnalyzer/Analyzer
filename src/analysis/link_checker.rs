use std::collections::HashMap;
use crate::analysis::{Analysis, AnalysisVerdict, MailAnalyzer};
use mail_parser::Message;
use regex::bytes::Regex;
use std::future::Future;
use std::pin::Pin;
use reqwest::{Client, ClientBuilder};
use reqwest::header::HeaderMap;
use reqwest::multipart::Form;
use serde::{Deserialize, Serialize};
use url::Url;

pub struct LinkAnalyzer;

impl MailAnalyzer for LinkAnalyzer {
    fn analyze(&self, email: Message) -> Analysis {
        let regex = Regex::new(r"https?:\/\/(?:www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b(?:[-a-zA-Z0-9()@:%_\+.~#?&\/=]*)").unwrap();

        let mut tasks = Vec::new();

        let mut headers = HeaderMap::new();
        headers.append("x-apikey", VT_KEY.into());

        let client = ClientBuilder::new()
            .default_headers(headers)
            .build()
            .expect("cannot build VT client");

        for part in email.text_bodies() {
            let text = part.text_contents().unwrap();

            for url_match in regex.captures_iter(text.as_ref()).map(|c| c.get(0)) {
                let url_match = url_match.unwrap();

                let url = std::str::from_utf8(url_match.as_bytes()).unwrap();
                let url = Url::parse(url).unwrap();

                let task: Pin<Box<dyn Future<Output=AnalysisVerdict> + Send + Sync>> =
                    Box::pin(analyze_url(url));

                tasks.push(task)
            }
        }

        Analysis {
            name: "Links analysis".to_string(),
            verdicts: tasks,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LinkAnalalysis {
    url: String,
    virus_total_score: usize,
    virus_total_community_score: usize,
}

const VT_KEY: &str = "KEY";

#[derive(Deserialize, Serialize)]
struct UrlAnalysis {
    data: UrlAnalysisData,
}

#[derive(Deserialize, Serialize)]
struct UrlAnalysisData {
    id: String,
    #[serde(name = "type")]
    tpe: String,
    attributes: UrlAnalysisAttributes,
}

#[derive(Deserialize, Serialize)]
struct UrlAnalysisAttributes {
    date: String,
    status: String,
    results: HashMap<String, UrlAnalysisVerdict>,
    stats: UrlAnalysisStats,
}

#[derive(Deserialize, Serialize)]
struct UrlAnalysisStats {
    harmless: usize,
    malicious: usize,
    suspicious: usize,
    timeout: usize,
    undected: usize,
}

#[derive(Deserialize, Serialize)]
struct UrlAnalysisVerdict {
    category: String,
    engine_name: String,
    method: String,
    result: String,
}

async fn analyze_url(url: &str, client: &Client) -> AnalysisVerdict {
    let response = client.post("https://www.virustotal.com/api/v3/urls")
        .body(Form::new().text("data", url).await)
        .send()
        .await
        .unwrap();

    let analysis_id: usize = response.json().await.unwrap();

    let response = client.get(format!("https://www.virustotal.com/api/v3/analyses/{analysis_id}"))
        .send()
        .await
        .unwrap();

    response.text()
        .await
        .map_or(AnalysisVerdict::Error(vec!["invalid VT response body".to_string()]), AnalysisVerdict::Completed)
}

async fn analyze_domain(domain: &str) -> AnalysisVerdict {
    
}

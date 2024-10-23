use crate::analysis::{Analysis, AnalysisVerdict, MailAnalyzer};
use base64::prelude::BASE64_STANDARD_NO_PAD;
use base64::Engine;
use mail_parser::Message;
use regex::bytes::Regex;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;

pub struct LinkAnalyzer;

impl MailAnalyzer for LinkAnalyzer {
    fn analyze(&self, email: Message) -> Analysis {
        let regex = Regex::new(r"https?:\/\/(?:www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b(?:[-a-zA-Z0-9()@:%_\+.~#?&\/=]*)").unwrap();

        let mut tasks = Vec::new();

        let client = Client::new();

        let mut urls = HashSet::new();
        let mut domains = HashSet::new();

        for part in email.text_bodies() {
            let text = part.text_contents().unwrap();

            for url_match in regex.captures_iter(text.as_ref()).map(|c| c.get(0)) {
                let url_match = url_match.unwrap();

                let url = std::str::from_utf8(url_match.as_bytes())
                    .unwrap()
                    .to_string();

                let url = url::Url::parse(&url).unwrap();
                let domain = url.domain().unwrap();

                domains.insert(domain.to_string());
                urls.insert(url.to_string());
            }
        }

        for url in urls {
            let task: Pin<Box<dyn Future<Output = AnalysisVerdict> + Send + Sync>> =
                Box::pin(analyze_url(url.to_string(), client.clone()));

            tasks.push(task)
        }

        for domain in domains {
            let task: Pin<Box<dyn Future<Output = AnalysisVerdict> + Send + Sync>> =
                Box::pin(analyze_domain(domain, client.clone()));

            tasks.push(task);
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

const VT_KEY: &str = "44a1be194e97364ce779c6cde6aa0df72e7dcf6940db213cafbafac44c2ba9e4";

async fn analyze_url(url: String, client: Client) -> AnalysisVerdict {
    let response = request_url_analysis(&url, &client).await;
    let mut response = match response {
        Ok(response) => response,
        Err(err) => return AnalysisVerdict::error(&vec![format!("{err:?}")]),
    };

    //if no analysis is found, request a new one to VT and wait, then try again
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        submit_url_analysis(&url, &client).await;

        response = match request_url_analysis(&url, &client).await {
            Ok(response) => response,
            Err(err) => return AnalysisVerdict::error(&vec![format!("{err:?}")]),
        };
    }

    let content = response.text().await;
    match content {
        Ok(content) => AnalysisVerdict::new("url", &content),
        Err(err) => AnalysisVerdict::error(&vec![format!("{err:?}")]),
    }
}

async fn request_url_analysis(url: &str, client: &Client) -> Result<Response, reqwest::Error> {
    let url64 = BASE64_STANDARD_NO_PAD.encode(url);
    client
        .get(format!("https://www.virustotal.com/api/v3/urls/{url64}"))
        .header("x-apikey", VT_KEY)
        .send()
        .await
}

#[derive(Deserialize)]
struct AnalysisResponse {
    data: AnalysisData,
}

#[derive(Deserialize)]
struct AnalysisData {
    attributes: AnalysisAttributes,
}

#[derive(Deserialize)]
struct AnalysisAttributes {
    results: AnalysisResults,
}

#[derive(Deserialize)]
struct AnalysisResults {
    status: String,
}

async fn submit_url_analysis(url: &str, client: &Client) {
    let response = client
        .post("https://www.virustotal.com/api/v3/urls".to_string())
        .header("x-apikey", VT_KEY)
        .form(&[("url", url)])
        .send()
        .await
        .unwrap();

    let analysis_id = response.text().await.unwrap();

    // check analysis completion status periodically until it is completed
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        let response = client
            .post(format!(
                "https://www.virustotal.com/api/v3/analyses/{analysis_id}"
            ))
            .header("x-apikey", VT_KEY)
            .form(&[("url", url)])
            .send()
            .await
            .unwrap();


        let response = response.json::<AnalysisData>().await.unwrap();

        if response.attributes.results.status == "completed" {
            break;
        }
    }
}

async fn analyze_domain(domain: String, client: Client) -> AnalysisVerdict {
    let response = client
        .get(format!(
            "https://www.virustotal.com/api/v3/domains/{domain}"
        ))
        .header("x-apikey", VT_KEY)
        .send()
        .await;

    let response = match response {
        Ok(response) => response,
        Err(err) => return AnalysisVerdict::error(&vec![format!("{err:?}")]),
    };

    let content = response.text().await;
    match content {
        Ok(content) => AnalysisVerdict::new("domain", &content),
        Err(err) => AnalysisVerdict::error(&vec![format!("{err:?}")]),
    }
}

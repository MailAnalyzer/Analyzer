use crate::analysis::{AnalysisCommand, AnalysisSetup, AnalysisVerdict, MailAnalyzer};
use async_trait::async_trait;
use base64::prelude::BASE64_STANDARD_NO_PAD;
use base64::Engine;
use mail_parser::{Address, Message};
use regex::bytes::Regex;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use url::Url;

pub struct LinkAnalyzer;

#[async_trait]
impl MailAnalyzer for LinkAnalyzer {
    fn name(&self) -> String {
        String::from("Links analysis")
    }

    fn analyze(&self, email: Message, command: AnalysisCommand) -> AnalysisSetup {
        let link_regex = Regex::new(r"https?:\/\/(?:www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b(?:[-a-zA-Z0-9()@:%_\+.~#?&\/=]*)").unwrap();

        let client = Client::new();

        let mut urls = HashSet::new();
        let mut domains = HashSet::new();

        for part in email.text_bodies() {
            let text = part.text_contents().unwrap();

            for url_match in link_regex.captures_iter(text.as_ref()).map(|c| c.get(0)) {
                let url_match = url_match.unwrap();

                let url = std::str::from_utf8(url_match.as_bytes())
                    .unwrap()
                    .to_string();

                let url = Url::parse(&url).unwrap();
                let domain = url.domain().unwrap();

                domains.insert(domain.to_string());
                urls.insert(url.to_string());
            }
        }

        let addresses = match email.from().unwrap() {
            Address::List(l) => {
                l.clone()
            }
            Address::Group(l) => {
                l.iter().flat_map(|g| g.addresses.clone()).collect()
            }
        };

        for address in addresses {
            let address = address.address().unwrap();
            match Url::try_from(address) {
                Ok(url) => {
                    urls.insert(url.to_string());
                }
                Err(_) => {
                    if let Some(domain) = address.rsplit('@').next() {
                        domains.insert(domain.to_string());
                    }
                }
            }
        }

        let command = Arc::new(command);

        for url in urls {
            let client = client.clone();
            command.spawn(analyze_url(url.to_string(), client));
        }

        for domain in domains {
            let client = client.clone();
            command.spawn(analyze_domain(domain.to_string(), client));
        }
        command.gen_setup()
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
        Err(err) => return AnalysisVerdict::error(&vec![format!("Error url analysis `{url}`: {err:?}")]),
    };

    //if no analysis is found, request a new one to VT and wait, then try again
    if response.status() == StatusCode::NOT_FOUND {
        submit_url_analysis(&url, &client).await;

        response = match request_url_analysis(&url, &client).await {
            Ok(response) => response,
            Err(err) => return AnalysisVerdict::error(&vec![format!("Error url analysis `{url}`: {err:?}")]),
        };
        if response.status() == StatusCode::NOT_FOUND {
            return AnalysisVerdict::error(&"404 Not Found");
        }
    }

    let content = response.text().await;
    match content {
        Ok(content) => AnalysisVerdict::new("url", &content),
        Err(err) => AnalysisVerdict::error(&vec![format!("Error url analysis `{url}`: {err:?}")]),
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
    status: String,
}

#[derive(Deserialize)]
struct AnalysisSubmitResponse {
    data: AnalysisSubmitResponseData,
}

#[derive(Deserialize)]
struct AnalysisSubmitResponseData {
    id: String,
}

async fn submit_url_analysis(url: &str, client: &Client) {
    let response = client
        .post("https://www.virustotal.com/api/v3/urls".to_string())
        .header("x-apikey", VT_KEY)
        .form(&[("url", url)])
        .send()
        .await
        .unwrap();

    let analysis_id = response
        .json::<AnalysisSubmitResponse>()
        .await
        .unwrap()
        .data
        .id;

    // check analysis completion status periodically until it is completed
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        let response = client
            .get(format!(
                "https://www.virustotal.com/api/v3/analyses/{analysis_id}"
            ))
            .header("x-apikey", VT_KEY)
            .send()
            .await
            .unwrap();

        let response = response.json::<AnalysisResponse>().await.unwrap();

        if response.data.attributes.status == "completed" {
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
        Err(err) => return AnalysisVerdict::error(&vec![format!("Error domain analysis `{domain}`: {err:?}")]),
    };

    let content = response.text().await;
    match content {
        Ok(content) => AnalysisVerdict::new("domain", &content),
        Err(err) => AnalysisVerdict::error(&vec![format!("Error domain analysis `{domain}`: {err:?}")]),
    }
}

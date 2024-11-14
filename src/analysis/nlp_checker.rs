use crate::analysis::{AnalysisSetup, AnalysisVerdict, MailAnalyzer};
use crate::command::AnalysisCommand;
use crate::email::OwnedEmail;
use crate::pipeline::Pipeline;
use lazy_static::lazy_static;
use mail_parser::Message;
use regex::Regex;
use reqwest::Client;
use rocket::futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use rocket::serde::json::serde_json;
use tl::Node;
use crate::entity::Entity;

pub struct NLPChecker;

impl MailAnalyzer for NLPChecker {
    fn name(&self) -> String {
        String::from("Natural Language Processing Analysis")
    }

    fn analyze(&self, email: OwnedEmail, command: AnalysisCommand) -> AnalysisSetup {
        command.spawn_pipeline(
            Pipeline::once_root(move |_: AnalysisCommand| extract_all_text(email))
                .next_fn(|text, _| analyze_text(text))
                .next_fn(|llm_result, c| async move {
                    c.result(AnalysisVerdict::new("nlp-summary", &llm_result.summary));
                    for entity in &llm_result.entities {
                        c.submit_entity(entity)
                    }
                }),
        );
        command.validate()
    }
}

struct LLMAnalysisResponse {
    summary: String,
    entities: Vec<Entity>,
}

async fn analyze_text(text: Arc<String>) -> LLMAnalysisResponse {
    match make_llm_request(text).await {
        Ok((summary, entities)) => LLMAnalysisResponse {
            summary,
            entities: serde_json::from_str(&entities).unwrap(),
        },
        //TODO handle error in a better way (ew)
        Err(err) => LLMAnalysisResponse {
            summary: err.clone(),
            entities: vec![],
        },
    }
}

async fn make_llm_request(text: Arc<String>) -> Result<(String, String), String> {
    let response = Client::new()
        .post("http://localhost:8080/llm/analyze")
        .body(text.to_string())
        .send()
        .await
        .map_err(|e| format!("error when requesting LLM server : {}", e))?;

    let result = response
        .text()
        .await
        .map_err(|e| format!("error when reading LLM server: {}", e))?;

    let summary_regex = Regex::new(r#"^summary:"([^"]*)"\nentities:([\s\S]*)$"#).unwrap();

    let captures = summary_regex.captures(&result).unwrap();

    let summary = String::from(captures.get(1).unwrap().as_str());
    let entities = String::from(captures.get(2).unwrap().as_str());
    Ok((summary, entities))
}

async fn extract_all_text(message: OwnedEmail) -> String {
    let ExtractedBodyInformation { text, images, .. } =
        extract_body_information(message.parse()).await;

    let ocr_response = Client::new()
        .post("http://localhost:8080/ocr")
        .body(images.join("\n"))
        .send()
        .await;

    match ocr_response {
        Err(e) => {
            println!("ocr error: {e:?}");
            text
        }
        Ok(response) => {
            let mut buff = text;

            if !response.status().is_success() {
                println!("ocr response is not 200: {}", response.status());
                return buff;
            }

            let ocr_text = response.json::<HashMap<String, String>>().await.unwrap();

            for value in ocr_text.values() {
                buff.push_str(value);
            }

            buff
        }
    }
}

fn get_body_string(message: Message) -> String {
    let mut buff = String::from("<html>");
    for part in message.html_bodies() {
        buff.push_str(&part.to_string())
    }
    buff.push_str("</html>");
    buff
}

lazy_static! {
    static ref SANITIZE_HTML_CHARS: Regex =
        Regex::new(r"&(#\d+|#x[0-9A-Fa-f]+|[a-zA-Z]+);?").unwrap();
}

#[derive(Debug, Default)]
struct ExtractedBodyInformation {
    text: String,
    images: Vec<String>,
    svgs: Vec<String>,
}

async fn extract_body_information(email: Message<'_>) -> ExtractedBodyInformation {
    let body = get_body_string(email);

    let dom = tl::parse(&body, tl::ParserOptions::default()).unwrap();
    let parser = dom.parser();

    let nodes = dom.nodes();
    let root = &nodes[0];

    let mut to_visit = vec![root];

    let mut info = ExtractedBodyInformation::default();

    while let Some(node) = to_visit.pop() {
        match node {
            Node::Tag(tag) => {
                let name = tag.name().as_utf8_str();
                if name == "style" || name == "script" {
                    continue;
                }
                if name == "img" {
                    if let Some(source) = tag.attributes().get("src").flatten() {
                        info.images.push(source.as_utf8_str().to_string());
                        continue;
                    }
                }
                if name == "svg" {
                    info.svgs.push(tag.raw().as_utf8_str().to_string());
                    continue;
                }
                to_visit.extend(tag.children().top().iter().flat_map(|nh| nh.get(parser)))
            }
            Node::Raw(bytes) => {
                let str = &bytes.as_utf8_str();
                let str = &SANITIZE_HTML_CHARS.replace_all(str, "");
                let str = str.trim_ascii();
                let str = snailquote::unescape(str).unwrap_or(str.to_string());

                if str.is_empty() {
                    continue;
                }
                info.text.push_str(&str);
                info.text.push('\n');
            }
            Node::Comment(_) => {}
        }
    }

    info
}

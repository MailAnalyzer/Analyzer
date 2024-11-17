use lazy_static::lazy_static;
use mail_parser::Message;
use regex::Regex;
use std::time::Duration;
use thirtyfour::{By, ChromiumLikeCapabilities, DesiredCapabilities, WebDriver};
use tl::Node;

const IGNORED_HTML_TAGS: &[&str] = &["style", "script"];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // selenium_stuff().await?;

    
    
    Ok(())
}

async fn selenium_stuff() -> Result<(), Box<dyn std::error::Error>>  {
    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--lang=en")?;
    // caps.set_headless()?;
    let driver = WebDriver::new("http://localhost:4444", caps)
        .await
        .unwrap();
    driver.goto("https://google.com/search?q=youtube&hl=en").await?;

    driver.find(By::XPath("//div[text()='Accept all']")).await?.click().await?;
    let element = driver.find(By::Css("span > a")).await?;



    println!("{:?}", element.value().await?);

    tokio::time::sleep(Duration::MAX).await;
    Ok(())
}

fn get_body_string(message: Message) -> String {
    let mut buff = String::from("<html>");
    for part in message.text_bodies() {
        buff.push_str(&part.to_string())
    }
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

fn extract_body_information(email: Message<'_>) -> ExtractedBodyInformation {
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

use lazy_static::lazy_static;
use mail_parser::Message;
use regex::Regex;
use reqwest::cookie::Jar;
use reqwest::ClientBuilder;
use std::sync::Arc;
use std::time::Duration;
use thirtyfour::{ChromiumLikeCapabilities, Cookie, DesiredCapabilities, SameSite, WebDriver};
use tl::Node;
use url::Url;

const IGNORED_HTML_TAGS: &[&str] = &["style", "script"];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    selenium_stuff().await?;


    Ok(())
}

async fn selenium_stuff() -> Result<(), Box<dyn std::error::Error>> {
    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--lang=en")?;
    caps.add_arg("--ignore-ssl-errors=yes")?;
    caps.add_arg("--ignore-ignore-certificate-errors")?;
    // caps.set_headless()?;
    let driver = WebDriver::new("http://localhost:4444", caps)
        .await
        .unwrap();

    let splunk_search_url = Url::parse("https://soc-siem.eu.airbus.corp:8000/en-US/app/search/search").unwrap();
    driver.goto(format!("{}?", splunk_search_url)).await?;


    loop {
        if driver.current_url().await.unwrap() == splunk_search_url {
            println!("Closed !");

            let cookies = driver.get_all_cookies().await.unwrap();
            let cookie_jar = Jar::default();


            for cookie in cookies {
                let domain = cookie.domain.as_ref().unwrap();
                let path = cookie.path.as_ref().unwrap();

                let url = Url::parse(&format!("https://{domain}{path}")).unwrap();
                cookie_jar.add_cookie_str(&cookie_to_str(cookie), &url)
            }

            let client = ClientBuilder::default()
                .cookie_store(true)
                .cookie_provider(Arc::new(cookie_jar))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
                .build()
                .unwrap();

            let csrf = driver.get_named_cookie("splunkweb_csrf_token_8000").await.unwrap();

            make_splunk_search(client, csrf.value).await;

            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

fn cookie_to_str(cookie: Cookie) -> String {
    let mut str = format!("{}={}", cookie.name, cookie.value);
    if let Some(domain) = cookie.domain {
        str.push_str(&format!("; Domain={}", domain))
    }
    if let Some(age) = cookie.expiry {
        str.push_str(&format!("; Max-Age={age}"))
    }
    if let Some(site) = cookie.same_site {
        str.push_str(&format!("; SameSite={}", match site {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
            SameSite::None => "None"
        }))
    }

    if let Some(path) = cookie.path {
        str.push_str(&format!("; Path={path}"))
    }

    println!("{str}");

    str
}

const SPLUNK_API_ENDPOINT: &str = "https://soc-siem.eu.airbus.corp:8000/en-US/splunkd/__raw/servicesNS/mbat3wm0/SplunkEnterpriseSecuritySuite/search/v2";
//const SPLUNK_API_ENDPOINT: &str = "http://localhost:7878/en-US/splunkd/__raw/servicesNS/mbat3wm0/SplunkEnterpriseSecuritySuite/search/v2";

async fn make_splunk_search(client: reqwest::Client, csrf: String) {
    let request = client.post(format!("{SPLUNK_API_ENDPOINT}/jobs"))
        .form(&vec![
            ("adhoc_search_level", "verbose"),
            ("earliest_time", "1736235120.000000000"),
            ("latest_time", "1736249520.000000000"),
            ("search", "test"),
            ("output_mode", "json"),
        ])
        //.header("Origin", "https://soc-siem.eu.airbus.corp:8000")
        .header("Accept", "application/json")
        .header("X-Splunk-Form-Key", csrf)
        .header("X-Requested-With", "XMLHttpRequest")
        .build()
        .unwrap();

    println!("{request:?}");

    let response = match client.execute(request).await {
        Ok(response) => response,
        Err(err) => {
            println!("{err:?}");
            return
        }
    };

    println!("{response:?}");

    let json = response.text().await.unwrap_or("error".to_string());

    println!("{json:?}")
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

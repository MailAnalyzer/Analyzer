use crate::splunk::{SplunkClient, SplunkClientConfig};
use reqwest::cookie::{CookieStore, Jar};
use reqwest::ClientBuilder;
use reqwest::{Client, RequestBuilder};
use std::sync::Arc;
use std::time::Duration;
use thirtyfour::{ChromiumLikeCapabilities, Cookie, DesiredCapabilities, SameSite, WebDriver};
use url::Url;

pub struct WebClient {
    config: SplunkClientConfig,
    client: Client,
    cookie_jar: Arc<Jar>,
}

impl WebClient {
    pub async fn new_via_web_portal(config: SplunkClientConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let mut caps = DesiredCapabilities::chrome();
        caps.add_arg("--lang=en")?;
        caps.add_arg("--ignore-ssl-errors=yes")?;
        caps.add_arg("--ignore-ignore-certificate-errors")?;
        let driver = WebDriver::new("http://localhost:4444", caps)
            .await
            .unwrap();

        driver.goto(config.portal.to_string()).await?;

        let target_url = Url::parse(&format!("{}en-US/app/launcher/home", config.portal)).unwrap();

        loop {
            //TODO find a better way to identify that the authentication is completed (using cookies for example)
            if driver.current_url().await.unwrap() == target_url {
                let cookies = driver.get_all_cookies().await.unwrap();
                let cookie_jar = Jar::default();

                for cookie in cookies {
                    let domain = cookie.domain.as_ref().unwrap();
                    let path = cookie.path.as_ref().unwrap();

                    let url = Url::parse(&format!("https://{domain}{path}")).unwrap();
                    cookie_jar.add_cookie_str(&cookie_to_str(cookie), &url)
                }

                let cookie_jar = Arc::new(cookie_jar);

                let client = ClientBuilder::default()
                    .cookie_store(true)
                    .cookie_provider(cookie_jar.clone())
                    .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
                    .build()
                    .unwrap();

                return Ok(Self {
                    config,
                    client,
                    cookie_jar,
                });
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

const SPLUNKWEB_CSRF_COOKIE: &str = "splunkweb_csrf_token_8000";

impl WebClient {
    fn decorate(&self, req: RequestBuilder) -> RequestBuilder {
        let cookies = self.cookie_jar
            .cookies(&self.config.endpoint)
            .unwrap();

        let cookies = cookies.to_str().unwrap();

        let csrf =
            cookies
                .split("; ")
                .filter(|c| c.starts_with(SPLUNKWEB_CSRF_COOKIE))
                .map(|c| c.split_once('=').unwrap().1)
                .next()
                .expect("could not find csrf token");

        req
            .header("X-Splunk-Form-Key", csrf)
            .header("X-Requested-With", "XMLHttpRequest")
    }
}

impl SplunkClient for WebClient {
    fn post(&self, url: &str) -> RequestBuilder {
        self.decorate(self.client.post(format!("{}{}", self.config.endpoint, url)))
    }

    fn get(&self, url: &str) -> RequestBuilder {
        self.decorate(self.client.get(format!("{}{}", self.config.endpoint, url)))
    }
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

    str
}

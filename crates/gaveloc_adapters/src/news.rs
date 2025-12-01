use async_trait::async_trait;
use gaveloc_core::{
    entities::{Banner, Headlines, NewsArticle},
    error::Error,
    ports::NewsRepository,
};
use reqwest::Client;
use scraper::{Html, Selector};
use std::time::{SystemTime, UNIX_EPOCH};

const BASE_URL: &str = "https://frontier.ffxiv.com";

pub struct HttpNewsRepository {
    client: Client,
}

impl HttpNewsRepository {
    pub fn new() -> Self {
        // Reusing the patch client config for now as it has reasonable timeouts
        let client = crate::network::build_patch_client().expect("Failed to create HTTP client");
        Self { client }
    }

    fn get_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[async_trait]
impl NewsRepository for HttpNewsRepository {
    async fn get_headlines(&self, language: &str) -> Result<Headlines, Error> {
        let timestamp = self.get_timestamp();
        let url = format!(
            "{}/news/headline.json?lang={}&media=pcapp&_={}",
            BASE_URL, language, timestamp
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(Error::Network(format!(
                "Failed to fetch headlines: {}",
                response.status()
            )));
        }

        response
            .json::<Headlines>()
            .await
            .map_err(|e| Error::Network(format!("Failed to parse headlines: {}", e)))
    }

    async fn get_banners(&self, language: &str) -> Result<Vec<Banner>, Error> {
        let timestamp = self.get_timestamp();
        let url = format!(
            "{}/v2/topics/{}/banner.json?lang={}&media=pcapp&_={}",
            BASE_URL, language, language, timestamp
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        // The banner endpoint returns existing format: {"banner": [...], ...}
        // We need a wrapper struct for deserialization
        #[derive(serde::Deserialize)]
        struct BannerResponse {
            banner: Vec<Banner>,
        }

        if !response.status().is_success() {
            return Err(Error::Network(format!(
                "Failed to fetch banners: {}",
                response.status()
            )));
        }

        let wrapper = response
            .json::<BannerResponse>()
            .await
            .map_err(|e| Error::Network(format!("Failed to parse banners: {}", e)))?;

        Ok(wrapper.banner)
    }

    async fn get_article(&self, url: &str) -> Result<NewsArticle, Error> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(Error::Network(format!(
                "Failed to fetch article: {}",
                response.status()
            )));
        }

        let html_content = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let document = Html::parse_document(&html_content);

        // Selectors
        // Note: These are based on standard Lodestone classes. Might need adjustment if they change.
        let title_selector = Selector::parse(".news__detail__title").unwrap();
        let body_selector = Selector::parse(".news__detail__wrapper").unwrap();
        // Topics sometimes have different structure
        let topic_title_selector = Selector::parse(".ldst__header__title").unwrap();
        let topic_body_selector = Selector::parse(".ldst__main").unwrap();

        let title = if let Some(el) = document.select(&title_selector).next() {
            el.text().collect::<String>()
        } else if let Some(el) = document.select(&topic_title_selector).next() {
            el.text().collect::<String>()
        } else {
            "No Title".to_string()
        };

        let content_html = if let Some(el) = document.select(&body_selector).next() {
            el.inner_html()
        } else if let Some(el) = document.select(&topic_body_selector).next() {
            el.inner_html()
        } else {
            "<p>Could not parse article content. Please open in browser.</p>".to_string()
        };

        // Date parsing is tricky as it's often in a script tag or complex structure.
        // For now, we'll leave it empty or try a simple selector if found.
        let date = String::new();

        Ok(NewsArticle {
            title: title.trim().to_string(),
            content_html,
            date,
            url: url.to_string(),
        })
    }
}

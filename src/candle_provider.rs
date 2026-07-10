use anyhow::{Result, bail};
use reqwest::{Client, StatusCode, Url, header::HeaderMap};
use serde_json_path::JsonPath;

use crate::{config::CandleFields, domain::Candle};

mod fields {
const TIMESTAMP: &str = "timestamp";
const OPEN: &str = "open";
const HIGH: &str = "high";
const LOW: &str = "low";
const CLOSE: &str = "close";
}

#[async_trait::async_trait]
pub trait CandleProvider {
    async fn get_candles(&self) -> Result<Vec<Candle>>; 
}

#[derive(Debug)]
pub struct HttpCandleProvider {
    client: Client,
    url: String,
    headers: Option<HeaderMap>,
    fields_config: CandleFields
}

impl HttpCandleProvider {
    fn to_candles(&self, json_str: &str) -> Result<Vec<Candle>> {
        
    }
}

#[async_trait::async_trait]
impl CandleProvider for HttpCandleProvider {
    async fn get_candles(&self) -> Result<Vec<Candle>> {
        let mut req_builder = self.client.get(&self.url);
        if let Some(headers) = &self.headers {
            req_builder = req_builder.headers(headers.clone());
        }

        let resp = req_builder.send().await?;
        let resp_status = resp.status();
        let resp_text = resp.text().await?;
        if resp.status().is_success() {
            self.to_candles(resp_text)?
        } else {
            bail!("HTTP status: {}, body: {}", resp_status.as_str(), resp_text);
        }
    }

}
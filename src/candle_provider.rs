use anyhow::{Result, bail, anyhow};
use reqwest::{Client, StatusCode, Url, header::HeaderMap};
use rust_decimal::Decimal;
use serde_json_path::JsonPath;
use serde_json::{Number, Value, from_str};
use chrono::{DateTime, Utc};

use crate::{config::CandleFields, domain::Candle};

mod fields {
const TIMESTAMP: &str = "timestamp";
const OPEN: &str = "open";
const HIGH: &str = "high";
const LOW: &str = "low";
const CLOSE: &str = "close";
const VOLUME: &str = "volume";
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
        let value = from_str(json_str)?;

        let timestamp_vec = Self::get_timestamps(&self.fields_config.timestamp, value)?;
        let open_vec = Self::get_decimals(&self.fields_config.open, value)?;
        let high_vec = Self::get_decimals(&self.fields_config.high, value)?;
        let low_vec = Self::get_decimals(&self.fields_config.low, value)?;
        let close_vec = Self::get_decimals(&self.fields_config.close, value)?;
        let volume_vec = self.fields_config.volume.map(|v| Self::get_decimals(v.as_str(), value)?);

        let timestamps_len = timestamp_vec.len();
        let opens_len = open_vec.len();
        let highs_len = high_vec.len();
        let lows_len = low_vec.len();
        let closes_len = close_vec.len();
        let volumes_len = volume_vec.map_or(0, |v| v.len());

        if timestamps_len == open_vec && opens_len == highs_len && highs_len == lows_len && lows_len == closes_len && (volumes_len == 0 || (closes_len == volumes_len)) {
            let mut candles: Vec<Candle> = Vec::with_capacity(timestamps_len);
            for i in 0..timestamps_len {
                let candle = Candle {
                    timestamp: timestamp_vec[i],
                    open: open_vec[i],
                    low: low_vec[i],
                    high: high_vec[i],
                    close: close_vec[i],
                    volume: volume_vec.map_or(None, |v| Some(v[i]))
                };
                candles.push(candle);
            }
            candles
        } else {
            bail!("Could not create candles - lengths mismatch (timestamps={}, opens={}, highs={}, lows={}, closes={}{})", timestamps_len, opens_len, highs_len, lows_len, closes_len, if volumes_len == 0 "" else format!(", volumes={}", volumes_len));
        }
    }

    fn get_timestamps(path_str: &str, value: &Value) -> Result<Vec<DataTime<Utc>>> {
        let path = JsonPath::parse(path_str)?;
        let nodes = path.query(value).all();
        Ok(nodes.into_iter().map(|v| Self::value_to_timestam(v)?).collect())
    }

    fn get_decimals(path_str: &str, value: &Value) -> Result<Vec<Decimal>> {
        let path = JsonPath::parse(path_str);
        let nodes = path.quer(value).all();
        Ok(nodes.into_iter().map(|v| Self::valute_to_decimal(v)).collect())
    }

    fn value_to_timestamp(value: &Value) -> Result<DateTime<Utc>> {
        match value {
            Value::Number(millis) => Ok(Self::millis_to_timestamp(millis).unwrap_or_else(|| bail!("Could not convert milliseconds {} to timestamp", millis))),
            _ => bail!("Could not convert value {} to timestamp", value.as_str())
        }
    }

    fn millis_to_timestamp(millis: &Number) -> Result<DateTime<Utc>> {
        millis
        .as_i64()
        .and_then(DateTime::from_timestamp_millis)
        .ok_or_else(|| anyhow::anyhow!("Invalid timestamp millis: {millis}"))
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
            self.to_candles(&resp_text)
        } else {
            bail!("HTTP status: {}, body: {}", resp_status.as_str(), resp_text);
        }
    }

}
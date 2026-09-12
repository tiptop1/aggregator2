use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use reqwest::{Client, header::HeaderMap};
use rust_decimal::Decimal;
use serde_json::{Number, Value, from_str};
use serde_json_path::JsonPath;

use crate::{config::CandleFields, domain::Candle};

#[async_trait::async_trait]
pub trait CandleProvider {
    async fn get_candles(&self) -> Result<Vec<Candle>>;
}

#[derive(Debug)]
pub struct HttpCandleProvider {
    client: Client,
    url: String,
    headers: Option<HeaderMap>,
    fields_config: CandleFields,
}

impl HttpCandleProvider {
    fn to_candles(&self, json_str: &str) -> Result<Vec<Candle>> {
        let value = &from_str(json_str)?;

        let timestamp_vec = Self::get_timestamps(&self.fields_config.timestamp, value)?;
        let open_vec = Self::get_decimals(&self.fields_config.open, value)?;
        let high_vec = Self::get_decimals(&self.fields_config.high, value)?;
        let low_vec = Self::get_decimals(&self.fields_config.low, value)?;
        let close_vec = Self::get_decimals(&self.fields_config.close, value)?;
        let volume_vec = self
            .fields_config
            .volume
            .as_ref()
            .map(|v| Self::get_decimals(v.as_str(), value))
            .transpose()?;

        let timestamps_len = timestamp_vec.len();
        let opens_len = open_vec.len();
        let highs_len = high_vec.len();
        let lows_len = low_vec.len();
        let closes_len = close_vec.len();
        let volumes_len = volume_vec.as_ref().map_or(0, |v| v.len());

        if timestamps_len == opens_len
            && opens_len == highs_len
            && highs_len == lows_len
            && lows_len == closes_len
            && (volumes_len == 0 || (closes_len == volumes_len))
        {
            let mut candles: Vec<Candle> = Vec::with_capacity(timestamps_len);
            for i in 0..timestamps_len {
                let candle = Candle {
                    timestamp: timestamp_vec[i],
                    open: open_vec[i],
                    low: low_vec[i],
                    high: high_vec[i],
                    close: close_vec[i],
                    volume: volume_vec.as_ref().map_or(None, |v| Some(v[i])),
                };
                candles.push(candle);
            }
            Ok(candles)
        } else {
            let volume_len_str = if volumes_len == 0 {
                String::new()
            } else {
                format!(", volumes={}", volumes_len)
            };
            bail!(
                "Could not create candles - lengths mismatch (timestamps={}, opens={}, highs={}, lows={}, closes={}{})",
                timestamps_len,
                opens_len,
                highs_len,
                lows_len,
                closes_len,
                volume_len_str
            )
        }
    }

    fn get_timestamps(path_str: &str, value: &Value) -> Result<Vec<DateTime<Utc>>> {
        let path = JsonPath::parse(path_str)?;
        let nodes = path.query(value).all();
        let mut timestamps: Vec<DateTime<Utc>> = Vec::with_capacity(nodes.len());
        for n in &nodes {
            timestamps.push(Self::value_to_timestamp(n)?);
        }
        Ok(timestamps)
    }

    fn get_decimals(path_str: &str, value: &Value) -> Result<Vec<Decimal>> {
        let path = JsonPath::parse(path_str)?;
        let nodes = path.query(value).all();
        let mut decimals = Vec::new();
        for v in nodes {
            decimals.push(Self::value_to_decimal(v)?);
        }
        Ok(decimals)
    }

    fn value_to_timestamp(value: &Value) -> Result<DateTime<Utc>> {
        match value {
            Value::Number(millis) => Self::millis_to_timestamp(millis)
                .map_err(|_| anyhow!("Could not convert milliseconds {} to timestamp", millis)),
            _ => bail!(
                "Could not convert value {} to timestamp",
                value.as_str().unwrap_or_else(|| "?")
            ),
        }
    }

    fn value_to_decimal(value: &Value) -> Result<Decimal> {
        match value {
            Value::Number(number) => Decimal::from_str_exact(&number.to_string())
                .map_err(|_| anyhow!("Could not convert number {} to decimal", number)),
            _ => bail!(
                "Could not convert value {} to decimal",
                value.as_str().unwrap_or_else(|| "?")
            ),
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
        if resp_status.is_success() {
            self.to_candles(&resp_text)
        } else {
            bail!("HTTP status: {}, body: {}", resp_status.as_str(), resp_text);
        }
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn check_ohlc_creation() {
        let mock_server = MockServer::start().await;

        // Load JSON payload from fixtures
        let mock_json = include_str!("../tests/fixtures/btc-ohlc.json");

        Mock::given(method("GET"))
            .and(path("x"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(mock_json, "application/json"))
            .mount(&mock_server)
            .await;

        let client = Client::new();
        let url = format!("{}/x", mock_server.uri());
        let headers = None;
        let fields_config = CandleFields {
            timestamp: String::from("$[*][0]"),
            open: String::from("$[*][1]"),
            high: String::from("$[*][2]"),
            low: String::from("$[*][3]"),
            close: String::from("$[*][4]"),
            volume: None,
        };
        let provider = HttpCandleProvider {
            client,
            url,
            headers,
            fields_config,
        };

        let candles = provider.get_candles().await;
        assert!(candles.is_ok());
        let candles = candles.unwrap();
        assert_eq!(48, candles.len(), "candles length is incorrect");

        let first_candle = candles.get(0).unwrap();
        assert_eq!(
            DateTime::from_timestamp_millis(1788953400000).unwrap(),
            first_candle.timestamp,
            "timestamp mismatch"
        );
        assert_eq!(
            Decimal::from_str_exact("78930.0").unwrap(),
            first_candle.open,
            "open price mismatch"
        );
        assert_eq!(
            Decimal::from_str_exact("78937.0").unwrap(),
            first_candle.high,
            "high price mismatch"
        );
        assert_eq!(
            Decimal::from_str_exact("78764.0").unwrap(),
            first_candle.low,
            "low price mismatch"
        );
        assert_eq!(
            Decimal::from_str_exact("78937.0").unwrap(),
            first_candle.close,
            "close price mismatch"
        );
    }
}

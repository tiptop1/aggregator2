use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub smtp: SmtpConfig,
    pub database_path: String,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SmtpConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub to_emails: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Symbol {
    pub name: String,
    pub url: String,
    pub http_headers: Option<HashMap<String, String>>,
    pub candle_fields: CandleFields,
    pub report_elements: Vec<ReportElements>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CandleFields {
    pub timestamp: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReportElements {
    pub caption: String,
    #[serde(rename = "type")]
    #[serde(flatten)]
    pub element_type: ReportElementType,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum ReportElementType {
    #[serde(rename = "last_candle")]
    LastCandle,
    #[serde(rename = "candle_chart")]
    CandleChart { period: ChartPeriod, limit: u32 },
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum ChartPeriod {
    #[serde(rename = "daily")]
    Daily,
    #[serde(rename = "monthly")]
    Monthly,
}

pub async fn read_configuration(path: impl AsRef<Path>) -> Result<Config> {
    let path = path.as_ref().to_path_buf();
    let config = tokio::task::spawn_blocking(move || -> Result<Config> {
        let content = read_to_string(path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    })
    .await??;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use crate::config::{ChartPeriod, ReportElementType, read_configuration};

    #[tokio::test]
    async fn check_config_load() {
        let config = read_configuration("config.toml").await;

        assert!(config.is_ok(), "{:#?}", config);

        let config = config.unwrap();
        assert_eq!(config.database_path, "db.txt");

        let symbols = config.symbols;
        assert_eq!(symbols.len(), 2);
        assert!(symbols.get(0).is_some());

        let symbol = symbols.get(0).unwrap();
        assert_eq!(symbol.name, "BTC");
        assert!(symbol.http_headers.is_none());

        let candle_fields = &symbol.candle_fields;
        assert_eq!(candle_fields.timestamp, "$[*][0]");
        assert_eq!(candle_fields.volume, Some(String::from("$[*][5]")));

        let report_elements = &symbol.report_elements;
        assert_eq!(report_elements.len(), 3);

        assert!(report_elements.get(0).is_some());
        let element = symbol.report_elements.get(0).unwrap();
        assert_eq!(element.element_type, ReportElementType::LastCandle);

        assert!(report_elements.get(1).is_some());
        let element = report_elements.get(1).unwrap();
        assert_eq!(
            element.element_type,
            ReportElementType::CandleChart {
                period: ChartPeriod::Daily,
                limit: 90
            }
        )
    }
}

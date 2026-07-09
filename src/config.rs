use serde::Deserialize;
use std::collections::HashMap;
use anyhow::Result;
use std::path::PathBuf;
use std::fs::read_to_string;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database_path: String,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub url: String,
    pub http_headers: HashMap<String, String>,
    pub candle_fields: CandleFields,
}

#[derive(Debug, Deserialize)]
pub struct CandleFields {
    pub timestamp: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
}

pub fn read_configuration(file_path: &PathBuf) -> Result<Config> {
    let config_file_content = read_to_string(file_path)?;
    let config = toml::from_str(&config_file_content)?;
    Ok(config)
}
use aggregator2::candle_provider::{CandleProvider, HttpCandleProvider};
use aggregator2::candle_repository::{CandleRepository, RedbCandleRepository};
use aggregator2::cli::Args;
use aggregator2::config::{ChartPeriod, Config, ReportElementType, read_configuration};
use aggregator2::report::ReportEmailBuilder;
use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Months, Utc};
use clap::Parser;
use reqwest::{
    Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;
use std::env::var_os;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinHandle;

const CONFIG_FILE: &str = "config.toml";

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let config_file = config_file(&args);

    let config = Arc::new(read_configuration(&config_file).await?);
    let repo = Arc::new((RedbCandleRepository::new(&config.database_path)).await?);

    if args.load {
        load_candles(&config, &repo).await?;
    }

    if args.report {
        let mut email_builder = ReportEmailBuilder::new();

        for symbol in config.symbols.iter() {
            let mut last_candle = None;
            let mut min_timestamp = None;
            for report_element in symbol.report_elements.iter() {
                // Discover how many data get from repository
                match &report_element.element_type {
                    ReportElementType::LastCandle => {
                        last_candle = repo.last_candle(&symbol.name).await?
                    }
                    ReportElementType::CandleChart { period, limit } => {
                        let timestamp = to_timestamp(period, *limit);
                        if min_timestamp > timestamp {
                            min_timestamp = timestamp;
                        }
                    }
                }
            }

            let mut candles = Vec::new();
            if last_candle.is_some() && last_candle.as_ref().map(|c| c.timestamp) < min_timestamp {
                if let Some(last_candle) = last_candle {
                    candles.push(last_candle)
                }
            } else {
                if let Some(min_timestamp) = min_timestamp {
                    candles = repo.get(&symbol.name, min_timestamp..Utc::now()).await?;
                }
            }

            email_builder.html(&format!("<h1>{}</h1>", &symbol.name));

            for report_elemement in symbol.report_elements.iter() {}
        }
    }

    Ok(())
}

/// Try to find config file in OUT_DIR (useful in development time),
/// if not found try to use config file from application argument.
fn config_file(args: &Args) -> PathBuf {
    match var_os("OUT_DIR") {
        Some(path) => PathBuf::from(path).join(CONFIG_FILE),
        None => PathBuf::from(&args.config_file),
    }
}

async fn load_candles<R>(config: &Arc<Config>, repo: &Arc<R>) -> Result<()>
where
    R: CandleRepository + Send + Sync + 'static,
{
    // let config = Arc::new(config);
    // let repo = Arc::new(repo);
    let client = Client::new();

    let mut handles = Vec::with_capacity(config.symbols.len());

    for s in config.symbols.iter() {
        let repo_clone = Arc::clone(repo);
        let s = s.clone();
        let client = client.clone();

        let handle: JoinHandle<Result<()>> = tokio::task::spawn(async move {
            let candle_provider = HttpCandleProvider {
                client,
                fields_config: s.candle_fields,
                headers: to_header_map(s.http_headers),
                url: s.url,
            };

            let candles = candle_provider.get_candles().await?;
            repo_clone.insert(&s.name, &candles).await?;

            Ok(())
        });

        handles.push(handle);
    }

    for h in handles {
        h.await??;
    }

    Ok(())
}

fn to_header_map(headers: Option<HashMap<String, String>>) -> Option<HeaderMap<HeaderValue>> {
    if let Some(headers) = headers {
        let mut header_map = HeaderMap::with_capacity(headers.len());
        for (k, v) in headers {
            if let (Ok(name), Ok(value)) = (HeaderName::from_str(&k), HeaderValue::from_str(&v)) {
                header_map.insert(name, value);
            }
        }
        Some(header_map)
    } else {
        None
    }
}

fn to_timestamp(period: &ChartPeriod, limit: u32) -> Option<DateTime<Utc>> {
    let curr_timestamp = Utc::now();
    match period {
        ChartPeriod::Daily => {
            let shifted = curr_timestamp.checked_sub_signed(Duration::days(limit.to_i64()?))?;
            shifted
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .map(|naive_dt| naive_dt.and_utc())
        }
        ChartPeriod::Monthly => {
            let shifted = curr_timestamp.checked_sub_months(Months::new(limit))?;
            shifted
                .date_naive()
                .with_day(1)
                .map(|naive_dt| naive_dt.and_hms_opt(0, 0, 0))
                .flatten()
                .map(|naive_dt| naive_dt.and_utc())
        }
    }
}

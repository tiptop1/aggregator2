use aggregator2::candle_provider::{CandleProvider, HttpCandleProvider};
use aggregator2::candle_repository::{CandleRepository, RedbCandleRepository};
use aggregator2::cli::Args;
use aggregator2::config::{ChartPeriod, Config, ReportElementType, read_configuration};
use aggregator2::domain::Candle;
use aggregator2::report::{Image, ImageType, ReportEmailBuilder, png_chart};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Datelike, Duration, Months, Utc};
use clap::Parser;
use reqwest::{
    Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::cmp::{max, min};
use std::collections::{HashMap, VecDeque};
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
        let mut email_builder = ReportEmailBuilder::default();

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
                        let timestamp = to_timestamp(
                            period,
                            limit.to_i32().ok_or(anyhow!(format!(
                                "can't convert limit {} to signed number",
                                limit
                            )))?,
                        );
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

            for e in symbol.report_elements.iter() {
                let caption = &e.caption;
                email_builder.html(&format!("<h2>{}</h2>", caption));
                match &e.element_type {
                    ReportElementType::LastCandle => {
                        if let Some(last_candle) = candles.last() {
                            email_builder.candle(last_candle);
                        }
                    }
                    ReportElementType::CandleChart { period, limit } => {
                        let chart_bytes = png_chart(
                            &aggregate_candles(
                                &candles,
                                period,
                                limit.to_i32().ok_or(anyhow!(format!(
                                    "can't convert limit {} to signed number",
                                    limit
                                )))?,
                            ),
                            caption,
                            800,
                            800,
                        )
                        .await?;
                        let image = Image {
                            bytes: chart_bytes,
                            image_type: ImageType::Png,
                        };
                        email_builder.image(image);
                    }
                }
            }
        }
        email_builder
            .send("[Aggregator2] Report", &config.smtp)
            .await?;
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

fn to_timestamp(period: &ChartPeriod, limit: i32) -> Option<DateTime<Utc>> {
    let curr_timestamp = Utc::now();
    match period {
        ChartPeriod::Daily => {
            let shifted = shift_by_days(&curr_timestamp, limit)?;
            to_start_of_day(&shifted)
        }
        ChartPeriod::Monthly => {
            let shifted = shift_by_months(&curr_timestamp, limit)?;
            to_start_of_month(&shifted)
        }
    }
}

fn aggregate_candles(candles: &[Candle], period: &ChartPeriod, limit: i32) -> Vec<Candle> {
    let mut aggr_candles = VecDeque::new();
    let lowest_timestamp = to_timestamp(period, limit);
    if let Some(lowest_timestamp) = lowest_timestamp {
        let mut low = Decimal::MAX;
        let mut high = Decimal::MIN;
        let mut open = Decimal::ZERO;
        let mut close = Decimal::ZERO;
        let mut volume = Decimal::ZERO;
        let mut timestamp_start = None;
        let mut timestamp_end = None;
        for c in candles.iter().rev() {
            let timestamp = c.timestamp;
            if timestamp >= lowest_timestamp {
                if timestamp_start.is_none() && timestamp_end.is_none() {
                    match period {
                        ChartPeriod::Daily => {
                            timestamp_end = to_start_of_day(&timestamp);
                            if let Some(timestamp_end) = timestamp_end {
                                timestamp_start = shift_by_days(&timestamp_end, -1);
                            }
                        }
                        ChartPeriod::Monthly => {
                            timestamp_end = to_start_of_month(&timestamp);
                            if let Some(timestamp_end) = timestamp_end {
                                timestamp_start = shift_by_months(&timestamp_end, -1);
                            }
                        }
                    }
                    open = c.open;
                }
                if timestamp_start.is_some_and(|timestamp_start| timestamp >= timestamp_start)
                    && timestamp_end.is_some_and(|timestamp_end| timestamp < timestamp_end)
                {
                    low = min(low, c.low);
                    high = max(high, c.high);
                    close = c.close;
                    volume += c.volume.map_or(Decimal::ZERO, |v| v);
                } else {
                    if let Some(timestamp) = timestamp_end {
                        let aggregate = Candle {
                            timestamp,
                            open,
                            low,
                            high,
                            close,
                            volume: if volume > Decimal::ZERO {
                                Some(volume)
                            } else {
                                None
                            },
                        };
                        aggr_candles.push_front(aggregate);
                    }
                    timestamp_start = None;
                    timestamp_end = None;
                    volume = Decimal::ZERO;
                }
            } else {
                break;
            }
        }
    }
    Vec::from(aggr_candles)
}

fn shift_by_days(timestamp: &DateTime<Utc>, days_shift: i32) -> Option<DateTime<Utc>> {
    if days_shift >= 0 {
        timestamp.checked_add_signed(Duration::days(days_shift.to_i64()?))
    } else {
        timestamp.checked_sub_signed(Duration::days(-days_shift.to_i64()?))
    }
}

fn shift_by_months(timestamp: &DateTime<Utc>, months_shift: i32) -> Option<DateTime<Utc>> {
    if months_shift >= 0 {
        timestamp.checked_add_months(Months::new(months_shift.to_u32()?))
    } else {
        timestamp.checked_sub_months(Months::new(months_shift.to_u32()?))
    }
}

fn to_start_of_day(timestamp: &DateTime<Utc>) -> Option<DateTime<Utc>> {
    timestamp
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|naive_dt| naive_dt.and_utc())
}

fn to_start_of_month(timestamp: &DateTime<Utc>) -> Option<DateTime<Utc>> {
    timestamp
        .date_naive()
        .with_day(1)
        .and_then(|naive_dt| naive_dt.and_hms_opt(0, 0, 0))
        .map(|naive_dt| naive_dt.and_utc())
}

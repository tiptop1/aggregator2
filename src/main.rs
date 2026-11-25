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
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let config = Arc::new(read_configuration(&args.config_file).await?);
    let repo = Arc::new((RedbCandleRepository::new(&config.database_path)).await?);

    if args.load {
        load_candles(&config, &repo).await?;
    }

    if args.report {
        let mut email_builder = ReportEmailBuilder::default();
        email_builder.html("<html>");

        for symbol in config.symbols.iter() {
            email_builder.html(&format!("<h1>{}</h1>", &symbol.name));

            let mut last_candle = None;
            let mut min_timestamp = Some(DateTime::<Utc>::MAX_UTC);
            for report_element in symbol.report_elements.iter() {
                // Discover how many data get from repository
                match &report_element.element_type {
                    ReportElementType::LastCandle => {
                        last_candle = repo.last_candle(&symbol.name).await?;
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
            if let Some(min_timestamp) = min_timestamp {
                candles = repo.get(&symbol.name, min_timestamp..Utc::now()).await?;
            }

            for e in symbol.report_elements.iter() {
                let caption = &e.caption;
                email_builder.html(&format!("<h2>{}</h2>", caption));
                match &e.element_type {
                    ReportElementType::LastCandle => {
                        if let Some(candle) = last_candle.as_ref() {
                            email_builder.candle(candle);
                        }
                    }
                    ReportElementType::CandleChart { period, limit } => {
                        let chart_bytes = png_chart(
                            &aggregate_candles2(
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
                            period
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
        email_builder.html("</html>");
        email_builder
            .send("[Aggregator2] Report", &config.smtp)
            .await?;
    }

    Ok(())
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
            let shifted = shift_by_days(&curr_timestamp, -limit)?;
            to_start_of_day(&shifted)
        }
        ChartPeriod::Monthly => {
            let shifted = shift_by_months(&curr_timestamp, -limit)?;
            to_start_of_month(&shifted)
        }
    }
}

fn aggregate_candles2(candles: &[Candle], period: &ChartPeriod, limit: i32) -> Vec<Candle> {
    let mut aggregates = VecDeque::new();
    if let Some(lowest_timestamp) = to_timestamp(period, limit) {
        let mut curr_aggregate: Option<Candle> = None;
        let mut prev_aggregate = None;
        let mut prev_start = None;
        for curr_candle in candles.iter().rev().filter(|c| c.timestamp >= lowest_timestamp) {
            let curr_timestamp = curr_candle.timestamp;

            let curr_start = period_start(&curr_timestamp, period);
            if prev_start.is_none() {
                prev_start = curr_start.clone();
            }

            if prev_start == curr_start {
                if let Some(ca) = &mut curr_aggregate {
                    ca.low = min(ca.low, curr_candle.low);
                    ca.high = max(ca.high, curr_candle.high);
                    ca.close = curr_candle.close;
                    ca.volume = match ca.volume {
                        Some(v) => Some(v + curr_candle.volume.unwrap_or(Decimal::ZERO)),
                        None => curr_candle.volume,
                    };
                } else {
                    curr_aggregate = Some(curr_candle.clone());
                }
            } else {
                if let Some(aggregate) = prev_aggregate {
                    aggregates.push_front(aggregate);
                }
                curr_aggregate = Some(curr_candle.clone());               
                prev_start = curr_start;
            }
            prev_aggregate = curr_aggregate.clone();
          }
    }
    Vec::from(aggregates) 
}

fn period_start(timestamp: &DateTime<Utc>, period: &ChartPeriod) -> Option<DateTime<Utc>> {
    match period {
        ChartPeriod::Daily => to_start_of_day(timestamp),
        ChartPeriod::Monthly => to_start_of_month(timestamp)
    }
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
        timestamp.checked_sub_months(Months::new((-months_shift).to_u32()?))
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

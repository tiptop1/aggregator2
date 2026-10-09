use aggregator2::candle_provider::{CandleProvider, HttpCandleProvider};
use aggregator2::candle_repository::{CandleRepository, RedbCandleRepository};
use aggregator2::cli::Args;
use aggregator2::config::{Config, read_configuration};
use anyhow::Result;
use clap::Parser;
use reqwest::{
    Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
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

    // if args.report {
    //     let mut report = String::new();
    //     for s in &config.symbols {
    //         let report_elements = s.report_elements;
    //         let earlier_timestamp = find_earlier_timestamp(&report_elements);
    //         let candles = repo.get(&s.name, earlier_timestamp..Utc::now());
    //         for re in &report_elements {
    //             let report_type = re.report_element_type();
    //             match report_type {
    //                 ReportElementType::LastCandle => {}
    //                 ReportElementType::CandleChart { period, limit } => {}
    //             }
    //         }
    //     }
    //     send_report(report);
    // }

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

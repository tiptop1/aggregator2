use aggregator2::config::read_configuration;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::env::{args, var_os};

const CONFIG_FILE: &str = "config.toml";

#[tokio::main]
async fn main() -> Result<()> {
    let config_file = config_file();
    let config= read_configuration(&config_file)?;

    println!("Database path: {}", config.database_path);
    println!("Fields open: {}", config.symbols[0].candle_fields.open);
    
    Ok(())
}

/// Try to find config file in OUT_DIR (useful in development time),
/// if not found try to use config file from application argument,
/// if not found try to use config file from current directory
fn config_file() -> PathBuf {
match var_os("OUT_DIR") {
        Some(path) => PathBuf::from(path).join(CONFIG_FILE),
        None => {
            let args: Vec<String> = args().collect();
            if args.len() > 1 {
                // Get from index 1, because in 0 is the application path
                if let Some(arg1) = args.get(1) {
                    PathBuf::from(arg1)
                } else {
                    PathBuf::from(CONFIG_FILE)
                }
            } else {
                PathBuf::from(CONFIG_FILE)
            }
        }
    }
}
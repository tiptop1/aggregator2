use chrono::{DateTime, Utc};
use anyhow::Result;

use crate::domain::Candle;

#[async_trait::async_trait]
pub trait CandleProvider {
    async fn get_candles(
        &self, 
        symbol: &str, 
        from: DateTime<Utc>, 
        to: DateTime<Utc>
    ) -> Result<Vec<Candle>>; 
}
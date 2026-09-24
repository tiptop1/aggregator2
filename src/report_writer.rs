use crate::domain::Candle;
use anyhow::Result;
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait ReportElementWriter {
    async fn write_element(
        candles: &Vec<Candle>,
        config: &HashMap<String, String>,
    ) -> Result<String>;
}

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

pub struct Candle {
    pub timestamp: DateTime<Utc>,
    pub open: Decimal,
    pub low: Decimal,
    pub high: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>
}
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

pub struct Candle {
    timestamp: DateTime<Utc>,
    open: Decimal,
    low: Decimal,
    high: Decimal,
    close: Decimal,
    volume: Decimal
}
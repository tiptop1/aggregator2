use super::domain::Candle;
use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use redb::{Database, ReadableDatabase, TableDefinition};
use rust_decimal::Decimal;
use std::ops::{Bound, RangeBounds};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task;

type Tabdef = TableDefinition<'static, i64, &'static [u8]>;

#[async_trait::async_trait]
pub trait CandleRepository {
    async fn get<R>(&self, symbol: &str, range: R) -> Result<Vec<Candle>>
    where
        R: RangeBounds<DateTime<Utc>> + Send + 'static;

    async fn insert(&mut self, symbol: &str, candles: Vec<Candle>) -> Result<()>;
}

pub struct RedbCandleRepository {
    db: Arc<Database>,
}

#[async_trait::async_trait]
impl CandleRepository for RedbCandleRepository {
    async fn get<R>(&self, symbol: &str, range: R) -> Result<Vec<Candle>>
    where
        R: RangeBounds<DateTime<Utc>> + Send + 'static,
    {
        let db = self.db.clone();
        let symbol_owned = symbol.to_string();
        let handle = task::spawn_blocking(move || -> Result<Vec<Candle>> {
            let read_txn = db.begin_read()?;
            let tabdef: Tabdef = TableDefinition::new(Box::leak(symbol_owned.into_boxed_str()));
            let table = read_txn.open_table(tabdef)?;
            let iter = table.range(Self::to_millis_range(range))?;
            let mut candles = Vec::new();
            for record in iter {
                let result = record?;
                let millis = result.0.value();
                let timestamp_opt: Option<DateTime<Utc>> = DateTime::from_timestamp_millis(millis);
                let timestamp =
                    timestamp_opt.ok_or_else(|| anyhow!("could not create timesamp"))?;

                let fields = result.1.value();
                let fields_len = fields.len();
                let volume: Option<Decimal>;
                if fields_len == (16 * 4) {
                    volume = None;
                } else if fields_len == (16 * 5) {
                    let bytes: [u8; 16] = fields[fields_len - 16..].try_into()?;
                    volume = Some(Decimal::deserialize(bytes));
                } else {
                    bail!("Fields length invalid");
                }
                let open = Decimal::deserialize(fields[0..16].try_into()?);
                let low = Decimal::deserialize(fields[16..32].try_into()?);
                let high = Decimal::deserialize(fields[32..64].try_into()?);
                let close = Decimal::deserialize(fields[64..80].try_into()?);
                candles.push(Candle {
                    timestamp,
                    open,
                    low,
                    high,
                    close,
                    volume,
                });
            }
            Ok(candles)
        });
        Ok(handle.await??)
    }

    async fn insert(&mut self, symbol: &str, candles: Vec<Candle>) -> Result<()> {
        let db = self.db.clone();
        let symbol_owned = symbol.to_string();
        let candles_owned = candles.to_vec();

        let handle = task::spawn_blocking(move || -> Result<()> {
            let write_txt = db.begin_write()?;

            {
                let tabdef: Tabdef = TableDefinition::new(Box::leak(symbol_owned.into_boxed_str()));
                let mut table = write_txt.open_table(tabdef)?;
                for c in &candles_owned {
                    table.insert(Self::get_db_key(c), Self::get_db_value(c).as_slice())?;
                }
            }

            write_txt.commit()?;
            Ok(())
        });

        Ok(handle.await??)
    }
}

impl RedbCandleRepository {
    async fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path_buf: PathBuf = path.as_ref().to_path_buf();
        let handle = task::spawn_blocking(move || -> Result<RedbCandleRepository> {
            let db = Database::create(path_buf)?;
            Ok(Self { db: Arc::new(db) })
        });
        Ok(handle.await??)
    }

    fn to_millis_range<V>(range: V) -> impl RangeBounds<i64>
    where
        V: RangeBounds<DateTime<Utc>>,
    {
        let map_bound =
            |bound: Bound<&DateTime<Utc>>| -> Bound<i64> { bound.map(|dt| dt.timestamp_millis()) };
        let start_millis_bound = map_bound(range.start_bound());
        let end_millis_bound = map_bound(range.end_bound());
        (start_millis_bound, end_millis_bound)
    }

    fn get_db_key(candle: &Candle) -> i64 {
        candle.timestamp.timestamp_millis()
    }

    fn get_db_value(candle: &Candle) -> Vec<u8> {
        let mut value = Vec::with_capacity(if candle.volume.is_some() { 5 } else { 4 } * 16);
        value.extend(candle.open.serialize());
        value.extend(candle.low.serialize());
        value.extend(candle.high.serialize());
        value.extend(candle.close.serialize());
        if let Some(volume) = candle.volume {
            value.extend(volume.serialize());
        }
        value
    }
}

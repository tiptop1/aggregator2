use std::ops::RangeBounds;

use anyhow::Result;
use chrono::{DateTime, Utc};
use redb::Database;
use tokio::task;

#[async_trait::async_trait]
pub trait CandleRepository {
    async fn get<R>(&self, symbol: &str, range: R) -> Result<Vec<Candle>>
    where
        R: RangeBounds<DateTime<Utc>>;

    async fn insert(&mut self, symbol: &str, candles: Vec<Candle>) -> Result<()>;
}

pub struct RedbCandleRepository {
    db: Arc<Database>,
}

#[async_trait::async_trait]
impl CandleRepository for RedbCandleRepository {
    async fn get<R>(&self, symbol: &str, range: R) -> Result<Vec<Candle>>
    where
        R: RangeBounds<DateTime<Utc>>,
    {
        let handle = task::spawn_blocking(|| {
            let read_txn = db.begin_read()?;
            let table = read_txn.open_table(symbol)?;
            let range_iter = table.range(range)?;
            for record in range_iter {
                let result = entry?;
                let key = entry.0.value();
                let value = entry.1.value();
            }
        });
    }
}

#[async_trait::async_trait]
impl RedbCandleRepository {
    async fn new(path: impl AsRef<Path>) -> Result<Self> {
        let handle = task::spawn_blocking(|| {
            let db = Database::create(path)?;
            Self { db: Arc::new(db) }
        });
        Ok(handle.await?)
    }
}

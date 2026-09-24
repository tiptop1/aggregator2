use anyhow::{Result, anyhow, bail};
use plotters::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::path::{Path, PathBuf};

use crate::domain::Candle;

#[async_trait::async_trait]
pub trait CandleChartRenderer {
    async fn render(&self, candles: &Vec<Candle>) -> Result<()>;
}

#[derive(Debug)]
pub struct FileCandleChartRenderer {
    path: PathBuf,
    caption: String,
    width: u32,
    height: u32,
}

#[async_trait::async_trait]
impl CandleChartRenderer for FileCandleChartRenderer {
    async fn render(&self, candles: &Vec<Candle>) -> Result<()> {
        let candles_len = candles.len();
        if candles_len > 0 {
            let root =
                BitMapBackend::new(&self.path, (self.width, self.height)).into_drawing_area();
            root.fill(&WHITE)?;

            let first_candle = candles
                .get(0)
                .ok_or(anyhow!("there is no candle in index 0"))?;
            let last_index = candles_len - 1;
            let last_candle = candles
                .get(last_index)
                .ok_or(anyhow!("there is no candle in index {}", last_index))?;

            let mut chart = ChartBuilder::on(&root)
                .caption(&self.caption, ("sans-serif", 30))
                .margin(10)
                .x_label_area_size(40)
                .y_label_area_size(40)
                .build_cartesian_2d(
                    first_candle.timestamp..last_candle.timestamp,
                    Decimal::ZERO.to_f64().unwrap()
                        ..Self::find_max_price(candles).to_f64().unwrap(),
                )?;

            chart.configure_mesh().draw()?;

            // 4. Rysowanie świec
            chart.draw_series(candles.into_iter().map(|c| {
                CandleStick::new(
                    c.timestamp,
                    c.open.to_f64().unwrap(),
                    c.high.to_f64().unwrap(),
                    c.low.to_f64().unwrap(),
                    c.close.to_f64().unwrap(),
                    GREEN.filled(),
                    RED.filled(),
                    15,
                )
            }))?;

            root.present()?;
            Ok(())
        } else {
            bail!("there is no candles");
        }
    }
}

impl FileCandleChartRenderer {
    fn new(path: impl AsRef<Path>, caption: &str, width: u32, height: u32) -> Self {
        FileCandleChartRenderer {
            path: path.as_ref().to_path_buf(),
            caption: String::from(caption),
            width,
            height,
        }
    }

    fn find_max_price(candles: &Vec<Candle>) -> Decimal {
        let mut max_price = Decimal::ZERO;
        for c in candles {
            let curr_max_price = [c.open, c.low, c.high, c.close].into_iter().max().unwrap();
            if max_price < curr_max_price {
                max_price = curr_max_price;
            }
        }
        max_price
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    #[ignore = "test generates image file to visual verification"]
    async fn generate_chart_without_volume() {
        let candles = vec![
            candle!(
                "2026-09-18 22:15:00",
                79123.12,
                78013.12,
                79500.79,
                78500.00
            ),
            candle!(
                "2026-09-18 22:16:00",
                80000.00,
                79000.15,
                79512.79,
                79500.13
            ),
            candle!(
                "2026-09-18 22:17:00",
                80050.00,
                78345.15,
                78900.79,
                79000.13
            ),
        ];
        let renderer =
            FileCandleChartRenderer::new("candle_chart_without_volme.png", "BTC", 800, 600);
        renderer.render(&candles).await.unwrap();
    }
}

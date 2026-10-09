use crate::{config::SmtpConfig, domain::Candle};
use anyhow::{Result, anyhow, bail};
use image::{ColorType, ImageFormat};
use lettre::message::{MultiPart, SinglePart, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use plotters::prelude::*;
use plotters::style::full_palette::{GREEN_500, RED_500};
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use std::fmt::Write;
use std::io::Cursor;

const MARGIN: f64 = 0.1;

async fn png_chart(candles: &[Candle], caption: &str, width: u32, height: u32) -> Result<Vec<u8>> {
    let candles_len = candles.len();
    if candles_len > 0 {
        let mut raw_pixels = vec![0u8; (width * height * 3) as usize];
        {
            let root =
                BitMapBackend::with_buffer(&mut raw_pixels, (width, height)).into_drawing_area();
            root.fill(&WHITE)?;

            let first_candle = candles
                .first()
                .ok_or(anyhow!("there is no candle in index 0"))?;
            let last_index = candles_len - 1;
            let last_candle = candles
                .get(last_index)
                .ok_or(anyhow!("there is no candle in index {}", last_index))?;

            let mut min_price = Decimal::MAX;
            let mut max_price = Decimal::MIN;
            for c in candles {
                let low_price = c.low;
                if min_price > low_price {
                    min_price = low_price;
                }
                let high_price = c.high;
                if max_price < high_price {
                    max_price = high_price;
                }
            }

            let mut chart = ChartBuilder::on(&root)
                .caption(caption, ("sans-serif", 30))
                .margin(10)
                .x_label_area_size(40)
                .y_label_area_size(40)
                .build_cartesian_2d(
                    first_candle.timestamp..last_candle.timestamp,
                    (min_price * Decimal::from_f64(1.0 - MARGIN).unwrap())
                        .to_f64()
                        .unwrap()
                        ..(max_price * Decimal::from_f64(1.0 + MARGIN).unwrap())
                            .to_f64()
                            .unwrap(),
                )?;

            chart.configure_mesh().draw()?;

            chart.draw_series(candles.iter().map(|c| {
                CandleStick::new(
                    c.timestamp,
                    c.open.to_f64().unwrap(),
                    c.high.to_f64().unwrap(),
                    c.low.to_f64().unwrap(),
                    c.close.to_f64().unwrap(),
                    GREEN_500.filled(),
                    RED_500.filled(),
                    15,
                )
            }))?;

            root.present()?;
        }
        let mut png_bytes = Vec::new();
        image::write_buffer_with_format(
            &mut Cursor::new(&mut png_bytes),
            &raw_pixels,
            width,
            height,
            ColorType::Rgb8,
            ImageFormat::Png,
        )?;
        Ok(png_bytes)
    } else {
        bail!("there is no candles");
    }
}

pub struct Image {
    bytes: Vec<u8>,
    cid: String,
    content_type: String,
}

pub struct ReportEmailBuilder {
    html_body: String,
    images: Vec<Image>,
}

impl ReportEmailBuilder {
    fn builder() -> Self {
        ReportEmailBuilder {
            html_body: String::new(),
            images: Vec::new(),
        }
    }

    fn write_fmt(&mut self, args: std::fmt::Arguments) {
        self.html_body
            .write_fmt(args)
            .expect("failed to write to String");
    }

    fn html(mut self, html: &str) -> Self {
        self.html_body.push_str(html);
        self
    }

    fn candle(mut self, candle: &Candle) -> Self {
        self.write_fmt(
            format_args!(
                "<p>timestamp: {}<br>open price: {}<br>high price: {}<br>low price: {}<br>close price: {}<br>",
                candle.timestamp.format("%Y-%m-%d %H:%M:%S"),
                candle.open,
                candle.high,
                candle.low,
                candle.close
            ),
        );
        if let Some(volume) = candle.volume {
            self.write_fmt(format_args!("volume: {}<br>", volume));
        }
        self.html_body.push_str("</p>");
        self
    }

    fn image(mut self, image: Image) -> Self {
        self.write_fmt(format_args!("<img src=\"cid:{}\" />", &image.cid));
        self.images.push(image);
        self
    }

    async fn send(self, subject: &str, smtp_config: &SmtpConfig) -> Result<()> {
        let mut multipart = MultiPart::related().singlepart(SinglePart::html(self.html_body));
        for i in self.images {
            let image_part = SinglePart::builder()
                .header(ContentType::parse(&i.content_type)?)
                // Kluczowe: ustawiamy Content-ID w ujęciu <id>
                .header(lettre::message::header::ContentId::from(format!(
                    "<{}>",
                    i.cid
                )))
                .body(i.bytes);
            multipart = multipart.singlepart(image_part);
        }

        let email = Message::builder()
            .from(smtp_config.from_email.parse()?)
            .to(smtp_config.to_emails.parse()?)
            .subject(subject)
            .multipart(multipart)?;

        // 5. Wysyłka SMTP
        let creds = Credentials::new(smtp_config.username.clone(), smtp_config.password.clone());
        let mailer: AsyncSmtpTransport<Tokio1Executor> =
            AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_config.server)?
                .credentials(creds)
                .build();

        mailer.send(email).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs::write;

    #[tokio::test]
    #[ignore = "test generates image file to visual verification"]
    async fn generate_chart_without_volume() {
        let candles = vec![
            candle!(
                "2026-09-18 22:15:00",
                79123.12,
                79013.12,
                78000.79,
                78500.00
            ),
            candle!(
                "2026-09-18 22:16:00",
                80000.00,
                79000.15,
                86512.79,
                85500.13
            ),
            candle!(
                "2026-09-18 22:17:00",
                80050.00,
                78345.15,
                78900.79,
                79000.13
            ),
            candle!(
                "2026-09-18 22:18:00",
                85050.00,
                98345.15,
                100900.79,
                99000.13
            ),
            candle!(
                "2026-09-18 22:19:00",
                80050.00,
                78345.15,
                78900.79,
                79000.13
            ),
            candle!(
                "2026-09-18 22:20:00",
                80050.00,
                78345.15,
                78900.79,
                79000.13
            ),
        ];
        let png_buffer = png_chart(&candles, "BTC", 800, 600).await.unwrap();
        write("candle_chart_without_volume.png", png_buffer)
            .await
            .unwrap();
    }
}

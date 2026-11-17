//! Local render benchmark for collages and genre charts.
//!
//! Measures only CPU work (cover decode/resize + canvas drawing + PNG encode).
//! Network downloads and Telegram uploads are intentionally excluded; those
//! are tracked by the bot's Prometheus histograms in production.

use collage::{BarItem, CollageItem};
use image::ImageFormat;
use std::io::Cursor;
use std::time::{Duration, Instant};

fn main() {
    let dir = tempfile::tempdir().expect("tempdir");
    let items = synthetic_collage(100);
    let bars: Vec<BarItem> = (0..10)
        .map(|index| BarItem {
            label: format!("Genre {index}"),
            value: 100 - index as u64 * 7,
        })
        .collect();

    let mut collage_samples = Vec::new();
    let mut chart_samples = Vec::new();
    for run in 0..5 {
        let path = dir.path().join(format!("collage-{run}.png"));
        let started = Instant::now();
        collage::render("Top tracks · 3 months", &items, 10, &path).expect("collage");
        collage_samples.push(started.elapsed());

        let path = dir.path().join(format!("chart-{run}.png"));
        let started = Instant::now();
        collage::render_bars("Your genres", "25 tracks · 10 genres", &bars, &path).expect("chart");
        chart_samples.push(started.elapsed());
    }

    report("collage 10x10 (100 covers)", &mut collage_samples);
    report("genre chart (10 bars)", &mut chart_samples);
}

fn synthetic_collage(count: usize) -> Vec<CollageItem> {
    (0..count)
        .map(|index| {
            // Distinct covers so the benchmark measures real decode work, not
            // just the duplicate-cover cache.
            let color = image::Rgb([
                (index * 37 % 256) as u8,
                (index * 53 % 256) as u8,
                (index * 97 % 256) as u8,
            ]);
            let image = image::RgbImage::from_pixel(400, 400, color);
            let mut encoded = Vec::new();
            image
                .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Jpeg)
                .expect("encode cover");
            CollageItem {
                name: format!("Track {index}"),
                secondary: Some("Artist".to_owned()),
                playcount: 1000 - index as u64,
                cover: Some(encoded.into()),
            }
        })
        .collect()
}

fn report(label: &str, samples: &mut [Duration]) {
    samples.sort();
    let total: Duration = samples.iter().sum();
    let avg = total / samples.len() as u32;
    let p50 = samples[samples.len() / 2];
    let max = samples[samples.len() - 1];
    println!("{label}");
    println!("  runs: {}", samples.len());
    println!("  avg:  {avg:?}");
    println!("  p50:  {p50:?}");
    println!("  max:  {max:?}");
}

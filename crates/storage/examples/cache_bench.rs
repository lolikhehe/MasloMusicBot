//! Microbenchmark for the cache layer: measures SQLite roundtrips for
//! `get_track` (the hot path for a cache hit) and `touch_track`.
//!
//! This is *only* the database component of S1 from docs/perf.md. It does NOT
//! measure the Telegram sendAudio roundtrip; that needs live traffic and is
//! tracked separately.

use common::domain::{CachedTrack, Provider, TrackKey, TrackMeta};
use std::time::Instant;
use storage::Db;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let db = Db::connect(&format!("sqlite://{}", dir.path().join("bench.db").display())).await?;

    const N: usize = 100;
    for i in 0..N {
        let key = TrackKey::new(format!("youtube:bench-{i}"));
        let track = CachedTrack {
            key,
            file_id: format!("file_id_{i}"),
            file_unique_id: format!("unique_{i}"),
            source_url: Some(format!("https://www.youtube.com/watch?v=bench{i}")),
            provider: Provider::YouTube,
            meta: TrackMeta {
                title: format!("Bench Track {i}"),
                performer: Some("Bench Artist".into()),
                album: Some("Bench Album".into()),
                duration_secs: Some(180),
                thumbnail_url: Some("https://example.test/cover.jpg".into()),
            },
        };
        db.put_track(&track, 192, Some(8_000_000)).await?;
    }

    let iterations = 1_000usize;
    let mut get_samples = Vec::with_capacity(iterations);
    let mut touch_samples = Vec::with_capacity(iterations);
    for i in 0..iterations {
        let key = TrackKey::new(format!("youtube:bench-{}", i % N));
        let t = Instant::now();
        let _ = db.get_track(&key, 192).await?;
        get_samples.push(t.elapsed());

        let t = Instant::now();
        db.touch_track(&key, 192).await?;
        touch_samples.push(t.elapsed());
    }

    report("get_track (cache hot path read)", &mut get_samples);
    report("touch_track (cache hit bookkeeping)", &mut touch_samples);
    Ok(())
}

fn report(label: &str, samples: &mut [std::time::Duration]) {
    samples.sort();
    let total: std::time::Duration = samples.iter().sum();
    let avg = total / samples.len() as u32;
    let p50 = samples[samples.len() / 2];
    let p95 = samples[samples.len() * 95 / 100];
    let max = samples[samples.len() - 1];
    println!("{label}");
    println!("  iterations: {}", samples.len());
    println!("  avg:        {avg:?}");
    println!("  p50:        {p50:?}");
    println!("  p95:        {p95:?}");
    println!("  max:        {max:?}");
}

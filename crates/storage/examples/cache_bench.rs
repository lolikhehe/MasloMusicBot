//! Microbenchmark for the cache layer: measures Postgres roundtrips for
//! `get_track` (the hot path for a cache hit), `touch_track` and `ensure_user`.
//!
//! This is *only* the database component of S1 from docs/perf.md. It does NOT
//! measure the Telegram sendAudio roundtrip; that needs live traffic and is
//! tracked separately. Run against a throwaway database:
//!
//! ```sh
//! DATABASE_URL=postgresql://musicbot:devpass@127.0.0.1:55432/bench \
//!   cargo run --release -p storage --example cache_bench
//! ```

use common::domain::{CachedTrack, Lang, Provider, TrackKey, TrackMeta};
use std::time::Instant;
use storage::Db;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Db::connect(
        &std::env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL must point at a throwaway Postgres database")?,
    )
    .await?;

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
                genres: vec!["Benchmark".into()],
            },
            chat_id: None,
            message_id: None,
        };
        db.put_track(&track, 192, Some(8_000_000), None, None)
            .await?;
    }

    let iterations = 1_000usize;
    let mut get_samples = Vec::with_capacity(iterations);
    let mut touch_samples = Vec::with_capacity(iterations);
    let mut user_samples = Vec::with_capacity(iterations);
    for i in 0..iterations {
        let key = TrackKey::new(format!("youtube:bench-{}", i % N));
        let t = Instant::now();
        let _ = db.get_track(&key, 192).await?;
        get_samples.push(t.elapsed());

        let t = Instant::now();
        db.touch_track(&key, 192).await?;
        touch_samples.push(t.elapsed());

        let t = Instant::now();
        let _ = db.ensure_user(i as i64 % 50, Lang::En).await?;
        user_samples.push(t.elapsed());
    }

    report("get_track (cache hot path read)", &mut get_samples);
    report("touch_track (cache hit bookkeeping)", &mut touch_samples);
    report("ensure_user (message hot path)", &mut user_samples);
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

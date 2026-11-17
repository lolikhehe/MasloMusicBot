//! End-to-end tests: the real `music-bot` binary against a mock Telegram Bot
//! API, a mock lyrics API and a throwaway Postgres schema.
//!
//! Run with:
//!   DATABASE_URL=postgresql://musicbot:devpass@127.0.0.1:55432/musicbot \
//!     cargo test -p app --test e2e
//! The slow load harness is ignored by default:
//!   cargo test -p app --test e2e -- --ignored --nocapture

mod common;

use common::{MockServer, inject_text, lyrics_api, telegram_api};
use serde_json::Value;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use storage::Db;

const DEV_URL: &str = "postgresql://musicbot:devpass@127.0.0.1:55432/musicbot";

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEV_URL.to_owned())
}

struct Bot {
    child: Child,
    api: MockServer,
    lyrics: MockServer,
}

impl Drop for Bot {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.api.stop();
        self.lyrics.stop();
    }
}

async fn spawn_bot() -> Bot {
    let api = telegram_api().await;
    let lyrics = lyrics_api().await;
    let child = Command::new(env!("CARGO_BIN_EXE_music-bot"))
        .env("TELOXIDE_TOKEN", "123:mock-token")
        .env("TELEGRAM_API_URL", format!("http://{}", api.addr))
        .env("DATABASE_URL", database_url())
        .env("LRCLIB_BASE_URL", format!("http://{}", lyrics.addr))
        .env("LYRICS_OVH_BASE_URL", format!("http://{}/v1", lyrics.addr))
        .env("METRICS_ADDR", "off")
        .env("RATE_LIMIT_PER_MIN", "10000")
        .env("RATE_LIMIT_INLINE_PER_MIN", "10000")
        .env("LOG_LEVEL", "warn")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn music-bot");
    let bot = Bot { child, api, lyrics };

    wait_until(20_000, || async {
        let calls = bot.api.shared.calls.lock().expect("calls").clone();
        calls.iter().any(|call| call == "getme") && calls.iter().any(|call| call == "getupdates")
    })
    .await
    .unwrap_or_else(|| {
        let calls = bot.api.shared.calls.lock().expect("calls").clone();
        panic!("bot did not connect to the mock API; calls={calls:?}")
    });
    bot
}

async fn wait_until<F, Fut>(timeout_ms: u64, mut condition: F) -> Option<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        if condition().await {
            return Some(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    None
}

fn messages(api: &MockServer) -> Vec<Value> {
    api.shared.messages.lock().expect("messages").clone()
}

#[tokio::test]
async fn help_command_is_answered() {
    let bot = spawn_bot().await;
    inject_text(&bot.api, 1, 101, "/help");

    wait_until(10_000, || async { !messages(&bot.api).is_empty() })
        .await
        .expect("no answer to /help");
    let text = messages(&bot.api)[0]["text"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    assert!(
        text.contains("How to use") || text.contains("Як користуватися"),
        "unexpected help text: {text}"
    );
    assert!(
        text.contains("/lyrics"),
        "lyrics command must be documented"
    );
}

#[tokio::test]
async fn lyrics_command_uses_public_mock_sources() {
    let bot = spawn_bot().await;
    inject_text(&bot.api, 2, 102, "/lyrics Artist — Title");

    wait_until(10_000, || async {
        messages(&bot.api).iter().any(|message| {
            message["text"]
                .as_str()
                .is_some_and(|t| t.contains("Hello from mock"))
        })
    })
    .await
    .expect("no lyrics answer");
    let text = messages(&bot.api)
        .iter()
        .find(|message| {
            message["text"]
                .as_str()
                .is_some_and(|t| t.contains("Hello from mock"))
        })
        .expect("lyrics message")["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(text.contains("LRCLIB"), "source must be shown: {text}");
    assert!(
        text.contains("Hello from mock"),
        "timestamp stripped: {text}"
    );
    assert!(!text.contains("[00:"), "LRC tags must be removed: {text}");
    assert!(text.contains("@maslougazIA"), "developer credit: {text}");
}

#[tokio::test]
async fn cached_track_is_sent_without_download() {
    let bot = spawn_bot().await;
    let db = Db::connect(&database_url()).await.expect("db");
    let key = providers::key_from_input("https://www.youtube.com/watch?v=e2e_cached_track");
    let track = common_domain_track(&key);
    db.put_track(&track, 192, Some(1_234_567))
        .await
        .expect("put cached track");

    inject_text(
        &bot.api,
        3,
        103,
        "https://www.youtube.com/watch?v=e2e_cached_track",
    );

    wait_until(10_000, || async {
        !bot.api.shared.audios.lock().expect("audios").is_empty()
    })
    .await
    .expect("no audio sent from cache");
    let audio = &bot.api.shared.audios.lock().expect("audios")[0];
    assert!(audio.to_string().contains("MOCK_FILE_ID"));
    let calls = bot.api.shared.calls.lock().expect("calls").clone();
    assert_eq!(
        calls.iter().filter(|call| **call == "sendaudio").count(),
        1,
        "exactly one cached audio send expected"
    );
}

fn common_domain_track(key: &common_domain::TrackKey) -> common_domain::CachedTrack {
    common_domain::CachedTrack {
        key: key.clone(),
        file_id: "MOCK_FILE_ID".to_owned(),
        file_unique_id: "MOCK_UNIQUE_ID".to_owned(),
        source_url: Some("https://www.youtube.com/watch?v=e2e_cached_track".to_owned()),
        provider: common_domain::Provider::YouTube,
        meta: common_domain::TrackMeta {
            title: "Cached Track".to_owned(),
            performer: Some("Mock Artist".to_owned()),
            duration_secs: Some(10),
            ..Default::default()
        },
    }
}

/// Local mock load harness. These numbers describe the whole mocked stack
/// (binary + mock Bot API + Postgres on localhost) and are NOT S1–S4 figures.
#[tokio::test]
#[ignore = "load harness; run explicitly"]
async fn load_harness_help_burst() {
    let bot = spawn_bot().await;
    const N: u64 = 200;
    let mut injected = Vec::with_capacity(N as usize);
    for index in 0..N {
        let at = Instant::now();
        inject_text(&bot.api, 10_000 + index, 200 + index as i64, "/help");
        injected.push(at);
    }
    wait_until(60_000, || async { messages(&bot.api).len() >= N as usize })
        .await
        .expect("load harness did not receive all answers");

    let answered = messages(&bot.api);
    let mut latencies: Vec<Duration> = answered
        .iter()
        .take(N as usize)
        .zip(injected.iter())
        .map(|(_, at)| at.elapsed())
        .collect();
    latencies.sort();
    let percentile = |p: f64| -> f64 {
        let index = ((latencies.len() - 1) as f64 * p) as usize;
        latencies[index].as_secs_f64() * 1000.0
    };
    let summary = serde_json::json!({
        "kind": "local mock",
        "scenario": "/help burst, real binary + mock Bot API + local Postgres",
        "requests": N,
        "p50_ms": percentile(0.50),
        "p95_ms": percentile(0.95),
        "p99_ms": percentile(0.99),
        "note": "NOT S1-S4; no real Telegram traffic or provider network"
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/e2e-load.json");
    std::fs::write(&path, serde_json::to_string_pretty(&summary).expect("json"))
        .expect("write load summary");
    println!("{}", serde_json::to_string_pretty(&summary).expect("json"));
}

// Re-export so the helper above can use the public domain path explicitly.
mod common_domain {
    pub use common::domain::*;
}

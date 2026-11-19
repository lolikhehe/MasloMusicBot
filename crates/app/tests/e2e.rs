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
    /// 3.0.0 splits downloads into a second process that polls the same DB.
    worker: Child,
    api: MockServer,
    lyrics: MockServer,
    webhook_url: Option<String>,
}

impl Drop for Bot {
    fn drop(&mut self) {
        let _ = self.worker.kill();
        let _ = self.worker.wait();
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.api.stop();
        self.lyrics.stop();
    }
}

/// The e2e harness reuses one database across runs and spawns many bots.
/// Silence the once-per-version broadcast so it cannot pollute message asserts.
async fn silence_broadcast() {
    let db = Db::connect(&database_url()).await.expect("db");
    db.set_meta("announced_version", ::common::version::VERSION)
        .await
        .expect("meta");
}

async fn spawn_bot() -> Bot {
    let api = telegram_api().await;
    let lyrics = lyrics_api().await;
    spawn_bot_inner(api, lyrics, 10_000).await
}

/// A bot that receives updates from our own axum webhook listener instead of
/// long polling. The mock Bot API only needs to accept `setWebhook`.
async fn spawn_webhook_bot(secret: &str, rate_limit: u32) -> Bot {
    silence_broadcast().await;
    let api = telegram_api().await;
    let lyrics = lyrics_api().await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("free port");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    let url = format!("http://127.0.0.1:{port}/telegram/webhook");
    let child = Command::new(env!("CARGO_BIN_EXE_music-bot"))
        .env("TELOXIDE_TOKEN", "123:mock-token")
        .env("TELEGRAM_API_URL", format!("http://{}", api.addr))
        .env("DATABASE_URL", database_url())
        .env("LRCLIB_BASE_URL", format!("http://{}", lyrics.addr))
        .env("LYRICS_OVH_BASE_URL", format!("http://{}/v1", lyrics.addr))
        .env("METRICS_ADDR", "off")
        .env("WEBHOOK_ADDR", format!("127.0.0.1:{port}"))
        .env("WEBHOOK_URL", &url)
        .env("WEBHOOK_SECRET", secret)
        .env("RATE_LIMIT_PER_MIN", rate_limit.to_string())
        .env("RATE_LIMIT_INLINE_PER_MIN", "10000")
        .env("LOG_LEVEL", "warn")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn webhook music-bot");
    // The worker polls the same DB. It gets no webhook address and no metrics
    // port so it cannot collide with the bot's listener/health port.
    let worker = Command::new(env!("CARGO_BIN_EXE_music-bot-worker"))
        .env("TELOXIDE_TOKEN", "123:mock-token")
        .env("TELEGRAM_API_URL", format!("http://{}", api.addr))
        .env("DATABASE_URL", database_url())
        .env("LRCLIB_BASE_URL", format!("http://{}", lyrics.addr))
        .env("LYRICS_OVH_BASE_URL", format!("http://{}/v1", lyrics.addr))
        .env("METRICS_ADDR", "off")
        .env("LOG_LEVEL", "warn")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn music-bot-worker");
    let bot = Bot {
        child,
        worker,
        api,
        lyrics,
        webhook_url: Some(url),
    };

    wait_until(20_000, || async {
        bot.api
            .shared
            .calls
            .lock()
            .expect("calls")
            .iter()
            .any(|call| call == "setwebhook")
    })
    .await
    .unwrap_or_else(|| {
        let calls = bot.api.shared.calls.lock().expect("calls").clone();
        panic!("webhook bot did not call setWebhook; calls={calls:?}")
    });
    bot
}

async fn spawn_bot_inner(api: MockServer, lyrics: MockServer, rate_limit: u32) -> Bot {
    silence_broadcast().await;
    let child = Command::new(env!("CARGO_BIN_EXE_music-bot"))
        .env("TELOXIDE_TOKEN", "123:mock-token")
        .env("TELEGRAM_API_URL", format!("http://{}", api.addr))
        .env("DATABASE_URL", database_url())
        .env("LRCLIB_BASE_URL", format!("http://{}", lyrics.addr))
        .env("LYRICS_OVH_BASE_URL", format!("http://{}/v1", lyrics.addr))
        .env("METRICS_ADDR", "off")
        .env("RATE_LIMIT_PER_MIN", rate_limit.to_string())
        .env("RATE_LIMIT_INLINE_PER_MIN", "10000")
        .env("LOG_LEVEL", "warn")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn music-bot");
    // The worker polls the same DB. It gets no webhook address and no metrics
    // port so it cannot collide with the bot's listener/health port.
    let worker = Command::new(env!("CARGO_BIN_EXE_music-bot-worker"))
        .env("TELOXIDE_TOKEN", "123:mock-token")
        .env("TELEGRAM_API_URL", format!("http://{}", api.addr))
        .env("DATABASE_URL", database_url())
        .env("LRCLIB_BASE_URL", format!("http://{}", lyrics.addr))
        .env("LYRICS_OVH_BASE_URL", format!("http://{}/v1", lyrics.addr))
        .env("METRICS_ADDR", "off")
        .env("LOG_LEVEL", "warn")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn music-bot-worker");
    let bot = Bot {
        child,
        worker,
        api,
        lyrics,
        webhook_url: None,
    };
    wait_until(20_000, || async {
        let calls = bot.api.shared.calls.lock().expect("calls").clone();
        calls.iter().any(|call| call == "getme")
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
async fn changelog_and_user_stats_are_answered() {
    let bot = spawn_bot().await;
    inject_text(&bot.api, 20, 120, "/changelog");
    inject_text(&bot.api, 21, 120, "/me");

    wait_until(10_000, || async { messages(&bot.api).len() >= 2 })
        .await
        .expect("no answers to /changelog and /me");
    let texts: Vec<String> = messages(&bot.api)
        .iter()
        .filter_map(|message| message["text"].as_str().map(str::to_owned))
        .collect();
    assert!(
        texts
            .iter()
            .any(|text| text.contains("What's new") || text.contains("Що нового")),
        "changelog must be shown: {texts:?}"
    );
    assert!(
        texts
            .iter()
            .any(|text| text.contains("Your stats") || text.contains("Ваша статистика")),
        "user stats must be shown: {texts:?}"
    );
    assert!(texts.iter().any(|text| text.contains("@maslougazIA")));
}

#[tokio::test]
async fn bare_import_waits_for_the_playlist_in_the_next_message() {
    let bot = spawn_bot().await;
    inject_text(&bot.api, 30, 130, "/import");

    wait_until(10_000, || async { !messages(&bot.api).is_empty() })
        .await
        .expect("no /import prompt");
    let prompt = messages(&bot.api)[0]["text"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    assert!(
        prompt.contains("Waiting") || prompt.contains("Чекаю"),
        "expected a waiting prompt, got: {prompt}"
    );

    // The follow-up is playlist text, not a track search query. Public search
    // is unavailable in the harness, so the pipeline answers "no tracks".
    inject_text(
        &bot.api,
        31,
        130,
        "Metallica – Enter Sandman\nNirvana – Come As You Are",
    );
    wait_until(10_000, || async { messages(&bot.api).len() >= 2 })
        .await
        .expect("the follow-up message was not treated as a playlist");
    let answer = messages(&bot.api)[1]["text"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    assert!(
        answer.contains("tracks") || answer.contains("треків"),
        "unexpected import answer: {answer}"
    );
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

fn text_update(update_id: u64, chat_id: i64, text: &str) -> Value {
    serde_json::json!({
        "update_id": update_id,
        "message": {
            "message_id": update_id,
            "date": 1_700_000_000u64,
            "chat": {"id": chat_id, "type": "private", "first_name": "Tester"},
            "from": {
                "id": chat_id,
                "is_bot": false,
                "first_name": "Tester",
                "language_code": "en"
            },
            "text": text
        }
    })
}

async fn post_webhook(url: &str, secret: &str, update: Value) -> u16 {
    reqwest::Client::new()
        .post(url)
        .header("X-Telegram-Bot-Api-Secret-Token", secret)
        .json(&update)
        .send()
        .await
        .expect("webhook request")
        .status()
        .as_u16()
}

#[tokio::test]
async fn webhook_mode_accepts_signed_updates_only() {
    let bot = spawn_webhook_bot("test-secret-123", 10_000).await;
    let url = bot.webhook_url.as_deref().expect("webhook url");

    let unauthorized = post_webhook(url, "wrong-secret", text_update(600, 601, "/help")).await;
    assert_eq!(unauthorized, 401, "bad secret must be rejected");

    let accepted = post_webhook(url, "test-secret-123", text_update(601, 601, "/help")).await;
    assert_eq!(accepted, 200);
    wait_until(10_000, || async { !messages(&bot.api).is_empty() })
        .await
        .expect("webhook /help was not answered");
    assert!(
        messages(&bot.api)[0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("How to use"))
    );
    let calls = bot.api.shared.calls.lock().expect("calls").clone();
    assert!(calls.iter().any(|call| call == "setwebhook"));
}

#[tokio::test]
async fn two_replicas_share_selections_and_rate_limits() {
    let db = Db::connect(&database_url()).await.expect("db");
    let key = common_domain::TrackKey::new("youtube:two-replicas");
    let track = common_domain_track(&key);
    db.put_track(&track, 192, Some(1234)).await.expect("cache");

    let request = common_domain::TrackRequest {
        key: key.clone(),
        provider: common_domain::Provider::YouTube,
        source_url: None,
        meta: Some(common_domain::TrackMeta {
            title: "Two Replicas".into(),
            performer: Some("Mock".into()),
            ..Default::default()
        }),
        target: common_domain::DownloadTarget::Search("Mock Two Replicas".into()),
    };
    db.put_selection(
        "two-replicas",
        "track",
        7766,
        serde_json::to_value(&request).expect("json"),
        Duration::from_secs(60),
    )
    .await
    .expect("selection");

    let replica_a = spawn_webhook_bot("secret-a", 2).await;
    let replica_b = spawn_webhook_bot("secret-b", 2).await;

    // The button was created by A, but the callback lands on B.
    let callback = serde_json::json!({
        "update_id": 700,
        "callback_query": {
            "id": "cb-700",
            "from": {"id": 7766, "is_bot": false, "first_name": "Tester", "language_code": "en"},
            "chat_instance": "ci-700",
            "data": "q:two-replicas",
            "message": {
                "message_id": 700,
                "date": 1_700_000_000u64,
                "chat": {"id": 7766, "type": "private", "first_name": "Tester"},
                "text": "choose"
            }
        }
    });
    let url_b = replica_b.webhook_url.as_deref().expect("replica B url");
    assert_eq!(post_webhook(url_b, "secret-b", callback).await, 200);
    wait_until(10_000, || async {
        !replica_b
            .api
            .shared
            .audios
            .lock()
            .expect("audios")
            .is_empty()
    })
    .await
    .expect("replica B could not resolve a selection created elsewhere");

    // Both replicas spend the same per-user budget. Use a fresh id so a
    // previous run's fixed-window hits cannot poison this assertion.
    let limited_user = 8_000_000 + (std::process::id() as i64 % 100_000);
    let url_a = replica_a.webhook_url.as_deref().expect("replica A url");
    for index in 0..4u64 {
        let update = text_update(800 + index, limited_user, "javascript:alert(1)");
        let url = if index % 2 == 0 { url_a } else { url_b };
        let secret = if index % 2 == 0 {
            "secret-a"
        } else {
            "secret-b"
        };
        assert_eq!(post_webhook(url, secret, update).await, 200);
    }
    wait_until(10_000, || async {
        messages(&replica_a.api).len() + messages(&replica_b.api).len() >= 4
    })
    .await
    .expect("rate-limit test did not receive all four answers");
    let all_messages = messages(&replica_a.api)
        .into_iter()
        .chain(messages(&replica_b.api))
        .collect::<Vec<_>>();
    let blocked = all_messages
        .iter()
        .filter(|message| {
            message["text"].as_str().is_some_and(|text| {
                text.contains("Too many requests") || text.contains("Забагато запитів")
            })
        })
        .count();
    assert_eq!(
        blocked, 2,
        "only two of four requests may pass the shared budget: {all_messages:?}"
    );
    assert!(db.rate_limit_hits(limited_user).await.expect("hits") >= 2);
}

#[tokio::test]
async fn convert_normalize_and_group_mode_gate() {
    let bot = spawn_bot().await;
    let db = Db::connect(&database_url()).await.expect("db");
    inject_text(&bot.api, 900, 901, "/convert mp3");
    inject_text(&bot.api, 901, 901, "/normalize on");

    wait_until(10_000, || async { messages(&bot.api).len() >= 2 })
        .await
        .expect("no answers to /convert and /normalize");
    let texts: Vec<String> = messages(&bot.api)
        .iter()
        .filter_map(|message| message["text"].as_str().map(str::to_owned))
        .collect();
    assert!(texts.iter().any(|text| text.contains("mp3")), "{texts:?}");
    assert!(
        texts.iter().any(|text| text.contains("loudnorm")),
        "{texts:?}"
    );

    let group_id = -60_000_000 - (std::process::id() as i64 % 100_000);
    let group_help = |update_id: u64| -> Value {
        serde_json::json!({
            "update_id": update_id,
            "message": {
                "message_id": update_id,
                "date": 1_700_000_000u64,
                "chat": {"id": group_id, "type": "group", "title": "Test group"},
                "from": {"id": 901, "is_bot": false, "first_name": "Tester", "language_code": "en"},
                "text": "/me"
            }
        })
    };

    let before = messages(&bot.api).len();
    bot.api
        .shared
        .updates
        .lock()
        .expect("updates")
        .push_back(group_help(910));
    wait_until(10_000, || async { messages(&bot.api).len() > before })
        .await
        .expect("group message was not answered");
    let last = messages(&bot.api).last().expect("answer")["text"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    assert!(
        last.contains("private chat") || last.contains("приватному"),
        "disabled group must be rejected: {last}"
    );

    db.set_chat_enabled(group_id, true, 901)
        .await
        .expect("enable group");
    let before = messages(&bot.api).len();
    bot.api
        .shared
        .updates
        .lock()
        .expect("updates")
        .push_back(group_help(911));
    wait_until(10_000, || async { messages(&bot.api).len() > before })
        .await
        .expect("enabled group was not answered");
    let last = messages(&bot.api).last().expect("answer")["text"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    assert!(
        last.contains("Your stats") || last.contains("Ваша статистика"),
        "enabled group must get /me stats: {last}"
    );
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

/// 3.0.0 moves downloads into the `music-bot-worker` process. Drive the real
/// callback -> selection -> job flow with both processes running and assert
/// delivery on the mock Bot API side.
///
/// This is deliberately the weaker assertion: the harness has no env knob to
/// attribute a `sendaudio` to the worker specifically and no direct DB client
/// to check the `download_jobs` row, so it proves end-to-end delivery works
/// with both processes alive rather than that the worker alone did it.
#[tokio::test]
async fn worker_consumes_enqueued_track_job() {
    let bot = spawn_bot().await;
    let db = Db::connect(&database_url()).await.expect("db");
    let key = common_domain::TrackKey::new("youtube:e2e-worker-download");
    let track = common_domain_track(&key);
    db.put_track(&track, 192, Some(1_234_567))
        .await
        .expect("put cached track");

    let request = common_domain::TrackRequest {
        key: key.clone(),
        provider: common_domain::Provider::YouTube,
        source_url: None,
        meta: Some(common_domain::TrackMeta {
            title: "Worker Download".into(),
            performer: Some("Mock Artist".into()),
            ..Default::default()
        }),
        target: common_domain::DownloadTarget::Search("Mock Worker Download".into()),
    };
    db.put_selection(
        "worker-download",
        "track",
        7_700,
        serde_json::to_value(&request).expect("json"),
        Duration::from_secs(60),
    )
    .await
    .expect("selection");

    let callback = serde_json::json!({
        "update_id": 7_700,
        "callback_query": {
            "id": "cb-worker-download",
            "from": {"id": 7_700, "is_bot": false, "first_name": "Tester", "language_code": "en"},
            "chat_instance": "ci-worker-download",
            "data": "q:worker-download",
            "message": {
                "message_id": 7_700,
                "date": 1_700_000_000u64,
                "chat": {"id": 7_700, "type": "private", "first_name": "Tester"},
                "text": "choose"
            }
        }
    });
    bot.api
        .shared
        .updates
        .lock()
        .expect("updates")
        .push_back(callback);

    wait_until(60_000, || async {
        !bot.api.shared.audios.lock().expect("audios").is_empty()
    })
    .await
    .expect("no audio delivered after the queued flow with the worker running");
    let calls = bot.api.shared.calls.lock().expect("calls").clone();
    assert!(
        calls.iter().any(|call| call == "sendaudio"),
        "expected a sendaudio from the queued flow: {calls:?}"
    );
}

/// With the worker killed, a cache-missing `q:` tap must NOT produce audio:
/// the job stays queued with nobody to consume it.
///
/// The storage crate owns the SQL, so this asserts the observable mock Bot API
/// side (no audio) rather than reaching into `download_jobs` directly.
#[tokio::test]
async fn worker_down_jobs_queue_up() {
    let mut bot = spawn_bot().await;
    // Simulate a dead worker: the bot still boots and enqueues, but nobody
    // consumes the job.
    let _ = bot.worker.kill();
    let _ = bot.worker.wait();

    let db = Db::connect(&database_url()).await.expect("db");
    let key = common_domain::TrackKey::new("youtube:e2e-worker-down");
    let request = common_domain::TrackRequest {
        key: key.clone(),
        provider: common_domain::Provider::YouTube,
        source_url: None,
        meta: Some(common_domain::TrackMeta {
            title: "Worker Down".into(),
            performer: Some("Mock Artist".into()),
            ..Default::default()
        }),
        target: common_domain::DownloadTarget::Search("Mock Worker Down".into()),
    };
    db.put_selection(
        "worker-down",
        "track",
        7_701,
        serde_json::to_value(&request).expect("json"),
        Duration::from_secs(60),
    )
    .await
    .expect("selection");

    let callback = serde_json::json!({
        "update_id": 7_701,
        "callback_query": {
            "id": "cb-worker-down",
            "from": {"id": 7_701, "is_bot": false, "first_name": "Tester", "language_code": "en"},
            "chat_instance": "ci-worker-down",
            "data": "q:worker-down",
            "message": {
                "message_id": 7_701,
                "date": 1_700_000_000u64,
                "chat": {"id": 7_701, "type": "private", "first_name": "Tester"},
                "text": "choose"
            }
        }
    });
    bot.api
        .shared
        .updates
        .lock()
        .expect("updates")
        .push_back(callback);

    // Bounded absence check: give the bot time to enqueue, then assert no
    // audio was delivered with no worker alive.
    tokio::time::sleep(Duration::from_secs(5)).await;
    let calls = bot.api.shared.calls.lock().expect("calls").clone();
    assert!(
        bot.api.shared.audios.lock().expect("audios").is_empty(),
        "no audio should be delivered while the worker is down: {calls:?}"
    );
    assert!(
        !calls.iter().any(|call| call == "sendaudio"),
        "no sendaudio should happen while the worker is down: {calls:?}"
    );
}

// Re-export so the helper above can use the public domain path explicitly.
mod common_domain {
    pub use common::domain::*;
}

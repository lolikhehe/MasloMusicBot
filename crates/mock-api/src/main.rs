//! Standalone mock Telegram Bot API for the k6 load stand.
//!
//! The bot replicas poll this service through `TELEGRAM_API_URL`; k6 pushes
//! updates to `POST /inject`. Because `getUpdates` pops from one shared queue,
//! every injected update is delivered to exactly one replica — the same
//! semantics as real long polling, without contacting Telegram.

use mock_api::{
    Shared, bot_api_response, json_response, lyrics_response, text_response, text_update,
};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr: SocketAddr = std::env::var("MOCK_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8081".to_owned())
        .parse()
        .expect("MOCK_ADDR must be host:port");
    let message_id = Arc::new(AtomicU64::new(100));
    let update_id = Arc::new(AtomicU64::new(1_000));

    let server = mock_api::MockServer::start_on(addr, move |request, shared| {
        let path = request.path.split('?').next().unwrap_or("");
        let started = Instant::now();
        match path {
            "/healthz" => Some(json_response(json!({"status": "ok"}))),
            // Alertmanager webhook sink for the self-contained load stand.
            "/alert" => {
                let alerts: Value = serde_json::from_str(&request.body)
                    .unwrap_or_else(|_| json!({"raw": request.body}));
                if let Some(items) = alerts["alerts"].as_array() {
                    for alert in items {
                        let name = alert["labels"]["alertname"].as_str().unwrap_or("unknown");
                        let status = alert["status"].as_str().unwrap_or("unknown");
                        eprintln!("[alert] {status} {name}");
                    }
                }
                Some(json_response(json!({"ok": true})))
            }
            "/metrics" => Some(text_response(&metrics(shared))),
            "/stats" => Some(json_response(json!({
                "injected": shared.injected.load(Ordering::Relaxed),
                "replies": shared.replies.load(Ordering::Relaxed),
                "queued": shared.updates.lock().expect("updates").len(),
            }))),
            "/inject" if request.method == "POST" => {
                let result: Value = serde_json::from_str(&request.body)
                    .unwrap_or_else(|_| json!({"user_id": 0, "text": ""}));
                let user_id = result["user_id"].as_i64().unwrap_or(0);
                let text = result["text"].as_str().unwrap_or("/help");
                let id = update_id.fetch_add(1, Ordering::Relaxed);
                shared
                    .updates
                    .lock()
                    .expect("updates")
                    .push_back(text_update(id, user_id, text));
                shared.injected.fetch_add(1, Ordering::Relaxed);
                let nanos = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
                shared
                    .inject_seconds_sum
                    .fetch_add(nanos, Ordering::Relaxed);
                Some(json_response(json!({"ok": true, "update_id": id})))
            }
            _ if path.starts_with("/api/") || path.starts_with("/v1/") => {
                lyrics_response(request)
            }
            _ => bot_api_response(request, shared, &message_id),
        }
    })
    .await?;

    println!("mock Bot API listening on http://{}", server.addr);
    // Keep the server task alive until the process is stopped.
    tokio::signal::ctrl_c().await?;
    server.stop();
    Ok(())
}

fn metrics(shared: &Shared) -> String {
    let injected = shared.injected.load(Ordering::Relaxed);
    let replies = shared.replies.load(Ordering::Relaxed);
    let queued = shared.updates.lock().expect("updates").len();
    let seconds = shared.inject_seconds_sum.load(Ordering::Relaxed) as f64 / 1e9;
    format!(
        "# HELP mock_injected_total Updates accepted from the load generator.\n\
         # TYPE mock_injected_total counter\n\
         mock_injected_total {injected}\n\
         # HELP mock_replies_total Messages/audios sent by bot replicas.\n\
         # TYPE mock_replies_total counter\n\
         mock_replies_total {replies}\n\
         # HELP mock_queue_length Updates waiting to be polled.\n\
         # TYPE mock_queue_length gauge\n\
         mock_queue_length {queued}\n\
         # HELP mock_inject_seconds_sum Time spent enqueueing updates.\n\
         # TYPE mock_inject_seconds_sum counter\n\
         mock_inject_seconds_sum {seconds:.6}\n"
    )
}

//! Mock Telegram Bot API + mock lyrics API.
//!
//! Used in two places:
//! * `crates/app/tests/e2e.rs` spawns the real bot against an in-process mock;
//! * the `mock-bot-api` binary runs the same code as a standalone service for
//!   the k6 load stand (`TELEGRAM_API_URL` / `LRCLIB_BASE_URL` point at it).
//!
//! No public Telegram endpoint and no real lyrics provider is ever contacted.

#![allow(dead_code)]

use serde_json::{Value, json};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[derive(Default)]
pub struct Shared {
    pub updates: Mutex<VecDeque<Value>>,
    pub messages: Mutex<Vec<Value>>,
    pub audios: Mutex<Vec<Value>>,
    pub calls: Mutex<Vec<String>>,
    /// Counters exported by `/metrics` in the standalone load-test service.
    pub injected: AtomicU64,
    pub replies: AtomicU64,
    pub inject_seconds_sum: AtomicU64,
}

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub body: String,
}

pub struct MockServer {
    pub addr: SocketAddr,
    pub shared: Arc<Shared>,
    task: JoinHandle<()>,
}

impl MockServer {
    /// Bind an ephemeral loopback port (used by the e2e tests).
    pub async fn start(
        handler: impl Fn(&HttpRequest, &Shared) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        Self::start_on(
            "127.0.0.1:0".parse().expect("loopback addr"),
            handler,
        )
        .await
        .expect("bind mock server")
    }

    pub async fn start_on(
        addr: SocketAddr,
        handler: impl Fn(&HttpRequest, &Shared) -> Option<String> + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let shared = Arc::new(Shared::default());
        let handler = Arc::new(handler);
        let shared_for_task = shared.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let handler = handler.clone();
                let shared = shared_for_task.clone();
                tokio::spawn(async move {
                    serve_connection(&mut socket, handler.as_ref(), shared.as_ref()).await;
                });
            }
        });
        Ok(Self { addr, shared, task })
    }

    pub fn stop(&self) {
        self.task.abort();
    }
}

/// Minimal HTTP/1.1 reader. One connection per request, `Connection: close`.
async fn serve_connection(
    socket: &mut tokio::net::TcpStream,
    handler: &(impl Fn(&HttpRequest, &Shared) -> Option<String> + Send + Sync),
    shared: &Shared,
) {
    let mut raw = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let Ok(read) = socket.read(&mut chunk).await else {
            return;
        };
        if read == 0 {
            return;
        }
        raw.extend_from_slice(&chunk[..read]);
        if let Some(pos) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            break Some(pos + 4);
        }
        if raw.len() > 1024 * 1024 {
            return;
        }
    };
    let Some(header_end) = header_end else {
        return;
    };
    let head = String::from_utf8_lossy(&raw[..header_end]).to_string();
    let mut line = head.lines().next().unwrap_or("").split_whitespace();
    let method = line.next().unwrap_or("").to_owned();
    let path = line.next().unwrap_or("/").to_owned();
    let content_length = head
        .lines()
        .find_map(|header| {
            let (name, value) = header.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0)
        .min(4 * 1024 * 1024);
    while raw.len() < header_end + content_length {
        let Ok(read) = socket.read(&mut chunk).await else {
            break;
        };
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    let body = String::from_utf8_lossy(
        &raw[header_end..(header_end + content_length).min(raw.len())],
    )
    .to_string();
    let request = HttpRequest { method, path, body };
    let response = handler(&request, shared)
        .unwrap_or_else(|| "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned());
    let _ = socket.write_all(response.as_bytes()).await;
}

pub fn json_response(value: Value) -> String {
    let body = value.to_string();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

pub fn text_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn api_method(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// The Bot API half of the mock. Kept as a plain function so both the e2e
/// helper and the standalone service use exactly the same behavior.
pub fn bot_api_response(
    request: &HttpRequest,
    shared: &Shared,
    message_id: &AtomicU64,
) -> Option<String> {
    let method = api_method(&request.path);
    shared.calls.lock().expect("calls").push(method.clone());
    match method.as_str() {
        "getme" => Some(json_response(json!({
            "ok": true,
            "result": {
                "id": 999,
                "is_bot": true,
                "first_name": "Mock",
                "username": "MockMusicBot",
                "can_join_groups": true,
                "can_read_all_group_messages": false,
                "supports_inline_queries": true,
                "can_connect_to_business": false,
                "has_main_web_app": false
            }
        }))),
        // Teloxide's polling listener probes this at startup. The catch-all
        // `{"result": true}` cannot be deserialized as WebhookInfo and produced
        // a misleading ERROR log on every replica.
        "getwebhookinfo" => Some(json_response(json!({
            "ok": true,
            "result": {
                "url": "",
                "has_custom_certificate": false,
                "pending_update_count": 0,
                "allowed_updates": ["message"]
            }
        }))),
        "getupdates" => {
            let update = shared.updates.lock().expect("updates").pop_front();
            Some(json_response(json!({
                "ok": true,
                "result": update.into_iter().collect::<Vec<_>>()
            })))
        }
        "sendmessage" | "sendaudio" | "sendphoto" | "editmessagetext" => {
            let payload: Value = serde_json::from_str(&request.body)
                .unwrap_or_else(|_| json!({"raw": request.body}));
            let chat_id = payload["chat_id"].clone();
            let id = message_id.fetch_add(1, Ordering::Relaxed);
            if method == "sendmessage" {
                shared.messages.lock().expect("messages").push(payload.clone());
            } else if method == "sendaudio" {
                shared.audios.lock().expect("audios").push(payload.clone());
            }
            shared.replies.fetch_add(1, Ordering::Relaxed);
            Some(json_response(json!({
                "ok": true,
                "result": {
                    "message_id": id,
                    "date": 1_700_000_000u64,
                    "chat": {
                        "id": chat_id,
                        "type": "private",
                        "first_name": "Tester"
                    },
                    "text": "",
                    "audio": {
                        "file_id": "MOCK_FILE_ID",
                        "file_unique_id": "MOCK_UNIQUE_ID",
                        "duration": 1
                    }
                }
            })))
        }
        _ => Some(json_response(json!({"ok": true, "result": true}))),
    }
}

/// Mock LRCLIB + lyrics.ovh. `/api/get` intentionally returns no lyrics so the
/// provider falls through to `/api/search`, which carries a synced text.
pub fn lyrics_response(request: &HttpRequest) -> Option<String> {
    if request.path.starts_with("/api/get") {
        Some(json_response(json!({
            "id": 1, "name": "Title", "artistName": "Artist",
            "plainLyrics": null, "syncedLyrics": null, "instrumental": false
        })))
    } else if request.path.starts_with("/api/search") {
        Some(json_response(json!([{
            "id": 1, "name": "Title", "artistName": "Artist",
            "plainLyrics": null,
            "syncedLyrics": "[00:01.00]Hello from mock\n[00:02.00]Second line",
            "instrumental": false
        }])))
    } else if request.path.starts_with("/v1/") {
        Some(json_response(json!({"lyrics": "Fallback from ovh"})))
    } else {
        None
    }
}

/// In-process Bot API for the e2e tests.
pub async fn telegram_api() -> MockServer {
    let message_id = Arc::new(AtomicU64::new(100));
    MockServer::start(move |request, shared| bot_api_response(request, shared, &message_id)).await
}

/// In-process lyrics API for the e2e tests.
pub async fn lyrics_api() -> MockServer {
    MockServer::start(|request, _| lyrics_response(request)).await
}

/// Enqueue a private-chat text update (used by the e2e assertions).
pub fn inject_text(api: &MockServer, update_id: u64, chat_id: i64, text: &str) {
    api.shared
        .updates
        .lock()
        .expect("updates")
        .push_back(text_update(update_id, chat_id, text));
}

pub fn text_update(update_id: u64, chat_id: i64, text: &str) -> Value {
    json!({
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

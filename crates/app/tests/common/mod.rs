//! Mock Telegram Bot API + mock lyrics API for end-to-end tests.
//!
//! The real `music-bot` binary is spawned against these servers. No public
//! Telegram endpoint and no real lyrics provider is contacted.

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
    pub async fn start(
        handler: impl Fn(&HttpRequest, &Shared) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let shared = Arc::new(Shared::default());
        let handler = Arc::new(handler);
        let shared_for_task = shared.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut raw = Vec::with_capacity(4096);
                let mut chunk = [0u8; 4096];
                let header_end = loop {
                    let Ok(read) = socket.read(&mut chunk).await else {
                        break None;
                    };
                    if read == 0 {
                        break None;
                    }
                    raw.extend_from_slice(&chunk[..read]);
                    if let Some(pos) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
                        break Some(pos + 4);
                    }
                };
                let Some(header_end) = header_end else {
                    continue;
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
                    .unwrap_or(0);
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
                let response = handler(&request, &shared_for_task).unwrap_or_else(|| {
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_owned()
                });
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        Self { addr, shared, task }
    }

    pub fn stop(&self) {
        self.task.abort();
    }
}

pub fn json_response(value: Value) -> String {
    let body = value.to_string();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Mock Telegram Bot API. Long-polling `getUpdates` hands out injected updates;
/// every outbound request is recorded for assertions.
pub async fn telegram_api() -> MockServer {
    let message_id = Arc::new(AtomicU64::new(100));
    MockServer::start(move |request, shared| {
        let api_method = request
            .path
            .rsplit('/')
            .next()
            .unwrap_or("")
            .split('?')
            .next()
            .unwrap_or("")
            .to_owned();
        let api_method = api_method.to_ascii_lowercase();
        shared.calls.lock().expect("calls").push(api_method.clone());
        match api_method.as_str() {
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
            "getupdates" => {
                let update = shared.updates.lock().expect("updates").pop_front();
                Some(json_response(json!({
                    "ok": true,
                    "result": update.into_iter().collect::<Vec<_>>()
                })))
            }
            "sendmessage" => {
                let payload: Value = serde_json::from_str(&request.body)
                    .unwrap_or_else(|_| json!({"raw": request.body}));
                let chat_id = payload["chat_id"].clone();
                let id = message_id.fetch_add(1, Ordering::Relaxed);
                shared.messages.lock().expect("messages").push(payload);
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
                        "text": ""
                    }
                })))
            }
            "sendaudio" => {
                let payload: Value = serde_json::from_str(&request.body)
                    .unwrap_or_else(|_| json!({"raw": request.body}));
                let chat_id = payload["chat_id"].clone();
                let id = message_id.fetch_add(1, Ordering::Relaxed);
                shared.audios.lock().expect("audios").push(payload);
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
    })
    .await
}

/// Mock LRCLIB + lyrics.ovh. `/api/get` returns no lyrics so the provider goes
/// through `/api/search` and the synced text there is used.
pub async fn lyrics_api() -> MockServer {
    MockServer::start(move |request, _| {
        if request.path.starts_with("/api/get") {
            Some(json_response(json!({
                "id": 1,
                "name": "Title",
                "artistName": "Artist",
                "plainLyrics": null,
                "syncedLyrics": null,
                "instrumental": false
            })))
        } else if request.path.starts_with("/api/search") {
            Some(json_response(json!([{
                "id": 1,
                "name": "Title",
                "artistName": "Artist",
                "plainLyrics": null,
                "syncedLyrics": "[00:01.00]Hello from mock\n[00:02.00]Second line",
                "instrumental": false
            }])))
        } else if request.path.starts_with("/v1/") {
            Some(json_response(json!({"lyrics": "Fallback from ovh"})))
        } else {
            None
        }
    })
    .await
}

/// Inject a private-chat message update with the given text.
pub fn inject_text(api: &MockServer, update_id: u64, chat_id: i64, text: &str) {
    api.shared
        .updates
        .lock()
        .expect("updates")
        .push_back(json!({
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
        }));
}

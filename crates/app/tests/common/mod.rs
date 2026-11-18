//! Mock Bot API / lyrics API for end-to-end tests.
//!
//! The implementation lives in the `mock-api` crate so the load stand's
//! standalone service and these tests exercise exactly the same behavior.

#![allow(dead_code)]

pub use mock_api::{MockServer, inject_text, lyrics_api, telegram_api};

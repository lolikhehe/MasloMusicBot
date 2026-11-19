//! In-memory album archiving: a ZIP writer with a byte budget and a filename
//! sanitizer. Audio is already compressed, so entries are STORED (no deflate).

use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use common::error::{AppError, AppResult};

/// Hard caps for the "album as one ZIP" feature. Local Bot API allows up to
/// 2 GB uploads; these limits stay far below that.
pub const MAX_ZIP_TRACKS: usize = 25;
pub const MAX_ZIP_BYTES: u64 = 900_000_000;

pub struct ZipOutcome {
    pub bytes: Vec<u8>,
    pub tracks: usize,
}

/// A ZIP writer over an in-memory tokio sink, bounded by a byte budget counted
/// as the sum of the raw entry sizes (archive overhead is negligible at our
/// scale).
pub struct ZipWriter {
    writer: ZipFileWriter<Vec<u8>>,
    budget: u64,
    used: u64,
}

impl ZipWriter {
    pub fn new(budget: u64) -> Self {
        Self {
            writer: ZipFileWriter::with_tokio(Vec::new()),
            budget,
            used: 0,
        }
    }

    pub async fn add_file(&mut self, name: &str, data: &[u8]) -> AppResult<()> {
        let next = self.used.saturating_add(data.len() as u64);
        if next > self.budget {
            return Err(AppError::Message(format!(
                "zip too large: {next} bytes would exceed the {} byte budget",
                self.budget
            )));
        }
        let entry = ZipEntryBuilder::new(name.to_owned().into(), Compression::Stored);
        self.writer
            .write_entry_whole(entry, data)
            .await
            .map_err(|error| AppError::Other(Box::new(error)))?;
        self.used = next;
        Ok(())
    }

    pub async fn finish(self) -> AppResult<Vec<u8>> {
        let sink = self
            .writer
            .close()
            .await
            .map_err(|error| AppError::Other(Box::new(error)))?;
        Ok(sink.into_inner())
    }
}

/// True when a track count cannot be zipped. Kept as a helper so the cap rule
/// is testable without building an archive.
pub(crate) fn exceeds_track_cap(count: usize) -> bool {
    count > MAX_ZIP_TRACKS
}

/// `{idx:02} - {sanitized}.{ext}` with path separators, quotes and control
/// characters removed, whitespace collapsed and the title capped at 120 chars.
pub fn zip_entry_name(idx: usize, title: &str, ext: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | '"' | '\'' | '<' | '>' => ' ',
            ch if ch.is_control() => ' ',
            ch => ch,
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let sanitized: String = if collapsed.is_empty() {
        "track".to_owned()
    } else {
        collapsed.chars().take(120).collect()
    };
    format!("{idx:02} - {sanitized}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_two_entries_with_zip_header() {
        let mut writer = ZipWriter::new(MAX_ZIP_BYTES);
        writer
            .add_file("01 - a.m4a", b"aaa")
            .await
            .expect("first entry");
        writer
            .add_file("02 - b.m4a", b"bbb")
            .await
            .expect("second entry");
        let bytes = writer.finish().await.expect("finish");
        assert!(bytes.starts_with(b"PK\x03\x04"));
        let outcome = ZipOutcome { bytes, tracks: 2 };
        assert_eq!(outcome.tracks, 2);
    }

    #[tokio::test]
    async fn budget_rejects_entry_that_crosses_cap() {
        let mut writer = ZipWriter::new(4);
        writer
            .add_file("a", b"123")
            .await
            .expect("first entry fits");
        let error = writer
            .add_file("b", b"123")
            .await
            .expect_err("second exceeds budget");
        assert!(error.to_string().contains("too large"));
    }

    #[test]
    fn sanitizer_strips_unsafe_characters_and_handles_empty_title() {
        let name = zip_entry_name(1, "a/b \\ \" < > c", "m4a");
        assert!(name.starts_with("01 - "));
        assert!(name.ends_with(".m4a"));
        for unsafe_char in ['/', '\\', '"', '\'', '<', '>'] {
            assert!(!name.contains(unsafe_char), "must strip {unsafe_char:?}");
        }
        assert!(!name.contains("  "), "spaces must be collapsed");
        assert_eq!(zip_entry_name(2, "", "mp3"), "02 - track.mp3");
    }

    #[test]
    fn track_cap_logic() {
        assert!(!exceeds_track_cap(MAX_ZIP_TRACKS));
        assert!(exceeds_track_cap(MAX_ZIP_TRACKS + 1));
    }
}

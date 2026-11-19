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
        Self::with_capacity(budget, 0)
    }

    /// Reserve the output buffer up front so a large archive doesn't repeatedly
    /// reallocate (and transiently hold two copies of a growing 900 MB Vec).
    pub fn with_capacity(budget: u64, capacity: usize) -> Self {
        Self {
            writer: ZipFileWriter::with_tokio(Vec::with_capacity(capacity)),
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

/// Unicode `Cf` (format) characters such as the bidi override U+202E, zero-
/// width joiners and the BOM. They have no visible width but can spoof a
/// filename, so they are stripped just like control characters.
fn is_format_char(ch: char) -> bool {
    matches!(ch,
        '\u{00AD}' | '\u{0600}'..='\u{0605}' | '\u{061C}' | '\u{06DD}' |
        '\u{070F}' | '\u{08E2}' | '\u{180E}' |
        '\u{200B}'..='\u{200F}' |
        '\u{202A}'..='\u{202E}' |
        '\u{2060}'..='\u{2064}' |
        '\u{2066}'..='\u{206F}' |
        '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}'
    )
}

/// Strip path/quote/control/format characters, collapse whitespace and cap the
/// result at `byte_budget` UTF-8 bytes without splitting a code point (a CJK
/// title is ~3 bytes per char, so a char count would overflow the ZIP limit).
fn sanitize_component(input: &str, byte_budget: usize) -> String {
    let cleaned: String = input
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | '"' | '\'' | '<' | '>' => ' ',
            ch if ch.is_control() || is_format_char(ch) => ' ',
            ch => ch,
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() <= byte_budget {
        return collapsed;
    }
    let mut end = byte_budget;
    while !collapsed.is_char_boundary(end) {
        end -= 1;
    }
    collapsed[..end].to_owned()
}

/// `{idx:02} - {sanitized}.{ext}` with unsafe characters removed, whitespace
/// collapsed and the title capped at a ~200-byte UTF-8 budget. The extension is
/// sanitized the same way so a malicious title can't escape through it.
pub fn zip_entry_name(idx: usize, title: &str, ext: &str) -> String {
    let sanitized = sanitize_component(title, 200);
    let sanitized = if sanitized.is_empty() {
        "track".to_owned()
    } else {
        sanitized
    };
    let ext = sanitize_component(ext, 16);
    let ext = if ext.is_empty() {
        "m4a".to_owned()
    } else {
        ext
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
    fn sanitizer_caps_by_utf8_bytes_not_chars() {
        // 100 CJK chars = 300 bytes; must be cut to <= 200 bytes on a char
        // boundary (66 chars = 198 bytes), never mid-codepoint.
        let title = "音".repeat(100);
        let name = zip_entry_name(3, &title, "m4a");
        let stem = name
            .strip_prefix("03 - ")
            .unwrap()
            .strip_suffix(".m4a")
            .unwrap();
        assert!(stem.len() <= 200, "stem was {} bytes", stem.len());
        assert!(stem.chars().all(|c| c == '音'), "no half code points");
        assert_eq!(stem.chars().count(), 66);
    }

    #[test]
    fn sanitizer_strips_format_chars_including_from_extension() {
        // U+202E (bidi override), U+200D (ZWJ) and U+FEFF (BOM) are invisible
        // format chars that must not survive in the name or the extension.
        let name = zip_entry_name(4, "good\u{202E}bad\u{200D}\u{FEFF}title", "m4\u{202E}a");
        assert!(!name.contains('\u{202E}'));
        assert!(!name.contains('\u{200D}'));
        assert!(!name.contains('\u{FEFF}'));
        assert!(
            name.ends_with(".m4 a"),
            "format char in ext becomes a space: {name}"
        );
    }

    #[test]
    fn track_cap_logic() {
        assert!(!exceeds_track_cap(MAX_ZIP_TRACKS));
        assert!(exceeds_track_cap(MAX_ZIP_TRACKS + 1));
    }
}

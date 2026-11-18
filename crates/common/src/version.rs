//! Release identity and the user-facing changelog.
//!
//! Bump `VERSION` and edit `PATCH_NOTES.md` in the SAME commit as every
//! user-visible change. On startup the bot compares VERSION with the value in
//! the `meta` table and PMs the notes to every known user exactly once per
//! version — a restart with the same build never spams anyone twice.

/// Current release. Bump on every deploy users should hear about.
pub const VERSION: &str = "2.6.3";

/// Plain-text changelog shown to users when VERSION changes. Escaped to HTML
/// before sending; kept at the repo root so releases are easy to edit.
pub const PATCH_NOTES: &str = include_str!("../../../PATCH_NOTES.md");

/// Appended to the caption of every delivered track.
pub const DEV_CHANNEL_LINE: &str = "канал розробника - @maslougazIA";
/// Short handle for compact inline captions: `@bot | channel`.
pub const DEV_CHANNEL_HANDLE: &str = "@maslougazIA";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_metadata_is_consistent() {
        assert!(!VERSION.is_empty());
        assert!(
            PATCH_NOTES.contains(VERSION),
            "PATCH_NOTES.md must mention the current VERSION"
        );
        assert!(DEV_CHANNEL_LINE.contains("@maslougazIA"));
    }
}

//! Translations for the extra UI languages. Keys are the English source
//! strings from `i18n.rs`; English itself is the fallback. Placeholders such
//! as `{nick}` and HTML tags must be preserved verbatim by every translation.

mod be;
mod cn;
mod cz;
mod de;
mod fr;
mod jp;
mod nl;
mod pl;

/// Look up a translated string by language code and English source text.
pub fn lookup(code: &str, english: &str) -> Option<&'static str> {
    match code {
        "be" => be::lookup(english),
        "cn" => cn::lookup(english),
        "cz" => cz::lookup(english),
        "de" => de::lookup(english),
        "fr" => fr::lookup(english),
        "jp" => jp::lookup(english),
        "nl" => nl::lookup(english),
        "pl" => pl::lookup(english),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_extra_language_has_the_core_strings() {
        for code in ["be", "cn", "cz", "de", "fr", "jp", "nl", "pl"] {
            assert!(lookup(code, "English").is_some(), "{code}");
            assert!(lookup(code, "🌐 Language").is_some(), "{code}");
            assert!(lookup(code, "✅ Settings saved.").is_some(), "{code}");
            assert!(
                lookup(
                    code,
                    "😕 Could not read that feed. Check the URL and try again."
                )
                .is_some(),
                "{code}"
            );
            assert!(
                lookup(code, "🎯 <b>Recommended for you</b>").is_some(),
                "{code}"
            );
            assert!(lookup(code, "not a real key").is_none(), "{code}");
        }
    }

    // Keys must stay on one source line: this scanner reads only the first literal per line.
    fn keys_in(src: &'static str) -> Vec<&'static str> {
        let mut keys = Vec::new();
        for line in src.lines() {
            let line = line.trim_start();
            if !line.starts_with('"') || !line.contains("=>") {
                continue;
            }
            let mut escaped = false;
            let mut end = None;
            for (i, ch) in line.char_indices() {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' && i != 0 {
                    end = Some(i);
                    break;
                }
            }
            if let Some(end) = end {
                keys.push(&line[1..end]);
            }
        }
        keys.sort_unstable();
        keys
    }

    #[test]
    fn all_locales_have_the_same_keys() {
        let sources = [
            ("be", include_str!("locales/be.rs")),
            ("cn", include_str!("locales/cn.rs")),
            ("cz", include_str!("locales/cz.rs")),
            ("de", include_str!("locales/de.rs")),
            ("fr", include_str!("locales/fr.rs")),
            ("jp", include_str!("locales/jp.rs")),
            ("nl", include_str!("locales/nl.rs")),
            ("pl", include_str!("locales/pl.rs")),
        ];
        let expected = keys_in(sources[0].1);
        assert!(!expected.is_empty());
        for (code, src) in sources {
            let keys = keys_in(src);
            assert!(!keys.is_empty(), "{code}");
            assert_eq!(keys, expected, "{code}");
        }
    }
}

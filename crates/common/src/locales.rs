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
            assert!(lookup(code, "not a real key").is_none(), "{code}");
        }
    }
}

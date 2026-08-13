//! Internationalization: loads the MComix3 `.mo` message catalogs (embedded
//! at build time) and resolves the user's language, mirroring
//! `mcomix/i18n.py` + `mcomix/messages`. No libintl dependency.

use std::collections::HashMap;
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/i18n_catalog.rs"));

pub struct Translations {
    map: HashMap<String, String>,
}

static TRANSLATIONS: OnceLock<Translations> = OnceLock::new();

/// Parse a gettext `.mo` binary catalog (little-endian format).
pub fn parse_mo(bytes: &[u8]) -> Option<HashMap<String, String>> {
    if bytes.len() < 28 {
        return None;
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    if magic != 0x9504_12de {
        return None;
    }
    let n = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
    let ooff = u32::from_le_bytes(bytes[12..16].try_into().ok()?) as usize;
    let toff = u32::from_le_bytes(bytes[16..20].try_into().ok()?) as usize;

    let read_table = |off: usize| -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        for i in 0..n {
            let p = off + i * 8;
            if p + 8 > bytes.len() {
                break;
            }
            let len = u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()) as usize;
            let o = u32::from_le_bytes(bytes[p + 4..p + 8].try_into().unwrap()) as usize;
            v.push((len, o));
        }
        v
    };

    let originals = read_table(ooff);
    let translations = read_table(toff);

    let mut map = HashMap::new();
    for (i, (len, off)) in originals.iter().copied().enumerate() {
        let Some((tlen, toff)) = translations.get(i).copied() else {
            break;
        };
        if off + len > bytes.len() || toff + tlen > bytes.len() {
            continue;
        }
        let key = String::from_utf8_lossy(&bytes[off..off + len]).into_owned();
        let val = String::from_utf8_lossy(&bytes[toff..toff + tlen]).into_owned();
        // Skip the empty header msgid and untranslated (empty) entries;
        // gettext falls back to the msgid for those.
        if !key.is_empty() && !val.is_empty() {
            map.insert(key, val);
        }
    }
    Some(map)
}

/// Language codes with an embedded catalog (plus "en" as the fallback).
pub fn available_languages() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = CATALOG.iter().map(|(l, _)| *l).collect();
    v.insert(0, "en");
    v
}

/// Map the preference value ("auto" or a language code) to a concrete code.
pub fn resolve_language(pref: &str) -> String {
    let has = |code: &str| CATALOG.iter().any(|(l, _)| *l == code);
    if pref.is_empty() || pref == "auto" {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(v) = std::env::var(var) {
                let base = v.split('.').next().unwrap_or("");
                let full = base.to_ascii_lowercase();
                if has(&full) {
                    return full;
                }
                let code = full.split('_').next().unwrap_or("").to_string();
                if has(&code) {
                    return code;
                }
            }
        }
        "en".to_string()
    } else {
        let c = pref.to_ascii_lowercase();
        if has(&c) {
            c
        } else {
            "en".to_string()
        }
    }
}

/// Initialize the global translations (call once at startup).
pub fn init(language: &str) {
    let lang = resolve_language(language);
    let map = CATALOG
        .iter()
        .find(|(l, _)| *l == lang)
        .and_then(|(_, bytes)| parse_mo(bytes))
        .unwrap_or_default();
    log::info!("i18n: using language '{lang}' ({} strings)", map.len());
    let _ = TRANSLATIONS.set(Translations { map });
}

/// Translate a msgid; falls back to the msgid itself.
pub fn tr(msgid: &str) -> String {
    match TRANSLATIONS.get() {
        Some(t) => t.map.get(msgid).cloned().unwrap_or_else(|| msgid.to_string()),
        None => msgid.to_string(),
    }
}

/// Translate with printf-style (`%s`/`%d`/`%i`/`%u`) or `{}` placeholder
/// substitution (arguments are already formatted strings).
pub fn trf(msgid: &str, args: &[&str]) -> String {
    let mut s = tr(msgid);
    for a in args {
        let pat = ["%s", "%d", "%i", "%u", "{}"]
            .iter()
            .find(|p| s.contains(**p))
            .map(|p| *p);
        if let Some(p) = pat {
            if let Some(pos) = s.find(p) {
                s.replace_range(pos..pos + p.len(), a);
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mo_and_translates() {
        init("de");
        let prev = tr("Previous page");
        assert!(!prev.is_empty());
        assert_ne!(prev, "Previous page", "German translation should differ");
        eprintln!("'Previous page' -> '{prev}'");
        // Untranslated/unknown strings fall back to the msgid.
        assert_eq!(tr("NoSuchStringXYZ"), "NoSuchStringXYZ");
    }

    #[test]
    fn resolves_language_from_env() {
        // "auto" with no env should yield "en".
        let lang = resolve_language("auto");
        assert!(lang == "en" || lang.len() == 2 || lang.len() == 5);
        // Explicit code.
        let lang = resolve_language("it");
        assert_eq!(lang, "it");
    }

    #[test]
    fn formatted_translation() {
        init("de");
        let s = trf("Page %d / %d", &["3", "10"]);
        eprintln!("'Page %d / %d' -> '{s}'");
        assert!(!s.is_empty());
    }
}

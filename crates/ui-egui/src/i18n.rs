//! Interface translations. Keys are the English text as the code writes it; command ids, file
//! formats and slide text stay as they are. A label a catalog lacks shows its English text, so
//! coverage can grow a string at a time.
//!
//! Catalogs are TSV files in `i18n/` (`English<TAB>translation`, `\n` for a line break, `#`
//! comments), baked in with `include_str!`. The language is chosen once for the whole interface
//! ([`set_language`]); English needs no catalog.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A language the interface can show.
pub struct Language {
    /// BCP 47 code (`nb`).
    pub code: &'static str,
    /// The language's own name for itself.
    pub name: &'static str,
    source: &'static str,
    catalog: OnceLock<HashMap<String, String>>,
}

/// English first: the source language and the fallback.
pub static LANGUAGES: [Language; 2] = [
    Language { code: "en", name: "English", source: "", catalog: OnceLock::new() },
    // Norwegian Bokmål; `no` (the macrolanguage) and Nynorsk `nn` read it too.
    Language { code: "nb", name: "Norsk (bokmål)", source: include_str!("i18n/nb.tsv"), catalog: OnceLock::new() },
];

/// Index into [`LANGUAGES`] of the language the interface shows.
static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// The [`LANGUAGES`] index for a language tag (`nb-NO`, `no_NO.UTF-8`), if there is a catalog.
pub fn find(tag: &str) -> Option<usize> {
    let base = tag.trim().split(['-', '_', '.', '@']).next()?.to_ascii_lowercase();
    let base = if base == "no" || base == "nn" { "nb".to_string() } else { base };
    LANGUAGES.iter().position(|l| l.code == base)
}

/// Shows the interface in the language of `tag`; English when there is no catalog for it.
pub fn set_language(tag: &str) {
    CURRENT.store(find(tag).unwrap_or(0), Ordering::Relaxed);
}

/// The language the interface shows.
pub fn current() -> &'static Language {
    LANGUAGES.get(CURRENT.load(Ordering::Relaxed)).unwrap_or(&LANGUAGES[0])
}

/// Reads a catalog: `English<TAB>translation` rows; malformed rows are skipped (and reported by
/// the tests).
fn parse(source: &str) -> HashMap<String, String> {
    source
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .map(|(k, v)| (k.replace("\\n", "\n"), v.replace("\\n", "\n")))
        .collect()
}

impl Language {
    fn catalog(&self) -> &HashMap<String, String> {
        self.catalog.get_or_init(|| parse(self.source))
    }
}

/// The interface text for `english` in the current language, with the host's product name put in.
pub fn tr(english: &str) -> &str {
    branded(tr_in(current(), english))
}

/// The text for `english` in `lang`, else `english`.
fn tr_in<'a>(lang: &'static Language, english: &'a str) -> &'a str {
    let text = if lang.source.is_empty() { None } else { lang.catalog().get(english).map(String::as_str) };
    text.unwrap_or(english)
}

/// Interface strings with the host's product name put in, made once each.
static BRANDED: std::sync::Mutex<Vec<(String, &'static str)>> = std::sync::Mutex::new(Vec::new());

/// `s` with the host's product name in place of "DeckCraft" (unchanged without one).
fn branded(s: &str) -> &str {
    let name = crate::product();
    if name == "DeckCraft" || !s.contains("DeckCraft") {
        return s;
    }
    let Ok(mut cache) = BRANDED.lock() else { return s };
    if let Some((_, b)) = cache.iter().find(|(k, _)| k == s) {
        return b;
    }
    // A bounded set: only the few interface strings that name the product.
    if cache.len() >= 256 {
        return s;
    }
    let b: &'static str = Box::leak(s.replace("DeckCraft", name).into_boxed_str());
    cache.push((s.to_owned(), b));
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_resolve_to_a_catalog() {
        assert_eq!(find("nb-NO"), Some(1));
        assert_eq!(find("no"), Some(1));
        assert_eq!(find("nn_NO.UTF-8"), Some(1));
        assert_eq!(find("en-US"), Some(0));
        assert_eq!(find("xx"), None);
    }

    #[test]
    fn catalogs_are_well_formed_and_keep_placeholders() {
        for lang in LANGUAGES.iter().filter(|l| !l.source.is_empty()) {
            let mut seen = std::collections::HashSet::new();
            for (i, line) in lang.source.lines().enumerate() {
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let Some((k, v)) = line.split_once('\t') else { panic!("{} line {}: no tab", lang.code, i + 1) };
                assert!(!k.is_empty() && !v.is_empty() && !v.contains('\t'), "{} line {}", lang.code, i + 1);
                assert!(seen.insert(k), "{} line {}: duplicate {k}", lang.code, i + 1);
                let holes = |s: &str| s.split('{').skip(1).filter_map(|p| p.split_once('}')).map(|(h, _)| h.to_string()).collect::<Vec<_>>();
                let (mut a, mut b) = (holes(k), holes(v));
                a.sort();
                b.sort();
                assert_eq!(a, b, "{} line {}: placeholders differ", lang.code, i + 1);
            }
        }
    }

    #[test]
    fn lookups_fall_back_to_english() {
        // Through `tr_in`: the interface language is global, and other tests draw in English.
        let (en, nb) = (&LANGUAGES[0], &LANGUAGES[1]);
        assert_eq!(tr_in(en, "Home"), "Home");
        assert_eq!(tr_in(nb, "Home"), "Hjem");
        assert_eq!(tr_in(nb, "No such label"), "No such label");
    }
}

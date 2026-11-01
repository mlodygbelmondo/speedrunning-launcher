//! UI strings from `assets/copy.json` (en) and `assets/copy.pl.json` (pl). No UI text is hard-coded.
//! Lookup order: current language, then English, then the key itself, so a gap is visible on
//! screen instead of silently blank.
use std::{
    collections::HashMap,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

use gpui::SharedString;

use crate::{
    assets::{COPY_JSON, COPY_PL_JSON},
    backend::Language,
};

/// The current language. It belongs to the app (one window); `Launcher::set_language` is the only
/// writer after startup. A process-wide value keeps `t()` callable from any render helper without
/// threading the language through every function.
static LANG: AtomicU8 = AtomicU8::new(0);

pub fn lang() -> Language {
    Language::ALL[LANG.load(Ordering::Relaxed) as usize]
}

pub fn set_lang(lang: Language) {
    let i = Language::ALL.iter().position(|l| *l == lang).unwrap_or(0);
    LANG.store(i as u8, Ordering::Relaxed);
}

type Table = HashMap<String, SharedString>;

fn parse(json: &str, name: &str) -> Table {
    let raw: HashMap<String, String> =
        serde_json::from_str(json).unwrap_or_else(|e| panic!("{name}: {e}"));
    raw.into_iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .map(|(k, v)| (k, SharedString::from(v)))
        .collect()
}

/// One table per `Language::ALL` entry.
fn tables() -> &'static [Table; 2] {
    static COPY: OnceLock<[Table; 2]> = OnceLock::new();
    COPY.get_or_init(|| {
        [
            parse(COPY_JSON, "copy.json"),
            parse(COPY_PL_JSON, "copy.pl.json"),
        ]
    })
}

/// Parse both copy files now, so a broken file fails at startup, not mid-render.
pub fn init() {
    tables();
}

/// The text for `key`. A missing key renders as the key itself.
pub fn t(key: &str) -> SharedString {
    t_opt(key).unwrap_or_else(|| SharedString::from(key.to_string()))
}

pub fn t_opt(key: &str) -> Option<SharedString> {
    let tables = tables();
    let i = Language::ALL.iter().position(|l| *l == lang()).unwrap_or(0);
    tables[i].get(key).or_else(|| tables[0].get(key)).cloned()
}

/// `t(key)` with `{name}` placeholders filled. Unknown placeholders stay as they are.
pub fn tf(key: &str, args: &[(&str, &str)]) -> SharedString {
    fill(&t(key), args)
}

pub fn fill(text: &str, args: &[(&str, &str)]) -> SharedString {
    let mut s = text.to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    SharedString::from(s)
}

/// A count with its noun: `key` with `{n}` filled, in the form the number needs. English has
/// `key_one` for 1 and `key` for the rest. Polish adds `key_few` (2-4, 22-24, ..., but not
/// 12-14: "22 mody") and `key_many` (5-21, 25-31, ...: "25 modów"); `key` itself is the
/// fallback for a form a key doesn't define.
pub fn tn(key: &str, n: u64) -> SharedString {
    let text = plural_form(lang(), n)
        .and_then(|f| t_opt(&format!("{key}_{f}")))
        .unwrap_or_else(|| t(key));
    fill(&text, &[("n", &n.to_string())])
}

/// The suffix of the plural form `n` takes in `lang`, `None` for the plain key.
fn plural_form(lang: Language, n: u64) -> Option<&'static str> {
    if n == 1 {
        return Some("one");
    }
    match lang {
        Language::En => None,
        Language::Pl if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) => {
            Some("few")
        }
        Language::Pl => Some("many"),
    }
}

/// A size in bytes the way the UI prints it: whole KB below 1 MB (a practice map can be 55 KB, and
/// "0 MB" would be wrong), whole MB below 1 GB, else GB with one decimal and the language's
/// decimal separator ("1.2 GB", "1,2 GB"). KB and MB here are KiB and MiB, as every launcher
/// counts.
pub fn size(bytes: u64) -> String {
    const MB: f64 = 1024. * 1024.;
    let mb = bytes as f64 / MB;
    if bytes < 1024 * 1024 {
        format!("{} KB", (bytes as f64 / 1024.).round().max(1.) as u64)
    } else if mb < 1000. {
        format!("{} MB", mb.round() as u64)
    } else {
        let gb = format!("{:.1} GB", mb / 1024.);
        match lang() {
            Language::Pl => gb.replace('.', ","),
            Language::En => gb,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_have_the_same_keys() {
        let [en, pl] = tables();
        let mut missing: Vec<_> = en.keys().filter(|k| !pl.contains_key(*k)).collect();
        // Polish plural forms (`tn`) have no English counterpart.
        missing.extend(
            pl.keys()
                .filter(|k| !en.contains_key(*k) && !k.ends_with("_few") && !k.ends_with("_many")),
        );
        assert!(missing.is_empty(), "keys in only one language: {missing:?}");
    }

    /// Every literal key passed to `t`, `tf` or `t_opt` in the sources exists in copy.json.
    #[test]
    fn every_literal_key_exists() {
        let [en, _] = tables();
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut stack = vec![std::path::PathBuf::from(dir)];
        let mut unknown = Vec::new();
        while let Some(p) = stack.pop() {
            if p.is_dir() {
                stack.extend(std::fs::read_dir(&p).unwrap().map(|e| e.unwrap().path()));
                continue;
            }
            let src = std::fs::read_to_string(&p).unwrap();
            for call in ["t(\"", "tf(\"", "t_opt(\"", "tn(\""] {
                for (i, _) in src.match_indices(call) {
                    // Skip identifiers that merely end in `t` (e.g. `format(`).
                    let before = src[..i].chars().last().unwrap_or(' ');
                    if before.is_alphanumeric() || before == '_' {
                        continue;
                    }
                    let rest = &src[i + call.len()..];
                    let key = &rest[..rest.find('"').unwrap()];
                    if !key.contains('{') && !en.contains_key(key) {
                        unknown.push(format!("{}: {key}", p.display()));
                    }
                }
            }
        }
        assert!(unknown.is_empty(), "unknown copy keys: {unknown:#?}");
    }

    #[test]
    fn polish_plurals() {
        let pl = |n| plural_form(Language::Pl, n);
        assert_eq!(pl(1), Some("one"));
        assert_eq!([pl(2), pl(4), pl(22), pl(104)], [Some("few"); 4]);
        assert_eq!(
            [pl(0), pl(5), pl(12), pl(14), pl(25), pl(111)],
            [Some("many"); 6]
        );
        assert_eq!(plural_form(Language::En, 22), None);
        assert_eq!(plural_form(Language::En, 1), Some("one"));
        // Every counter has all its Polish forms, so none falls back to the bare key.
        let [en, pl_table] = tables();
        for key in en.keys().filter(|k| k.ends_with("_one")) {
            let base = key.trim_end_matches("_one");
            for form in ["few", "many"] {
                assert!(
                    pl_table.contains_key(&format!("{base}_{form}")),
                    "{base}_{form} missing in copy.pl.json"
                );
            }
        }
    }

    #[test]
    fn sizes() {
        set_lang(Language::En);
        assert_eq!(size(94 * 1024 * 1024), "94 MB");
        assert_eq!(size(612 * 1024 * 1024), "612 MB");
        assert_eq!(size(1288 * 1024 * 1024), "1.3 GB");
        // BT Practice's zip.
        assert_eq!(size(56_314), "55 KB");
        assert_eq!(size(1), "1 KB");
    }
}

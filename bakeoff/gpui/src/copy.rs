//! UI strings from `assets/copy.json` (en) and `assets/copy.pl.json` (pl), SCREENS.md "Language".
//! No UI text is hard-coded. Lookup order: current language, then English, then the key itself.
use std::{
    collections::HashMap,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

use gpui::SharedString;

use crate::assets::{COPY_JSON, COPY_PL_JSON};

/// UI language. The index matches the order of `settings.languages` in screens.json.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En = 0,
    Pl = 1,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Pl];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Lang {
        Self::ALL.get(i).copied().unwrap_or(Lang::En)
    }

    /// `BAKEOFF_LANG` value (`en`, `pl`).
    pub fn from_code(code: &str) -> Option<Lang> {
        match code.trim().to_ascii_lowercase().as_str() {
            "en" => Some(Lang::En),
            "pl" => Some(Lang::Pl),
            _ => None,
        }
    }
}

/// The current language. It belongs to the app (one window); `Scene::set_language` is the only
/// writer after startup. A process-wide value keeps `t()` callable from any render helper.
static LANG: AtomicU8 = AtomicU8::new(Lang::En as u8);

pub fn lang() -> Lang {
    Lang::from_index(LANG.load(Ordering::Relaxed) as usize)
}

pub fn set_lang(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
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

fn tables() -> &'static [Table; 2] {
    static COPY: OnceLock<[Table; 2]> = OnceLock::new();
    COPY.get_or_init(|| {
        [
            parse(COPY_JSON, "copy.json"),
            parse(COPY_PL_JSON, "copy.pl.json"),
        ]
    })
}

/// Parse both copy files now, so a broken file fails at startup.
pub fn init() {
    tables();
}

/// The text for `key`. A missing key renders as the key itself, so gaps are visible.
pub fn t(key: &str) -> SharedString {
    t_opt(key).unwrap_or_else(|| SharedString::from(key.to_string()))
}

pub fn t_opt(key: &str) -> Option<SharedString> {
    let tables = tables();
    tables[lang().index()]
        .get(key)
        .or_else(|| tables[Lang::En.index()].get(key))
        .cloned()
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

/// The window title in the current language.
pub fn window_title() -> SharedString {
    tf("app.window_title", &[("stack", "gpui")])
}

//! What each setting of a node is and what it accepts: locales/<code>/settings_help.toml,
//! compiled in (English is complete and fills what a language lacks), then
//! `lang/<code>/settings_help.toml` in the data folder (a translation made by someone, i18n.rs),
//! then settings_help.toml in the data folder when there is one (its entries replace the others:
//! a newer copy of the firmware's documentation needs no new program).
//!
//! The rules are the firmware's own (CommonCLI.cpp, CommonRadioPrefs.cpp, MyMesh.cpp): edit mode
//! refuses a key that cannot be part of a valid value (`refuse`) and checks a value before it is
//! taken (`check`).

use std::path::Path;
use std::sync::RwLock;

use serde::Deserialize;

use crate::i18n::{self, t, tf};

/// The help file of each language, built in and in `lang/<code>/`
pub const FILE: &str = "settings_help.toml";

/// (code, file). The first one is the fallback.
const LANGUAGES: &[(&str, &str)] = &[
    ("en", include_str!("../locales/en/settings_help.toml")),
    ("fr", include_str!("../locales/fr/settings_help.toml")),
];

#[derive(Deserialize, Clone, Default)]
pub struct Help {
    pub id: String,
    /// "companion", "console", or "" for both
    #[serde(default)]
    pub node: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub warning: String,
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub since: String,
    /// number, integer, choice, text, radio, key (see the file's header)
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// 0 is accepted too, outside min..max ("off")
    #[serde(default)]
    pub zero_off: bool,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub max_bytes: Option<usize>,
    #[serde(default)]
    pub forbidden: String,
    #[serde(default)]
    pub empty_ok: bool,
    #[serde(default)]
    pub values: Vec<Choice>,
}

#[derive(Deserialize, Clone, Default)]
pub struct Choice {
    /// What is sent
    pub value: String,
    /// What the node answers for it, when that differs ("logRx" for "rx")
    #[serde(default)]
    pub shows: String,
    #[serde(default)]
    pub means: String,
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default)]
    setting: Vec<Help>,
}

/// The most notes and values one setting's help keeps (the firmware's longest list is under 20)
const NOTES_MAX: usize = 20;
const VALUES_MAX: usize = 50;
/// The largest settings_help.toml read from the data folder
const OWN_MAX: u64 = 512 * 1024;

/// The help in use. Read again when the language changes: the new list is kept for the rest of the
/// run (`Box::leak`, a few kilobytes each time), so a `&'static Help` an edit holds stays valid.
static LIST: RwLock<&'static [Help]> = RwLock::new(&[]);

/// Reads the built-in help (the chosen language over English) and the user's own file: at start,
/// and again when the language changes. Returns a line to show when the user's file could not be
/// used.
pub fn init(own: &Path) -> Option<String> {
    let parse = |text: &str| toml::from_str::<File>(text).map(|f| f.setting);
    let mut list = parse(LANGUAGES[0].1).unwrap_or_default();
    // later entries replace earlier ones with the same id and node
    let merge = |more: Vec<Help>, list: &mut Vec<Help>| {
        for help in more {
            list.retain(|h| !(h.id == help.id && h.node == help.node));
            list.push(help);
        }
    };
    // a translation, built in then the language's own file in the `lang` folder (made by
    // someone): its words only, for settings already known. Never the rules (type, range,
    // length, allowed characters, values sent): those always come from the English file, so a
    // translation cannot change what reaches a node. A file with a mistake is left out.
    let chosen = i18n::chosen();
    let built_in = LANGUAGES.iter().skip(1).find(|l| l.0 == chosen).map(|(_, text)| text.to_string());
    for text in built_in.into_iter().chain(i18n::own_file(&chosen, FILE)) {
        for theirs in parse(&text).unwrap_or_default() {
            if let Some(ours) = list.iter_mut().find(|h| h.id == theirs.id && h.node == theirs.node) {
                ours.words_from(&theirs);
            }
        }
    }
    let mut note = None;
    if own.exists() {
        // the same size limit as a language file: anything bigger is not a help file
        let read = match std::fs::metadata(own) {
            Ok(meta) if meta.len() > OWN_MAX => Err(tf("file-too-big", &[("max", (OWN_MAX / 1024).into())])),
            _ => std::fs::read_to_string(own).map_err(|e| e.to_string()),
        };
        match read.and_then(|s| parse(&s).map_err(|e| e.to_string())) {
            Ok(more) => merge(more, &mut list),
            Err(error) => {
                note = Some(tf("help-unreadable", &[("file", own.display().to_string().into()), ("error", error.into())]))
            }
        }
    }
    // every text made safe to show (a file may come from someone else)
    for help in &mut list {
        help.clean();
    }
    let list: &'static [Help] = Box::leak(list.into_boxed_slice());
    *LIST.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = list;
    note
}

/// The built-in help file of language `code` (written into the `lang` folder, i18n::mirror)
pub fn built_in(code: &str) -> Option<&'static str> {
    LANGUAGES.iter().find(|(c, _)| *c == code).map(|(_, text)| *text)
}

/// The help of setting `id` for a companion (`companion`) or a console node; one made for that
/// kind of node wins over one for both
pub fn find(id: &str, companion: bool) -> Option<&'static Help> {
    let own = if companion { "companion" } else { "console" };
    let list: &'static [Help] = *LIST.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    list.iter().find(|h| h.id == id && h.node == own).or_else(|| list.iter().find(|h| h.id == id && h.node.is_empty()))
}

impl Help {
    /// The words of a translation over these: title, summary, notes, warning, the default as
    /// written ("depends on the board": shown, never checked), what each value means (matched
    /// by its value). Not the rules.
    fn words_from(&mut self, theirs: &Help) {
        let take = |ours: &mut String, theirs: &str| {
            if !theirs.trim().is_empty() {
                *ours = theirs.to_string();
            }
        };
        take(&mut self.title, &theirs.title);
        take(&mut self.summary, &theirs.summary);
        take(&mut self.warning, &theirs.warning);
        take(&mut self.default, &theirs.default);
        if !theirs.notes.is_empty() {
            self.notes = theirs.notes.clone();
        }
        for value in &mut self.values {
            if let Some(t) = theirs.values.iter().find(|v| v.value == value.value) {
                take(&mut value.means, &t.means);
            }
        }
    }

    /// Every text shown made safe (i18n::clean); a list of notes or values kept to a sane size
    fn clean(&mut self) {
        for text in [&mut self.title, &mut self.summary, &mut self.warning, &mut self.default, &mut self.since, &mut self.unit] {
            *text = i18n::clean(text);
        }
        self.notes.truncate(NOTES_MAX);
        for note in &mut self.notes {
            *note = i18n::clean(note);
        }
        self.values.truncate(VALUES_MAX);
        for value in &mut self.values {
            value.means = i18n::clean(&value.means);
            value.shows = i18n::clean(&value.shows);
        }
    }

    /// The values to pick from (empty: typed)
    pub fn choices(&self) -> Vec<String> {
        if self.kind == "choice" {
            self.values.iter().map(|c| c.value.clone()).collect()
        } else {
            Vec::new()
        }
    }

    /// The value as it should be typed, from what the node answered: "10.0%" -> "10.0",
    /// "logRx" -> "rx"
    pub fn editable(&self, shown: &str) -> String {
        let shown = shown.trim();
        match self.kind.as_str() {
            "number" | "integer" => shown.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect(),
            "choice" => self
                .values
                .iter()
                .find(|c| c.value.eq_ignore_ascii_case(shown) || (!c.shows.is_empty() && c.shows.eq_ignore_ascii_case(shown)))
                .map_or_else(|| shown.to_string(), |c| c.value.clone()),
            _ => shown.to_string(),
        }
    }

    /// Can `c` follow `text`? None = yes; else why not, in words (the limit, what is allowed). A
    /// number takes digits, one point, a minus first when the range is below 0; a choice takes
    /// what leads to one of its values; a text takes what fits.
    pub fn refuse(&self, text: &str, c: char) -> Option<String> {
        let what = self.title.as_str();
        let negative = self.min.is_some_and(|m| m < 0.0);
        let ok = match self.kind.as_str() {
            "integer" => c.is_ascii_digit() || (c == '-' && negative && text.is_empty()),
            "number" => c.is_ascii_digit() || (c == '.' && !text.contains('.')) || (c == '-' && negative && text.is_empty()),
            "radio" => c.is_ascii_digit() || c == '.' || (c == ',' && text.matches(',').count() < 3),
            // a key: hex digits, two per byte, max_bytes bytes
            "key" => {
                if !c.is_ascii_hexdigit() {
                    return Some(tf("edit-key-hex", &[("what", what.into())]));
                }
                match self.max_bytes {
                    Some(bytes) if text.len() >= bytes * 2 => return Some(too_long(&self.title, bytes * 2)),
                    _ => true,
                }
            }
            "choice" => {
                let next = format!("{text}{c}").to_ascii_lowercase();
                self.values.iter().any(|v| v.value.to_ascii_lowercase().starts_with(&next))
            }
            "text" => {
                if c.is_control() || self.forbidden.contains(c) {
                    return Some(bad_char(&self.title, c));
                }
                match self.max_bytes {
                    Some(max) if text.len() + c.len_utf8() > max => return Some(too_long(&self.title, max)),
                    _ => true,
                }
            }
            _ => !c.is_control(),
        };
        if ok {
            return None;
        }
        Some(match self.kind.as_str() {
            "integer" => tf("edit-key-whole", &[("what", what.into()), ("range", self.range().into())]),
            "number" => tf("edit-key-number", &[("what", what.into()), ("range", self.range().into())]),
            "radio" => t("edit-bad-radio"),
            "choice" => {
                let all = self.values.iter().map(|c| c.value.as_str()).collect::<Vec<_>>().join(", ");
                tf("edit-not-a-choice", &[("what", what.into()), ("values", all.into())])
            }
            _ => bad_char(what, c),
        })
    }

    /// While typing: (used, at most) for a text or a key with a length limit, so the limit is
    /// shown before it is reached
    pub fn room(&self, text: &str) -> Option<(usize, usize)> {
        match (self.kind.as_str(), self.max_bytes) {
            ("text", Some(max)) => Some((text.len(), max)),
            ("key", Some(bytes)) => Some((text.len(), bytes * 2)),
            _ => None,
        }
    }

    /// "1 – 100 %", "3 – 168 h, or 0 = off"; "" when it has no range
    pub fn range(&self) -> String {
        let (Some(low), Some(high)) = (self.min, self.max) else { return String::new() };
        let range = format!("{low} – {high} {}", self.unit).trim().to_string();
        if self.zero_off {
            format!("{range}, {}", t("edit-or-off"))
        } else {
            range
        }
    }

    /// The whole value, as the firmware would take it; Err = why not (shown to the user)
    pub fn check(&self, value: &str) -> Result<(), String> {
        let value = value.trim();
        let out_of_range = || tf("edit-out-of-range-of", &[("what", self.title.as_str().into()), ("range", self.range().into())]);
        let within = |n: f64| {
            (self.zero_off && n == 0.0)
                || (self.min.is_none_or(|low| n >= low) && self.max.is_none_or(|high| n <= high))
        };
        match self.kind.as_str() {
            "integer" => match value.parse::<i64>() {
                Ok(n) if within(n as f64) => Ok(()),
                Ok(_) => Err(out_of_range()),
                Err(_) => Err(tf("edit-not-a-whole-number", &[("what", self.title.as_str().into())])),
            },
            "number" => match value.parse::<f64>() {
                Ok(n) if n.is_finite() && within(n) => Ok(()),
                Ok(_) => Err(out_of_range()),
                Err(_) => Err(tf("edit-not-a-number", &[("what", self.title.as_str().into()), ("value", value.into())])),
            },
            "choice" if self.values.iter().any(|c| c.value.eq_ignore_ascii_case(value)) => Ok(()),
            "choice" => {
                let all = self.values.iter().map(|c| c.value.as_str()).collect::<Vec<_>>().join(", ");
                Err(tf("edit-not-a-choice", &[("what", self.title.as_str().into()), ("values", all.into())]))
            }
            "radio" => check_radio(value).map_err(|_| tf("edit-bad-radio", &[])),
            "key" => {
                let hex = value.chars().all(|c| c.is_ascii_hexdigit());
                match self.max_bytes {
                    Some(bytes) if !hex || value.len() != bytes * 2 => {
                        Err(tf("edit-bad-hex", &[("what", self.title.as_str().into()), ("digits", (bytes * 2).into())]))
                    }
                    _ => Ok(()),
                }
            }
            "text" => {
                if value.is_empty() && !self.empty_ok {
                    return Err(tf("edit-bad-text", &[("what", self.title.as_str().into())]));
                }
                if let Some(c) = value.chars().find(|c| c.is_control() || self.forbidden.contains(*c)) {
                    return Err(bad_char(&self.title, c));
                }
                match self.max_bytes {
                    Some(max) if value.len() > max => Err(too_long(&self.title, max)),
                    _ => Ok(()),
                }
            }
            _ => Ok(()),
        }
    }
}

/// "<what> is full: at most <max> characters" (said when one more is typed, not "does not fit")
pub fn too_long(what: &str, max: usize) -> String {
    tf("edit-key-too-long", &[("what", what.into()), ("max", max.into())])
}

/// "<what> cannot hold "<c>"" (a control key shown as "?")
pub fn bad_char(what: &str, c: char) -> String {
    let c = if c.is_control() { "?".to_string() } else { c.to_string() };
    tf("edit-bad-char", &[("what", what.into()), ("char", c.into())])
}

/// "freq,bw,sf,cr" as `set radio` takes it (CommonRadioPrefs.cpp): 150-2500 MHz, 7-500 kHz,
/// SF 5-12, CR 5-8
fn check_radio(value: &str) -> Result<(), ()> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    let [freq, bw, sf, cr] = parts.as_slice() else { return Err(()) };
    let freq: f64 = freq.parse().map_err(|_| ())?;
    let bw: f64 = bw.parse().map_err(|_| ())?;
    let sf: u8 = sf.parse().map_err(|_| ())?;
    let cr: u8 = cr.parse().map_err(|_| ())?;
    let ok = (150.0..=2500.0).contains(&freq) && (7.0..=500.0).contains(&bw) && (5..=12).contains(&sf) && (5..=8).contains(&cr);
    if ok {
        Ok(())
    } else {
        Err(())
    }
}

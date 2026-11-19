//! Texts shown to people, by language, in the Fluent format (Project Fluent,
//! https://projectfluent.org: the `.ftl` files used by Firefox and most Rust programs).
//!
//! Two places, the second over the first:
//! - built in: `locales/<code>/mcmanager.ftl`, compiled into the program (`BUILT_IN`);
//! - the `lang` folder in the data folder: `lang/<code>/mcmanager.ftl` (and `settings_help.toml`,
//!   help.rs). Anyone can add a language there, or correct one, with no new program: a file there
//!   gives its own messages, the built-in file of that language the ones it lacks.
//! English is complete and is the fallback for any message a language lacks. Variables are
//! written `{ $name }`; plurals and other forms use Fluent selectors. A language names itself with
//! the message `language-name` ("Français").
//!
//! Chosen with `[ui] language` ("auto" = the system's language) and on the Settings page: `init`
//! can be called again, the next drawing is in the new language.

use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard};

use crate::help;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// (code, file) compiled in. The first one is the fallback, complete.
const BUILT_IN: &[(&str, &str)] = &[
    ("en", include_str!("../locales/en/mcmanager.ftl")),
    ("fr", include_str!("../locales/fr/mcmanager.ftl")),
];

/// The file of each language, in the built-in folders and in `lang/`
const TEXTS: &str = "mcmanager.ftl";

/// The chosen language first (the `lang` folder's file, then the built-in one), then English
static BUNDLES: RwLock<Vec<FluentBundle<FluentResource>>> = RwLock::new(Vec::new());

/// The language asked for ("fr"), also for the other files by language (settings help)
static CHOSEN: RwLock<String> = RwLock::new(String::new());

/// The `lang` folder in the data folder (None: only the built-in languages)
static FOLDER: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Where the people's own languages are (`<data>/lang`): before `init`
pub fn set_folder(folder: PathBuf) {
    *FOLDER.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(folder);
}

/// The largest language file read (a whole translation is about 50 KB): a bigger one is not one
const FILE_MAX: u64 = 512 * 1024;
/// The longest text shown from one message (the longest English one is under 400 characters)
const TEXT_MAX: usize = 2000;

/// A language code as a folder name may hold it: a language and its variants, BCP 47 style
/// ("fr", "pt-BR", "zh-Hant", "es-419", "sr-Latn"): parts of ASCII letters and digits joined by
/// `-`, the first letters only. Never a path (no `.`, `/`, `\`): a name never leaves the folder.
pub fn valid_code(code: &str) -> bool {
    let mut parts = code.split('-');
    let first_ok = parts.next().is_some_and(|p| (2..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphabetic()));
    first_ok && code.len() <= 20 && parts.all(|p| (1..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// The same code written one way: "pt_BR.UTF-8" / "PT-br" -> "pt-br" (folder names are found
/// whatever their case: `language_folder`)
pub fn normal(code: &str) -> String {
    let code = code.trim().split(['.', '@']).next().unwrap_or("").replace('_', "-");
    code.to_ascii_lowercase()
}

/// The folder of language `code` in `lang/`, whatever case its name has ("pt-BR" for "pt-br")
fn language_folder(code: &str) -> Option<PathBuf> {
    let folder = FOLDER.read().ok()?.clone()?;
    let exact = folder.join(code);
    if exact.is_dir() {
        return Some(exact);
    }
    std::fs::read_dir(&folder)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_str().is_some_and(|n| n.eq_ignore_ascii_case(code)) && e.path().is_dir())
        .map(|e| e.path())
}

/// A file of language `code` in the `lang` folder (`lang/<code>/<name>`), if there is one and it
/// can be one: the code is a language code (`valid_code`: never a path), the file is at most
/// FILE_MAX and is UTF-8 text (any script: Latin, Cyrillic, Greek, Arabic, CJK...). Anything
/// else: as if there were no file. Its texts are cleaned when shown (`clean`): what someone wrote
/// in it is never more than text.
pub fn own_file(code: &str, name: &str) -> Option<String> {
    if !valid_code(code) {
        return None;
    }
    let path = language_folder(code)?.join(name);
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > FILE_MAX {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    // read again bigger than it said (changed meanwhile): not taken
    if bytes.len() as u64 > FILE_MAX {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Text from a file someone else may have written, made safe to show: no control characters (a
/// terminal escape sequence could move the cursor, change colours, set the clipboard or the
/// window title), no direction marks (they can make a text read other than it is), and no longer
/// than TEXT_MAX. Line breaks stay.
pub fn clean(text: &str) -> String {
    let unsafe_char = |c: char| {
        (c.is_control() && c != '\n')
            || matches!(c, '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
    };
    text.chars().filter(|c| !unsafe_char(*c)).take(TEXT_MAX).collect()
}

/// The languages there are, built in and in the `lang` folder: (code, its own name), by code
pub fn languages() -> Vec<(String, String)> {
    let mut codes: Vec<String> = BUILT_IN.iter().map(|(code, _)| code.to_string()).collect();
    let folder = FOLDER.read().ok().and_then(|f| f.clone());
    if let Some(entries) = folder.and_then(|f| std::fs::read_dir(f).ok()) {
        for entry in entries.flatten() {
            let code = normal(&entry.file_name().to_string_lossy());
            if valid_code(&code) && entry.path().join(TEXTS).is_file() && !codes.contains(&code) {
                codes.push(code);
            }
        }
    }
    codes.sort();
    codes
        .into_iter()
        .map(|code| {
            let name = files(&code)
                .iter()
                .find_map(|text| message(&bundle(&code, text), "language-name"))
                .map(|n| clean(&n).replace('\n', " ").chars().take(40).collect::<String>())
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| code.clone());
            (code, name)
        })
        .collect()
}

/// The built-in text file of language `code`
fn built_in(code: &str) -> Option<&'static str> {
    BUILT_IN.iter().find(|(c, _)| *c == code).map(|(_, text)| *text)
}

/// The files of every built-in language, as they are in the program: (`lang/<code>/<name>`
/// path, text). English and French, each with its texts and its settings help.
fn originals(folder: &Path) -> Vec<(PathBuf, &'static str)> {
    let mut files = Vec::new();
    for (code, _) in BUILT_IN {
        for (name, text) in [(TEXTS, built_in(code)), (help::FILE, help::built_in(code))] {
            if let Some(text) = text {
                files.push((folder.join(code).join(name), text));
            }
        }
    }
    files
}

/// The sha256 of a text, in hex
fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Next to each copy: the sha256 of the original it was written from (`.<name>.sha256`). The
/// copy still matching it was not changed by anyone: a newer program may replace it.
fn mark_of(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.sha256"))
}

/// Writes an original and its mark
fn write_original(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)?;
    std::fs::write(mark_of(path), sha256(text.as_bytes()))
}

/// At start: the built-in languages' files (English and French) in the `lang` folder, so they
/// can be read, corrected or used as the start of another language; the program reads them from
/// there (a corrected text is used).
/// - not there: written;
/// - there and unchanged since written (it matches its mark) but not this program's original
///   (a newer version of the program): written again, so new wording arrives by itself;
/// - there without a mark (written by an older version, before the marks): an old original as
///   far as anyone can tell, so written again, the old file first renamed `<name>.<date>.bak`
///   (in case someone corrected it: never lost). Without this, old wording stayed for good;
/// - changed by someone: never touched (Settings > original files puts them back, keeping theirs).
///
/// Returns a line for each that could not be written (the built-in texts are used then).
pub fn mirror() -> Vec<String> {
    let Some(folder) = FOLDER.read().ok().and_then(|f| f.clone()) else { return Vec::new() };
    let mut notes = Vec::new();
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    for (path, text) in originals(&folder) {
        let (stale, keep_old) = match std::fs::read(&path) {
            Err(_) => (true, false),
            Ok(now) if now == text.as_bytes() => {
                // the original already: its mark given if it has none (the next version then
                // replaces it without a .bak)
                if !mark_of(&path).exists() {
                    let _ = std::fs::write(mark_of(&path), sha256(text.as_bytes()));
                }
                (false, false)
            }
            Ok(now) => match std::fs::read_to_string(mark_of(&path)) {
                Ok(mark) => (mark.trim() == sha256(&now), false),
                Err(_) => (true, true),
            },
        };
        if !stale {
            continue;
        }
        if keep_old {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if let Err(e) = std::fs::rename(&path, path.with_file_name(format!("{name}.{stamp}.bak"))) {
                notes.push(tf("lang-not-written", &[("file", path.display().to_string().into()), ("error", e.to_string().into())]));
                continue;
            }
        }
        if let Err(e) = write_original(&path, text) {
            notes.push(tf("lang-not-written", &[("file", path.display().to_string().into()), ("error", e.to_string().into())]));
        }
    }
    notes
}

/// The Settings page's "original files": every built-in language's files written again into the
/// `lang` folder. A file that differs from the original (someone's corrections) is first renamed
/// `<name>.<date and time>.bak`, never lost. Returns the files written again.
pub fn restore_originals() -> anyhow::Result<Vec<PathBuf>> {
    let folder = FOLDER.read().ok().and_then(|f| f.clone()).ok_or_else(|| anyhow::anyhow!(t("lang-no-folder")))?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let mut written = Vec::new();
    for (path, text) in originals(&folder) {
        match std::fs::read(&path) {
            // already the original: nothing to do
            Ok(now) if now == text.as_bytes() => continue,
            // someone's version: kept beside it, renamed (never written over)
            Ok(_) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                std::fs::rename(&path, path.with_file_name(format!("{name}.{stamp}.bak")))?;
            }
            Err(_) => {}
        }
        write_original(&path, text)?;
        written.push(path);
    }
    Ok(written)
}

/// The language in use ("" before `init`)
pub fn chosen() -> String {
    CHOSEN.read().map(|c| c.clone()).unwrap_or_default()
}

/// The texts, even after a thread panicked holding the lock (they are only ever replaced whole)
fn bundles() -> RwLockReadGuard<'static, Vec<FluentBundle<FluentResource>>> {
    BUNDLES.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The texts of language `code`: the `lang` folder's file first, then the built-in one
fn files(code: &str) -> Vec<String> {
    let mut found: Vec<String> = own_file(code, TEXTS).into_iter().collect();
    found.extend(BUILT_IN.iter().filter(|(c, _)| *c == code).map(|(_, text)| text.to_string()));
    found
}

fn bundle(code: &str, text: &str) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = code.parse().unwrap_or_default();
    let mut b = FluentBundle::new_concurrent(vec![id]);
    // no invisible Unicode marks around variables: they upset terminal column widths
    b.set_use_isolating(false);
    // a file with a mistake still gives its correct messages; the others fall back
    let res = FluentResource::try_new(text.to_string()).unwrap_or_else(|(res, _)| res);
    let _ = b.add_resource(res);
    b
}

/// A message with no variables, if the bundle has it
fn message(b: &FluentBundle<FluentResource>, id: &str) -> Option<String> {
    let pattern = b.get_message(id)?.value()?;
    let mut errors = Vec::new();
    Some(b.format_pattern(pattern, None, &mut errors).into_owned())
}

/// The languages the user wants, most wanted first, region kept ("pt-br" from "pt_BR.UTF-8" or
/// "pt-BR"), as each system defines it (sys-locale): Linux = LANGUAGE (GNU gettext's list,
/// "pt_BR:pt:en"), then LC_ALL, LC_MESSAGES, LANG (POSIX order); Windows = the user's display
/// languages in their order (GetUserPreferredUILanguages). "C" / "POSIX" say nothing.
fn system_languages() -> Vec<String> {
    sys_locale::get_locales()
        .map(|wanted| normal(&wanted))
        .filter(|code| code != "c" && code != "posix")
        .map(|code| if valid_code(&code) { code } else { code.chars().take_while(|c| c.is_ascii_alphabetic()).collect() })
        .filter(|code| !code.is_empty())
        .collect()
}

/// The codes to try for `code`, most precise first: "zh-hant-tw", "zh-hant", "zh"
fn candidates(code: &str) -> Vec<String> {
    let mut list = Vec::new();
    let mut rest = code;
    while !rest.is_empty() {
        list.push(rest.to_string());
        rest = rest.rfind('-').map_or("", |at| &rest[..at]);
    }
    list
}

/// The language actually shown: the chosen one when there are texts for it, else English. For
/// "automatic (Français)" on the Settings page.
pub fn active() -> String {
    let chosen = chosen();
    if chosen != BUILT_IN[0].0 && !files(&chosen).is_empty() {
        chosen
    } else {
        BUILT_IN[0].0.to_string()
    }
}

/// Loads the language: a code ("en", "fr", "pt-BR"), or "auto" / "" = the first of the system's
/// (`system_languages`) there are texts for. A regional code with no texts of its own takes its
/// language's ("fr-ch" -> "fr"); unknown -> English. At start, and again when it is changed.
pub fn init(language: &str) {
    let wanted = match language.trim() {
        "" | "auto" => system_languages(),
        other => vec![normal(other)],
    };
    let fallback = BUILT_IN[0].0;
    let code = wanted
        .iter()
        .flat_map(|w| candidates(w))
        .find(|c| valid_code(c) && !files(c).is_empty())
        .unwrap_or_else(|| fallback.to_string());
    let mut list: Vec<FluentBundle<FluentResource>> = Vec::new();
    if code != fallback {
        list.extend(files(&code).iter().map(|text| bundle(&code, text)));
    }
    list.extend(files(fallback).iter().map(|text| bundle(fallback, text)));
    // both replaced whole: a text is never read half in one language, half in another
    *BUNDLES.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = list;
    *CHOSEN.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = code;
}

/// The text of message `id` ("tab-fleet"); the id itself when it is missing (easy to spot)
pub fn t(id: &str) -> String {
    tf(id, &[])
}

/// `t` with variables: `tf("fleet-title", &[("count", n.into())])`.
/// Numbers stay numbers, so plural selectors work.
pub fn tf(id: &str, args: &[(&str, FluentValue)]) -> String {
    if bundles().is_empty() {
        init("auto");
    }
    let mut fargs = FluentArgs::new();
    for (k, v) in args {
        fargs.set(*k, v.clone());
    }
    for b in bundles().iter() {
        if let Some(pattern) = b.get_message(id).and_then(|m| m.value()) {
            let mut errors = Vec::new();
            // cleaned: a translation is someone else's text (the built-in ones pass unchanged).
            // Fluent itself stops a message that refers to itself or grows without end.
            return clean(&b.format_pattern(pattern, Some(&fargs), &mut errors));
        }
    }
    id.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_parses() {
        for (code, text) in BUILT_IN {
            assert!(FluentResource::try_new(text.to_string()).is_ok(), "locales/{code}/mcmanager.ftl has errors");
        }
    }
}

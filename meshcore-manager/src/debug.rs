//! The debug log: when it is on (`[debug] log`, Settings page), every action and error is written
//! to `logs/debug-<date>.jsonl` in the data folder: JSON Lines, one JSON object per line, so a
//! program (or `grep`) reads it fast and a crash never leaves it unreadable (the last line is
//! whole or absent). Each line:
//! `{"t":"2026-11-18T19:01:24.123+01:00","lvl":"ERROR","mod":"mcmanager::node::cli","thread":"mcmanager-job-3","msg":"..."}`
//! (`t` local time with its offset, `lvl` ERROR / WARN / INFO / DEBUG, `mod` the part of the
//! program, `thread` which job or the worker): to find a problem, or to send with a bug report.
//!
//! It holds nothing private, by design:
//! - what is exchanged with a node is written by its code and size only, never its bytes or text
//!   (a frame can hold a private key, a console reply a password);
//! - a console command by its first words only (`set name`, never its value);
//! - every line is passed through `redact`: the names, keys, serial numbers and folders seen so
//!   far (`private`) become `<node>`, `<key>`, `<serial>`, `<folder>`; any long hex run (a key, a
//!   secret) becomes `<hex>`; a number with 5 or more decimals (a position) `<number>`; control
//!   characters go (i18n::clean).
//!
//! The files are never deleted by the program (the user's, like the backups); a day's file stops
//! at DAY_MAX, saying so. Each line is written at once (a crash keeps every line before it).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};

use crate::i18n;

/// The most a day's file holds: past it, one last line says so and the day writes no more
const DAY_MAX: u64 = 20 * 1024 * 1024;
/// The most private texts remembered (a big contact list is a few hundred)
const PRIVATE_MAX: usize = 20_000;

static ON: AtomicBool = AtomicBool::new(false);
static FOLDER: RwLock<Option<PathBuf>> = RwLock::new(None);
/// Today's file: (its date, the file, bytes written; None after DAY_MAX)
static OUT: Mutex<Option<(String, Option<File>, u64)>> = Mutex::new(None);
/// Private texts and what they become, longest first
static PRIVATE: RwLock<Vec<(String, &'static str)>> = RwLock::new(Vec::new());

/// The kinds of private text, as they show in the log
pub const NODE: &str = "<node>";
pub const KEY: &str = "<key>";
pub const SERIAL: &str = "<serial>";
pub const FOLDER_TAG: &str = "<folder>";
pub const NAME: &str = "<name>";

/// At start: where the files go, and whether it is on
pub fn start(folder: PathBuf, on: bool) {
    *FOLDER.write().unwrap_or_else(|p| p.into_inner()) = Some(folder);
    // the person's own folders and name never show
    for var in ["USERPROFILE", "HOME", "APPDATA", "LOCALAPPDATA"] {
        if let Some(path) = std::env::var_os(var) {
            private(&path.to_string_lossy(), FOLDER_TAG);
        }
    }
    for var in ["USERNAME", "USER", "COMPUTERNAME", "HOSTNAME"] {
        if let Ok(name) = std::env::var(var) {
            private(&name, NAME);
        }
    }
    set(on);
}

/// Turns it on or off (the Settings page); the change itself is the first or last line
pub fn set(on: bool) {
    if on {
        ON.store(true, Ordering::SeqCst);
        write("mcmanager::debug", "INFO", &format!("debug log on, mcmanager {}", env!("CARGO_PKG_VERSION")));
    } else {
        write("mcmanager::debug", "INFO", "debug log off");
        ON.store(false, Ordering::SeqCst);
    }
}

pub fn is_on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Today's file, for the Settings page
pub fn today_file() -> Option<PathBuf> {
    let folder = FOLDER.read().ok()?.clone()?;
    Some(folder.join(format!("debug-{}.jsonl", chrono::Local::now().format("%Y-%m-%d"))))
}

/// A text that must never be in the log (a node's name, a key, a serial number, a folder): from
/// now on it is written as `tag`. Short texts (under 3 characters) are left: they would hide
/// everything else.
pub fn private(text: &str, tag: &'static str) {
    let text = text.trim();
    if text.chars().count() < 3 {
        return;
    }
    let mut list = PRIVATE.write().unwrap_or_else(|p| p.into_inner());
    if list.len() >= PRIVATE_MAX || list.iter().any(|(t, _)| t == text) {
        return;
    }
    list.push((text.to_string(), tag));
    // the longest first: "Node A2" is replaced before "Node A"
    list.sort_by_key(|(t, _)| std::cmp::Reverse(t.len()));
}

/// A line made safe: private texts, long hex runs, positions replaced; control characters gone
pub fn redact(text: &str) -> String {
    let mut out = i18n::clean(text).replace('\n', " | ");
    if let Ok(list) = PRIVATE.read() {
        for (secret, tag) in list.iter() {
            if out.contains(secret.as_str()) {
                out = out.replace(secret.as_str(), tag);
            }
        }
    }
    hide_runs(&out)
}

/// Hex runs of 16 or more (keys, secrets) -> `<hex>`; numbers with 5 or more decimals
/// (positions) -> `<number>`
fn hide_runs(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        // a run of hex digits
        let hex = chars[i..].iter().take_while(|c| c.is_ascii_hexdigit()).count();
        if hex >= 16 {
            out.push_str("<hex>");
            i += hex;
            continue;
        }
        // digits, a point, 5+ digits
        let int = chars[i..].iter().take_while(|c| c.is_ascii_digit()).count();
        if int > 0 && chars.get(i + int) == Some(&'.') {
            let dec = chars[i + int + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            if dec >= 5 {
                out.push_str("<number>");
                i += int + 1 + dec;
                continue;
            }
        }
        // a word is copied whole, so a hex run is only counted from the start of a word
        let word = chars[i..].iter().take_while(|c| c.is_ascii_alphanumeric()).count().max(1);
        out.extend(&chars[i..i + word]);
        i += word;
    }
    out
}

/// Writes one line (when on). `module`: `module_path!()` of the caller.
pub fn write(module: &str, level: &str, text: &str) {
    if !is_on() {
        return;
    }
    let Some(folder) = FOLDER.read().ok().and_then(|f| f.clone()) else { return };
    let now = chrono::Local::now();
    let day = now.format("%Y-%m-%d").to_string();
    let thread = std::thread::current();
    let entry = serde_json::json!({
        "t": now.to_rfc3339_opts(chrono::SecondsFormat::Millis, false),
        "lvl": level,
        "mod": module,
        "thread": thread.name().unwrap_or("?"),
        "msg": redact(text),
    });
    // serde_json escapes quotes and line breaks: one object, one line, always
    let line = format!("{entry}\n");
    let mut out = OUT.lock().unwrap_or_else(|p| p.into_inner());
    // a new day: a new file
    if out.as_ref().is_none_or(|(d, _, _)| *d != day) {
        let _ = std::fs::create_dir_all(&folder);
        let path = folder.join(format!("debug-{day}.jsonl"));
        let file = OpenOptions::new().create(true).append(true).open(&path).ok();
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        *out = Some((day, file, size));
    }
    let Some((_, file, size)) = out.as_mut() else { return };
    let Some(f) = file.as_mut() else { return };
    if *size + line.len() as u64 > DAY_MAX {
        let full = serde_json::json!({ "t": now.to_rfc3339_opts(chrono::SecondsFormat::Millis, false), "lvl": "WARN", "mod": "mcmanager::debug", "msg": "the debug log is full for today: nothing more is written" });
        let _ = f.write_all(format!("{full}\n").as_bytes());
        *file = None;
        return;
    }
    if f.write_all(line.as_bytes()).is_ok() {
        *size += line.len() as u64;
    }
}

/// `debug_log!("ERROR", "port {} lost: {}", port, e)`: one line in the debug log, with the module
/// it is written from. Nothing is formatted while the log is off.
#[macro_export]
macro_rules! debug_log {
    ($level:expr, $($arg:tt)*) => {
        if $crate::debug::is_on() {
            $crate::debug::write(module_path!(), $level, &format!($($arg)*));
        }
    };
}

//! Settings: config.toml in the data folder. Written with its comments on first start.
//!
//! The file stays readable by every newer version, and grows without breaking:
//! - a setting that is missing takes its default (`#[serde(default)]` everywhere); a setting this
//!   version does not know is ignored and kept;
//! - a new setting goes in TEMPLATE with its comment and in the Default impls (the two say the
//!   same); `upgrade` adds it, with its comment, to the files people already have, at start;
//! - never rename or reuse a key: the old one would stay in people's files and the new one take
//!   its default. A key that is no longer used is left alone (and ignored);
//! - the file is changed only through `set` (one value in place, comments and the rest kept),
//!   which writes through `write` (the one before kept as `.bak`, a new file synced, then renamed
//!   over: never half written).

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::i18n::tf;

const TEMPLATE: &str = r#"# Meshcore Manager settings. Delete a line to get its default back.

[ui]
# Language of the texts: "auto" (from the system), or a code that has a file in locales/ ("en")
language = "auto"

[scan]
# How often the USB ports are looked at, in seconds
every_secs = 2
# How long a node gets to answer when it is plugged in, in milliseconds
# (an ESP32 board restarts when its port opens: under 3000 it may be missed)
answer_ms = 4000

[backup]
# Keep the node's private key in the backup (needed to restore the same identity).
# The key is stored as it is read: keep the backups folder to yourself.
private_key = true
# Keep the USB serial number of the board (the unique number of its chip), to tell
# "the same board" from "another board" when restoring
hardware_id = true
# Where backups are saved: a folder of your choice, or "" for "backups" in the data folder.
# Changing it moves nothing: backups already saved stay where they are.
folder = ""

[bluetooth]
# Look for MeshCore companions nearby over Bluetooth (they show in Fleet; nothing connects to one
# until you open it or back it up). A companion connected to a phone is not seen.
enabled = true
# The PIN a companion asks the first time (pairing). MeshCore's default is 123456; a companion
# with a screen shows its own.
pin = "123456"

[debug]
# A detailed log of every action and error, with the part of the program it comes from, in
# logs/ in the data folder: to find a problem, or to send with a bug report. Never holds a
# key, a password, a node's name, position or messages (they are replaced by <...>).
log = false
"#;

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub ui: Ui,
    pub scan: Scan,
    pub backup: Backup,
    pub bluetooth: Bluetooth,
    pub debug: Debug,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Bluetooth {
    pub enabled: bool,
    /// Digits only (checked when used: ble::pin)
    pub pin: String,
}

impl Default for Bluetooth {
    fn default() -> Self {
        Bluetooth { enabled: true, pin: crate::ble::DEFAULT_PIN.into() }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Ui {
    pub language: String,
}

impl Default for Ui {
    fn default() -> Self {
        Ui { language: "auto".into() }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Scan {
    pub every_secs: u64,
    pub answer_ms: u64,
}

impl Default for Scan {
    fn default() -> Self {
        Scan { every_secs: 2, answer_ms: 4000 }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Backup {
    pub private_key: bool,
    pub hardware_id: bool,
    /// "" = backups/ in the data folder
    pub folder: String,
}

impl Default for Backup {
    fn default() -> Self {
        Backup { private_key: true, hardware_id: true, folder: String::new() }
    }
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
pub struct Debug {
    pub log: bool,
}

impl Backup {
    /// The folder backups go to: the chosen one, else `backups` in the data folder. Any path the
    /// system allows (spaces, accents, `\\server\share`): it is used as a path, never through a
    /// shell. Quotes around it (Windows "Copy as path") are taken off, `~` is the home folder,
    /// and a relative path is inside the data folder (never "wherever the program was started").
    pub fn folder_in(&self, data: &Path) -> PathBuf {
        let chosen = self.folder.trim().trim_matches('"').trim_matches('\'').trim();
        if chosen.is_empty() {
            return data.join("backups");
        }
        let home = || std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
        let path = match chosen.strip_prefix("~/").or_else(|| chosen.strip_prefix("~\\")) {
            Some(rest) => home().map_or_else(|| PathBuf::from(chosen), |h| h.join(rest)),
            None if chosen == "~" => home().unwrap_or_else(|| PathBuf::from(chosen)),
            None => PathBuf::from(chosen),
        };
        if path.is_absolute() {
            path
        } else {
            data.join(path)
        }
    }
}

/// Reads the settings; writes the commented file first when there is none, and adds the settings
/// of this version an older file does not have yet (`upgrade`). An empty or unreadable file is an
/// error, never "all defaults". Returns the settings and a line to show when the upgrade could
/// not be written (the file is then read as it is).
pub fn load(file: &Path) -> Result<(Config, Option<String>)> {
    let name = || file.display().to_string();
    if !file.exists() {
        write(file, TEMPLATE)?;
    }
    let text = std::fs::read_to_string(file).with_context(|| tf("config-unreadable", &[("file", name().into())]))?;
    if text.trim().is_empty() {
        bail!(tf("config-empty", &[("file", name().into())]));
    }
    // read as it is first: a mistake in it is said, never "fixed" by writing over it
    let table: toml::Table = toml::from_str(&text).with_context(|| tf("config-unreadable", &[("file", name().into())]))?;
    let mut note = None;
    let text = match upgrade(&text, &table) {
        Some(newer) => match write(file, &newer) {
            Ok(()) => newer,
            Err(e) => {
                note = Some(format!("{e:#}"));
                text
            }
        },
        None => text,
    };
    let mut config: Config = toml::from_str(&text).with_context(|| tf("config-unreadable", &[("file", name().into())]))?;
    config.scan.every_secs = config.scan.every_secs.clamp(1, 60);
    config.scan.answer_ms = config.scan.answer_ms.clamp(500, 30_000);
    Ok((config, note))
}

/// One setting of TEMPLATE: its section, its key, and its lines (the comments above it, the line
/// itself)
struct Entry {
    section: String,
    key: String,
    lines: Vec<String>,
}

fn template_entries() -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut section = String::new();
    let mut comments: Vec<String> = Vec::new();
    for line in TEMPLATE.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = name.trim().to_string();
            comments.clear();
        } else if trimmed.starts_with('#') {
            comments.push(line.to_string());
        } else if let Some((key, _)) = trimmed.split_once('=') {
            let mut lines = std::mem::take(&mut comments);
            lines.push(line.to_string());
            entries.push(Entry { section: section.clone(), key: key.trim().to_string(), lines });
        } else {
            comments.clear();
        }
    }
    entries
}

/// The file with every setting of TEMPLATE it lacks added, each with its comment, at the end of its
/// section (or in a new section at the end). None: nothing to add. A section the user wrote as an
/// inline table (`ui = { ... }`) or with dotted keys is left as it is: its settings keep their
/// defaults until written there.
fn upgrade(text: &str, table: &toml::Table) -> Option<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut added = false;
    for entry in template_entries() {
        let has = table.get(&entry.section).and_then(|v| v.as_table()).is_some_and(|t| t.contains_key(&entry.key));
        if has {
            continue;
        }
        match section_end(&lines, &entry.section) {
            Some(end) => {
                lines.splice(end..end, entry.lines);
            }
            // a section written another way: not touched
            None if table.contains_key(&entry.section) => continue,
            None => {
                lines.push(String::new());
                lines.push(format!("[{}]", entry.section));
                lines.extend(entry.lines);
            }
        }
        added = true;
    }
    added.then(|| lines.join("\n") + "\n")
}

/// Where a new line of `[section]` goes: after its last line that is not empty. None: no such
/// `[section]` header.
fn section_end(lines: &[String], section: &str) -> Option<usize> {
    let header = format!("[{section}]");
    let start = lines.iter().position(|l| l.trim() == header)?;
    let next = lines.iter().skip(start + 1).position(|l| l.trim().starts_with('[')).map_or(lines.len(), |i| start + 1 + i);
    let last = (start..next).rev().find(|&i| !lines[i].trim().is_empty()).unwrap_or(start);
    Some(last + 1)
}

/// Sets one setting in the file, in place: its line gets the new value, every other line (and
/// every comment) stays. The key must be one of TEMPLATE's (it is there after `upgrade`).
pub fn set(file: &Path, section: &str, key: &str, value: toml::Value) -> Result<()> {
    let name = || file.display().to_string();
    let text = std::fs::read_to_string(file).with_context(|| tf("config-unreadable", &[("file", name().into())]))?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let header = format!("[{section}]");
    let start = lines.iter().position(|l| l.trim() == header);
    let found = start.and_then(|start| {
        lines
            .iter()
            .enumerate()
            .skip(start + 1)
            .take_while(|(_, l)| !l.trim().starts_with('['))
            .find(|(_, l)| l.split_once('=').is_some_and(|(k, _)| k.trim() == key) && !l.trim().starts_with('#'))
            .map(|(i, _)| i)
    });
    match (start, found) {
        (_, Some(i)) => lines[i] = format!("{key} = {value}"),
        (Some(start), None) => {
            let end = section_end(&lines, section).unwrap_or(start + 1);
            lines.insert(end, format!("{key} = {value}"));
        }
        // no [section] header (an inline table, dotted keys): the user's own way, not rewritten
        (None, None) => bail!(tf("config-by-hand", &[("file", name().into()), ("key", format!("{section}.{key}").into())])),
    }
    let newer = lines.join("\n") + "\n";
    // what is written must read back, with the value in it
    let check: toml::Table = toml::from_str(&newer).with_context(|| tf("config-unwritable", &[("file", name().into())]))?;
    if check.get(section).and_then(|s| s.get(key)) != Some(&value) {
        bail!(tf("config-unwritable", &[("file", name().into())]));
    }
    write(file, &newer)
}

/// Writes the file: the one before kept as `<file>.bak`, the new text in a temporary file synced to
/// the disk, then renamed over it (in one step: never half written)
fn write(file: &Path, text: &str) -> Result<()> {
    let fail = || tf("config-unwritable", &[("file", file.display().to_string().into())]);
    if file.exists() {
        let mut bak = file.as_os_str().to_owned();
        bak.push(".bak");
        std::fs::copy(file, PathBuf::from(bak)).with_context(fail)?;
    }
    let mut temp = file.as_os_str().to_owned();
    temp.push(".new");
    let temp = PathBuf::from(temp);
    let mut out = std::fs::File::create(&temp).with_context(fail)?;
    out.write_all(text.as_bytes()).with_context(fail)?;
    out.sync_all().with_context(fail)?;
    drop(out);
    std::fs::rename(&temp, file).with_context(fail)
}

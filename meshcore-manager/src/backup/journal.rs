//! An interrupted restore, on disk: one file per board in `restore-journals/` (restores run side
//! by side, each on its own board, so each has its own journal). Written before the first change
//! goes to the node, removed when the restore ends, verified or put back. Found at the next start,
//! it means a restore was cut short (the program closed, the computer stopped): the app then
//! offers to finish it or to put the node back as it was (the backup made just before, `safety`),
//! both through the same restore. It is this program's own working file, not a backup: removing
//! it when the restore ended deletes nothing of the user's.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::diff::Mode;
use crate::i18n::tf;
use crate::node::Kind;
use crate::serial::Port;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Journal {
    /// The backup being put onto the node
    pub backup: PathBuf,
    /// The node's own backup, made just before: how to put it back as it was
    pub safety: PathBuf,
    pub mode: Mode,
    /// The board: its USB serial number ("" unknown) and the port it was on
    pub serial_number: String,
    pub port: String,
    pub kind: Kind,
    /// Its name when the restore started (to show)
    pub name: String,
    /// When it started (local time)
    pub started: String,
    /// It was being put back as it was (the restore had failed)
    pub putting_back: bool,
}

/// The journal file of this board: named by its USB serial number when it has a real one, else by
/// its port (only letters and digits kept: a safe file name on every system)
pub fn file(folder: &Path, port: &Port) -> PathBuf {
    let id = if port.serial_number.len() >= 8 { &port.serial_number } else { &port.path };
    let name: String = id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    folder.join(format!("{name}.json"))
}

/// The journal file of the cut-short restore whose way back is `safety`: found by what it holds,
/// not by its name (the board may be on another port now, and a name made from the port would
/// miss it: it would then stay, and be offered again at every start)
pub fn find(folder: &Path, safety: &Path) -> Option<PathBuf> {
    std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .find(|p| load(p).is_some_and(|j| j.safety == safety))
}

/// Every restore that did not end (one per board)
pub fn load_all(folder: &Path) -> Vec<Journal> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut found: Vec<Journal> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| load(&p))
        .collect();
    found.sort_by(|a, b| a.started.cmp(&b.started));
    found
}

/// Writes it, synced to the disk (in a new file first, then renamed over: never half written)
pub fn save(path: &Path, journal: &Journal) -> Result<()> {
    let fail = || tf("backup-unwritable", &[("file", path.display().to_string().into())]);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).with_context(fail)?;
    }
    let temp = path.with_extension("json.new");
    let mut file = std::fs::File::create(&temp).with_context(fail)?;
    file.write_all(&serde_json::to_vec_pretty(journal)?).with_context(fail)?;
    file.sync_all().with_context(fail)?;
    std::fs::rename(&temp, path).with_context(fail)
}

/// The restore that did not end, if any
pub fn load(path: &Path) -> Option<Journal> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// The restore ended (verified, or put back as it was)
pub fn clear(path: &Path) {
    let _ = std::fs::remove_file(path);
}

//! Where backups live: `backups/<node>/<date_time>/` with a manifest.json and one file per section.
//! - `<node>` belongs to a board (`NodeId::is_board`), not a name: a renamed node's next backup
//!   goes in the folder it already has.
//! - A backup is written in `<date_time>.partial`, each file synced to the disk. When it is
//!   whole, every file is read back and checked against its sha256 (and the manifest is read
//!   back too); only then is the folder renamed. A folder without that ending is always a
//!   finished, verified backup.
//! - An attempt that fails (the node stopped answering, the USB went, a file did not read back
//!   right) removes its own `.partial` folder, made by this very attempt: no orphans. A
//!   `.partial` left by a crash or a power cut is never touched (not ours to judge), only hidden.
//! - Nothing here deletes or replaces a finished backup. Ever. Deleting is the user's own,
//!   explicit action.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use super::{Data, Manifest, NodeId, Part, Reason, Section, State, FORMAT};
use crate::i18n::tf;
use crate::node::Kind;

const PARTIAL: &str = ".partial";

/// One backup folder
pub struct Snapshot {
    /// "2026-11-18_16-47-05" (local time: the folder's name)
    pub stamp: String,
    /// The backup's folder
    pub path: PathBuf,
    /// None: the folder has no readable manifest.json
    pub manifest: Option<Manifest>,
}

/// Every backup of one board, newest first. A board is known by its USB serial number, else
/// its public key (`NodeId::is_board`), not by its name: the backups of a node renamed in a
/// MeshCore app stay together, even when they are in two folders (made before and after).
pub struct NodeBackups {
    /// As its newest backup has them
    pub name: String,
    pub kind: Option<Kind>,
    /// The newest backup's node, None when no manifest could be read
    pub id: Option<NodeId>,
    pub snapshots: Vec<Snapshot>,
}

impl Snapshot {
    /// When it was made, as a person reads it ("today 13:03", "yesterday 22:10", "2026-11-12 09:30")
    pub fn shown(&self) -> String {
        stamp_text(&self.stamp)
    }
}

/// A backup folder's name ("2026-11-18_13-03-48", local time) as a person reads it:
/// "2026-11-18 13:03", always the same width (no "today": columns stay aligned). A name that is
/// not a date is shown as it is.
pub fn stamp_text(stamp: &str) -> String {
    match chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d_%H-%M-%S") {
        Ok(at) => at.format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => stamp.to_string(),
    }
}

/// The manifest's `created` (UTC, RFC 3339) as a person reads it, in this computer's time:
/// "2026-11-18 22:57" (as the list shows the backups)
pub fn created_text(created: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(created) {
        Ok(at) => stamp_text(&at.with_timezone(&chrono::Local).format("%Y-%m-%d_%H-%M-%S").to_string()),
        Err(_) => created.to_string(),
    }
}

impl NodeBackups {
    /// These are backups of the board with this USB serial number and public key
    pub fn is_board(&self, hardware_id: &str, public_key: &str) -> bool {
        self.id.as_ref().is_some_and(|id| id.is_board(hardware_id, public_key))
    }
}

fn sub_folders(dir: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    list.sort();
    list
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Every board that has backups, by name, each with its backups newest first.
/// Folders of the same board (`NodeId::is_board`) are shown as one.
pub fn list(root: &Path) -> Vec<NodeBackups> {
    let mut nodes: Vec<NodeBackups> = Vec::new();
    for folder in sub_folders(root) {
        let snapshots: Vec<Snapshot> = sub_folders(&folder)
            .into_iter()
            .filter(|dir| !file_name(dir).ends_with(PARTIAL))
            .map(|dir| Snapshot {
                stamp: file_name(&dir),
                manifest: std::fs::read(dir.join("manifest.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()),
                path: dir,
            })
            .collect();
        if snapshots.is_empty() {
            continue;
        }
        let newest = snapshots.iter().rev().find_map(|s| s.manifest.as_ref()).map(|m| m.node.clone());
        let same = newest.as_ref().and_then(|id| nodes.iter().position(|n| n.is_board(&id.hardware_id, &id.public_key)));
        match same {
            Some(i) => nodes[i].snapshots.extend(snapshots),
            None => nodes.push(NodeBackups { name: file_name(&folder), kind: None, id: newest, snapshots }),
        }
    }
    for node in &mut nodes {
        // newest first, over every folder of the board
        node.snapshots.sort_by(|a, b| b.stamp.cmp(&a.stamp));
        // name, kind and identity as the newest readable backup has them
        if let Some(m) = node.snapshots.iter().find_map(|s| s.manifest.as_ref()) {
            if !m.node.name.is_empty() {
                node.name = m.node.name.clone();
            }
            node.kind = Some(m.node.kind);
            node.id = Some(m.node.clone());
        }
    }
    nodes.sort_by_key(|n| n.name.to_lowercase());
    nodes
}

/// The folder for a new backup of `node`: the folder of its newest backup when the board was
/// backed up before (under any name), else a new one named after it
fn folder_for(root: &Path, node: &NodeId) -> PathBuf {
    list(root)
        .into_iter()
        .find(|n| n.is_board(&node.hardware_id, &node.public_key))
        .and_then(|n| n.snapshots.first().and_then(|s| s.path.parent().map(Path::to_path_buf)))
        .unwrap_or_else(|| root.join(node_folder(node)))
}

/// The folder of a node: its name made safe for any file system, and the start of its public key
/// (two nodes can have the same name; one node can be renamed). Plain ASCII letters, digits, `-`
/// and `_` only (a name in another alphabet becomes `_`s, and then the key alone names it): the
/// same on Windows, Linux and macOS, case-insensitive or not, in any language. Never a name
/// Windows reserves (CON, NUL, COM1...).
fn node_folder(node: &NodeId) -> String {
    let name: String =
        node.name.trim().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let key: String = node.public_key.chars().filter(char::is_ascii_hexdigit).take(8).collect();
    let folder = match (name.trim_matches('_').is_empty(), key.is_empty()) {
        (true, true) => "unknown".into(),
        (true, false) => key,
        (false, true) => name,
        (false, false) => format!("{name}_{key}"),
    };
    if reserved(&folder) {
        format!("{folder}_node")
    } else {
        folder
    }
}

/// A name Windows keeps for devices, in any case and with any extension
fn reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem[3..].chars().all(|c| c.is_ascii_digit()))
}

/// Writes one backup: `begin`, `add` each section, `finish`. Dropped before `finish` succeeded
/// (any error on the way): its own `.partial` folder is removed.
pub struct Writer {
    partial: PathBuf,
    done: PathBuf,
    manifest: Manifest,
    finished: bool,
}

impl Writer {
    /// `at`: when the node was read (the backup's name and date): now, or the moment the view
    /// read it (a backup from that reading)
    pub fn begin(root: &Path, node: NodeId, reason: Reason, at: chrono::DateTime<chrono::Local>) -> Result<Writer> {
        let stamp = at.format("%Y-%m-%d_%H-%M-%S").to_string();
        let folder = folder_for(root, &node);
        let done = folder.join(&stamp);
        let partial = folder.join(format!("{stamp}{PARTIAL}"));
        // two backups of one board in the same second (or of one reading): the second fails, it
        // never writes into the first one's folder. `create_dir` (not `_all`) is the check
        // itself: it fails when the folder is there, in one step, even with jobs running side by
        // side.
        if done.exists() {
            anyhow::bail!(crate::i18n::t("backup-already"));
        }
        std::fs::create_dir_all(&folder).with_context(|| unwritable(&folder))?;
        std::fs::create_dir(&partial).with_context(|| unwritable(&partial))?;
        let manifest = Manifest {
            format: FORMAT,
            created: at.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            tool: format!("mcmanager {}", env!("CARGO_PKG_VERSION")),
            node,
            sections: Vec::new(),
            complete: false,
            missing: Vec::new(),
            verified: false,
            reason,
        };
        Ok(Writer { partial, done, manifest, finished: false })
    }

    /// Saves one section as `<id>.json`, synced to the disk
    pub fn add(&mut self, part: &Part) -> Result<()> {
        let file = format!("{}.json", part.id);
        let (bytes, items) = part.data.to_file()?;
        write_synced(&self.partial.join(&file), &bytes)?;
        self.manifest.sections.push(Section { id: part.id.clone(), file, items, sha256: sha256(&bytes), secret: part.secret });
        Ok(())
    }

    /// Writes the manifest, reads every file back and checks it, then gives the backup its final
    /// name. Returns its folder (named after the date and time).
    pub fn finish(mut self, missing: Vec<String>) -> Result<PathBuf> {
        self.manifest.complete = true;
        self.manifest.missing = missing;
        self.manifest.verified = true;
        let manifest_path = self.partial.join("manifest.json");
        write_synced(&manifest_path, &serde_json::to_vec_pretty(&self.manifest)?)?;
        // read back from the disk: each section as it was hashed, the manifest as written
        for section in &self.manifest.sections {
            let path = self.partial.join(&section.file);
            let bytes = std::fs::read(&path).with_context(|| unreadable(&path))?;
            if sha256(&bytes) != section.sha256 {
                anyhow::bail!(unreadable(&path));
            }
        }
        let back: Manifest = serde_json::from_slice(&std::fs::read(&manifest_path).with_context(|| unreadable(&manifest_path))?)
            .with_context(|| unreadable(&manifest_path))?;
        if back.sections.len() != self.manifest.sections.len() {
            anyhow::bail!(unreadable(&manifest_path));
        }
        std::fs::rename(&self.partial, &self.done).with_context(|| unwritable(&self.done))?;
        self.finished = true;
        // there under its name, readable
        let named = self.done.join("manifest.json");
        if !named.is_file() {
            anyhow::bail!(unreadable(&named));
        }
        Ok(self.done.clone())
    }
}

impl Drop for Writer {
    /// An attempt that did not finish: its own `.partial` folder goes (never a finished backup:
    /// after the rename `finished` is set and nothing is removed)
    fn drop(&mut self) {
        if !self.finished && file_name(&self.partial).ends_with(PARTIAL) {
            let _ = std::fs::remove_dir_all(&self.partial);
        }
    }
}

/// A finished backup read back for a restore: its manifest, and every section, each file checked
/// against the sha256 the manifest keeps. Any file missing, changed or unreadable = an error
/// (nothing is restored from a backup that is not exactly what was saved).
pub fn load(folder: &Path) -> Result<(Manifest, State)> {
    let manifest_path = folder.join("manifest.json");
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(&manifest_path).with_context(|| damaged(&manifest_path))?)
        .with_context(|| damaged(&manifest_path))?;
    // made by a newer version in a layout this one cannot read right: never put onto a node
    if manifest.format > FORMAT {
        anyhow::bail!(tf("backup-newer-format", &[("format", manifest.format.into()), ("tool", manifest.tool.as_str().into())]));
    }
    let mut state = State { parts: Vec::new(), missing: manifest.missing.clone() };
    for section in &manifest.sections {
        // a file named outside the folder ("../x"): not a backup this program wrote
        if section.file.contains(['/', '\\']) || section.file.contains("..") {
            anyhow::bail!(damaged(&folder.join(&section.file)));
        }
        let path = folder.join(&section.file);
        let bytes = std::fs::read(&path).with_context(|| damaged(&path))?;
        if sha256(&bytes) != section.sha256 {
            anyhow::bail!(damaged(&path));
        }
        match Data::from_file(&bytes) {
            Some(data) => state.parts.push(Part { id: section.id.clone(), secret: section.secret, data }),
            // whole (its sha256 matched) but in a shape this version does not know (a newer
            // version's section): kept on disk, left out here, and named
            None => state.missing.push(section.id.clone()),
        }
    }
    Ok((manifest, state))
}

/// Writes a file and waits until it is on the disk (not only in the system's cache)
fn write_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::File::create(path).with_context(|| unwritable(path))?;
    file.write_all(bytes).with_context(|| unwritable(path))?;
    file.sync_all().with_context(|| unwritable(path))
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).as_slice().iter().map(|b| format!("{b:02x}")).collect()
}

fn unwritable(path: &Path) -> String {
    tf("backup-unwritable", &[("file", path.display().to_string().into())])
}

fn unreadable(path: &Path) -> String {
    tf("backup-unverified", &[("file", path.display().to_string().into())])
}

fn damaged(path: &Path) -> String {
    tf("backup-damaged", &[("file", path.display().to_string().into())])
}

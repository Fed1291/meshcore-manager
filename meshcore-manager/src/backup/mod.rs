//! Backups of nodes: what one is (this file), where they live (store.rs), reading a node into one
//! (capture.rs), putting one back (restore.rs). The layout on disk (manifest.json, one file per
//! section, `FORMAT` below) is a promise to every backup already made: raise FORMAT when an older
//! program could no longer read the result.
//!
//! A backup keeps what the node answered, as it answered (raw frames in hex, console replies as
//! text): nothing is interpreted when saving, so a backup stays complete even for settings this
//! version does not know yet.

pub mod capture;
/// What changed between two backups of one node
pub mod compare;
/// What differs between a backup and a node, as the node's own commands
pub mod diff;
/// An interrupted restore, kept on disk until it is finished or put back
pub mod journal;
/// The address book's part of a backup: custom names, roles, favourites of the node's keys
pub mod names;
/// Reading a node's whole state (for a backup and for a restore)
pub mod read;
pub mod restore;
pub mod store;

use serde::{Deserialize, Serialize};

use crate::node::Kind;

/// Version of the backup layout (manifest.json `format`). Raised only for a change an older
/// program could not read right (a field that changes meaning, a section in a new shape under an
/// old id). Adding never raises it: a new section (a new id), a new field in manifest.json (with
/// `#[serde(default)]` here), a new reason or kind (read as `Unknown` by older programs). An older
/// program ignores what it does not know and keeps it; a backup of a higher format is listed but
/// not restored (store::load). The other way is a promise: this program reads and restores every
/// format up to FORMAT, in full. Raising it means keeping a reader for every older one in
/// store::load (old shape -> new, in memory; files on disk never rewritten).
pub const FORMAT: u32 = 1;

/// manifest.json: what a backup folder holds and which node it came from
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Manifest {
    pub format: u32,
    /// UTC, RFC 3339
    pub created: String,
    /// "mcmanager 0.1.0"
    pub tool: String,
    pub node: NodeId,
    pub sections: Vec<Section>,
    /// false: something could not be read (named in `missing`)
    pub complete: bool,
    /// Ids of what is not in it: what this firmware does not have, or this version does not read
    /// yet (a failure to read ends the backup instead: there is no half backup)
    #[serde(default)]
    pub missing: Vec<String>,
    /// Every file was read back from the disk and matched its sha256 before the backup got its
    /// name (false in backups made by older versions)
    #[serde(default)]
    pub verified: bool,
    /// Why it was made
    #[serde(default)]
    pub reason: Reason,
}

/// Why a backup was made: shown in the lists, so a backup found days later says what it was for.
/// Saved as its name, read through a plain text: a reason a newer version adds reads as `Unknown`
/// here, and the backup still opens.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(from = "String", into = "String")]
pub enum Reason {
    /// A backup made by an older version
    #[default]
    Unknown,
    /// Asked for (b)
    Manual,
    /// Before a config commit was written
    BeforeConfig,
    BeforeRestore,
    BeforeFirmware,
    BeforeReset,
}

/// The names on disk: never change one (old backups would read as `Unknown`)
const REASON_NAMES: [(Reason, &str); 6] = [
    (Reason::Unknown, "unknown"),
    (Reason::Manual, "manual"),
    (Reason::BeforeConfig, "before-config"),
    (Reason::BeforeRestore, "before-restore"),
    (Reason::BeforeFirmware, "before-firmware"),
    (Reason::BeforeReset, "before-reset"),
];

impl From<String> for Reason {
    fn from(name: String) -> Reason {
        REASON_NAMES.iter().find(|(_, n)| *n == name).map_or(Reason::Unknown, |(r, _)| *r)
    }
}

impl From<Reason> for String {
    fn from(reason: Reason) -> String {
        REASON_NAMES.iter().find(|(r, _)| *r == reason).map_or("unknown", |(_, n)| n).to_string()
    }
}

impl Reason {
    pub fn text(self) -> String {
        crate::i18n::t(match self {
            Reason::Unknown => "reason-unknown",
            Reason::Manual => "reason-manual",
            Reason::BeforeConfig => "reason-before-config",
            Reason::BeforeRestore => "reason-before-restore",
            Reason::BeforeFirmware => "reason-before-firmware",
            Reason::BeforeReset => "reason-before-reset",
        })
    }
}

/// The node a backup came from
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NodeId {
    pub kind: Kind,
    pub name: String,
    #[serde(default)]
    pub public_key: String,
    /// The board as the firmware names it
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub firmware: String,
    /// USB vendor:product
    #[serde(default)]
    pub usb_id: String,
    /// The board's USB serial number (its chip's unique number); "" when not kept
    #[serde(default)]
    pub hardware_id: String,
}

impl NodeId {
    /// Came from the board with this USB serial number and public key: the same serial number
    /// when both are known (it never changes), else the same public key. The name does not count:
    /// a node renamed in a MeshCore app is still the same node.
    /// A short serial number ("0001", given by many cheap USB-serial chips to every board) is not
    /// trusted: the key decides then.
    pub fn is_board(&self, hardware_id: &str, public_key: &str) -> bool {
        let unique = |id: &str| id.len() >= 8;
        if unique(&self.hardware_id) && unique(hardware_id) {
            return self.hardware_id == hardware_id;
        }
        !self.public_key.is_empty() && self.public_key == public_key
    }
}

/// One file of a backup
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Section {
    /// "contacts", "channels", "settings"...
    pub id: String,
    pub file: String,
    /// How many frames / replies it holds
    pub items: usize,
    pub sha256: String,
    /// Holds a private key or a password
    #[serde(default)]
    pub secret: bool,
}

/// A section file of a companion: the reply frames, in hex, in the order received
#[derive(Serialize, Deserialize)]
pub struct Frames {
    pub frames: Vec<String>,
}

/// A section file of a console node: each command with its reply
#[derive(Serialize, Deserialize)]
pub struct Replies {
    pub replies: Vec<Reply>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Reply {
    pub command: String,
    pub reply: String,
}

/// What one section holds, in memory
#[derive(Clone)]
pub enum Data {
    /// Companion reply frames, whole (reply code first)
    Frames(Vec<Vec<u8>>),
    /// Console commands and their replies
    Replies(Vec<Reply>),
}

/// One section of a node's state ("contacts", "settings"...)
#[derive(Clone)]
pub struct Part {
    pub id: String,
    /// Holds a private key, channel keys or a password
    pub secret: bool,
    pub data: Data,
}

/// Everything read from a node at one time: what a backup keeps, what a restore compares. The
/// same whether it was just read (read.rs) or loaded from a backup (store::load).
#[derive(Clone, Default)]
pub struct State {
    pub parts: Vec<Part>,
    /// Section ids this firmware does not have (or this version does not read yet)
    pub missing: Vec<String>,
}

/// A node's whole state as its view read it, kept with the node until it is read again (`r`):
/// a manual backup (`b`) writes it to disk at once, without connecting again. The backup is
/// then the node as it was at `at`. Secrets included (left out when written, unless
/// `[backup] private_key` is on).
pub struct Reading {
    pub state: State,
    pub who: capture::Who,
    /// When it was read (this computer's time): the backup's date
    pub at: chrono::DateTime<chrono::Local>,
}

impl std::fmt::Debug for Reading {
    /// Never what it holds (keys, names): only that it is there
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reading({} parts)", self.state.parts.len())
    }
}

impl State {
    pub fn add(&mut self, id: &str, secret: bool, data: Data) {
        self.parts.push(Part { id: id.to_string(), secret, data });
    }

    fn data(&self, id: &str) -> Option<&Data> {
        self.parts.iter().find(|p| p.id == id).map(|p| &p.data)
    }

    /// Every reply of section `id` joined, one per line (a list kept a line per reply, the access
    /// list); None when the section is not there (an empty list is Some(""))
    pub fn lines(&self, id: &str) -> Option<String> {
        self.data(id)?;
        Some(self.replies(id).iter().map(|r| r.reply.as_str()).collect::<Vec<_>>().join("\n"))
    }

    /// The frames of section `id` (none when it is not there or holds replies)
    pub fn frames(&self, id: &str) -> &[Vec<u8>] {
        match self.data(id) {
            Some(Data::Frames(frames)) => frames,
            _ => &[],
        }
    }

    /// The one frame of a single-frame section
    pub fn frame(&self, id: &str) -> Option<&[u8]> {
        self.frames(id).first().map(Vec::as_slice)
    }

    pub fn replies(&self, id: &str) -> &[Reply] {
        match self.data(id) {
            Some(Data::Replies(replies)) => replies,
            _ => &[],
        }
    }

    /// The reply to `command` in section `id`
    pub fn reply(&self, id: &str, command: &str) -> Option<&str> {
        self.replies(id).iter().find(|r| r.command == command).map(|r| r.reply.as_str())
    }
}

impl Data {
    /// The file a section is kept in (`Frames` in hex, or `Replies`), and how many items
    pub fn to_file(&self) -> serde_json::Result<(Vec<u8>, usize)> {
        match self {
            Data::Frames(frames) => {
                let file = Frames { frames: frames.iter().map(|f| crate::proto::hex(f)).collect() };
                Ok((serde_json::to_vec_pretty(&file)?, frames.len()))
            }
            Data::Replies(replies) => Ok((serde_json::to_vec_pretty(&Replies { replies: replies.clone() })?, replies.len())),
        }
    }

    /// A section file read back (None: neither layout, or bad hex)
    pub fn from_file(bytes: &[u8]) -> Option<Data> {
        if let Ok(file) = serde_json::from_slice::<Frames>(bytes) {
            let frames: Option<Vec<Vec<u8>>> = file.frames.iter().map(|h| unhex(h)).collect();
            return frames.map(Data::Frames);
        }
        serde_json::from_slice::<Replies>(bytes).ok().map(|file| Data::Replies(file.replies))
    }
}

/// "a1b2" -> [0xa1, 0xb2]
fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len() / 2).map(|i| u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()).collect()
}

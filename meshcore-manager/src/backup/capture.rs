//! Reads a node into a new backup: open, read everything (read.rs), make sure it is still the same
//! board, write and verify (store.rs). Only reads: nothing here changes the node.
//! All or nothing: any failure on the way and nothing is kept (store::Writer removes its own
//! half-written folder).

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Result};

use super::store::Writer;
use super::{names, read, NodeId, Reading, Reason, State};
use crate::config::Config;
use crate::i18n::t;
use crate::node::cli::Cli;
use crate::node::companion::Companion;
use crate::node::{Kind, Node};
use crate::serial::Link;

/// Backs the node up into `root`, for `reason`. `progress(step, done, total)` is called as it
/// goes. Returns the new backup's folder, once whole and verified.
/// A backup made before a restore always keeps the private key (a restore that has to be undone
/// may need it), whatever `[backup] private_key` says.
/// `book`: the address book file; what is yours about the node's keys (custom names, roles,
/// favourites) goes into the backup too (names.rs). A book that cannot be read: the backup is
/// made without that part, and says so (`missing`).
pub fn capture(
    node: &Node,
    config: &Config,
    root: &Path,
    book: &Path,
    reason: Reason,
    progress: &mut dyn FnMut(&str, usize, usize),
) -> Result<PathBuf> {
    if !node.kind.is_node() {
        bail!(t("backup-not-a-node"));
    }
    let keys = config.backup.private_key || reason == Reason::BeforeRestore;
    let wait = Duration::from_millis(config.scan.answer_ms);
    let (state, who) = read_node(node, keys, wait, progress)?;
    write(node, state, who, chrono::Local::now(), config, root, book, reason)
}

/// Backs the node up from the reading its view made (node::Reading), without opening it again:
/// instant. The backup is the node as it was then, dated then. Its secrets are left out unless
/// `[backup] private_key` is on, as for a backup read now.
pub fn save(node: &Node, reading: &Reading, config: &Config, root: &Path, book: &Path, reason: Reason) -> Result<PathBuf> {
    if !node.kind.is_node() {
        bail!(t("backup-not-a-node"));
    }
    let mut state = reading.state.clone();
    if !(config.backup.private_key || reason == Reason::BeforeRestore) {
        // what read.rs leaves out when not asked for the secrets
        state.parts.retain(|p| !SECRET_PARTS.contains(&p.id.as_str()));
        state.missing.retain(|id| !SECRET_PARTS.contains(&id.as_str()));
    }
    let who = Who {
        name: reading.who.name.clone(),
        public_key: reading.who.public_key.clone(),
        model: reading.who.model.clone(),
        firmware: reading.who.firmware.clone(),
    };
    write(node, state, who, reading.at, config, root, book, reason)
}

/// The sections read.rs reads only when asked for the secrets: the companion's private key, a
/// console node's secret settings
const SECRET_PARTS: [&str; 2] = ["private-key", "secrets"];

/// Adds the address book's part, then writes the backup, dated `at`
#[allow(clippy::too_many_arguments)]
fn write(
    node: &Node,
    mut state: State,
    who: Who,
    at: chrono::DateTime<chrono::Local>,
    config: &Config,
    root: &Path,
    book: &Path,
    reason: Reason,
) -> Result<PathBuf> {
    match crate::book::load(book) {
        Ok(book) => {
            if let Some(part) = names::part(&book, &state) {
                state.parts.push(part);
            }
        }
        Err(_) => state.missing.push(names::ID.to_string()),
    }
    // who it is, as the node itself said when it was opened (a Bluetooth companion not read
    // before has nothing in its Fleet row: its backup would have no key, no board, no firmware)
    let or = |said: String, listed: &str| if said.is_empty() { listed.to_string() } else { said };
    let id = NodeId {
        kind: node.kind,
        name: or(who.name, &node.name),
        public_key: or(who.public_key, &node.public_key),
        model: or(who.model, &node.model),
        firmware: or(who.firmware, &node.firmware),
        usb_id: node.port.usb_id(),
        hardware_id: if config.backup.hardware_id { node.port.serial_number.clone() } else { String::new() },
    };
    // never a backup that cannot be told apart from another one's
    if id.public_key.is_empty() {
        bail!(t("backup-no-answer"));
    }
    let mut out = Writer::begin(root, id, reason, at)?;
    for part in &state.parts {
        out.add(part)?;
    }
    out.finish(state.missing)
}

/// Who answered when the port was opened: the node's own words (empty when it did not say)
#[derive(Default)]
pub struct Who {
    pub name: String,
    pub public_key: String,
    pub model: String,
    pub firmware: String,
}

/// Opens the node's port, reads its whole state, and checks that the same board answers at the
/// start and at the end (another board, or none = an error). The key it gave at the start must
/// also be the one the Fleet knows for this port, when it knows one.
pub fn read_node(node: &Node, keys: bool, wait: Duration, progress: &mut dyn FnMut(&str, usize, usize)) -> Result<(State, Who)> {
    let link = Link::open(&node.port.path)?;
    // the key at the start: the board on the port, checked again at the end
    let check = |who: &Who| -> Result<()> {
        if who.public_key.is_empty() || (!node.public_key.is_empty() && !who.public_key.eq_ignore_ascii_case(&node.public_key)) {
            bail!(t("backup-board-changed"));
        }
        Ok(())
    };
    let same = |found: Option<String>, start: &str| found.is_some_and(|k| k.eq_ignore_ascii_case(start));
    if node.kind == Kind::Companion {
        let mut companion = Companion::new(link);
        // the board may have restarted when the port opened: wait until it answers
        let Some(hello) = companion.hello(wait)? else { bail!(t("backup-no-answer")) };
        let mut who = Who::default();
        if let Some(me) = hello.me {
            who.name = me.name;
            who.public_key = me.public_key;
        }
        if let Some(device) = hello.device {
            who.model = device.model;
            who.firmware = format!("{} ({})", device.version, device.build_date);
        }
        check(&who)?;
        let state = read::companion(&mut companion, keys, progress)?;
        if !same(companion.hello(wait)?.and_then(|h| h.me).map(|m| m.public_key), &who.public_key) {
            bail!(t("backup-board-changed"));
        }
        Ok((state, who))
    } else {
        let mut console = Cli::new(link);
        let Some(hello) = console.hello()? else { bail!(t("backup-no-answer")) };
        let who = Who { name: hello.name, public_key: hello.public_key, model: hello.board, firmware: hello.version };
        check(&who)?;
        let state = read::console(&mut console, node.kind, keys, progress)?;
        if !same(console.hello()?.map(|h| h.public_key), &who.public_key) {
            bail!(t("backup-board-changed"));
        }
        Ok((state, who))
    }
}

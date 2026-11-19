//! Putting a backup onto a node, so that it ends one of two ways only: the node holds the backup
//! (read back and verified), or it is put back as it was before. Never left half done without
//! saying so, and what was done is on disk (journal.rs) if the program itself stops.
//!
//! How (in this order; nothing reaches the node before step 4):
//! 1. the backup: every file checked against its sha256 (store::load);
//! 2. the node: it answers, and is the kind the backup came from;
//! 3. a backup of the node as it is now, verified ("before a restore", private key included);
//! 4. the journal, then the writes, by convergence: read the node, compare with the backup
//!    (diff.rs), send only what differs, read again... until nothing differs (= verified).
//!    The node cannot take many changes as one, so a lost link is met by waiting for the same
//!    board (its USB serial number, else its port) and reading it again: what is left is then
//!    sent, nothing twice on a guess. A console setting used only after a restart (radio, key)
//!    gets the restart, then one more check;
//! 5. a refusal, or a node that does not come back in time: the same convergence towards the
//!    backup of step 3 (put back as it was). If even that cannot end (the node is gone), the
//!    journal stays and the app says so, and offers to finish at the next start.
//!
//! Three ways (diff::Mode): same board, move (another board takes the identity), copy (settings,
//! channels, contacts; the target keeps its identity).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};

use super::diff::{self, Mode};
use super::journal::{self, Journal};
use super::{capture, names, read, store, Reason, State};
use crate::book::Book;
use crate::config::Config;
use crate::i18n::{t, tf};
use crate::node::cli::Cli;
use crate::node::companion::Companion;
use crate::node::edit::Change;
use crate::node::{self, Answer, Kind, Node};
use crate::paths::Paths;
use crate::serial::{self, Link, Port};

/// Rounds of "read, compare, send what differs" before giving up (a restart counts as one)
const ROUNDS: usize = 5;
/// How long a node that went away (unplugged, restarting) is waited for
const COME_BACK: Duration = Duration::from_secs(90);
/// The most lines of changes a question shows (the rest counted)
const SHOWN: usize = 16;

/// A restore to do (or to show first)
#[derive(Clone, Debug)]
pub struct Request {
    /// The backup's folder
    pub backup: PathBuf,
    /// The node's port when it was chosen (it is found again by its USB serial number)
    pub port: Port,
    pub mode: Mode,
    /// The node's backup made before, when a cut-short restore is finished from its journal
    pub safety: Option<PathBuf>,
    /// Finishing a cut-short "put back as it was" (the backup is then that safety backup)
    pub putting_back: bool,
}

/// What a restore would do, to ask before doing it
pub struct Preview {
    pub request: Request,
    /// For the question: onto which node, how, what changes, what cannot go
    pub lines: Vec<String>,
    /// How many changes (0: the node already holds the backup)
    pub changes: usize,
    /// The node takes another identity (a move): asked twice
    pub moves: bool,
}

/// How a restore ended once something was written
pub enum Outcome {
    /// The node holds the backup, read back and verified. Onto another board: what that board
    /// refused is left as it has it, one line each (empty: all of it went on)
    Restored(Vec<String>),
    /// It could not be done (why): the node was put back as it was, verified
    PutBack(String),
    /// Not even that could end (why): the journal is kept, the app offers to finish later
    Stuck(String),
    /// Restored without a backup first, and it could not finish (why): the node holds part of
    /// the backup; running the restore again sends what is left
    Stopped(String),
}

/// The open link to the node, by its own protocol
enum Conn {
    Companion(Companion),
    Console(Cli),
    /// Let go, to connect again (a Bluetooth companion takes one connection at a time)
    Closed,
}

struct Session {
    port: Port,
    kind: Kind,
    conn: Conn,
}

impl Session {
    /// Opens the port and makes sure the node of this kind answers there
    fn open(port: &Port, kind: Kind, wait: Duration) -> Result<Session> {
        let link = Link::open(&port.path)?;
        let conn = if kind == Kind::Companion {
            let mut c = Companion::new(link);
            if c.hello(wait)?.is_none() {
                bail!(t("backup-no-answer"));
            }
            Conn::Companion(c)
        } else {
            let mut c = Cli::new(link);
            match c.hello()? {
                Some(hello) if hello.kind == kind => Conn::Console(c),
                _ => bail!(t("backup-no-answer")),
            }
        };
        Ok(Session { port: port.clone(), kind, conn })
    }

    /// Its whole state, keys included (to compare them)
    fn read(&mut self, progress: &mut dyn FnMut(&str, usize, usize)) -> Result<State> {
        match &mut self.conn {
            Conn::Companion(c) => read::companion(c, true, progress),
            Conn::Console(c) => read::console(c, self.kind, true, progress),
            Conn::Closed => bail!(t("ble-closed")),
        }
    }

    /// The link went or a command got no answer: let go of the old link first (Bluetooth: one
    /// connection at a time), then connect again (`come_back`)
    fn again(&mut self, wait: Duration, progress: &mut dyn FnMut(&str, usize, usize)) -> std::result::Result<(), String> {
        self.conn = Conn::Closed;
        *self = come_back(&self.port, self.kind, wait, progress).map_err(|e| format!("{e:#}"))?;
        Ok(())
    }

    fn send(&mut self, change: &Change) -> Result<Answer> {
        match (&mut self.conn, change) {
            (Conn::Companion(c), Change::Frame(frame)) => c.command(frame),
            (Conn::Console(c), Change::Set { name, value, secret }) => c.set(name, value, *secret),
            (Conn::Console(c), Change::Console(command)) => c.command(command),
            _ => Ok(Answer::Refused(t("edit-wrong-kind"))),
        }
    }

    /// Restarts the node (no answer comes: it is gone until it is back)
    fn restart(&mut self) {
        match &mut self.conn {
            Conn::Companion(c) => {
                let _ = c.command(&crate::proto::reboot());
            }
            Conn::Console(c) => {
                let _ = c.run("reboot", Duration::from_millis(500));
            }
            Conn::Closed => {}
        }
    }
}

/// Waits for the board to answer again (plugged back in, restarted, the Bluetooth link back):
/// on USB on any port, found by its USB serial number; on Bluetooth at its own address
/// (connected again directly: a Bluetooth companion is not in the USB port list)
fn come_back(port: &Port, kind: Kind, wait: Duration, progress: &mut dyn FnMut(&str, usize, usize)) -> Result<Session> {
    let until = Instant::now() + COME_BACK;
    loop {
        let left = until.saturating_duration_since(Instant::now()).as_secs() as usize;
        progress(&tf("restore-waiting", &[("secs", left.into())]), 0, 0);
        let candidates = if port.is_ble() { vec![port.clone()] } else { serial::ports().into_iter().filter(|p| p.same_board(port)).collect() };
        for found in candidates {
            if let Ok(session) = Session::open(&found, kind, wait) {
                return Ok(session);
            }
        }
        if Instant::now() >= until {
            bail!(t("restore-gone"));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Read, compare, send what differs, until nothing differs (Ok = verified). Err = why it could not
/// end: a refusal (nothing more is sent then), the node did not come back, or it does not settle.
/// Onto another board (move, copy) a refusal does not stop it: that board may not take that
/// value (another radio chip, a lower power, a firmware without that setting), so it keeps its
/// own, the rest goes on, and Ok lists what was left so. The same board takes all or stops.
fn converge(
    session: &mut Session,
    want: &State,
    mode: Mode,
    wait: Duration,
    progress: &mut dyn FnMut(&str, usize, usize),
) -> std::result::Result<Vec<String>, String> {
    let mut restart = false;
    // another board: the steps it refused (by their line), not sent again, and why
    let mut refused: Vec<(String, String)> = Vec::new();
    for _ in 0..ROUNDS {
        let now = match session.read(progress) {
            Ok(now) => now,
            Err(_) => {
                session.again(wait, progress)?;
                continue;
            }
        };
        let mut d = diff::diff(session.kind, want, &now, mode);
        d.steps.retain(|s| !refused.iter().any(|(line, _)| *line == s.line));
        if d.steps.is_empty() {
            if !restart {
                let left = refused.iter().map(|(line, why)| tf("restore-left-line", &[("what", line.as_str().into()), ("why", why.as_str().into())]));
                return Ok(left.collect());
            }
            // the settings are in; the node uses them after a restart: then one more check
            progress(&t("restore-restarting"), 0, 0);
            session.restart();
            std::thread::sleep(Duration::from_secs(3));
            session.again(wait, progress)?;
            restart = false;
            continue;
        }
        restart |= d.reboot;
        let total = d.steps.len();
        for (i, step) in d.steps.iter().enumerate() {
            progress(&step.line, i, total);
            match session.send(&step.change) {
                Ok(Answer::Done) => {}
                Ok(Answer::Refused(why)) if mode != Mode::Same => refused.push((step.line.clone(), why)),
                Ok(Answer::Refused(why)) => {
                    return Err(tf("restore-refused", &[("what", step.line.as_str().into()), ("why", why.into())]));
                }
                // no answer, or the link went: the node is read again before anything more
                Ok(Answer::Silent) | Err(_) => {
                    session.again(wait, progress)?;
                    break;
                }
            }
        }
    }
    Err(t("restore-not-settled"))
}

/// Reads the node on `port` as a backup sees it, to compare a backup with it. Nothing is written.
pub fn read_now(port: &Port, wait: Duration, progress: &mut dyn FnMut(&str, usize, usize)) -> Result<(Kind, State)> {
    let node = node::identify(port, wait);
    if !node.kind.is_node() {
        bail!(t("restore-no-target"));
    }
    let mut session = Session::open(port, node.kind, wait)?;
    Ok((node.kind, session.read(progress)?))
}

/// Reads the backup and the node and says what a restore would do. Nothing is written.
pub fn preview(request: Request, book: &Book, wait: Duration, progress: &mut dyn FnMut(&str, usize, usize)) -> Result<Preview> {
    let (manifest, want) = store::load(&request.backup)?;
    let node = node::identify(&request.port, wait);
    if !node.kind.is_node() {
        bail!(t("restore-no-target"));
    }
    if node.kind != manifest.node.kind {
        bail!(tf("restore-kind-differs", &[("backup", manifest.node.kind.label().into()), ("target", node.kind.label().into())]));
    }
    let mut session = Session::open(&request.port, node.kind, wait)?;
    let now = session.read(progress)?;
    let d = diff::diff(node.kind, &want, &now, request.mode);

    let mut lines = vec![tf("restore-onto", &[("name", node.name.as_str().into()), ("board", node.board().into())])];
    lines.push(t(match request.mode {
        Mode::Same => "restore-mode-same",
        Mode::Move => "restore-mode-move",
        Mode::Copy => "restore-mode-copy",
    }));
    if !manifest.node.firmware.is_empty() && manifest.node.firmware != node.firmware {
        lines.push(tf("restore-firmware", &[("backup", manifest.node.firmware.as_str().into()), ("node", node.firmware.as_str().into())]));
    }
    // onto another board: a clone must never run next to the one it copies; what this board
    // cannot take is left as it has it
    if request.mode == Mode::Move {
        lines.push(tf("restore-clone-warning", &[("name", manifest.node.name.as_str().into())]));
    }
    if request.mode != Mode::Same {
        lines.push(t("restore-other-note"));
    }
    lines.push(String::new());
    if d.steps.is_empty() {
        lines.push(t("restore-nothing"));
    } else {
        lines.push(tf("restore-changes", &[("count", d.steps.len().into())]));
        lines.extend(d.summary.iter().take(SHOWN).map(|l| format!("  · {l}")));
        if d.summary.len() > SHOWN {
            lines.push(tf("restore-more", &[("count", (d.summary.len() - SHOWN).into())]));
        }
    }
    // left as they are on purpose (the contacts gained since the backup)
    for note in &d.notes {
        lines.push(format!("  · {note}"));
    }
    // yours (custom names, roles): back in the address book, never on the node
    let book_lines = names::changes(book, &want);
    if !book_lines.is_empty() {
        lines.push(String::new());
        lines.push(tf("restore-book", &[("count", book_lines.len().into())]));
        lines.extend(book_lines.iter().take(SHOWN).map(|l| format!("  · {l}")));
        if book_lines.len() > SHOWN {
            lines.push(tf("restore-more", &[("count", (book_lines.len() - SHOWN).into())]));
        }
    }
    if !d.cannot.is_empty() {
        lines.push(String::new());
        lines.push(tf("restore-cannot", &[("what", d.cannot.join(", ").into())]));
    }
    if !d.steps.is_empty() {
        lines.push(String::new());
        lines.push(t("restore-how"));
    }
    // a restore that only puts names and roles back is still one to offer
    Ok(Preview { changes: d.steps.len() + book_lines.len(), moves: request.mode == Mode::Move, request, lines })
}

/// Does the restore (after the question was answered yes). Err = stopped before anything was
/// written (the backup or the node did not check out, the backup before failed): the node is as
/// it was. Ok = how it ended once writing started.
pub fn run(request: &Request, config: &Config, paths: &Paths, wait: Duration, progress: &mut dyn FnMut(&str, usize, usize)) -> Result<Outcome> {
    // the backup to put on (not needed, nor read, when only putting back is left)
    let backup = if request.putting_back { None } else { Some(store::load(&request.backup)?) };
    let node = node::identify(&request.port, wait);
    let kind = backup.as_ref().map_or(node.kind, |(manifest, _)| manifest.node.kind);
    if !node.kind.is_node() || node.kind != kind {
        bail!(t("restore-no-target"));
    }
    // this board's journal: a cut-short restore of it is finished or put back first, never
    // replaced (its way back, the node as it was before, would be lost)
    // (a cut-short one being finished keeps its own file, wherever the board is plugged now)
    let file = request
        .safety
        .as_deref()
        .and_then(|safety| journal::find(&paths.journals, safety))
        .unwrap_or_else(|| journal::file(&paths.journals, &node.port));
    if request.safety.is_none() && journal::load(&file).is_some() {
        bail!(t("restore-journal-open"));
    }
    // straight onto the node: only what differs is sent, read back and checked. No backup
    // before (it would pile up one at every restore): one that cannot finish says so, and
    // running it again sends what is left. (A cut-short restore of an older version, found
    // with its journal, still finishes or goes back below.)
    if request.safety.is_none() {
        let Some((_, want)) = &backup else { bail!(t("restore-no-target")) };
        let mut session = Session::open(&node.port, node.kind, wait)?;
        return Ok(match converge(&mut session, want, request.mode, wait, progress) {
            Ok(left) => Outcome::Restored(left),
            Err(why) => Outcome::Stopped(why),
        });
    }
    // the node as it is now: the way back (a cut-short restore already has it)
    let safety = match &request.safety {
        Some(folder) => folder.clone(),
        None => capture::capture(&node, config, &paths.backups, &paths.book, Reason::BeforeRestore, progress)?,
    };
    let (_, as_it_was) = store::load(&safety)?;
    let mut entry = Journal {
        backup: request.backup.clone(),
        safety: safety.clone(),
        mode: request.mode,
        serial_number: node.port.serial_number.clone(),
        port: node.port.path.clone(),
        kind: node.kind,
        name: node.name.clone(),
        started: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        putting_back: request.putting_back,
    };
    journal::save(&file, &entry)?;
    let mut session = match Session::open(&node.port, node.kind, wait) {
        Ok(session) => session,
        Err(e) => {
            // nothing was sent yet (a journal from before, being finished, stays)
            if request.safety.is_none() {
                journal::clear(&file);
            }
            return Err(e);
        }
    };
    let why = match &backup {
        Some((_, want)) => match converge(&mut session, want, request.mode, wait, progress) {
            Ok(left) => {
                journal::clear(&file);
                return Ok(Outcome::Restored(left));
            }
            Err(why) => why,
        },
        // finishing a cut-short "put back", or asked to: the only way left is back
        None => t("restore-was-putting-back"),
    };
    entry.putting_back = true;
    journal::save(&file, &entry)?;
    match converge(&mut session, &as_it_was, Mode::Same, wait, progress) {
        Ok(_) => {
            journal::clear(&file);
            Ok(Outcome::PutBack(why))
        }
        Err(still) => Ok(Outcome::Stuck(format!("{why} / {still}"))),
    }
}

/// A cut-short restore found at start, as the request that finishes it (`back`: put the node back
/// as it was instead). The board is found again by its USB serial number.
pub fn resume(entry: &Journal, back: bool) -> Request {
    // Bluetooth: path `ble:<address>`, dev the address (Link::open and unpairing use them)
    let dev = entry.port.strip_prefix(crate::ble::PREFIX).unwrap_or(&entry.port).to_string();
    let saved = Port { path: entry.port.clone(), dev, serial_number: entry.serial_number.clone(), ..empty_port() };
    // USB: found again on any port by its serial number; Bluetooth: at its own address
    let port = if saved.is_ble() { saved } else { serial::ports().into_iter().find(|p| p.same_board(&saved)).unwrap_or(saved) };
    Request {
        backup: entry.backup.clone(),
        port,
        mode: entry.mode,
        safety: Some(entry.safety.clone()),
        putting_back: back || entry.putting_back,
    }
}

fn empty_port() -> Port {
    Port { path: String::new(), dev: String::new(), product: String::new(), maker: String::new(), ids: None, serial_number: String::new() }
}

/// The backup came from this very board (a target chosen on the Backups page): then everything
/// goes back (`Mode::Same`); another board needs a choice (move or copy)
pub fn is_own(manifest: &super::Manifest, node: &Node) -> bool {
    manifest.node.is_board(&node.port.serial_number, &node.public_key)
}

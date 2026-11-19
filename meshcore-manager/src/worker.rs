//! The worker: it watches the USB ports, asks each new one who is there, and starts the jobs the
//! app sends (read a node, back up, write a commit, restore), each on its own thread, several
//! boards at once. A board is worked on by one job at a time (src/jobs.rs: claims); a job for a
//! board that is busy waits its turn, in the order it was asked. The scan never opens a port a
//! job holds. The screen never waits for a node.

use std::collections::{HashSet, VecDeque};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SendError, Sender};
use std::time::{Duration, Instant};

use crate::backup::diff::Mode;
use crate::backup::journal;
use crate::backup::restore::{self, Outcome, Request};
use crate::backup::compare::{self, Report};
use crate::backup::{capture, names, store, Reason};
use crate::book;
use crate::config::{self, Config};
use crate::debug;
use crate::ble;
use crate::i18n;
use crate::event::{Event, Job, Level};
use crate::folder_pick::{self, Picked};
use crate::host;
use crate::i18n::{t, tf};
use crate::jobs::{Claim, Claims, JobId};
use crate::node::edit::{self, Step};
use crate::node::{self, Answer, Kind, Node};
use crate::paths::{self, Paths};
use crate::regions::{self, Region};
use crate::serial::{self, Port};

/// While jobs run or wait: how often the worker looks at their news and at the queue
const TICK: Duration = Duration::from_millis(200);

/// The name of a job's thread starts with this: a panic there is reported by the job itself, in
/// the log, and the screen stays (app.rs keeps the terminal's panic handling for the others)
pub const JOB_THREAD: &str = "mcmanager-job-";

/// What the app asks the worker to do
pub enum Cmd {
    /// Ask this port again who is there; None = forget everything and ask every port again
    Rescan(Option<Port>),
    /// Read everything the node on this port can tell (its own view): contacts, channels, more
    Inspect(Port),
    /// The same reading, started by the worker itself for a Bluetooth companion just heard: it
    /// connects with the PIN it knows (saved, else Settings), and a refused PIN is only shown in
    /// its row (`p`), never asked in a box
    Probe(Port),
    Backup(Node),
    /// Write a commit of edit mode to the node (each step once), then read it again.
    /// `backup_first`: back it up before, and write nothing if that backup fails.
    Apply { node: Node, steps: Vec<Step>, backup_first: bool },
    /// Write the user's region list (regions.txt)
    SaveRegions(Vec<Region>),
    /// Read a backup and a node, and say what restoring it would do (nothing written)
    RestorePreview(Request),
    /// Restore (backup/restore.rs: verified, or put back as it was)
    Restore(Request),
    /// Delete these backup folders (the user confirmed it, twice for several)
    DeleteBackups(Vec<PathBuf>),
    /// What changed between two backups of one node (files only, nothing sent)
    CompareBackups { older: PathBuf, newer: PathBuf },
    /// What changed on the node on `port` since this backup (the node read, nothing written)
    CompareNode { backup: PathBuf, port: Port },
    /// Write one setting to config.toml (the app already uses it)
    SaveSetting { section: &'static str, key: &'static str, value: toml::Value },
    /// Test a folder for the backups before anything is asked (made when missing, a file written
    /// and read back, not inside the current one): `Event::FolderChecked`
    CheckFolder(String),
    /// The system's folder window (folder_pick.rs), on a thread of its own: the folder chosen is
    /// tested as `CheckFolder` is
    PickFolder,
    /// Backups go to this folder from now on ("" = backups in the data folder), and what becomes
    /// of the backups already saved (`Carry`)
    BackupsFolder { folder: String, carry: Carry },
    /// Write the original files of every built-in language into lang/ again (the ones someone
    /// changed kept as .bak), then use them
    RestoreLanguages,
    /// The pairing PIN of the Bluetooth companion at `address`, this session only (None: back to
    /// the one from Settings). Kept for the session (ble::give_pin); the PIN that connects is
    /// saved in bluetooth.toml (ble::save_pin).
    BlePin { address: String, pin: Option<String> },
    /// Unpair this computer from the Bluetooth companion on this port (it is then read again:
    /// not paired, `p` pairs it again)
    BleUnpair(Node),
    /// A change of yours to the address book (contacts.toml): a custom name for a key, its role, its favourite mark,
    /// a key out of it
    Book(book::Change),
}

/// What becomes of the backups already saved when the folder changes
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Carry {
    /// They stay where they are (only new backups go to the new folder)
    Leave,
    /// Copied there, each verified; the old ones stay
    Copy,
    /// Copied there, each verified; the old ones removed only once every copy checked out and
    /// the new folder is saved
    Move,
}

impl Cmd {
    /// For the debug log: what it is, never what it holds
    fn name(&self) -> &'static str {
        match self {
            Cmd::Rescan(_) => "rescan",
            Cmd::Inspect(_) => "inspect",
            Cmd::Probe(_) => "probe",
            Cmd::Backup(_) => "backup",
            Cmd::Apply { .. } => "apply",
            Cmd::SaveRegions(_) => "save-regions",
            Cmd::RestorePreview(_) => "restore-preview",
            Cmd::Restore(_) => "restore",
            Cmd::DeleteBackups(_) => "delete-backups",
            Cmd::CompareBackups { .. } => "compare-backups",
            Cmd::CompareNode { .. } => "compare-node",
            Cmd::SaveSetting { .. } => "save-setting",
            Cmd::CheckFolder(_) => "check-folder",
            Cmd::PickFolder => "pick-folder",
            Cmd::BackupsFolder { .. } => "backups-folder",
            Cmd::RestoreLanguages => "restore-languages",
            Cmd::BlePin { .. } => "ble-pin",
            Cmd::Book(_) => "book",
            Cmd::BleUnpair(_) => "ble-unpair",
        }
    }

    /// The same job again, for one that only reads (it can run again once the PIN is given):
    /// a reading, a backup, a restore's check. Writing jobs are asked again by the user.
    pub fn retry_copy(&self) -> Option<Cmd> {
        match self {
            Cmd::Inspect(port) => Some(Cmd::Inspect(port.clone())),
            Cmd::Backup(node) => Some(Cmd::Backup(node.clone())),
            Cmd::RestorePreview(request) => Some(Cmd::RestorePreview(request.clone())),
            _ => None,
        }
    }

    /// The board this works on: it runs as a job, on its own thread, once it holds the board.
    /// None: the worker does it itself, at once (files only).
    fn board(&self) -> Option<&Port> {
        match self {
            Cmd::Inspect(port) | Cmd::Probe(port) | Cmd::CompareNode { port, .. } => Some(port),
            Cmd::Backup(node) | Cmd::Apply { node, .. } | Cmd::BleUnpair(node) => Some(&node.port),
            Cmd::RestorePreview(request) | Cmd::Restore(request) => Some(&request.port),
            Cmd::Rescan(_)
            | Cmd::SaveRegions(_)
            | Cmd::DeleteBackups(_)
            | Cmd::CompareBackups { .. }
            | Cmd::SaveSetting { .. }
            | Cmd::CheckFolder(_)
            | Cmd::PickFolder
            | Cmd::BackupsFolder { .. }
            | Cmd::RestoreLanguages
            | Cmd::BlePin { .. }
            | Cmd::Book(_) => None,
        }
    }
}

/// What a job tells the worker (the screen it tells itself, through `events`)
enum Back {
    /// The node as it was just read (a reading, the read-back after a commit)
    Node(Node),
    /// The board may be another node now, or on another port (a restore): asked again
    Forget(Port),
}

/// What every job needs, cloned into its thread
#[derive(Clone)]
struct Shared {
    config: Config,
    paths: Paths,
    wait: Duration,
    events: Sender<Event>,
    back: Sender<Back>,
}

pub fn spawn(config: Config, paths: Paths, events: Sender<Event>) -> Sender<Cmd> {
    let (cmds, inbox) = mpsc::channel();
    std::thread::spawn(move || run(config, paths, events, inbox));
    cmds
}

fn run(config: Config, paths: Paths, events: Sender<Event>, inbox: Receiver<Cmd>) {
    let every = Duration::from_secs(config.scan.every_secs);
    let wait = Duration::from_millis(config.scan.answer_ms);
    let (back, news) = mpsc::channel();
    let mut shared = Shared { config, paths, wait, events: events.clone(), back };
    let claims = Claims::default();
    // the PIN each companion last connected with: tried first
    ble::use_pin_file(shared.paths.bluetooth.clone());
    // Bluetooth companions nearby, listened for in the background (`[bluetooth] enabled`)
    let listen = shared.config.bluetooth.enabled;
    let bluetooth = listen.then(ble::Watcher::start);
    if bluetooth.is_none() {
        crate::debug_log!("INFO", "bluetooth: off in config.toml ([bluetooth] enabled = false)");
    }
    let mut ble_problem: Option<String> = None;
    let mut queue: VecDeque<Cmd> = VecDeque::new();
    let mut next_id: JobId = 1;
    let mut nodes: Vec<Node> = Vec::new();
    let mut announce = true;
    let mut scanned: Option<Instant> = None;
    // the Bluetooth companions already read by themselves this session (by path)
    let mut probed: HashSet<String> = HashSet::new();

    // restores cut short (the program or the computer stopped): asked about first
    for entry in journal::load_all(&shared.paths.journals) {
        if events.send(Event::Interrupted(entry)).is_err() {
            return;
        }
    }
    loop {
        // what the jobs found
        let mut changed = false;
        while let Ok(back) = news.try_recv() {
            changed = true;
            match back {
                Back::Node(node) => {
                    hide(&node);
                    if let Some(known) = nodes.iter_mut().find(|n| n.port.path == node.port.path) {
                        *known = node;
                    }
                }
                Back::Forget(port) => {
                    for known in nodes.iter_mut().filter(|n| n.port.same_board(&port)) {
                        *known = Node::new(known.port.clone());
                        known.probing = true;
                    }
                }
            }
        }
        if changed && events.send(Event::Fleet(nodes.clone())).is_err() {
            return;
        }
        // jobs whose board is free now
        start_ready(&mut queue, &claims, &mut next_id, &shared);
        // the ports: at start, every `every`, and at once when asked
        if announce || scanned.is_none_or(|at| at.elapsed() >= every) {
            if scan(&mut nodes, wait, &events, announce, &claims, bluetooth.as_ref()).is_err() {
                return;
            }
            announce = false;
            scanned = Some(Instant::now());
            // Bluetooth companions heard and not read yet: connected and read by themselves, once
            // each (a companion that advertises is free: one held by a phone does not). The PIN
            // known for it is used (saved, else Settings); a refusal shows `p` in its row. Never
            // again after an unpairing or a failure: then it is the user's move.
            for node in nodes.iter().filter(|n| n.port.is_ble() && !n.inspected && !n.probing && !claims.busy(&n.port)) {
                if probed.insert(node.port.path.clone()) {
                    crate::debug_log!("INFO", "bluetooth: companion heard, read by itself");
                    queue.push_back(Cmd::Probe(node.port.clone()));
                }
            }
            start_ready(&mut queue, &claims, &mut next_id, &shared);
            // the backups folder deleted or unplugged meanwhile: said now, not at the next backup
            // (a running job writes where it started: only when none runs)
            if claims.count() == 0 && queue.is_empty() {
                backups_folder_check(&mut shared.paths, &events);
            }
            // Bluetooth that cannot listen (no adapter, switched off): said once in the log, and
            // shown on the Fleet page for as long as it lasts
            let problem = bluetooth.as_ref().and_then(ble::Watcher::problem);
            if problem != ble_problem {
                match &problem {
                    Some(why) => {
                        crate::debug_log!("WARN", "bluetooth: {why}");
                        // the Fleet page shows it: here for the Log page only
                        let _ = events.send(Event::LogOnly(Level::Info, tf("log-ble-problem", &[("why", why.as_str().into())])));
                    }
                    None => crate::debug_log!("INFO", "bluetooth: listening"),
                }
                let _ = events.send(Event::Bluetooth(problem.clone()));
                ble_problem = problem;
            }
        }
        // quick ticks while jobs run or wait (their news, their turn), else the scan's pace
        let tick = if claims.count() > 0 || !queue.is_empty() { TICK } else { every };
        let received = inbox.recv_timeout(tick);
        if let Ok(cmd) = &received {
            crate::debug_log!("INFO", "command: {} {}", cmd.name(), cmd.board().map(|p| serial::short(&p.dev)).unwrap_or_default());
        }
        match received {
            Ok(Cmd::Rescan(None)) => {
                // the boards a job holds are kept as they are: never asked while busy
                nodes.retain(|n| claims.busy(&n.port));
                announce = true;
            }
            Ok(Cmd::Rescan(Some(port))) if port.is_ble() => {
                // a Bluetooth companion is read as a job (connecting takes seconds: never on
                // this thread), with the PINs it knows; it waits if its board is busy
                if !queue.iter().any(|c| same(c, &port)) {
                    queue.push_back(Cmd::Inspect(port));
                    start_ready(&mut queue, &claims, &mut next_id, &shared);
                }
            }
            Ok(Cmd::Rescan(Some(port))) => {
                // marked as new: the next look asks it again (not while a job holds it)
                if !claims.busy(&port) {
                    if let Some(node) = nodes.iter_mut().find(|n| n.port == port) {
                        *node = Node::new(port);
                        node.probing = true;
                    }
                }
                announce = true;
            }
            Ok(Cmd::Book(change)) => match book::change(&shared.paths.book, change) {
                Ok(book) => {
                    let _ = events.send(Event::Book(Box::new(book)));
                }
                Err(e) => {
                    let _ = events.send(Event::Log(Level::Bad, format!("{e:#}")));
                }
            },
            Ok(Cmd::SaveRegions(list)) => match regions::save(&shared.paths.regions, &list) {
                Ok(()) => {
                    let _ = events.send(Event::Regions(list));
                }
                Err(e) => {
                    let _ = events.send(Event::Log(Level::Bad, format!("{e:#}")));
                }
            },
            Ok(Cmd::DeleteBackups(folders)) => delete_backups(&folders, &shared.paths, &events),
            Ok(Cmd::CompareBackups { older, newer }) => compare_backups(&older, &newer, &events),
            Ok(Cmd::SaveSetting { section, key, value }) => {
                if let Err(e) = config::set(&shared.paths.config, section, key, value) {
                    let lines = vec![t("settings-not-saved"), format!("{e:#}")];
                    let _ = events.send(Event::Log(Level::Bad, lines.join(" ")));
                    let _ = events.send(Event::Notice(t("tab-settings"), lines));
                }
            }
            Ok(Cmd::CheckFolder(folder)) => {
                let result = check_folder(&folder, &shared.paths).map_err(|e| format!("{e:#}"));
                let _ = events.send(Event::FolderChecked { folder, result });
            }
            Ok(Cmd::PickFolder) => pick_folder(&shared.paths, &events),
            Ok(Cmd::BackupsFolder { folder, carry }) => {
                // a running job writes where it started: the folder changes only when none runs
                if claims.count() > 0 || !queue.is_empty() {
                    let _ = events.send(Event::Notice(t("tab-settings"), vec![t("settings-folder-busy")]));
                } else {
                    move_backups_folder(&folder, carry, &mut shared.paths, &events);
                }
            }
            Ok(Cmd::RestoreLanguages) => restore_languages(&events),
            Ok(Cmd::BlePin { address, pin }) => {
                // never the PIN in the log: only that one was given
                crate::debug_log!("INFO", "bluetooth {address}: pin {}", if pin.is_some() { "given" } else { "back to settings" });
                ble::give_pin(&address, pin);
            }
            Ok(cmd) => {
                // a backup or a restore writes into the backups folder: still there?
                if claims.count() == 0 && queue.is_empty() {
                    backups_folder_check(&mut shared.paths, &events);
                }
                // a job: it waits when its board is busy, and says so
                if let Some(port) = cmd.board().filter(|p| claims.busy(p) || queue.iter().any(|c| same(c, p))) {
                    crate::debug_log!("INFO", "{} waits: {} is busy", cmd.name(), serial::short(&port.dev));
                    let line = tf("log-job-waiting", &[("port", serial::short(&port.dev).into())]);
                    let _ = events.send(Event::Log(Level::Info, line));
                }
                queue.push_back(cmd);
                start_ready(&mut queue, &claims, &mut next_id, &shared);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// This queued command is for that board
fn same(cmd: &Cmd, port: &Port) -> bool {
    cmd.board().is_some_and(|p| p.same_board(port))
}

/// Starts every queued job whose board is free, in the order they were asked (a job never
/// overtakes an earlier one for the same board)
fn start_ready(queue: &mut VecDeque<Cmd>, claims: &Claims, next_id: &mut JobId, shared: &Shared) {
    let mut waiting: VecDeque<Cmd> = VecDeque::new();
    while let Some(cmd) = queue.pop_front() {
        let Some(port) = cmd.board().cloned() else { continue };
        let behind = waiting.iter().any(|c| same(c, &port));
        let claim = if behind { None } else { claims.take(*next_id, &port) };
        match claim {
            Some(claim) => {
                start(*next_id, claim, port, cmd, shared.clone());
                *next_id += 1;
            }
            None => waiting.push_back(cmd),
        }
    }
    *queue = waiting;
}

/// Runs one job on its own thread. However it ends (done, failed, panicked), the board is let go
/// and the app is told (`JobEnd`): never a job left "running" on screen, never a board left held.
fn start(id: JobId, claim: Claim, port: Port, cmd: Cmd, shared: Shared) {
    let thread = std::thread::Builder::new().name(format!("{JOB_THREAD}{id}"));
    let events = shared.events.clone();
    // a Bluetooth job that may meet a refused PIN: kept to run again once the user gives it
    let retry = if port.is_ble() { cmd.retry_copy() } else { None };
    // started by the worker itself: a refused PIN is shown in the row, not asked in a box
    let quiet = matches!(cmd, Cmd::Probe(_));
    let spawned = thread.spawn(move || {
        let name = cmd.name();
        crate::debug_log!("INFO", "job {id} {name} on {} starts", serial::short(&port.dev));
        let started = Instant::now();
        let ended = panic::catch_unwind(AssertUnwindSafe(|| job(id, &port, cmd, &shared)));
        if ended.is_err() {
            // where it stopped is in the debug log (the panic hook, app.rs); the message itself is
            // not shown: it could hold a value read from the node
            crate::debug_log!("ERROR", "job {id} {name} stopped on a panic");
            let line = tf("log-job-crashed", &[("port", serial::short(&port.dev).into())]);
            let _ = shared.events.send(Event::Log(Level::Bad, line));
        }
        crate::debug_log!("INFO", "job {id} {name} ends after {:.1} s", started.elapsed().as_secs_f32());
        // the board first, then the news: when the app hears the end, the board is free
        drop(claim);
        let _ = shared.events.send(Event::JobEnd(id));
        // the companion refused the PIN: the user is asked for it (Fleet), then it runs again
        let refused = port.path.strip_prefix(ble::PREFIX).is_some_and(ble::take_refused);
        if refused && quiet {
            crate::debug_log!("INFO", "job {id} {name}: pin refused, p offered in its row");
        } else if refused {
            crate::debug_log!("INFO", "job {id} {name}: pin refused, asking the user");
            let _ = shared.events.send(Event::PinNeeded { port: port.clone(), retry });
        }
    });
    // no thread could be made (the system is out of them): said, and nothing was done; the claim
    // went with the closure that was not run
    if spawned.is_err() {
        let _ = events.send(Event::Log(Level::Bad, t("log-job-not-started")));
        let _ = events.send(Event::JobEnd(id));
    }
}

/// One job, on its thread
fn job(id: JobId, port: &Port, cmd: Cmd, shared: &Shared) {
    // each job is on screen the moment it starts: "connecting…" until its first step (opening
    // the link, pairing over Bluetooth, the node's hello take a few seconds)
    // started by the worker itself: its Fleet row shows it, not the bottom line of every page
    let background = matches!(cmd, Cmd::Probe(_));
    let progress = |title: &str| {
        let title = title.to_string();
        let events = shared.events.clone();
        let port = port.clone();
        let first = Job { id, port: port.clone(), title: title.clone(), step: t("job-connecting"), done: 0, total: 0, background };
        let _ = events.send(Event::Job(first));
        move |step: &str, done: usize, total: usize| {
            let job = Job { id, port: port.clone(), title: title.clone(), step: step.to_string(), done, total, background };
            let _ = events.send(Event::Job(job));
        }
    };
    match cmd {
        Cmd::Inspect(port) | Cmd::Probe(port) => {
            let _report = progress(&tf("job-inspect", &[("port", serial::short(&port.dev).into())]));
            inspect(&port, shared, background);
        }
        Cmd::Backup(node) => {
            let mut report = progress(&tf("job-backup", &[("name", node.name.as_str().into())]));
            backup(&node, Reason::Manual, shared, &mut report);
        }
        Cmd::CompareNode { backup, port } => {
            let mut report = progress(&tf("job-compare-node", &[("port", serial::short(&port.dev).into())]));
            compare_node(&backup, &port, shared, &mut report);
        }
        // the node stays in the list as it was found (its name, its signal), not read: `p`
        // pairs it again
        Cmd::BleUnpair(node) => {
            let _report = progress(&tf("job-unpair", &[("name", node.name.as_str().into())]));
            let forgot = ble::forget(&node.port.dev);
            let (level, line) = match forgot {
                Ok(()) => {
                    // its saved PIN goes too: the next pairing starts from Settings
                    ble::forget_pin(&node.port.dev);
                    (Level::Good, tf("log-unpaired", &[("name", node.name.as_str().into())]))
                }
                Err(e) => (Level::Bad, tf("log-unpair-failed", &[("name", node.name.as_str().into()), ("error", format!("{e:#}").into())])),
            };
            let _ = shared.events.send(Event::Log(level, line));
            let mut left = Node::new(node.port.clone());
            left.kind = Kind::Companion;
            left.name = node.name.clone();
            left.signal = node.signal;
            let _ = shared.back.send(Back::Node(left));
        }
        Cmd::Apply { node, steps, backup_first } => {
            let mut report = progress(&tf("job-backup", &[("name", node.name.as_str().into())]));
            if !backup_first || backup(&node, Reason::BeforeConfig, shared, &mut report) {
                let mut report = progress(&tf("job-apply", &[("name", node.name.as_str().into())]));
                apply(&node, &steps, shared, &mut report);
            } else {
                let line = tf("log-edit-no-backup", &[("name", node.name.as_str().into())]);
                let _ = shared.events.send(Event::Log(Level::Bad, line));
            }
            // read back: the view shows what the node holds now, not what was asked
            let _report = progress(&tf("job-inspect", &[("port", serial::short(&node.port.dev).into())]));
            inspect(&node.port, shared, false);
        }
        Cmd::RestorePreview(request) => {
            let mut report = progress(&tf("job-restore-check", &[("port", serial::short(&request.port.dev).into())]));
            restore_preview(request, shared, &mut report);
        }
        Cmd::Restore(request) => {
            let mut report = progress(&tf("job-restore", &[("port", serial::short(&request.port.dev).into())]));
            restore_run(&request, shared, &mut report);
            if request.port.is_ble() {
                // Bluetooth: the same address; read again at once, so its view (and its
                // backups, matched by its key) are there again, not "unknown"
                let _report = progress(&tf("job-inspect", &[("port", serial::short(&request.port.dev).into())]));
                inspect(&request.port, shared, false);
            } else {
                // USB: the node may be on another port, with another identity: asked again
                let _ = shared.back.send(Back::Forget(request.port.clone()));
            }
        }
        // not jobs: done by the worker itself (`run`)
        _ => {}
    }
}

/// Looks at the ports: a port that left is dropped, a new one is asked who it is. A port a job
/// holds is never asked (it shows as it was, or as "being identified" when it came back on
/// another port). The list is sent each time it changes.
fn scan(
    nodes: &mut Vec<Node>,
    wait: Duration,
    events: &Sender<Event>,
    announce: bool,
    claims: &Claims,
    bluetooth: Option<&ble::Watcher>,
) -> Result<(), SendError<Event>> {
    let mut ports = serial::ports();
    // the Bluetooth companions heard (ble.rs: nothing connects to them here)
    let heard = bluetooth.map(ble::Watcher::seen).unwrap_or_default();
    ports.extend(heard.iter().map(Port::ble));
    // one a job is connected to does not advertise meanwhile: kept while it is busy
    for n in nodes.iter().filter(|n| n.port.is_ble() && claims.busy(&n.port)) {
        if !ports.iter().any(|p| p.path == n.port.path) {
            ports.push(n.port.clone());
        }
    }
    let signal = |port: &Port| heard.iter().find(|s| port.path.ends_with(&s.address)).and_then(|s| s.rssi);
    let mut next: Vec<Node> = ports
        .into_iter()
        .map(|port| {
            // a Bluetooth one by its address (its advertised name can come later)
            let same = |n: &&Node| if port.is_ble() { n.port.path == port.path } else { n.port == port };
            match nodes.iter().find(same) {
                Some(known) => {
                    let mut known = known.clone();
                    known.signal = signal(&known.port).or(known.signal);
                    if known.port.product.is_empty() {
                        known.port.product = port.product.clone();
                    }
                    // a Bluetooth name heard after the companion was listed (it comes in the scan
                    // response): shown until the companion itself is read
                    if known.port.is_ble() && known.name.is_empty() && !port.product.is_empty() {
                        known.name = port.product.clone();
                        hide(&known);
                    }
                    known
                }
                // heard over Bluetooth: listed as the companion it advertises; the worker then
                // reads it by itself once (`Cmd::Probe`, with the PIN it knows)
                None if port.is_ble() => {
                    let mut new = Node::new(port);
                    new.kind = Kind::Companion;
                    new.name = new.port.product.clone();
                    new.signal = signal(&new.port);
                    new.note = t("node-ble-not-read");
                    // its address and name never reach the debug log, from the start
                    hide(&new);
                    crate::debug_log!("INFO", "bluetooth: companion heard {}", new.port.dev);
                    new
                }
                None => {
                    let mut new = Node::new(port);
                    new.probing = true;
                    new
                }
            }
        })
        .collect();
    let to_ask = |n: &Node| n.probing && !claims.busy(&n.port);
    let gone = next.len() != nodes.len()
        || next.iter().zip(nodes.iter()).any(|(a, b)| a.port != b.port || a.signal != b.signal || a.name != b.name);
    if announce || gone || next.iter().any(to_ask) {
        events.send(Event::Fleet(next.clone()))?;
    }
    for i in 0..next.len() {
        if !to_ask(&next[i]) {
            continue;
        }
        next[i] = node::identify(&next[i].port, wait);
        let found = &next[i];
        hide(found);
        crate::debug_log!("INFO", "{}: {:?}, firmware {}", serial::short(&found.port.dev), found.kind, found.firmware);
        let line = tf(
            "log-found",
            &[
                ("port", serial::short(&found.port.dev).into()),
                ("kind", found.kind.label().into()),
                ("name", found.name.as_str().into()),
            ],
        );
        events.send(Event::Log(Level::Info, line))?;
        events.send(Event::Fleet(next.clone()))?;
    }
    *nodes = next;
    Ok(())
}

/// What a node tells that must never be in the debug log: its name, keys, serial number, its
/// contacts' and channels' names (debug.rs replaces them from now on)
fn hide(node: &Node) {
    debug::private(&node.name, debug::NODE);
    debug::private(&node.public_key, debug::KEY);
    debug::private(&node.port.serial_number, debug::SERIAL);
    // a Bluetooth address is the device's own, like a serial number
    if node.port.is_ble() {
        debug::private(&node.port.dev, debug::SERIAL);
        debug::private(&node.port.product, debug::NODE);
    }
    for contact in &node.contacts {
        debug::private(&contact.name, debug::NAME);
        debug::private(&contact.public_key, debug::KEY);
    }
    for channel in &node.channels {
        debug::private(&channel.name, debug::NAME);
    }
}

/// Reads the node on `port` in depth and hands it to the worker's list (kept until it is asked
/// again or unplugged). `background`: started by the worker itself, its news goes to the Log page
/// only (the Fleet row says it).
fn inspect(port: &Port, shared: &Shared, background: bool) {
    let node = node::inspect(port, shared.wait);
    let line = if node.inspected && node.kind == Kind::Companion {
        let line = tf(
            "log-inspect-companion",
            &[
                ("name", node.name.as_str().into()),
                ("contacts", node.contacts.len().into()),
                ("channels", node.channels.len().into()),
            ],
        );
        (Level::Good, line)
    } else if node.inspected {
        (Level::Good, tf("log-inspect-console", &[("name", node.name.as_str().into()), ("settings", node.facts.len().into())]))
    } else {
        (Level::Bad, tf("log-inspect-failed", &[("name", node.name.as_str().into()), ("error", node.note.as_str().into())]))
    };
    let _ = shared.events.send(if background { Event::LogOnly(line.0, line.1) } else { Event::Log(line.0, line.1) });
    remember(&node, shared);
    let _ = shared.back.send(Back::Node(node));
}

/// What a node just read has for the address book: a companion's contacts, a node's access
/// list. A book that cannot be read is said in the log and left as it is.
fn remember(node: &Node, shared: &Shared) {
    if !node.inspected || node.public_key.is_empty() {
        return;
    }
    let companion = node.kind == Kind::Companion;
    if !companion && node.acl.is_none() {
        return;
    }
    let result = book::update(&shared.paths.book, |book| {
        if companion {
            book.add_contacts(&node.public_key, &node.name, &node.contacts);
        }
        if let Some(entries) = &node.acl {
            book.add_access(&node.public_key, &node.name, entries);
        }
    });
    match result {
        Ok(book) => {
            crate::debug_log!("INFO", "address book: {} keys", book.people.len());
            let _ = shared.events.send(Event::Book(Box::new(book)));
        }
        Err(e) => {
            let _ = shared.events.send(Event::Log(Level::Bad, format!("{e:#}")));
        }
    }
}

/// Sends a commit and logs what the node answered to each step
fn apply(node: &Node, steps: &[Step], shared: &Shared, progress: &mut dyn FnMut(&str, usize, usize)) {
    let log = |level: Level, text: String| {
        let _ = shared.events.send(Event::Log(level, text));
    };
    match edit::apply(node, steps, shared.wait, progress) {
        Ok(answers) => {
            let done = answers.iter().filter(|a| matches!(a, Answer::Done)).count();
            for (step, answer) in steps.iter().zip(&answers) {
                match answer {
                    Answer::Done => log(Level::Good, tf("log-edit-done", &[("what", step.line.as_str().into())])),
                    Answer::Refused(why) => log(
                        Level::Bad,
                        tf("log-edit-refused", &[("what", step.line.as_str().into()), ("why", why.as_str().into())]),
                    ),
                    Answer::Silent => log(Level::Bad, tf("log-edit-silent", &[("what", step.line.as_str().into())])),
                }
            }
            let not_sent = steps.len() - answers.len();
            if not_sent > 0 {
                log(Level::Bad, tf("log-edit-not-sent", &[("count", not_sent.into())]));
            }
            let level = if done == steps.len() { Level::Good } else { Level::Bad };
            log(level, tf("log-edit-summary", &[("name", node.name.as_str().into()), ("done", done.into()), ("total", steps.len().into())]));
        }
        Err(e) => log(Level::Bad, tf("log-edit-failed", &[("name", node.name.as_str().into()), ("error", format!("{e:#}").into())])),
    }
}

/// Reads the backup and the node, and sends the question (or why it cannot be done)
fn restore_preview(request: Request, shared: &Shared, progress: &mut dyn FnMut(&str, usize, usize)) {
    // the address book as it is now: what the backup's custom names and roles would change
    let book = book::load(&shared.paths.book).unwrap_or_default();
    let event = match restore::preview(request, &book, shared.wait, progress) {
        Ok(preview) => Event::RestorePreview(Box::new(preview)),
        Err(e) => Event::Notice(t("restore-title"), vec![t("restore-not-started"), format!("{e:#}")]),
    };
    let _ = shared.events.send(event);
}

/// Restores, then says how it ended (in the log and in the middle of the screen)
fn restore_run(request: &Request, shared: &Shared, progress: &mut dyn FnMut(&str, usize, usize)) {
    let outcome = restore::run(request, &shared.config, &shared.paths, shared.wait, progress);
    // what the backup keeps of yours (custom names, roles) back in the address book, once the
    // node took the backup (whole, or as far as it went); not when it was put back as it was
    let book_lines = match &outcome {
        Ok(Outcome::Restored(_) | Outcome::Stopped(_)) if !request.putting_back => book_back(request, shared),
        _ => Vec::new(),
    };
    let (level, mut lines) = match outcome {
        Ok(Outcome::Restored(left)) => {
            let mut lines = vec![t("restore-done")];
            if request.mode == Mode::Move {
                lines.push(t("restore-done-move"));
            }
            // another board: what it would not take, left as it has it
            if !left.is_empty() {
                lines.push(tf("restore-left", &[("count", left.len().into())]));
                lines.extend(left.into_iter().map(|l| format!("  · {l}")));
            }
            (Level::Good, lines)
        }
        Ok(Outcome::PutBack(why)) => (Level::Bad, vec![t("restore-put-back"), why]),
        Ok(Outcome::Stuck(why)) => (Level::Bad, vec![t("restore-stuck"), why, t("restore-stuck-how")]),
        Ok(Outcome::Stopped(why)) => (Level::Bad, vec![t("restore-stopped"), why, t("restore-stopped-how")]),
        Err(e) => (Level::Bad, vec![t("restore-not-started"), format!("{e:#}")]),
    };
    lines.extend(book_lines);
    let _ = shared.events.send(Event::Log(level, lines.join(" ")));
    let _ = shared.events.send(Event::Notice(t("restore-title"), lines));
}

/// The backup's custom names and roles put back in the address book; the line that says how many
/// (none: nothing), and the book sent to the screen
fn book_back(request: &Request, shared: &Shared) -> Vec<String> {
    let put = store::load(&request.backup).and_then(|(_, state)| names::put_back(&shared.paths.book, &state));
    match put {
        Ok(0) => Vec::new(),
        Ok(count) => {
            if let Ok(book) = book::load(&shared.paths.book) {
                let _ = shared.events.send(Event::Book(Box::new(book)));
            }
            vec![tf("restore-book-done", &[("count", count.into())])]
        }
        Err(e) => vec![format!("{e:#}")],
    }
}

/// Deletes the backup folders the user confirmed, one by one, and logs each. Never one outside the
/// backups folder, nor one a cut-short restore still needs (its way back). A node's folder left
/// empty goes too (it held nothing else).
fn delete_backups(folders: &[PathBuf], paths: &Paths, events: &Sender<Event>) {
    let log = |level: Level, text: String| {
        let _ = events.send(Event::Log(level, text));
    };
    let root = std::fs::canonicalize(&paths.backups).ok();
    // every backup a restore that has not ended still needs (running or cut short)
    let needed: Vec<PathBuf> = journal::load_all(&paths.journals)
        .into_iter()
        .flat_map(|j| [j.backup, j.safety])
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .collect();
    for folder in folders {
        let shown = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let real = std::fs::canonicalize(folder).ok();
        let inside = matches!((&real, &root), (Some(real), Some(root)) if real.starts_with(root) && real != root);
        if !inside {
            log(Level::Bad, tf("log-delete-refused", &[("stamp", shown.into())]));
            continue;
        }
        if real.as_ref().is_some_and(|r| needed.contains(r)) {
            log(Level::Bad, tf("log-delete-needed", &[("stamp", shown.into())]));
            continue;
        }
        match std::fs::remove_dir_all(folder) {
            Ok(()) => {
                log(Level::Good, tf("log-delete-done", &[("stamp", shown.into())]));
                // the node's folder (never the backups folder): only goes when it is empty
                if let Some(parent) = folder.parent().filter(|p| std::fs::canonicalize(p).ok() != root) {
                    let _ = std::fs::remove_dir(parent);
                }
            }
            Err(e) => log(Level::Bad, tf("log-delete-failed", &[("stamp", shown.into()), ("error", e.to_string().into())])),
        }
    }
    // the app reads the backups again
    let _ = events.send(Event::BackupsChanged);
}

/// What `CheckFolder` found: the folder can take backups, and what there is to bring over
pub struct FolderCheck {
    /// The folder as it will be used (the default one spelled out)
    pub path: PathBuf,
    /// It is the folder in use already
    pub same: bool,
    /// The finished backups in the folder in use now, and their size in bytes
    pub backups: usize,
    pub bytes: u64,
}

/// The folder a setting names ("" = backups in the data folder; quotes and `~` taken off)
fn folder_path(folder: &str, paths: &Paths) -> PathBuf {
    config::Backup { folder: folder.to_string(), ..Default::default() }.folder_in(&paths.root)
}

/// Tests a folder for the backups before anything is asked or done: it can be made, takes a file
/// (written, synced, read back, removed), and is not inside the folder in use (or the other way
/// round: a copy would copy into itself). Folders made only for the test are removed again (while
/// empty): it is made for good only when it is used. Counts what there is to bring over.
fn check_folder(folder: &str, paths: &Paths) -> anyhow::Result<FolderCheck> {
    let new = folder_path(folder, paths);
    let made = missing_folders(&new);
    let same = test_folder(&new, paths);
    remove_made(&made);
    let same = same?;
    let (mut backups, mut bytes) = (0, 0);
    for backup in finished_backups(&paths.backups) {
        backups += 1;
        bytes += std::fs::read_dir(&backup)
            .map(|d| d.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum::<u64>())
            .unwrap_or(0);
    }
    Ok(FolderCheck { path: new, same, backups, bytes })
}

/// `check_folder`'s test: the folder takes a file and is not inside the one in use, nor holds it.
/// Whether it is the folder in use.
fn test_folder(new: &Path, paths: &Paths) -> anyhow::Result<bool> {
    paths::writable(new)?;
    debug::private(&new.to_string_lossy(), debug::FOLDER_TAG);
    let (a, b) = (std::fs::canonicalize(&paths.backups).ok(), std::fs::canonicalize(new).ok());
    if let (Some(a), Some(b)) = (&a, &b) {
        if a != b && (a.starts_with(b) || b.starts_with(a)) {
            anyhow::bail!(t("settings-folder-nested"));
        }
    }
    Ok(a.is_some() && a == b)
}

/// `folder` and the folders above it that are not there yet, deepest first: what making it makes
fn missing_folders(folder: &Path) -> Vec<PathBuf> {
    folder
        .ancestors()
        .take_while(|a| !a.as_os_str().is_empty() && matches!(a.try_exists(), Ok(false)))
        .map(Path::to_path_buf)
        .collect()
}

/// Those folders removed again, deepest first, each only while empty (remove_dir refuses
/// anything else: nothing of anyone's is ever removed here)
fn remove_made(folders: &[PathBuf]) {
    for folder in folders {
        let _ = std::fs::remove_dir(folder);
    }
}

/// Opens the system's folder window on a thread of its own (it waits for the person, the worker
/// does not): the folder chosen is tested and asked about as a typed one is. On a console-only
/// machine it says so: the path is typed or pasted there.
fn pick_folder(paths: &Paths, events: &Sender<Event>) {
    let (paths, events) = (paths.clone(), events.clone());
    std::thread::spawn(move || match folder_pick::pick(&paths.backups, &t("settings-folder-window")) {
        // a path that is not valid text (Linux allows it) cannot be saved in config.toml as it is:
        // refused by name, never changed into another path
        Picked::Folder(folder) => match host::path_text(&folder) {
            Some(folder) => {
                let result = check_folder(&folder, &paths).map_err(|e| format!("{e:#}"));
                let _ = events.send(Event::FolderChecked { folder, result });
            }
            None => {
                let _ = events.send(Event::Notice(t("tab-settings"), vec![t("settings-folder-not-text")]));
            }
        },
        Picked::NoWindow(why) => {
            crate::debug_log!("INFO", "no folder window: {why}");
            let _ = events.send(Event::Notice(t("tab-settings"), vec![why, t("settings-folder-type-instead")]));
        }
        Picked::Cancelled | Picked::Busy => {}
    });
}

/// The finished backups under a backups folder (`<node>/<stamp>/` with a manifest, not `.partial`)
fn finished_backups(root: &Path) -> Vec<PathBuf> {
    let folders = |p: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(p).map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default()
    };
    folders(root)
        .iter()
        .flat_map(|node| folders(node))
        .filter(|b| !b.to_string_lossy().ends_with(".partial") && b.join("manifest.json").is_file())
        .collect()
}

/// The chosen backups folder gone while the program runs (`paths::missing`): backups go to the
/// default folder, the user is told (in red, and a popup). Deleted: the setting goes back to the
/// default too. Its drive not there: the setting is kept (it may be plugged in again). Never made
/// again empty: that would hide where the backups went. Checked at each scan while no job runs,
/// and before a job starts.
fn backups_folder_check(paths: &mut Paths, events: &Sender<Event>) {
    let fallback = paths.root.join("backups");
    if paths.backups == fallback {
        return;
    }
    let missing = paths::missing(&paths.backups);
    if missing == paths::Missing::No {
        return;
    }
    let (shown, default) = (paths.backups.display().to_string(), fallback.display().to_string());
    if let Err(e) = paths.use_backups(fallback.clone()) {
        let _ = events.send(Event::Log(Level::Bad, format!("{e:#}")));
        return;
    }
    let mut lines = if missing == paths::Missing::Deleted {
        vec![
            tf("backups-folder-gone", &[("folder", shown.into())]),
            tf("backups-folder-gone-now", &[("default", default.into())]),
        ]
    } else {
        vec![
            tf("backups-folder-away", &[("folder", shown.into())]),
            tf("backups-folder-away-now", &[("default", default.into())]),
        ]
    };
    if missing == paths::Missing::Deleted {
        if let Err(e) = config::set(&paths.config, "backup", "folder", toml::Value::String(String::new())) {
            lines.push(t("settings-not-saved"));
            lines.push(format!("{e:#}"));
        }
    }
    let _ = events.send(Event::Log(Level::Bad, lines.join(" ")));
    let _ = events.send(Event::BackupsFolder(fallback));
    let _ = events.send(Event::Notice(t("tab-settings"), lines));
}

/// Backups go to `folder` from now on ("" = backups in the data folder). All or nothing, in
/// this order, and the first step that fails ends it with everything as it was (the setting,
/// the folder in use, every backup where it is; what this change made in the new folder
/// removed again):
/// 1. the folder tested again (`check_folder`: things may have changed since the question);
/// 2. `carry` (copy or move): everything copied there and each copy checked (`copy_backups`),
///    nothing removed yet;
/// 3. the folder tested once more, the setting saved (config.toml in one step);
/// 4. only then the folder in use changes, and a move removes the originals (`remove_originals`):
///    their copies are checked and the new folder is saved, so nothing can be lost there.
fn move_backups_folder(folder: &str, carry: Carry, paths: &mut Paths, events: &Sender<Event>) {
    let old = paths.backups.clone();
    let kept = tf("settings-folder-kept", &[("folder", old.display().to_string().into())]);
    let refused = |mut lines: Vec<String>| {
        lines.push(kept.clone());
        let _ = events.send(Event::Log(Level::Bad, lines.join(" ")));
        let _ = events.send(Event::Notice(t("tab-settings"), lines));
    };
    // the folders this change makes (the new folder, and above it): removed again if it stops
    let made = missing_folders(&folder_path(folder, paths));
    let check = match check_folder(folder, paths) {
        Ok(check) => check,
        Err(e) => return refused(vec![t("settings-folder-unusable"), format!("{e:#}")]),
    };
    let new = check.path;
    let copied = if carry != Carry::Leave && !check.same {
        match copy_backups(&old, &new) {
            Ok(copied) => Some(copied),
            Err(e) => {
                remove_made(&made);
                return refused(vec![t("settings-folder-not-carried"), format!("{e:#}")]);
            }
        }
    } else {
        None
    };
    let undo = || {
        if let Some(copied) = &copied {
            copied.undo();
        }
        remove_made(&made);
    };
    if let Err(e) = paths::writable(&new) {
        undo();
        return refused(vec![t("settings-folder-unusable"), format!("{e:#}")]);
    }
    // saved as the folder in full ("" for the default one): what was typed may have had quotes
    // or a `~` around it
    let saved = if new == paths.root.join("backups") { String::new() } else { new.display().to_string() };
    if let Err(e) = config::set(&paths.config, "backup", "folder", toml::Value::String(saved)) {
        undo();
        return refused(vec![t("settings-folder-not-saved"), format!("{e:#}")]);
    }
    paths.backups = new.clone();
    let mut lines = vec![tf("settings-folder-now", &[("folder", new.display().to_string().into())])];
    if let Some(copied) = copied {
        let moving = carry == Carry::Move;
        let left = if moving { remove_originals(&old, &copied.originals, &paths.journals) } else { 0 };
        let id = if moving { "settings-folder-moved" } else { "settings-folder-copied" };
        lines.push(tf(id, &[("done", copied.backups.into()), ("there", copied.there.into())]));
        if copied.files > 0 {
            let id = if moving { "settings-folder-files-moved" } else { "settings-folder-files-copied" };
            lines.push(tf(id, &[("count", copied.files.into())]));
        }
        if left > 0 {
            lines.push(tf("settings-folder-left", &[("count", left.into()), ("folder", old.display().to_string().into())]));
        }
    }
    let _ = events.send(Event::Log(Level::Good, lines.join(" ")));
    let _ = events.send(Event::BackupsFolder(new));
    let _ = events.send(Event::Notice(t("tab-settings"), lines));
}

/// The empty folders under `root` removed, deepest first (`root` itself stays). A folder that
/// holds anything is never touched (remove_dir refuses it).
fn remove_empty_folders(root: &Path) {
    let mut folders = folders_under(root);
    folders.sort_by_key(|f| std::cmp::Reverse(f.components().count()));
    for folder in folders {
        let _ = std::fs::remove_dir(&folder);
    }
}

/// What `copy_backups` brought to the new folder: every copy checked, nothing removed
#[derive(Default)]
struct Copied {
    /// Finished backups copied (each checked against its manifest)
    backups: usize,
    /// Every other file (an unreadable or older backup, a leftover, a file of the user's) copied,
    /// each compared with its original byte for byte
    files: usize,
    /// Backups and files already in the new folder, the same as their original
    there: usize,
    /// The originals (backup folders and files) whose copy is checked: what a move removes
    originals: Vec<PathBuf>,
    /// The backups and files this copy made in the new folder
    made: Vec<PathBuf>,
    /// The folders it made there, shallowest first
    folders: Vec<PathBuf>,
}

impl Copied {
    /// What this copy made in the new folder removed again, and only that: what was there
    /// before stays, and the originals were never touched
    fn undo(&self) {
        for path in &self.made {
            let _ = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
        }
        for folder in self.folders.iter().rev() {
            let _ = std::fs::remove_dir(folder);
        }
    }
}

/// Copies everything in the backups folder `from` to `to`, and checks each copy; nothing in
/// `from` is changed. All or nothing: at the first one that cannot be brought over, what was
/// copied is removed again and the error says which one and why.
/// - Each finished backup is copied to `<stamp>.partial`, read back and checked against its
///   manifest (every file's sha256: store::load), and only then gets its name.
/// - Every other file (backups without a readable manifest, leftovers, the user's own files), at
///   the same place under `to`, each read back and compared with its original.
///
/// Nothing in `to` is ever written over: a backup or file already there counts when it is the
/// same as its original; a different one stops the copy.
fn copy_backups(from: &Path, to: &Path) -> anyhow::Result<Copied> {
    let mut folders: Vec<PathBuf> =
        folders_under(from).iter().filter_map(|f| f.strip_prefix(from).ok().map(|r| to.join(r))).filter(|f| !f.exists()).collect();
    folders.sort_by_key(|f| f.components().count());
    let mut copied = Copied { folders, ..Default::default() };
    let result = copy_all(from, to, &mut copied);
    if result.is_err() {
        copied.undo();
    }
    result.map(|()| copied)
}

/// `copy_backups`' work: stops at the first failure (which it names)
fn copy_all(from: &Path, to: &Path, copied: &mut Copied) -> anyhow::Result<()> {
    let failed = |relative: &Path, e: anyhow::Error| e.context(tf("log-copy-failed", &[("what", relative.display().to_string().into())]));
    let backups = finished_backups(from);
    for backup in &backups {
        let relative = backup.strip_prefix(from)?;
        let target = to.join(relative);
        match copy_backup(backup, &target) {
            Ok(true) => {
                copied.backups += 1;
                copied.made.push(target);
            }
            Ok(false) => copied.there += 1,
            Err(e) => return Err(failed(relative, e)),
        }
        copied.originals.push(backup.clone());
    }
    // everything else, file by file (what is inside a finished backup was copied above)
    for file in files_under(from).into_iter().filter(|f| !backups.iter().any(|b| f.starts_with(b))) {
        let relative = file.strip_prefix(from)?;
        let target = to.join(relative);
        match copy_other(&file, &target) {
            Ok(true) => {
                copied.files += 1;
                copied.made.push(target);
            }
            Ok(false) => copied.there += 1,
            Err(e) => return Err(failed(relative, e)),
        }
        copied.originals.push(file);
    }
    Ok(())
}

/// One finished backup copied and checked: true = copied, false = the same one is already there
fn copy_backup(backup: &Path, target: &Path) -> anyhow::Result<bool> {
    let exists = |path: &Path| anyhow::anyhow!(tf("log-copy-exists", &[("file", path.display().to_string().into())]));
    if target.exists() {
        return if same_tree(backup, target)? { Ok(false) } else { Err(exists(target)) };
    }
    // a `.partial` there is not ours: never written into, never removed
    let partial = PathBuf::from(format!("{}.partial", target.display()));
    if partial.exists() {
        return Err(exists(&partial));
    }
    let result = copy_tree(backup, &partial)
        .and_then(|()| store::load(&partial).map(|_| ()))
        .and_then(|()| std::fs::rename(&partial, target).map_err(anyhow::Error::from));
    if result.is_err() {
        // our own unfinished copy goes (the original is untouched)
        let _ = std::fs::remove_dir_all(&partial);
    }
    result.map(|()| true)
}

/// One other file copied and compared: true = copied, false = the same file is already there
fn copy_other(file: &Path, target: &Path) -> anyhow::Result<bool> {
    if target.exists() {
        return match same_file(file, target)? {
            true => Ok(false),
            false => Err(anyhow::anyhow!(tf("log-copy-exists", &[("file", target.display().to_string().into())]))),
        };
    }
    let result = copy_file(file, target).and_then(|()| match same_file(file, target)? {
        true => Ok(()),
        false => Err(anyhow::anyhow!(tf("log-copy-differs", &[("file", target.display().to_string().into())]))),
    });
    if result.is_err() {
        // our own copy, whole or not (it was not there before)
        let _ = std::fs::remove_file(target);
    }
    result.map(|()| true)
}

/// A move, once every copy is checked and the new folder saved: the originals go, never one a
/// restore not yet finished still names (its journal points at it); then the folders left empty
/// (the old folder itself stays). One that cannot be removed simply stays: its copy is there.
/// Gives how many files are still in the old folder.
fn remove_originals(from: &Path, originals: &[PathBuf], journals: &Path) -> usize {
    let needed: Vec<PathBuf> = journal::load_all(journals)
        .into_iter()
        .flat_map(|j| [j.backup, j.safety])
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .collect();
    for original in originals {
        if std::fs::canonicalize(original).ok().is_some_and(|p| needed.contains(&p)) {
            continue;
        }
        let _ = if original.is_dir() { std::fs::remove_dir_all(original) } else { std::fs::remove_file(original) };
    }
    remove_empty_folders(from);
    files_under(from).len()
}

/// Every plain file under `root`, at any depth (links are not followed)
fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(folder) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => todo.push(entry.path()),
                Ok(kind) if kind.is_file() => files.push(entry.path()),
                _ => {}
            }
        }
    }
    files
}

/// Every folder under `root` (not `root` itself), at any depth
fn folders_under(root: &Path) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(folder) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|k| k.is_dir()) {
                folders.push(entry.path());
                todo.push(entry.path());
            }
        }
    }
    folders
}

/// One file copied, its folders made, synced to the disk (through a handle that may write:
/// Windows refuses to flush a file opened only to read)
fn copy_file(from: &Path, to: &Path) -> anyhow::Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from, to)?;
    std::fs::OpenOptions::new().write(true).open(to)?.sync_all()?;
    Ok(())
}

/// A folder's whole content into a new folder (made here: it must not be there yet)
fn copy_tree(from: &Path, to: &Path) -> anyhow::Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(to)?;
    for file in files_under(from) {
        let relative = file.strip_prefix(from)?;
        copy_file(&file, &to.join(relative))?;
    }
    Ok(())
}

/// The two files hold the same bytes (compared by size, then sha256)
fn same_file(a: &Path, b: &Path) -> anyhow::Result<bool> {
    use sha2::{Digest, Sha256};
    if std::fs::metadata(a)?.len() != std::fs::metadata(b)?.len() {
        return Ok(false);
    }
    Ok(Sha256::digest(std::fs::read(a)?) == Sha256::digest(std::fs::read(b)?))
}

/// The two folders hold the same files (by name, at any depth), each with the same bytes
fn same_tree(a: &Path, b: &Path) -> anyhow::Result<bool> {
    let names = |root: &Path| {
        let mut names: Vec<PathBuf> = files_under(root).iter().filter_map(|f| f.strip_prefix(root).ok().map(Path::to_path_buf)).collect();
        names.sort();
        names
    };
    let files = names(a);
    if files != names(b) {
        return Ok(false);
    }
    for file in &files {
        if !same_file(&a.join(file), &b.join(file))? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Puts the original language files back in lang/ (i18n::restore_originals: a changed one kept
/// as `.bak`), then the app reads the texts again (`Event::LanguagesRestored`)
fn restore_languages(events: &Sender<Event>) {
    let lines = match i18n::restore_originals() {
        Ok(written) if written.is_empty() => vec![t("settings-originals-same")],
        Ok(written) => {
            let _ = events.send(Event::LanguagesRestored);
            let files: Vec<String> = written.iter().map(|p| p.display().to_string()).collect();
            vec![tf("settings-originals-done", &[("files", files.join(", ").into())]), t("settings-originals-kept")]
        }
        Err(e) => vec![t("settings-originals-failed"), format!("{e:#}")],
    };
    let _ = events.send(Event::Log(Level::Info, lines.join(" ")));
    let _ = events.send(Event::Notice(t("tab-settings"), lines));
}

/// Reads two backups of one node (each file checked against its sha256) and sends what changed
/// from the older to the newer, or why it could not be compared
fn compare_backups(older: &Path, newer: &Path, events: &Sender<Event>) {
    let stamp = |p: &Path| store::stamp_text(&p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    let report = || -> anyhow::Result<Report> {
        let (old_manifest, old_state) = store::load(older)?;
        let (new_manifest, new_state) = store::load(newer)?;
        if old_manifest.node.kind != new_manifest.node.kind {
            anyhow::bail!(t("compare-kinds-differ"));
        }
        Ok(Report {
            name: new_manifest.node.name.clone(),
            older: stamp(older),
            newer: stamp(newer),
            comparison: compare::compare(new_manifest.node.kind, &old_state, &new_state),
        })
    };
    let event = match report() {
        Ok(report) => Event::Compared(Box::new(report)),
        Err(e) => Event::Notice(t("compare-title"), vec![format!("{e:#}")]),
    };
    let _ = events.send(event);
}

/// What changed on the node since `backup`: the backup as "before", the node read now as
/// "after" (the same view as two backups compared)
fn compare_node(backup: &Path, port: &Port, shared: &Shared, progress: &mut dyn FnMut(&str, usize, usize)) {
    let stamp = store::stamp_text(&backup.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    let mut report =|| -> anyhow::Result<Report> {
        let (manifest, want) = store::load(backup)?;
        let (kind, mut now) = restore::read_now(port, shared.wait, progress)?;
        // "now" for your custom names and roles too: the address book as it is
        if let Some(part) = book::load(&shared.paths.book).ok().and_then(|book| names::part(&book, &now)) {
            now.parts.push(part);
        }
        if kind != manifest.node.kind {
            anyhow::bail!(t("compare-kinds-differ"));
        }
        Ok(Report { name: manifest.node.name.clone(), older: stamp.clone(), newer: t("compare-node-now"), comparison: compare::compare(kind, &want, &now) })
    };
    let event = match report() {
        Ok(report) => Event::Compared(Box::new(report)),
        Err(e) => Event::Notice(t("compare-title"), vec![format!("{e:#}")]),
    };
    let _ = shared.events.send(event);
}

/// Backs the node up; true when the backup was saved (whole and verified)
fn backup(node: &Node, reason: Reason, shared: &Shared, progress: &mut dyn FnMut(&str, usize, usize)) -> bool {
    let (config, root, book) = (&shared.config, &shared.paths.backups, &shared.paths.book);
    // asked for (b): from the reading the view made, at once, without opening the node again.
    // Before a change (an edit, a restore): always read now, it is the way back.
    let result = match (&node.reading, reason) {
        (Some(reading), Reason::Manual) => capture::save(node, reading, config, root, book, reason),
        _ => capture::capture(node, config, root, book, reason, progress),
    };
    let saved = result.is_ok();
    let (level, line) = match result {
        Ok(folder) => {
            let stamp = store::stamp_text(&folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            (Level::Good, tf("log-backup-done", &[("name", node.name.as_str().into()), ("stamp", stamp.into())]))
        }
        Err(e) => (
            Level::Bad,
            tf("log-backup-failed", &[("name", node.name.as_str().into()), ("error", format!("{e:#}").into())]),
        ),
    };
    let _ = shared.events.send(Event::Log(level, line));
    // a new backup on disk: the lists show it now, not only when the job ends
    let _ = shared.events.send(Event::BackupsChanged);
    saved
}

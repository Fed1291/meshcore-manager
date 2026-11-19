//! The full-screen app: one loop that takes events (keys, news from the worker), updates the
//! shared state (`Ctx`) and draws. Top: the logo, the name and the pages; middle: the open page;
//! bottom: its keys and what the worker is doing. The pages themselves are in src/pages/.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{DefaultTerminal, Frame};

use crate::backup::compare::Report;
use crate::backup::journal::Journal;
use crate::backup::restore::{self, Preview};
use crate::backup::store::{self, NodeBackups};
use crate::book::{self, Book};
use crate::config::Config;
use crate::debug;
use crate::help;
use crate::i18n;
use crate::event::{self, Event, Job, Level};
use crate::i18n::{t, tf};
use crate::jobs::JobId;
use crate::node::Node;
use crate::pages::{self, Page};
use crate::paths::Paths;
use crate::regions::{self, Region};
use crate::serial::Port;
use crate::worker::{self, Carry, Cmd, FolderCheck, JOB_THREAD};
use crate::{theme, widgets};

/// Lines kept for the Log page
const LOG_KEEP: usize = 500;

/// "MC" in pixels: quarter blocks, 2 x 2 pixels per cell (so the M has diagonals, not steps),
/// 2 rows high = 4 pixels. The pixels, 16 x 4 (X = lit):
///   XX...XX...XXXXX.
///   XXX.XXX..XX.....
///   XX.X.XX..XX.....
///   XX...XX...XXXXX.
/// Windows Terminal and most terminals draw these blocks themselves, whatever the font.
const LOGO: [&str; 2] = [" █▖▟▌▗▛▀▘", " █▝▐▌▝▙▄▖"];

pub struct LogLine {
    pub time: String,
    pub level: Level,
    pub text: String,
    /// When it came: the bottom line shows it a while, then it is only in the Log page
    at: Instant,
    /// Its number since the start (`Ctx::logged`): a job started after it makes it old news
    number: u64,
    /// News of what the worker did by itself (a Bluetooth companion read when heard): in the Log
    /// page only, its Fleet row says it
    quiet: bool,
}

/// How long the newest log line stays in the bottom line (a problem a little longer)
const STATUS_SHOW: Duration = Duration::from_secs(6);
const STATUS_SHOW_BAD: Duration = Duration::from_secs(12);

/// What every page can read: the state of the world, kept up to date by the app
pub struct Ctx {
    /// The nodes on the USB ports
    pub nodes: Vec<Node>,
    /// The address book (contacts.toml): every contact and access-list key seen, your names
    pub book: Book,
    /// false until the worker looked at the ports once
    pub scanned: bool,
    /// The backups on disk, read again after each job
    pub backups: Vec<NodeBackups>,
    pub log: VecDeque<LogLine>,
    /// The jobs running, side by side (one per board), in the order they started
    pub jobs: Vec<Job>,
    /// The latest comparison of two backups, with a number that grows with each new one (the
    /// Backups page opens it when the number is new)
    pub compared: Option<(u64, Report)>,
    /// The user's list of region names (regions.txt): saved by the worker, `Cmd::SaveRegions`
    pub regions: Vec<Region>,
    /// A question is open: the button that has the focus (a page can show what it would do,
    /// live: the regions a removal takes with it)
    pub asking: Option<usize>,
    /// The settings as they are used now (the Settings page shows and changes them)
    pub config: Config,
    /// Where the files are (the backups folder follows the Settings page)
    pub paths: Paths,
    /// Why Bluetooth cannot listen for companions (None: it listens, or is off in the settings)
    pub bluetooth: Option<String>,
    /// The latest PIN the Fleet must ask for: a companion refused the PIN tried (the number grows
    /// with each ask, so the Fleet opens each one once)
    pub pin_ask: Option<PinAsk>,
    /// Log lines written since the start: a job's own lines are those after its mark
    logged: u64,
    /// Each running job and the count of log lines when it started
    job_marks: Vec<(JobId, u64)>,
    /// How each job ended (its last log line), shown a while where its progress was
    ended: Vec<Ended>,
}

/// The last log line of a job that ended: a node's view shows it in its top line, where the
/// job's progress was, not only at the bottom
pub struct Ended {
    pub port: Port,
    pub level: Level,
    pub text: String,
    at: Instant,
}

impl Ended {
    /// Still to be shown (a problem a little longer, as in the bottom line)
    fn fresh(&self) -> bool {
        self.at.elapsed() < if self.level == Level::Bad { STATUS_SHOW_BAD } else { STATUS_SHOW }
    }
}

/// A Bluetooth companion refused the PIN: asked on the Fleet page, then `retry` is sent again
pub struct PinAsk {
    pub number: u64,
    pub port: Port,
    pub retry: Option<Cmd>,
}

impl Ctx {
    /// `quiet`: for the Log page only, never flashed in the bottom line
    fn log(&mut self, level: Level, text: String, quiet: bool) {
        // every line of the Log page in the debug log too (redacted there)
        let lvl = match level {
            Level::Bad => "ERROR",
            Level::Good | Level::Info => "INFO",
        };
        crate::debug_log!(lvl, "log: {text}");
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logged += 1;
        self.log.push_back(LogLine { time, level, text, at: Instant::now(), number: self.logged, quiet });
        while self.log.len() > LOG_KEEP {
            self.log.pop_front();
        }
    }

    /// A job ended: its last log line (if it wrote one) is kept for its board's view
    fn job_ended(&mut self, id: JobId) {
        let Some(at) = self.job_marks.iter().position(|(j, _)| *j == id) else { return };
        let (_, mark) = self.job_marks.remove(at);
        let port = self.jobs.iter().find(|j| j.id == id).map(|j| j.port.clone());
        let own = (self.logged - mark) as usize;
        if let (Some(port), true) = (port, own > 0) {
            if let Some(last) = self.log.back() {
                self.ended.retain(|e| !e.port.same_board(&port) && e.fresh());
                self.ended.push(Ended { port, level: last.level, text: last.text.clone(), at: Instant::now() });
            }
        }
    }

    /// How the latest job on this board ended, while it is still shown
    pub fn ended_on(&self, port: &Port) -> Option<&Ended> {
        self.ended.iter().rev().find(|e| e.port.same_board(port) && e.fresh())
    }

    fn reload_backups(&mut self) {
        self.backups = store::list(&self.paths.backups);
        // the names and keys of nodes backed up (also those not plugged in) never reach the debug log
        for node in self.backups.iter().filter_map(|b| b.id.as_ref()) {
            debug::private(&node.name, debug::NODE);
            debug::private(&node.public_key, debug::KEY);
            debug::private(&node.hardware_id, debug::SERIAL);
        }
    }

    /// The job working on this board, if any (a node's view shows its own, not another's)
    pub fn job_on(&self, port: &Port) -> Option<&Job> {
        self.jobs.iter().find(|j| j.port.same_board(port))
    }
}

/// A message in the middle of the screen: a note (`Popup::note`, one OK button) or a question
/// (`Popup::ask`, buttons that each name what they do: "back up, then write" / "write now" /
/// "keep editing"). ← → / Tab move the focus, Enter presses it, Esc presses none (cancel).
pub struct Popup {
    pub title: String,
    pub lines: Vec<String>,
    /// Left to right: the text and what pressing it does (None: close, nothing done)
    pub buttons: Vec<(String, Option<Confirm>)>,
    /// The button Enter presses
    pub focus: usize,
    /// The buttons that do something remove or throw something away: drawn in red (`safe`)
    pub danger: bool,
}

/// What a button does
pub enum Confirm {
    /// Hand these to the worker, in order (the page is told too: `Page::confirmed`)
    Send(Vec<Cmd>),
    /// Only tell the page (`Page::confirmed`): leave edit mode and drop the changes
    Page,
    /// One of several choices the page offered (`Page::chosen`): what to do is the page's
    Choose(usize),
    /// Hand these to the worker; the page is not told (a question of the app's own: a restore)
    Run(Vec<Cmd>),
    /// Ask once more before doing it (a second, stronger question: a node taking another identity)
    Ask(Box<Popup>),
    /// Quit, although jobs are running (asked first: `App::quit_or_ask`)
    Quit,
}

impl Popup {
    pub fn note(title: String, lines: Vec<String>) -> Popup {
        Popup { title, lines, buttons: vec![(t("popup-ok"), None)], focus: 0, danger: false }
    }

    /// `yes` does `confirm`, `no` closes: each in a few words. The focus starts on `yes` (the
    /// user asked for this action); see `safe` for a question where "yes" loses work.
    pub fn ask(title: String, lines: Vec<String>, confirm: Confirm, yes: String, no: String) -> Popup {
        Popup { title, lines, buttons: vec![(yes, Some(confirm)), (no, None)], focus: 0, danger: false }
    }

    /// A recommended answer before the others (first, with the focus): "back up, then write"
    pub fn first(mut self, text: String, confirm: Confirm) -> Popup {
        self.buttons.insert(0, (text, Some(confirm)));
        self.focus = 0;
        self
    }

    /// "Yes" removes or throws something away: the focus starts on the button that does nothing
    /// (a reflex Enter must not lose anything), and the others are red
    pub fn safe(mut self) -> Popup {
        self.focus = self.buttons.iter().position(|(_, c)| c.is_none()).unwrap_or(0);
        self.danger = true;
        self
    }
}

/// What a page wants after a key
pub enum Action {
    None,
    /// Hand these to the worker, in order
    Send(Vec<Cmd>),
    /// Show this, and ask first when it has a `confirm`
    Ask(Popup),
    /// Change a setting: used at once, saved to config.toml by the worker
    Set(Setting),
}

/// A setting the Settings page changes (the ones used live; the backups folder goes through the
/// worker, `Cmd::BackupsFolder`, as it is checked and may copy)
pub enum Setting {
    /// "auto" or a language code
    Language(String),
    DebugLog(bool),
    /// The PIN Bluetooth pairing uses (digits, checked: ble::pin)
    BluetoothPin(String),
}

struct App {
    ctx: Ctx,
    pages: Vec<Box<dyn Page>>,
    open: usize,
    popup: Option<Popup>,
    /// Popups from the worker that came while another was open
    waiting: VecDeque<Popup>,
    cmds: Sender<Cmd>,
    quit: bool,
}

/// After the new backups folder was tested: why it cannot be used (a note), or what to do with the
/// backups already saved. Copy is the recommended answer (it has the focus: nothing is removed);
/// move removes the old ones once their copies checked out, so it is red and asked twice.
fn folder_question(folder: String, result: Result<FolderCheck, String>) -> Popup {
    let check = match result {
        Ok(check) => check,
        Err(why) => return Popup::note(t("settings-backups-folder"), vec![t("settings-folder-unusable"), why]),
    };
    let target = check.path.display().to_string();
    let run = |carry: Carry| Confirm::Run(vec![Cmd::BackupsFolder { folder: folder.clone(), carry }]);
    let mut lines = vec![tf("settings-folder-ok", &[("folder", target.as_str().into())])];
    // the same folder, or nothing to bring: one question
    if check.same || check.backups == 0 {
        if check.backups == 0 && !check.same {
            lines.push(t("settings-folder-nothing"));
        }
        return Popup::ask(t("settings-backups-folder"), lines, run(Carry::Leave), t("settings-folder-use"), t("answer-cancel"));
    }
    let size = format!("{:.1}", check.bytes as f64 / 1_048_576.0);
    lines.push(tf("settings-folder-has", &[("count", check.backups.into()), ("size", size.into())]));
    lines.push(t("settings-folder-how"));
    let sure = Popup::ask(
        t("settings-backups-folder"),
        vec![tf("settings-folder-move-sure", &[("count", check.backups.into())])],
        run(Carry::Move),
        t("settings-folder-move"),
        t("answer-cancel"),
    )
    .safe();
    Popup {
        title: t("settings-backups-folder"),
        lines,
        buttons: vec![
            (t("settings-folder-copy"), Some(run(Carry::Copy))),
            (t("settings-folder-move"), Some(Confirm::Ask(Box::new(sure)))),
            (t("settings-folder-leave"), Some(run(Carry::Leave))),
            (t("answer-cancel"), None),
        ],
        focus: 0,
        danger: false,
    }
}

/// The `[backup] folder` setting for a folder: "" for the default one (backups in the data folder)
fn folder_setting(folder: &std::path::Path, root: &std::path::Path) -> String {
    if folder == root.join("backups") {
        String::new()
    } else {
        folder.display().to_string()
    }
}

/// The question before a restore: what it changes, then "restore" (red) / "cancel". No backup
/// before (that would pile up a backup at every restore). A node that takes another identity (a
/// move) is asked a second time. Nothing to change: a note.
fn restore_question(preview: Preview) -> Popup {
    let Preview { request, lines, changes, moves } = preview;
    if changes == 0 {
        return Popup::note(t("restore-title"), lines);
    }
    let run = Confirm::Run(vec![Cmd::Restore(request)]);
    let confirm = if moves {
        let again = Popup::ask(t("restore-move-title"), vec![t("restore-move-sure")], run, t("restore-move-yes"), t("answer-cancel"));
        Confirm::Ask(Box::new(again.safe()))
    } else {
        run
    };
    Popup::ask(t("restore-title"), lines, confirm, t("restore-yes"), t("answer-cancel")).safe()
}

/// A restore was cut short: finish it, put the node back as it was, or decide later (asked
/// again at the next start: the journal stays)
fn interrupted_question(entry: &Journal) -> Popup {
    let mut lines = vec![tf(
        "restore-interrupted",
        &[("name", entry.name.as_str().into()), ("started", entry.started.as_str().into())],
    )];
    if entry.putting_back {
        lines.push(t("restore-was-putting-back"));
    }
    lines.push(t("restore-interrupted-how"));
    let finish = Confirm::Run(vec![Cmd::Restore(restore::resume(entry, false))]);
    let back = Confirm::Run(vec![Cmd::Restore(restore::resume(entry, true))]);
    let mut popup = Popup::ask(t("restore-interrupted-title"), lines, back, t("restore-put-back-button"), t("restore-later"));
    // putting back first (the safe way); finishing only when it was not already putting back
    if !entry.putting_back {
        popup.buttons.insert(1, (t("restore-finish-button"), Some(finish)));
    }
    popup
}

pub fn run(paths: Paths, config: Config, notes: Vec<String>) -> Result<()> {
    let (events, inbox) = mpsc::channel();
    event::spawn_input(events.clone());
    let mut app = App::new(paths, config, notes, events);

    // ratatui::init also restores the terminal if the program panics
    let mut terminal = ratatui::init();
    // a paste comes whole (`Event::Paste`), not as keys: a line break in it is not Enter.
    // A terminal without it (old Windows consoles) pastes as keys: no harm, the error is ignored
    let _ = ratatui::crossterm::execute!(std::io::stdout(), EnableBracketedPaste);
    // ... except in a job: its panic is caught and logged by the job itself (worker.rs), its board
    // let go, and the screen stays as it is (restoring the terminal there would break it)
    let terminal_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // where it happened (file and line: the module) in the debug log, the message redacted
        let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        crate::debug_log!("ERROR", "panic at {at}: {message}");
        if std::thread::current().name().is_some_and(|n| n.starts_with(JOB_THREAD)) {
            return;
        }
        terminal_hook(info);
    }));
    let result = app.main_loop(&mut terminal, inbox);
    let _ = ratatui::crossterm::execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

impl App {
    /// The app on its data, its worker started
    fn new(paths: Paths, config: Config, notes: Vec<String>, events: Sender<Event>) -> App {
        let (regions, regions_note) = regions::load(&paths.regions);
        // the address book: a file that cannot be read is said, and never written over
        let (book, book_note) = match book::load(&paths.book) {
            Ok(book) => (book, None),
            Err(e) => (Book::default(), Some(format!("{e:#}"))),
        };
        let mut ctx = Ctx {
            book,
            nodes: Vec::new(),
            scanned: false,
            backups: Vec::new(),
            log: VecDeque::new(),
            jobs: Vec::new(),
            compared: None,
            regions,
            asking: None,
            config: config.clone(),
            paths: paths.clone(),
            bluetooth: None,
            pin_ask: None,
            logged: 0,
            job_marks: Vec::new(),
            ended: Vec::new(),
        };
        ctx.reload_backups();
        for note in notes.into_iter().chain(regions_note).chain(book_note) {
            ctx.log(Level::Bad, note, false);
        }
        let cmds = worker::spawn(config, paths, events);
        App { ctx, pages: pages::all(), open: 0, popup: None, waiting: VecDeque::new(), cmds, quit: false }
    }

    fn main_loop(&mut self, terminal: &mut DefaultTerminal, inbox: Receiver<Event>) -> Result<()> {
        while !self.quit {
            terminal.draw(|f| self.draw(f))?;
            // wait for one event, then take all that are already there before drawing again
            self.handle(inbox.recv()?);
            while let Ok(event) = inbox.try_recv() {
                self.handle(event);
            }
        }
        Ok(())
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.on_key(key),
            Event::Paste(text) => self.paste(&text),
            Event::Redraw => {}
            Event::Fleet(nodes) => {
                self.ctx.nodes = nodes;
                self.ctx.scanned = true;
            }
            Event::Job(job) => match self.ctx.jobs.iter_mut().find(|j| j.id == job.id) {
                Some(known) => *known = job,
                None => {
                    self.ctx.job_marks.push((job.id, self.ctx.logged));
                    self.ctx.jobs.push(job);
                }
            },
            Event::JobEnd(id) => {
                self.ctx.job_ended(id);
                self.ctx.jobs.retain(|j| j.id != id);
                self.ctx.reload_backups();
            }
            Event::BackupsChanged => self.ctx.reload_backups(),
            Event::FolderChecked { folder, result } => self.show(folder_question(folder, result)),
            // the Fleet asks for the PIN (it is the first page): shown at once, wherever one was
            Event::PinNeeded { port, retry } => {
                let number = self.ctx.pin_ask.as_ref().map_or(1, |a| a.number + 1);
                self.ctx.pin_ask = Some(PinAsk { number, port, retry });
                self.open = 0;
            }
            Event::Bluetooth(problem) => self.ctx.bluetooth = problem,
            Event::BackupsFolder(folder) => {
                self.ctx.config.backup.folder = folder_setting(&folder, &self.ctx.paths.root);
                self.ctx.paths.backups = folder;
                self.ctx.reload_backups();
            }
            Event::LanguagesRestored => {
                i18n::init(&self.ctx.config.ui.language);
                if let Some(note) = help::init(&self.ctx.paths.help) {
                    self.ctx.log(Level::Bad, note, false);
                }
            }
            Event::Log(level, text) => self.ctx.log(level, text, false),
            Event::LogOnly(level, text) => self.ctx.log(level, text, true),
            Event::Regions(list) => self.ctx.regions = list,
            Event::Book(book) => self.ctx.book = *book,
            Event::Notice(title, lines) => self.show(Popup::note(title, lines)),
            Event::RestorePreview(preview) => self.show(restore_question(*preview)),
            Event::Interrupted(entry) => self.show(interrupted_question(&entry)),
            Event::Compared(report) => {
                let number = self.ctx.compared.as_ref().map_or(1, |(n, _)| n + 1);
                self.ctx.compared = Some((number, *report));
            }
        }
    }

    /// A popup from the worker: shown now, or after the one that is open (never over it)
    fn show(&mut self, popup: Popup) {
        if self.popup.is_none() {
            self.popup = Some(popup);
        } else {
            self.waiting.push_back(popup);
        }
    }

    /// The popup closed: the next one waiting, if any
    fn next_popup(&mut self) {
        if self.popup.is_none() {
            self.popup = self.waiting.pop_front();
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        if let Some(mut popup) = self.popup.take() {
            let count = popup.buttons.len().max(1);
            match key.code {
                // the focused button: what it does, then closed
                KeyCode::Enter => {
                    let pressed = (popup.focus < popup.buttons.len()).then(|| popup.buttons.swap_remove(popup.focus));
                    match pressed.and_then(|(_, confirm)| confirm) {
                        Some(Confirm::Send(cmds)) => {
                            for cmd in cmds {
                                let _ = self.cmds.send(cmd);
                            }
                            self.pages[self.open].confirmed();
                        }
                        Some(Confirm::Page) => self.pages[self.open].confirmed(),
                        Some(Confirm::Choose(choice)) => {
                            let action = self.pages[self.open].chosen(choice, &self.ctx);
                            self.perform(action);
                        }
                        Some(Confirm::Run(cmds)) => {
                            for cmd in cmds {
                                let _ = self.cmds.send(cmd);
                            }
                        }
                        Some(Confirm::Ask(next)) => self.popup = Some(*next),
                        Some(Confirm::Quit) => self.quit = true,
                        None => {}
                    }
                    self.next_popup();
                }
                // nothing done
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => self.next_popup(),
                KeyCode::Right | KeyCode::Tab => {
                    popup.focus = (popup.focus + 1) % count;
                    self.popup = Some(popup);
                }
                KeyCode::Left | KeyCode::BackTab => {
                    popup.focus = (popup.focus + count - 1) % count;
                    self.popup = Some(popup);
                }
                // any other key: the popup stays
                _ => self.popup = Some(popup),
            }
            return;
        }
        let pages = self.pages.len();
        // text being typed, or a view of its own open (a node): q, Tab and digits are the page's
        let captured = self.pages[self.open].typing() || self.pages[self.open].owns_keys();
        match key.code {
            _ if widgets::ctrl(&key, 'c') => self.quit_or_ask(),
            _ if captured => self.act(key),
            KeyCode::Char('q') => self.quit_or_ask(),
            KeyCode::Tab => self.open = (self.open + 1) % pages,
            KeyCode::BackTab => self.open = (self.open + pages - 1) % pages,
            KeyCode::Char(c @ '1'..='9') if (c as usize - '1' as usize) < pages => self.open = c as usize - '1' as usize,
            _ => self.act(key),
        }
    }

    /// Text pasted in one go: typed into the field being typed, as if each character were a key.
    /// Line breaks and other control characters are left out (a pasted path ends with one; as a
    /// key it would be Enter). No field being typed, or a popup open: nothing (pasted letters
    /// would be taken as commands).
    fn paste(&mut self, text: &str) {
        if self.popup.is_some() || !self.pages[self.open].typing() {
            return;
        }
        for c in text.chars().filter(|c| !c.is_control()) {
            self.act(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
    }

    /// Quits; while jobs run, asks first (quitting cuts them off), the focus on "wait"
    fn quit_or_ask(&mut self) {
        let count = self.ctx.jobs.len();
        if count == 0 {
            self.quit = true;
            return;
        }
        let lines = vec![tf("quit-ask", &[("count", count.into())]), t("quit-why")];
        self.popup = Some(Popup::ask(t("quit-title"), lines, Confirm::Quit, t("quit-yes"), t("quit-wait")).safe());
    }

    /// Hands the key to the open page and does what it asks
    fn act(&mut self, key: KeyEvent) {
        let action = self.pages[self.open].on_key(key, &self.ctx);
        self.perform(action);
    }

    /// What a page asked for
    /// A setting changed on the Settings page: used at once (the next drawing is in the new
    /// language; the debug log starts or stops), then saved to config.toml by the worker
    fn set(&mut self, setting: Setting) {
        let (section, key, value) = match setting {
            Setting::Language(code) => {
                i18n::init(&code);
                // the settings help follows the language (its note, if any, in the Log page)
                if let Some(note) = help::init(&self.ctx.paths.help) {
                    self.ctx.log(Level::Bad, note, false);
                }
                self.ctx.config.ui.language = code.clone();
                ("ui", "language", toml::Value::String(code))
            }
            Setting::DebugLog(on) => {
                debug::set(on);
                self.ctx.config.debug.log = on;
                ("debug", "log", toml::Value::Boolean(on))
            }
            // the next pairing uses it (a companion already paired needs no PIN)
            Setting::BluetoothPin(pin) => {
                crate::serial::set_pin(&pin);
                self.ctx.config.bluetooth.pin = pin.clone();
                ("bluetooth", "pin", toml::Value::String(pin))
            }
        };
        let _ = self.cmds.send(Cmd::SaveSetting { section, key, value });
    }

    fn perform(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Send(cmds) => {
                for cmd in cmds {
                    let _ = self.cmds.send(cmd);
                }
            }
            Action::Ask(popup) => self.popup = Some(popup),
            Action::Set(setting) => self.set(setting),
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let width = f.area().width as usize;
        // bottom: the keys, wrapped by whole "key  what" pairs; the status on the right of the
        // last line, or on a line of its own when it does not fit there
        let mut lines: Vec<Vec<Span>> = vec![Vec::new()];
        let mut used = 0;
        // the app's own keys, unless the page holds them (a node's view: Esc goes back)
        let page = &self.pages[self.open];
        let global = if page.typing() || page.owns_keys() {
            Vec::new()
        } else {
            vec![("Tab", t("key-switch")), ("q", t("key-quit"))]
        };
        // x removes or deletes, on every page: red here too, as everywhere
        for (key, what) in page.keys().into_iter().chain(global) {
            let style = if key == "x" { theme::bad() } else { theme::dim() };
            let pair = [Span::styled(format!(" {key}"), theme::key()), Span::styled(format!(" {what}  "), style)];
            let w = pair[0].width() + pair[1].width();
            if used > 0 && used + w > width {
                lines.push(Vec::new());
                used = 0;
            }
            used += w;
            if let Some(line) = lines.last_mut() {
                line.extend(pair);
            }
        }
        let status = widgets::ellipsis(self.status(), width);
        let status_alone = used + status.width() + 1 > width;
        let rows = lines.len() + status_alone as usize;

        // the logo (its first row with the name and the pages), the page, the keys
        let [top, middle, bottom] =
            // the logo's two rows and an empty one: the page does not touch the logo
            Layout::vertical([Constraint::Length(3), Constraint::Min(0), Constraint::Length(rows as u16)])
                .areas(f.area());

        let mut tabs = vec![
            Span::styled(LOGO[0], theme::title()),
            Span::styled(format!("  {} ", t("app-name")), theme::title()),
        ];
        for (i, page) in self.pages.iter().enumerate() {
            // inside a node's view the pages are out of reach (Esc first): the open one is dimmed too
            let style = match (i == self.open, self.pages[self.open].owns_keys()) {
                (true, false) => theme::tab_open(),
                (true, true) => theme::heading(),
                _ => theme::dim(),
            };
            tabs.push(Span::raw(" "));
            tabs.push(Span::styled(format!(" {} {} ", i + 1, page.title()), style));
        }
        // the logo's second row is the space between the pages and the page
        let tabs = widgets::ellipsis(Line::from(tabs), width);
        // under the name: the author credit (the license asks for it), lined up with the name
        let credit = Line::from(vec![Span::styled(LOGO[1], theme::title()), Span::styled(format!("  {}", t("app-by")), theme::dim())]);
        f.render_widget(Paragraph::new(vec![tabs, widgets::ellipsis(credit, width)]), top);

        self.ctx.asking = self.popup.as_ref().map(|p| p.focus);
        self.pages[self.open].draw(f, middle, &self.ctx);

        let last = lines.len() - 1;
        for (i, keys) in lines.into_iter().enumerate() {
            let row = Rect { y: bottom.y + i as u16, height: 1, ..bottom };
            f.render_widget(Paragraph::new(Line::from(keys)), row);
            if i == last && !status_alone {
                f.render_widget(Paragraph::new(status.clone()).right_aligned(), row);
            }
        }
        if status_alone {
            let row = Rect { y: bottom.y + rows as u16 - 1, height: 1, ..bottom };
            f.render_widget(Paragraph::new(status).right_aligned(), row);
        }

        if let Some(popup) = &self.popup {
            widgets::popup(f, middle, popup);
        }
    }

    /// The running jobs (the first with a spinner, "+2 more" for the others), else the latest log
    /// line. The jobs a page shows itself (a node's view: its own board's) are left out here:
    /// progress is shown in one place only.
    fn status(&self) -> Line<'static> {
        let shown = self.pages[self.open].shown_boards();
        // what the worker does by itself (Bluetooth companions read when heard) is in its Fleet
        // row: never here, on every page
        let jobs: Vec<&Job> =
            self.ctx.jobs.iter().filter(|j| !j.background && !shown.iter().any(|p| p.same_board(&j.port))).collect();
        // the newest log line, for a few seconds only: after that it is in the Log page
        // ...unless the open page shows it already, where that job's progress was, or the page
        // shows a newer job on its board (reading it again: the old error is out of date)
        let recent = self.ctx.log.iter().rev().find(|l| !l.quiet).filter(|l| {
            let show = if l.level == Level::Bad { STATUS_SHOW_BAD } else { STATUS_SHOW };
            let shown_above = shown.iter().any(|p| self.ctx.ended_on(p).is_some_and(|e| e.text == l.text));
            let newer_job = self.ctx.job_marks.iter().any(|(id, mark)| {
                *mark >= l.number
                    && self.ctx.jobs.iter().any(|j| j.id == *id && shown.iter().any(|p| p.same_board(&j.port)))
            });
            l.at.elapsed() < show && !shown_above && !newer_job
        });
        match (jobs.first(), recent) {
            (Some(job), _) => {
                let mut text = format!("{} {} ", widgets::spinner(), widgets::job_text(job));
                if jobs.len() > 1 {
                    text.push_str(&tf("jobs-more", &[("count", (jobs.len() - 1).into())]));
                    text.push(' ');
                }
                Line::styled(text, theme::title())
            }
            (None, Some(last)) => Line::styled(format!("{} ", last.text), theme::level(last.level)),
            (None, None) => Line::raw(""),
        }
    }
}

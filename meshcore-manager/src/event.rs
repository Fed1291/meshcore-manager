//! Everything that can happen, as one stream the app reads (src/app.rs): keys from the keyboard
//! thread here, the rest from the worker thread (src/worker.rs).

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event as Input, KeyEvent, KeyEventKind};

use crate::backup::compare::Report;
use crate::backup::journal::Journal;
use crate::backup::restore::Preview;
use crate::book::Book;
use crate::jobs::JobId;
use crate::node::Node;
use crate::serial::Port;
use crate::worker::{Cmd, FolderCheck};
use crate::regions::Region;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Good,
    Bad,
}

/// A long task in the worker, as it goes. Several run side by side (one per board): `id` tells
/// them apart, `port` says which board it works on (a node's view shows its own).
pub struct Job {
    pub id: JobId,
    pub port: Port,
    pub title: String,
    pub step: String,
    pub done: usize,
    pub total: usize,
    /// Started by the worker itself (a Bluetooth companion read when heard): shown in its Fleet
    /// row only, never in the bottom line of every page
    pub background: bool,
}

pub enum Event {
    Key(KeyEvent),
    /// Text pasted in one go (terminals with bracketed paste: Linux, Windows Terminal), line breaks
    /// and all; elsewhere a paste comes as keys
    Paste(String),
    /// The terminal changed size, or time passed (animations)
    Redraw,
    /// The nodes on the USB ports, each time something changes
    Fleet(Vec<Node>),
    Job(Job),
    /// That job ended (however): its line goes, the backups are read again
    JobEnd(JobId),
    /// Backups were added or deleted outside a job: read them again
    BackupsChanged,
    Log(Level, String),
    /// A line for the Log page only, never flashed in the bottom line: news of what the worker
    /// did by itself (its Fleet row shows it already)
    LogOnly(Level, String),
    /// The user's region list, as saved
    Regions(Vec<Region>),
    /// The address book, as saved (after a node was read, or a change of yours)
    Book(Box<Book>),
    /// Something to tell in the middle of the screen (a title, its lines): how a restore ended,
    /// why it could not start
    Notice(String, Vec<String>),
    /// What a restore would do: the question before it
    RestorePreview(Box<Preview>),
    /// A restore was cut short (found at start): finish it, or put the node back as it was
    Interrupted(Journal),
    /// What changed between two backups (`c` on the Backups page)
    Compared(Box<Report>),
    /// Backups go to this folder now (Settings page)
    BackupsFolder(PathBuf),
    /// The original language files are back in lang/: the texts are read again
    LanguagesRestored,
    /// A folder for the backups was tested (as typed): what there is to bring over, or why it
    /// cannot be used
    FolderChecked { folder: String, result: Result<FolderCheck, String> },
    /// A Bluetooth companion refused the PIN: the Fleet asks the user for its PIN, then sends
    /// `retry` again (the job that could not connect; None: nothing to run again)
    PinNeeded { port: Port, retry: Option<Cmd> },
    /// Why Bluetooth cannot listen for companions (None: it listens), each time it changes
    Bluetooth(Option<String>),
}

/// How often the screen is drawn again with nothing happening (the animations)
const TICK: Duration = Duration::from_millis(150);

/// Reads the keyboard for as long as the program runs; a `Redraw` every TICK when it is quiet
pub fn spawn_input(events: Sender<Event>) {
    std::thread::spawn(move || loop {
        let event = match event::poll(TICK) {
            Ok(false) => Event::Redraw,
            Ok(true) => match event::read() {
                // Windows reports key releases too: only presses count
                Ok(Input::Key(key)) if key.kind == KeyEventKind::Press => Event::Key(key),
                Ok(Input::Resize(..)) => Event::Redraw,
                Ok(Input::Paste(text)) => Event::Paste(text),
                Ok(_) => continue,
                Err(_) => break,
            },
            Err(_) => break,
        };
        if events.send(event).is_err() {
            break;
        }
    });
}

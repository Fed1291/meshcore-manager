//! The pages of the app, one file each. A page draws itself from the shared state (`Ctx`) and
//! turns keys into an `Action`; it never opens a port or touches a file itself (the worker does).
//!
//! Adding a page: a new file here with a struct that implements `Page`, one line in `all()`
//! (the order there is the order of the tabs), its texts in locales/en/mcmanager.ftl.

mod about;
mod backups;
mod boards;
mod fleet;
/// Not a tab: several checked nodes of one kind edited together, opened from Fleet
mod group;
mod log;
/// Not a tab: the view of one node, opened from Fleet
mod node;
/// The address book (contacts.toml): every contact and access-list key seen, your names for them
mod people;
mod regions;
mod settings;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::{Action, Ctx};
use crate::i18n::{t, tf};
use crate::node::Kind;
use crate::serial::Port;

pub trait Page {
    /// The name on its tab
    fn title(&self) -> String;
    /// Its own keys, for the bottom line: (key, what it does)
    fn keys(&self) -> Vec<(&'static str, String)>;
    fn draw(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx);
    /// A key the app did not take itself (it keeps Tab, 1-9, q, unless the page is `typing`)
    fn on_key(&mut self, key: KeyEvent, ctx: &Ctx) -> Action;
    /// The page is taking text: every key goes to it (only Ctrl+C still quits)
    fn typing(&self) -> bool {
        false
    }
    /// The page has a view of its own open (a node): Tab, 1-9 and q are its keys then, not the
    /// app's (Tab goes through the view's sections, Esc goes back one step)
    fn owns_keys(&self) -> bool {
        false
    }
    /// The boards whose jobs the page shows itself (a node's view: where the result will appear),
    /// so the bottom line does not repeat them (it shows the other boards' jobs)
    fn shown_boards(&self) -> Vec<Port> {
        Vec::new()
    }
    /// The question this page asked (`Action::Ask` with a `confirm`) was answered yes
    fn confirmed(&mut self) {}
    /// The button `Confirm::Choose(choice)` of a question this page asked was pressed
    fn chosen(&mut self, _choice: usize, _ctx: &Ctx) -> Action {
        Action::None
    }
}

pub fn all() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(fleet::Fleet::default()),
        Box::new(backups::Backups::default()),
        Box::new(people::People::default()),
        Box::new(regions::Regions::default()),
        Box::new(boards::Boards::default()),
        Box::new(log::Log),
        Box::new(settings::Settings::default()),
        Box::new(about::About::default()),
    ]
}

/// Which kind of node a list shows (Fleet, Backups); `f` goes to the next one
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Only {
    #[default]
    All,
    Of(Kind),
}

impl Only {
    const ORDER: [Kind; 4] = [Kind::Companion, Kind::Repeater, Kind::RoomServer, Kind::Sensor];

    /// all -> companions -> repeaters -> room servers -> sensors -> all
    fn next(self) -> Only {
        let at = match self {
            Only::All => return Only::Of(Self::ORDER[0]),
            Only::Of(kind) => Self::ORDER.iter().position(|k| *k == kind).unwrap_or(Self::ORDER.len()),
        };
        Self::ORDER.get(at + 1).map_or(Only::All, |k| Only::Of(*k))
    }

    /// A row of this kind (None: not known) is in the list
    fn shows(self, kind: Option<Kind>) -> bool {
        match self {
            Only::All => true,
            Only::Of(only) => kind == Some(only),
        }
    }

    /// A panel title with the filter in it: "3 nodes  [repeater]"
    fn title(self, title: String) -> String {
        match self {
            Only::All => title,
            Only::Of(kind) => tf("only-title", &[("title", title.into()), ("kind", kind.label().into())]),
        }
    }

    /// For the bottom line: what `f` shows now ("all", "repeater")
    fn key(self) -> (&'static str, String) {
        let now = match self {
            Only::All => t("only-all"),
            Only::Of(kind) => kind.label(),
        };
        ("f", tf("key-only", &[("now", now.into())]))
    }
}

/// Moves a selection in a list of `len` rows with the arrow keys, PgUp / PgDn, Home / End.
/// Always gives a valid row (0 for an empty list).
fn step(selected: usize, len: usize, key: KeyCode) -> usize {
    let last = len.saturating_sub(1);
    let next = match key {
        KeyCode::Up => selected.saturating_sub(1),
        KeyCode::Down => selected + 1,
        KeyCode::PageUp => selected.saturating_sub(10),
        KeyCode::PageDown => selected + 10,
        KeyCode::Home => 0,
        KeyCode::End => last,
        _ => selected,
    };
    next.min(last)
}

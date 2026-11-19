//! The view of one node, opened from Fleet with Enter. Sections, ← → to go from one to the next:
//! - overview: who it is (port, board, keys, radio), its settings (first, the ones it lets change)
//!   and what else it told when read (battery, clock...)
//! - contacts, channels: what a companion holds (companions only)
//! - backups: the backups of this very board (by its USB serial number, else its key: a rename
//!   does not lose them)
//!
//! `e` = edit mode (node/edit.rs here): change settings, rename / favourite / remove contacts,
//! rename / remove channels; `c` shows every change and sends them all at once.
//! Opening the view asks the worker to read the node in depth (`Cmd::Inspect`); `r` reads again.
//! Like every page it only draws from `Ctx` and returns actions.

/// Edit mode (also used by the group edit of several nodes, pages/group.rs)
pub(super) mod edit;
/// The Access tab's chooser of people from the address book (`a` in edit mode)
mod acl_pick;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, TableState, Wrap};
use ratatui::Frame;

use self::acl_pick::{AclPick, Picked as AclPicked};
use self::edit::{Editor, Target};
use super::backups::{ask_delete, ask_restore, compare_with, describe, report_lines, snapshots_table};
use super::fleet::{ask_backup, identity, identity_lines, pairing};
use super::regions::{Picked, RegionPicker};
use super::step;
use crate::app::{Action, Ctx, Popup};
use crate::backup::diff::Mode;
use crate::backup::store::{NodeBackups, Snapshot};
use std::path::PathBuf;
use crate::book::{self, Book};
use crate::help;
use crate::i18n::{t, tf};
use std::collections::BTreeMap;

use crate::node::edit as node_edit;
use crate::node::{acl, Editable, Fact, Kind, Node};
use crate::presets::{self, Preset};
use crate::proto::{Channel, Contact, NAME_MAX};
use crate::regions::{self, NodeRegions, Region};
use crate::serial::{self, Port};
use crate::theme;
use crate::event::{Job, Level};
use crate::widgets::{
    self, ago, clean, details_below, ellipsis, fact, facts, facts_in, facts_width, inner_width, input, job_text, label_width,
    more_marks, note_below, panel, panel_focus, room_line, scroll_max, scroll_sheet, sheet, spinner, text_rows, when, Col, Grid, PANEL_EDGES,
    READING,
};

/// Narrower than this, the overview shows one panel at a time (← → switch): two halves of less
/// than 40 columns cut every line
const NARROW: u16 = 80;
use crate::worker::Cmd;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Overview,
    Contacts,
    Channels,
    Regions,
    /// A console node's access list (who may log in as what)
    Access,
    Backups,
}

impl Section {
    /// The sections this node has: contacts and channels are a companion's, regions and the
    /// access list a console node's whose firmware has them (known once it is read)
    fn of(node: &Node) -> Vec<Section> {
        let mut sections = vec![Section::Overview];
        if node.kind == Kind::Companion {
            sections.extend([Section::Contacts, Section::Channels]);
        }
        if node.regions.is_some() {
            sections.push(Section::Regions);
        }
        if node.acl.is_some() {
            sections.push(Section::Access);
        }
        sections.push(Section::Backups);
        sections
    }
}

pub struct NodeView {
    /// The node is found again by its port in every new list from the worker
    port: Port,
    section: Section,
    /// The selected row of the section (overview: the selected setting in edit mode, else the
    /// first line shown)
    row: usize,
    table: TableState,
    /// Edit mode, with what was changed so far
    editor: Option<Editor>,
    /// The help of the selected setting is open (`?`), scrolled this far
    help: Option<u16>,
    /// The radio preset chooser is open (`p` in edit mode)
    presets: Option<PresetPicker>,
    /// Secrets (passwords, private key) shown (`s`); hidden again when the view closes
    reveal: bool,
    /// The region chooser is open (`a` in edit mode, Regions section)
    regions: Option<RegionPicker>,
    /// The chooser of people from the address book is open (`a` in edit mode, Access section)
    acl_pick: Option<AclPick>,
    /// Contacts checked in edit mode (Space, `a`), by key: `x` takes them all out
    checked: Vec<String>,
    /// Backups checked in the Backups section (Space, `a`): `x` deletes them (asked first)
    checked_backups: Vec<PathBuf>,
    /// The overview's left panel (Info) has the keyboard (←), not "What it says" (→)
    info_focus: bool,
    /// How far Info is scrolled (a narrow window: its lines stacked)
    info_scroll: usize,
    /// The order of the contacts (`o`)
    contact_sort: ContactSort,
    /// Your name for a contact being typed (`n`): its key, the text so far. Saved in the address
    /// book (book.rs), never on the node: its next advert would write its own name over it
    own_name: Option<(String, String)>,
    /// What the contacts are searched for (`/`), and whether it is being typed
    find: String,
    finding: bool,
    /// Bluetooth: Some(true) not paired (`p`), Some(false) paired (`u`), None USB. Noted when
    /// drawn: the key bar has no `Ctx`
    pairing: Option<bool>,
    /// A backup compared with this node (Enter in Backups): asked, the last one seen
    /// (`Ctx::compared`), and the open one's scroll
    compare_asked: bool,
    compare_seen: u64,
    compare_scroll: Option<u16>,
}

/// The longest name of yours for a contact (a line in a table, not a firmware limit)
const OWN_NAME_MAX: usize = 32;

/// The order of a companion's contacts, `o` goes to the next
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ContactSort {
    /// A to Z, first (the user's choice)
    #[default]
    Name,
    /// Favourites, then chat, repeater, room, sensor; by name inside each
    Kind,
    /// Last heard first (as the companion's apps list them)
    Heard,
}

impl ContactSort {
    fn next(self) -> ContactSort {
        match self {
            ContactSort::Name => ContactSort::Kind,
            ContactSort::Kind => ContactSort::Heard,
            ContactSort::Heard => ContactSort::Name,
        }
    }

    fn word(self) -> String {
        t(match self {
            ContactSort::Heard => "sort-heard",
            ContactSort::Kind => "sort-kind",
            ContactSort::Name => "sort-name",
        })
    }
}

/// The contacts in the order chosen (`o`), only those that hold what was searched (`/`: in the
/// advertised name, in your name for it, or the start of its key): the rows of the Contacts section
fn contacts_in_order<'a>(node: &'a Node, sort: ContactSort, find: &str, book: &Book) -> Vec<&'a Contact> {
    let find = find.to_lowercase();
    let mut list: Vec<&Contact> = node
        .contacts
        .iter()
        .filter(|c| {
            find.is_empty()
                || c.name.to_lowercase().contains(&find)
                || c.public_key.to_lowercase().starts_with(&find)
                || book.find(&c.public_key).is_some_and(|p| p.own_name.to_lowercase().contains(&find))
        })
        .collect();
    // A to Z by its letters: an emoji or a sign in front does not move a name to the end
    let name = |c: &Contact| clean(&c.name).trim_start_matches(|ch: char| !ch.is_alphanumeric()).to_lowercase();
    match sort {
        // read last heard first already (companion.rs)
        ContactSort::Heard => {}
        ContactSort::Kind => list.sort_by_key(|c| (!c.favourite, if c.kind == 0 { u8::MAX } else { c.kind }, name(c))),
        ContactSort::Name => list.sort_by_key(|c| name(c)),
    }
    list
}

/// A contact's type: its icon (2 columns wide each, so the
/// column stays straight), its word, the colour of that kind of node
pub(super) fn contact_kind(kind: u8) -> Line<'static> {
    let icon = match kind {
        1 => "💬",
        2 => "📡",
        3 => "🏠",
        4 => "📟",
        _ => "· ",
    };
    let (word, of) = contact_type(kind);
    Line::styled(format!("{icon} {word}"), theme::kind(of))
}

/// Choosing a radio preset: what was typed to narrow the list, and the selected one in it. Also
/// used by the group edit of several nodes (pages/group.rs).
#[derive(Default)]
pub(super) struct PresetPicker {
    filter: String,
    row: usize,
}

/// What a key in the preset chooser did
pub(super) enum PresetKey {
    /// Still choosing
    Not,
    Closed,
    Chosen(&'static Preset),
}

impl PresetPicker {
    /// Opened on the preset `node` has now (the first one when none matches)
    pub(super) fn on(node: &Node) -> PresetPicker {
        let now = presets::matching(node);
        let row = presets::list().iter().position(|p| now.iter().any(|m| m.title == p.title)).unwrap_or(0);
        PresetPicker { filter: String::new(), row }
    }

    /// The presets whose title holds what was typed
    fn shown(&self) -> Vec<&'static Preset> {
        let filter = self.filter.to_lowercase();
        presets::list().iter().filter(|p| p.title.to_lowercase().contains(&filter)).collect()
    }

    pub(super) fn on_key(&mut self, key: KeyEvent) -> PresetKey {
        let shown = self.shown();
        match key.code {
            KeyCode::Esc => return PresetKey::Closed,
            KeyCode::Enter => return shown.get(self.row).map_or(PresetKey::Closed, |p| PresetKey::Chosen(p)),
            KeyCode::Backspace => {
                self.filter.pop();
                self.row = 0;
            }
            KeyCode::Char(c) if !c.is_control() => {
                self.filter.push(c);
                self.row = 0;
            }
            code => self.row = step(self.row, shown.len(), code),
        }
        PresetKey::Not
    }
}

impl NodeView {
    /// Opens the view of `node`. The action reads it in depth when that was not done yet.
    pub fn open(node: &Node) -> (NodeView, Action) {
        let view = NodeView {
            port: node.port.clone(),
            section: Section::Overview,
            row: 0,
            table: TableState::default(),
            editor: None,
            help: None,
            presets: None,
            reveal: false,
            regions: None,
            acl_pick: None,
            checked: Vec::new(),
            checked_backups: Vec::new(),
            info_focus: false,
            info_scroll: 0,
            contact_sort: ContactSort::default(),
            own_name: None,
            find: String::new(),
            finding: false,
            pairing: pairing(node),
            compare_asked: false,
            compare_seen: 0,
            compare_scroll: None,
        };
        let read = if node.kind.is_node() && !node.probing && !node.inspected {
            Action::Send(vec![Cmd::Inspect(node.port.clone())])
        } else {
            Action::None
        };
        (view, read)
    }

    fn node<'a>(&self, ctx: &'a Ctx) -> Option<&'a Node> {
        ctx.nodes.iter().find(|n| n.port == self.port)
    }

    /// The rows of the Contacts section: in the order chosen, narrowed by the search
    fn contacts<'a>(&self, node: &'a Node, ctx: &Ctx) -> Vec<&'a Contact> {
        contacts_in_order(node, self.contact_sort, &self.find, &ctx.book)
    }

    /// The contact of the selected row
    fn contact_key(&self, node: &Node, ctx: &Ctx) -> Option<String> {
        self.contacts(node, ctx).get(self.row).map(|c| c.public_key.clone())
    }

    /// Typing your name for a contact, or a search: Enter / Esc end it, every other key is text
    fn contact_typing(&mut self, key: KeyEvent) -> Action {
        if let Some((contact, text)) = &mut self.own_name {
            match key.code {
                KeyCode::Esc => self.own_name = None,
                KeyCode::Enter => {
                    let change = book::Change::OwnName { key: contact.clone(), name: text.trim().to_string() };
                    self.own_name = None;
                    return Action::Send(vec![Cmd::Book(change)]);
                }
                KeyCode::Backspace => {
                    text.pop();
                }
                _ => {
                    if let Some(c) = widgets::typed(&key).filter(|c| !c.is_control()) {
                        if text.chars().count() < OWN_NAME_MAX {
                            text.push(c);
                        }
                    }
                }
            }
            return Action::None;
        }
        // the search: the list narrows as it is typed; Enter keeps it, Esc clears it
        match key.code {
            KeyCode::Esc => {
                self.find.clear();
                self.finding = false;
            }
            KeyCode::Enter => self.finding = false,
            KeyCode::Backspace => {
                self.find.pop();
            }
            _ => {
                if let Some(c) = widgets::typed(&key).filter(|c| !c.is_control()) {
                    self.find.push(c);
                }
            }
        }
        self.row = 0;
        Action::None
    }

    /// The board shown (its jobs show here, not in the bottom line)
    pub fn port(&self) -> &Port {
        &self.port
    }

    /// Text is being typed (a value, or a preset's name to find it): every key is for it
    pub fn typing(&self) -> bool {
        self.presets.is_some()
            || self.regions.is_some()
            || self.acl_pick.is_some()
            || self.own_name.is_some()
            || self.finding
            || self.editor.as_ref().is_some_and(|e| e.input.is_some())
    }

    /// Only looking at the Overview (no edit mode, no chooser, no help open): the Fleet's own
    /// key may act there (`p`: a Bluetooth companion's PIN)
    pub fn pairs_here(&self) -> bool {
        self.section == Section::Overview
            && self.help.is_none()
            && self.presets.is_none()
            && self.regions.is_none()
            && self.editor.is_none()
    }

    /// The commit was answered yes: the worker writes and reads the node again
    pub fn confirmed(&mut self) {
        self.editor = None;
    }

    pub fn keys(&self) -> Vec<(&'static str, String)> {
        if self.help.is_some() || self.compare_scroll.is_some() {
            return vec![("Esc", t("key-close")), ("↑↓", t("key-scroll"))];
        }
        if self.presets.is_some() {
            return vec![("↑↓", t("key-select")), ("Enter", t("key-use-preset")), ("a-z", t("key-find")), ("Esc", t("key-close"))];
        }
        if self.regions.is_some() {
            return vec![("↑↓", t("key-select")), ("Enter", t("key-region-take")), ("a-z", t("key-find")), ("Esc", t("key-close"))];
        }
        if self.acl_pick.is_some() {
            return vec![
                ("↑↓", t("key-select")),
                ("Space", t("key-check")),
                ("+", t("key-check-all")),
                ("←→", t("col-role")),
                ("a-z", t("key-find")),
                ("Enter", t("key-acl-give")),
                ("Esc", t("key-close")),
            ];
        }
        if self.own_name.is_some() {
            return vec![("Enter", t("key-save")), ("Esc", t("key-cancel"))];
        }
        if self.finding {
            return vec![("a-z", t("key-find")), ("Enter", t("key-ok")), ("Esc", t("key-find-clear"))];
        }
        let Some(editor) = &self.editor else {
            let mut keys = vec![("Esc", t("key-back-to-list")), ("Tab", t("key-section")), ("↑↓", t("key-select"))];
            if self.section == Section::Overview {
                keys.push(("←→", t("key-other-panel")));
            }
            if self.section == Section::Backups {
                keys.push(("Enter", t("key-compare-node")));
                keys.push(("R", t("key-restore-here")));
                keys.push(("Space", t("key-check")));
                keys.push(("a", t(if self.checked_backups.is_empty() { "key-check-all" } else { "key-uncheck-all" })));
                // x deletes the checked ones only: shown once one is checked (Space first)
                if !self.checked_backups.is_empty() {
                    keys.push(("x", tf("key-delete-n", &[("count", self.checked_backups.len().into())])));
                }
            }
            if self.section == Section::Regions {
                keys.push(("+", t("key-region-to-list")));
            }
            if self.section == Section::Contacts {
                keys.push(("n", t("key-own-name")));
                keys.push(("*", t("key-favourite")));
                keys.push(("/", t("key-search")));
                keys.push(("o", tf("key-sort", &[("now", self.contact_sort.word().into())])));
            }
            // only the keys that act in this section
            if self.section != Section::Backups {
                keys.push(("e", t("key-edit")));
            }
            // reading the node is the Overview's; a new backup belongs there and with the backups
            if self.section == Section::Overview {
                keys.push(("r", t("key-read-again")));
            }
            if matches!(self.section, Section::Overview | Section::Backups) {
                keys.push(("b", t("key-backup")));
            }
            if self.section == Section::Overview {
                // a Bluetooth companion: pair it when it is not (or lost it: the Fleet's PIN
                // box), else unpair it
                match self.pairing {
                    Some(true) => keys.push(("p", t("key-pin"))),
                    Some(false) => keys.push(("u", t("key-unpair"))),
                    None => {}
                }
                keys.push(("s", t(if self.reveal { "key-secrets-hide" } else { "key-secrets-show" })));
            }
            return keys;
        };
        if editor.input.is_some() && !editor.choices.is_empty() {
            return vec![("←→", t("key-pick")), ("Enter", t("key-ok")), ("Esc", t("key-cancel")), ("a-z", t("key-type-instead"))];
        }
        if editor.input.is_some() {
            return vec![("Enter", t("key-ok")), ("Esc", t("key-cancel"))];
        }
        // edit mode stays on the section it was opened in: no Tab here (Esc first)
        let mut keys = vec![("Esc", t("key-leave-edit")), ("↑↓", t("key-select"))];
        if self.section == Section::Contacts {
            keys.push(("n", t("key-own-name")));
            keys.push(("/", t("key-search")));
        } else if !matches!(self.section, Section::Backups | Section::Regions | Section::Access) {
            keys.push(("Enter", t("key-change")));
        }
        if self.section == Section::Access {
            keys.extend([("←→", t("key-acl-role")), ("a", t("key-acl-add")), ("x", t("key-remove"))]);
        }
        if self.section == Section::Regions {
            keys.extend([
                ("Enter", t("key-region-offer")),
                ("a", t("key-region-add")),
                ("Space", t("key-region-flood")),
                ("h", t("key-region-home")),
                ("d", t("key-region-default")),
                ("x", t("key-remove")),
            ]);
        }
        if self.section == Section::Overview {
            keys.push(("?", t("key-help")));
            keys.push(("p", t("key-presets")));
            keys.push(("s", t(if self.reveal { "key-secrets-hide" } else { "key-secrets-show" })));
        }
        if self.section == Section::Contacts {
            keys.push(("*", t("key-favourite")));
            keys.push(("Space", t("key-check")));
            keys.push(("a", t(if self.checked.is_empty() { "key-check-all" } else { "key-uncheck-all" })));
        }
        if matches!(self.section, Section::Contacts | Section::Channels) {
            keys.push(("x", t("key-remove")));
        }
        if editor.can_undo() {
            keys.push(("u", t("key-undo")));
        }
        if editor.can_redo() {
            keys.push(("U", t("key-redo")));
        }
        keys.push(("c", tf("key-commit", &[("count", editor.draft.count().into())])));
        keys
    }

    /// Secrets in full: asked for with `s`, and always in edit mode (the person is changing them)
    fn shows_secrets(&self) -> bool {
        self.reveal
    }

    /// None: close the view (back to the list). In config, a key that changed the draft (a
    /// value taken, a toggle, a preset, a region added) is remembered for `u`.
    pub fn on_key(&mut self, key: KeyEvent, ctx: &Ctx) -> Option<Action> {
        let before = self.editor.as_ref().map(|e| e.draft.clone());
        // `u` / `U` themselves move through the history: not a change to remember
        let undoing = matches!(key.code, KeyCode::Char('u' | 'U')) && !self.typing() && self.help.is_none();
        let action = self.key(key, ctx);
        if let (Some(before), Some(editor), false) = (before, &mut self.editor, undoing) {
            editor.track(before);
        }
        action
    }

    fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Option<Action> {
        let Some(node) = self.node(ctx) else {
            // unplugged: any key goes back
            return None;
        };
        if let Some(picker) = &mut self.presets {
            match picker.on_key(key) {
                PresetKey::Not => {}
                PresetKey::Closed => self.presets = None,
                PresetKey::Chosen(preset) => {
                    if let Some(editor) = &mut self.editor {
                        editor.apply_preset(node, preset);
                        // back on the radio's first setting, to see the changes
                        self.row = settings(node).iter().position(|(_, e)| is_radio(&e.id)).unwrap_or(self.row);
                    }
                    self.presets = None;
                }
            }
            return Some(Action::None);
        }
        // the access list's chooser: keys from the address book, each with the role you gave it
        if let Some(picker) = &mut self.acl_pick {
            let have = acl_have(node, self.editor.as_ref());
            match picker.on_key(key, &ctx.book, &node.public_key, &have) {
                AclPicked::Not => {}
                AclPicked::Closed => self.acl_pick = None,
                AclPicked::Add(keys) => {
                    if let Some(editor) = &mut self.editor {
                        editor.add_acl(node, &keys);
                    }
                    self.acl_pick = None;
                }
            }
            return Some(Action::None);
        }
        // the region chooser: a region of the user's list, or a new name (added to the list too)
        if let Some(picker) = &mut self.regions {
            let have: Vec<String> = node.regions.iter().flat_map(|r| r.list.iter().map(|r| r.name.clone())).collect();
            let (name, save) = match picker.on_key(key, &ctx.regions) {
                Picked::Not => return Some(Action::None),
                Picked::Closed => (None, None),
                Picked::Region(name) => (Some(name), None),
                Picked::New(name) => {
                    let mut list = ctx.regions.clone();
                    list.push(Region::new(name.clone(), None));
                    (Some(name), Some(list))
                }
            };
            self.regions = None;
            if let (Some(name), Some(editor)) = (name, &mut self.editor) {
                editor.add_region(&ctx.regions, &name, &have);
            }
            return Some(save.map_or(Action::None, |list| Action::Send(vec![Cmd::SaveRegions(list)])));
        }
        // the comparison of a backup with the node: scrolled, Esc / Enter closes it
        if let Some(scroll) = self.compare_scroll {
            self.compare_scroll = match key.code {
                KeyCode::Esc | KeyCode::Enter => None,
                KeyCode::Up => Some(scroll.saturating_sub(1)),
                KeyCode::Down => Some(scroll.saturating_add(1)),
                KeyCode::PageUp => Some(scroll.saturating_sub(10)),
                KeyCode::PageDown => Some(scroll.saturating_add(10)),
                KeyCode::Home => Some(0),
                KeyCode::End => Some(u16::MAX),
                _ => Some(scroll),
            };
            return Some(Action::None);
        }
        if let Some(scroll) = self.help {
            self.help = match key.code {
                KeyCode::Up => Some(scroll.saturating_sub(1)),
                KeyCode::Down => Some(scroll.saturating_add(1)),
                KeyCode::PageUp => Some(scroll.saturating_sub(10)),
                KeyCode::PageDown => Some(scroll.saturating_add(10)),
                // Esc, ?, Enter or anything else: closed
                _ => None,
            };
            return Some(Action::None);
        }
        if self.own_name.is_some() || self.finding {
            return Some(self.contact_typing(key));
        }
        if self.editor.is_some() {
            return Some(self.edit_key(key, node, ctx));
        }
        let sections = Section::of(node);
        let action = match key.code {
            KeyCode::Esc | KeyCode::Backspace => return None,
            // Tab goes through the sections; ← → between the panels of one (the overview's two)
            KeyCode::Tab | KeyCode::BackTab => {
                self.next_section(&sections, key.code == KeyCode::BackTab);
                Action::None
            }
            KeyCode::Left | KeyCode::Right if self.section == Section::Overview => {
                self.info_focus = key.code == KeyCode::Left;
                Action::None
            }
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
                if self.section == Section::Overview && self.info_focus =>
            {
                self.info_scroll = step(self.info_scroll, usize::MAX, key.code);
                Action::None
            }
            // a region the node has, into the user's list (with the ones above it), noting this
            // node as where it came from
            KeyCode::Char('+') if self.section == Section::Regions => {
                let Some(now) = &node.regions else { return Some(Action::None) };
                let rows = region_rows(now, None, &ctx.regions);
                let Some(row) = rows.get(self.row).filter(|r| !r.offer && !r.separator) else { return Some(Action::None) };
                let source = clean(&node.name);
                let mut list = ctx.regions.clone();
                for region in regions::with_parents(&now.list, &row.name) {
                    match list.iter_mut().find(|r| r.name == region.name) {
                        Some(known) if !known.from.contains(&source) => known.from.push(source.clone()),
                        Some(_) => {}
                        None => list.push(Region { flood: false, from: vec![source.clone()], ..region }),
                    }
                }
                if list == ctx.regions {
                    Action::None
                } else {
                    Action::Send(vec![Cmd::SaveRegions(list)])
                }
            }
            KeyCode::Char('e') if self.section != Section::Backups => self.start_editing(node),
            // only where the key bar shows them (context aware)
            KeyCode::Char('r') if self.section == Section::Overview && node.kind.is_node() && !node.probing => {
                Action::Send(vec![Cmd::Inspect(self.port.clone())])
            }
            KeyCode::Char('b') if matches!(self.section, Section::Overview | Section::Backups) => ask_backup(&[node]),
            KeyCode::Char('s') if self.section == Section::Overview => {
                self.reveal = !self.reveal;
                Action::None
            }
            // the contacts' next order; the selected contact stays selected
            KeyCode::Char('o') if self.section == Section::Contacts => {
                let key = self.contact_key(node, ctx);
                self.contact_sort = self.contact_sort.next();
                let shown = self.contacts(node, ctx);
                self.row = key.and_then(|k| shown.iter().position(|c| c.public_key == k)).unwrap_or(0);
                Action::None
            }
            // your name for the selected contact, next to the one it advertises
            KeyCode::Char('n') | KeyCode::Enter if self.section == Section::Contacts => {
                if let Some(key) = self.contact_key(node, ctx) {
                    let now = ctx.book.find(&key).map(|p| p.own_name.clone()).unwrap_or_default();
                    self.own_name = Some((key, now));
                }
                Action::None
            }
            KeyCode::Char('/') if self.section == Section::Contacts => {
                self.finding = true;
                Action::None
            }
            // a favourite: carried to the companion, so it goes through edit mode (c sends it)
            KeyCode::Char('*') if self.section == Section::Contacts => {
                let Some(key) = self.contact_key(node, ctx) else { return Some(Action::None) };
                if let Action::Ask(note) = self.start_editing(node) {
                    return Some(Action::Ask(note));
                }
                let before = self.editor.as_ref().map(|e| e.draft.clone());
                if let Some(editor) = &mut self.editor {
                    editor.toggle_favourite(node, &key);
                    if let Some(before) = before {
                        editor.track(before);
                    }
                }
                // the same contact stays selected (its place in the order may change)
                self.row = self.contacts(node, ctx).iter().position(|c| c.public_key == key).unwrap_or(0);
                Action::None
            }
            // the selected backup against this node as it is now
            KeyCode::Enter if self.section == Section::Backups => {
                let Some(snapshot) = own_backups(ctx, node).and_then(|b| b.snapshots.get(self.row)) else { return Some(Action::None) };
                self.compare_asked = true;
                compare_with(snapshot.path.clone(), self.port.clone())
            }
            // restore: a capital, on purpose (it writes to the node). Here only onto this node, as
            // it was; onto another device: the Backups page (R there)
            KeyCode::Char('R') if self.section == Section::Backups => restore_here(node, own_backups(ctx, node), self.row),
            // backups checked (Space, a), then x deletes them, asked first
            KeyCode::Char(' ') if self.section == Section::Backups => {
                if let Some(s) = own_backups(ctx, node).and_then(|b| b.snapshots.get(self.row)) {
                    match self.checked_backups.iter().position(|p| *p == s.path) {
                        Some(at) => {
                            self.checked_backups.remove(at);
                        }
                        None => self.checked_backups.push(s.path.clone()),
                    }
                }
                self.row = step(self.row, self.rows(ctx, node), KeyCode::Down);
                Action::None
            }
            KeyCode::Char('a') if self.section == Section::Backups => {
                let all: Vec<PathBuf> = own_backups(ctx, node).map(|b| b.snapshots.iter().map(|s| s.path.clone()).collect()).unwrap_or_default();
                self.checked_backups = if self.checked_backups.len() == all.len() { Vec::new() } else { all };
                Action::None
            }
            KeyCode::Char('x') | KeyCode::Delete if self.section == Section::Backups => {
                let Some(backups) = own_backups(ctx, node) else { return Some(Action::None) };
                let doomed: Vec<&Snapshot> = backups.snapshots.iter().filter(|s| self.checked_backups.contains(&s.path)).collect();
                let action = ask_delete(backups, &doomed);
                self.checked_backups.clear();
                action
            }
            code => {
                self.row = step(self.row, self.rows(ctx, node), code);
                Action::None
            }
        };
        Some(action)
    }

    fn next_section(&mut self, sections: &[Section], back: bool) {
        let at = sections.iter().position(|s| *s == self.section).unwrap_or(0);
        let next = if back { at + sections.len() - 1 } else { at + 1 };
        self.section = sections[next % sections.len()];
        self.row = 0;
    }

    /// Edit mode, for a node that answers and was read (a console node's settings come from
    /// that reading; a companion's settings are known from the start)
    fn start_editing(&mut self, node: &Node) -> Action {
        if !node.kind.is_node() || node.probing || (node.kind != Kind::Companion && !node.inspected) {
            return Action::Ask(Popup::note(t("edit-title"), vec![t("edit-read-first")]));
        }
        self.editor = Some(Editor::default());
        self.checked.clear();
        // a list (contacts, channels, access, regions) has the same rows in edit mode: what was
        // selected stays selected. The overview's row is its scroll when viewing and the setting
        // when editing: the first setting.
        if self.section == Section::Overview {
            self.row = 0;
        }
        Action::None
    }

    /// A key in edit mode
    fn edit_key(&mut self, key: KeyEvent, node: &Node, ctx: &Ctx) -> Action {
        // secrets stay dots in edit mode too, until `s` (shown on purpose, never by entering it)
        let typing = self.editor.as_ref().is_some_and(|e| e.input.is_some());
        if !typing && key.code == KeyCode::Char('s') && self.section == Section::Overview {
            self.reveal = !self.reveal;
            return Action::None;
        }
        let Some(editor) = &mut self.editor else { return Action::None };
        if editor.input.is_some() {
            editor.typing_key(key, node);
            return Action::None;
        }
        if key.code == KeyCode::Esc {
            // nothing changed: just leave; else ask, in the middle, with what would be lost
            editor.problem = None;
            return match editor.leave(node) {
                Some(question) => question,
                None => {
                    self.editor = None;
                    Action::None
                }
            };
        }
        editor.problem = None;
        // contacts: a custom name (n) goes to the address book, not to the companion (its next
        // advert would write its own name over it); search with /
        if self.section == Section::Contacts && editor.input.is_none() {
            match key.code {
                KeyCode::Char('n') => {
                    let contact = contacts_in_order(node, self.contact_sort, &self.find, &ctx.book).get(self.row).map(|c| c.public_key.clone());
                    if let Some(contact) = contact {
                        let now = ctx.book.find(&contact).map(|p| p.own_name.clone()).unwrap_or_default();
                        self.own_name = Some((contact, now));
                    }
                    return Action::None;
                }
                KeyCode::Char('/') => {
                    self.finding = true;
                    return Action::None;
                }
                _ => {}
            }
        }
        let contacts = contacts_in_order(node, self.contact_sort, &self.find, &ctx.book);
        let target = row_target(self.section, node, self.row, &contacts);
        match key.code {
            KeyCode::Char('a') if self.section == Section::Regions => self.regions = Some(RegionPicker::default()),
            // the access list: keys picked from the address book (your admins first, each with
            // the role you gave it; a whole key pasted there too), a role with ← →, out with x
            KeyCode::Char('a') if self.section == Section::Access => self.acl_pick = Some(AclPick::new(&ctx.book)),
            KeyCode::Left | KeyCode::Right | KeyCode::Char('x') | KeyCode::Delete if self.section == Section::Access => {
                let rows = acl_rows(node, Some(&editor.draft.acl));
                let Some(row) = rows.get(self.row) else { return Action::None };
                match key.code {
                    KeyCode::Left | KeyCode::Right => editor.acl_role(node, &row.key, key.code == KeyCode::Left),
                    _ => editor.toggle_remove_acl(node, &row.key),
                }
            }
            // a region of the user's list (below the line): Enter adds it, with the ones above it
            KeyCode::Enter if self.section == Section::Regions => {
                let Some(now) = &node.regions else { return Action::None };
                let rows = region_rows(now, Some(&editor.draft.regions), &ctx.regions);
                if let Some(row) = rows.get(self.row).filter(|r| r.offer) {
                    let have: Vec<String> = now.list.iter().map(|r| r.name.clone()).collect();
                    editor.add_region(&ctx.regions, &row.name, &have);
                }
            }
            KeyCode::Char(' ' | 'h' | 'd' | 'x') if self.section == Section::Regions => {
                let Some(now) = &node.regions else { return Action::None };
                let rows = region_rows(now, Some(&editor.draft.regions), &ctx.regions);
                let Some(row) = rows.get(self.row).filter(|r| !r.offer && !r.separator) else { return Action::None };
                let root = row.name == regions::ROOT;
                let was = if root { Some(now.root_flood) } else { now.list.iter().find(|r| r.name == row.name).map(|r| r.flood) };
                match key.code {
                    KeyCode::Char(' ') => editor.toggle_flood(&row.name, was),
                    // the root is not a region to be home, default or removed: only its flood
                    _ if root => editor.problem = Some(t("regions-root-flood-only")),
                    KeyCode::Char('h') => editor.set_home(&row.name, now.home.as_ref()),
                    KeyCode::Char('d') => editor.toggle_default(&row.name, now.default.as_ref()),
                    _ => {
                        let mut list = now.list.clone();
                        list.extend(editor.draft.regions.add.iter().cloned());
                        let mut under = regions::with_children(&list, &row.name);
                        under.retain(|n| *n != row.name);
                        editor.toggle_remove_region(&row.name, &under);
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(target) = target {
                    editor.start(target, node);
                }
            }
            KeyCode::Char('*') => {
                if let Some(Target::Contact(key)) = &target {
                    editor.toggle_favourite(node, key);
                }
            }
            // contacts checked (Space): x takes them all out at once (the commit lists each)
            KeyCode::Char(' ') if self.section == Section::Contacts => {
                if let Some(Target::Contact(key)) = &target {
                    match self.checked.iter().position(|k| k == key) {
                        Some(at) => {
                            self.checked.remove(at);
                        }
                        None => self.checked.push(key.clone()),
                    }
                    self.row = (self.row + 1).min(contacts.len().saturating_sub(1));
                }
            }
            // every contact shown checked, or none
            KeyCode::Char('a') if self.section == Section::Contacts => {
                if contacts.iter().all(|c| self.checked.contains(&c.public_key)) {
                    self.checked.clear();
                } else {
                    self.checked = contacts.iter().map(|c| c.public_key.clone()).collect();
                }
            }
            KeyCode::Char('x') | KeyCode::Delete if self.section == Section::Contacts && !self.checked.is_empty() => {
                editor.remove_contacts(&self.checked);
                self.checked.clear();
            }
            KeyCode::Char('x') | KeyCode::Delete => match &target {
                Some(Target::Contact(key)) => editor.toggle_remove_contact(key),
                Some(Target::Channel(slot)) => editor.toggle_remove_channel(*slot),
                _ => {}
            },
            // back / forward through the changes, one at a time, whatever they were
            KeyCode::Char('u') => editor.undo_last(),
            KeyCode::Char('U') => editor.redo_last(),
            KeyCode::Char('c') => return editor.commit(node),
            KeyCode::Char('?') if matches!(target, Some(Target::Setting(_))) => self.help = Some(0),
            // the radio presets: opened on the one the node has now
            KeyCode::Char('p') if self.section == Section::Overview && settings(node).iter().any(|(_, e)| is_radio(&e.id)) => {
                self.presets = Some(PresetPicker::on(node));
            }
            code => self.row = step(self.row, self.rows(ctx, node), code),
        }
        Action::None
    }

    /// How many rows the open section has (the overview in edit mode: its settings)
    fn rows(&self, ctx: &Ctx, node: &Node) -> usize {
        match self.section {
            Section::Overview if self.editor.is_some() => settings(node).len(),
            Section::Overview => overview_lines(node),
            Section::Contacts => self.contacts(node, ctx).len(),
            Section::Channels => node.channels.len(),
            Section::Regions => {
                let edits = self.editor.as_ref().map(|e| &e.draft.regions);
                node.regions.as_ref().map_or(0, |now| region_rows(now, edits, &ctx.regions).len())
            }
            Section::Access => acl_rows(node, self.editor.as_ref().map(|e| &e.draft.acl)).len(),
            Section::Backups => own_backups(ctx, node).map_or(0, |b| b.snapshots.len()),
        }
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        let Some(node) = self.node(ctx) else {
            let text = Paragraph::new(t("node-gone")).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel(&t("node-title"))), area);
            return;
        };
        self.pairing = pairing(node);
        // a node read again can turn out to be another kind (another firmware was flashed)
        let sections = Section::of(node);
        if !sections.contains(&self.section) {
            self.section = Section::Overview;
        }
        self.row = self.row.min(self.rows(ctx, node).saturating_sub(1));

        // two lines of who it is, a blank line, the sections, then the open one
        let [head, state, _, tabs, body] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);
        // each line cut with "…" when the window is narrow; the second one leaves the room its
        // right part (the job, edit mode's keys) needs, so the two never overlap
        let width = area.width as usize;
        let (who, about) = self.head_lines(node);
        let right = self.state_line(ctx, node);
        let left_room = width.saturating_sub(right.width() + 2);
        f.render_widget(Paragraph::new(ellipsis(who, width)), head);
        f.render_widget(Paragraph::new(ellipsis(about, left_room)), state);
        f.render_widget(Paragraph::new(ellipsis(right, width)).right_aligned(), state);
        f.render_widget(Paragraph::new(ellipsis(self.tabs_line(ctx, node, &sections), width)), tabs);
        match self.section {
            Section::Overview => self.draw_overview(f, body, ctx, node),
            Section::Contacts => self.draw_contacts(f, body, ctx, node),
            Section::Channels => self.draw_channels(f, body, ctx, node),
            Section::Regions => self.draw_regions(f, body, ctx, node),
            Section::Access => self.draw_access(f, body, ctx, node),
            Section::Backups => self.draw_backups(f, body, ctx, node),
        }
        if let Some(picker) = &self.regions {
            let have: Vec<String> = node.regions.iter().flat_map(|r| r.list.iter().map(|r| r.name.clone())).collect();
            picker.draw(f, area, &ctx.regions, &have);
        }
        if let Some(picker) = &self.acl_pick {
            picker.draw(f, area, &ctx.book, &node.public_key, &acl_have(node, self.editor.as_ref()));
        }
        if let Some(scroll) = self.help {
            if let Some((fact, edit)) = settings(node).get(self.row) {
                // kept at its end: Down past it, then Up, moves at once
                self.help = Some(draw_help(f, area, node, fact, edit, scroll, self.shows_secrets()));
            }
        }
        if let Some(picker) = &self.presets {
            draw_presets(f, area, node, picker);
        }
        // a comparison this view asked for (Enter on a backup): opens by itself when it comes
        if let Some((number, report)) = &ctx.compared {
            if *number != self.compare_seen {
                self.compare_seen = *number;
                if std::mem::take(&mut self.compare_asked) {
                    self.compare_scroll = Some(0);
                }
            }
            if let Some(scroll) = self.compare_scroll {
                let width = READING.min(area.width.saturating_sub(8));
                self.compare_scroll = Some(widgets::scroll_sheet(f, area, &t("compare-title"), width, report_lines(report), scroll, &t("compare-close")));
            }
        }
    }

    /// " MyNode  [repeater]          [EDIT]" and, dimmed, "board · firmware · port"
    fn head_lines(&self, node: &Node) -> (Line<'static>, Line<'static>) {
        // a node that gave no name: its port
        let name = if node.name.is_empty() { serial::short(&node.port.dev) } else { clean(&node.name) };
        let mut who = vec![Span::styled(format!(" {name}  "), theme::heading())];
        // the start of its key, as in the lists
        who.extend(widgets::key_prefix(&node.public_key).spans.into_iter().map(|s| Span::styled(format!("{}  ", s.content), s.style)));
        who.push(Span::styled(format!(" {} ", node.kind.label()), theme::kind_badge(node.kind)));
        // in edit mode a two-part pill: " EDIT " and how many changes wait
        if let Some(editor) = &self.editor {
            let count = editor.draft.count();
            who.push(Span::raw("   "));
            who.push(Span::styled(format!(" {} ", t("edit-badge")), theme::edit_badge()));
            let pending = format!(" {} ", tf("edit-pending", &[("count", count.into())]));
            who.push(Span::styled(pending, if count == 0 { theme::field() } else { theme::edit_count() }));
        }
        let mut about = vec![node.board()];
        if !node.firmware.is_empty() {
            about.push(node.firmware.clone());
        }
        let id = if node.port.is_ble() { "node-link-ble" } else { "node-link-usb" };
        let link = tf(id, &[("name", node.port.name().into())]);
        about.push(link);
        (Line::from(who), Line::styled(format!(" {}", about.join("  ·  ")), theme::dim()))
    }

    /// Right of the second header line: edit mode's next keys, else the running job once the node
    /// is on screen. (A refused key or value is shown where the user is looking: next to the
    /// setting or under the list, `problem_line`.)
    fn state_line(&self, ctx: &Ctx, node: &Node) -> Line<'static> {
        // edit mode: its keys are in the key bar (each once, for the section it is in), its count
        // in the pill: nothing more here
        if self.editor.is_some() {
            return Line::raw("");
        }
        // a job: here when the view already shows the node (a backup, writing a commit, reading
        // it again); before the first reading the panels show it instead (`loading`), and
        // "not read yet" is said by the panels too
        // after it: how it ended ("Backup saved", or what went wrong), a few seconds, same place
        match (ctx.job_on(&node.port), ctx.ended_on(&node.port)) {
            (Some(job), _) if node.inspected => Line::styled(format!("{} {} ", spinner(), job_text(job)), theme::title()),
            (None, Some(end)) => {
                let mark = if end.level == Level::Bad { "×" } else { "√" };
                Line::styled(format!("{mark} {} ", end.text), theme::level(end.level).add_modifier(Modifier::BOLD))
            }
            _ => Line::raw(""),
        }
    }

    /// " Overview   Contacts 42   Channels 3   Backups 5 ", the open one marked
    fn tabs_line(&self, ctx: &Ctx, node: &Node, sections: &[Section]) -> Line<'static> {
        let mut spans = Vec::new();
        for section in sections {
            let (id, count) = match section {
                Section::Overview => ("node-overview", None),
                Section::Contacts => ("node-contacts", node.inspected.then_some(node.contacts.len())),
                Section::Channels => ("node-channels", node.inspected.then_some(node.channels.len())),
                Section::Regions => ("node-regions", node.regions.as_ref().map(|r| r.list.len())),
                Section::Access => ("acl-title", node.acl.as_ref().map(Vec::len)),
                Section::Backups => ("node-backups", Some(own_backups(ctx, node).map_or(0, |b| b.snapshots.len()))),
            };
            let text = match count {
                Some(n) => format!(" {} {n} ", t(id)),
                None => format!(" {} ", t(id)),
            };
            let style = if *section == self.section { theme::section_open() } else { theme::dim() };
            spans.push(Span::raw("  "));
            spans.push(Span::styled(text, style));
        }
        Line::from(spans)
    }

    fn draw_overview(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx, node: &Node) {
        if self.editor.is_some() {
            return self.draw_editing(f, area, node);
        }
        let mut pairs = identity(node);
        pairs.push(match own_backups(ctx, node).and_then(|b| b.snapshots.first().map(|s| (s, b.snapshots.len()))) {
            Some((last, count)) => fact(
                t("field-last-backup"),
                tf("node-last-backup", &[("stamp", last.shown().into()), ("count", count.into())]),
            ),
            None if node.kind.is_node() => (t("field-last-backup"), Line::styled(t("node-never-backed-up"), theme::warn())),
            None => fact(t("field-last-backup"), "-".into()),
        });
        // Info as wide as it needs side by side, up to half; what it says gets the rest. A narrow
        // window: one panel at a time, all the width (two squeezed halves cut every line)
        let narrow = area.width < NARROW;
        let (left, right) = if narrow {
            (area, area)
        } else {
            let want = facts_width(&pairs) as u16 + PANEL_EDGES;
            let [left, right] = Layout::horizontal([Constraint::Length(want.min(area.width / 2)), Constraint::Min(0)]).areas(area);
            (left, right)
        };
        // ← gives it the keyboard (thick border), ↑↓ then scroll it; → goes back to the right one
        if !narrow || self.info_focus {
            let lines = identity_lines(node, pairs, inner_width(left));
            let rows = text_rows(&lines, inner_width(left));
            let shown = left.height.saturating_sub(2) as usize;
            self.info_scroll = self.info_scroll.min(scroll_max(&lines, inner_width(left), shown));
            let text = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((self.info_scroll as u16, 0));
            f.render_widget(text.block(panel_focus(&t("node-identity"), self.info_focus)), left);
            more_marks(f, left, self.info_scroll, shown, rows);
            if narrow {
                return;
            }
        }

        // being read for the first time: the answer comes here, so the progress shows here
        if let (Some(job), false) = (ctx.job_on(&node.port), node.inspected) {
            return loading(f, right, &t("node-told"), job);
        }
        // settings first (what can be changed), then what it only tells; ↑↓ scroll
        // (inside: borders and padding)
        let inner = inner_width(right);
        let all: Vec<&Fact> = node.facts.iter().collect();
        let width = label_width(all.iter().map(|f| f.label.as_str()));
        // every line "label  value", or (narrow) every label above its value: all alike
        let stacked = stacks(&all, inner);
        let set = settings(node);
        let info: Vec<&Fact> = node.facts.iter().filter(|f| f.edit.is_none()).collect();
        let mut lines: Vec<Line> = Vec::new();
        if node.facts.is_empty() {
            // a Bluetooth companion not connected yet: why is on the left, `p` pairs
            let id = match (node.kind.is_node(), node.port.is_ble()) {
                (_, true) => "node-ble-not-connected",
                (true, false) => "node-not-read",
                (false, false) => "node-nothing-to-read",
            };
            lines.push(Line::styled(t(id), theme::dim()));
        }
        if !set.is_empty() {
            lines.push(rule(&t("node-settings"), inner));
            for (fact, edit) in &set {
                lines.extend(self.setting_lines(fact, edit, width, false, stacked, inner));
            }
        }
        if !info.is_empty() {
            lines.push(rule(&t("node-read-only"), inner));
            for fact in &info {
                let value = vec![value_span(fact, self.shows_secrets())];
                let label = Span::styled(fact.label.clone(), theme::dim());
                lines.extend(pair_lines(Span::raw("  "), label, value, width, stacked, inner));
            }
        }
        // scrolled no farther than its end: the last line stays at the bottom, no empty rows
        let shown = right.height.saturating_sub(2) as usize;
        self.row = self.row.min(scroll_max(&lines, inner, shown));
        let rows = text_rows(&lines, inner);
        let text = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((self.row as u16, 0));
        f.render_widget(text.block(panel_focus(&t("node-told"), !self.info_focus)), right);
        more_marks(f, right, self.row, shown, rows);
    }

    /// Edit mode's overview: the settings on one side (one line each), the selected one explained
    /// on the other (below it when the window is narrow). What is read only is left out.
    fn draw_editing(&mut self, f: &mut Frame, area: Rect, node: &Node) {
        // side by side when the explanation gets a reading width beside the list, else below it
        let (list, side) = if area.width >= READING + READING / 2 {
            let [list, side] = Layout::horizontal([Constraint::Min(0), Constraint::Percentage(40)]).areas(area);
            (list, side)
        } else {
            let [list, side] = Layout::vertical([Constraint::Min(5), Constraint::Percentage(45)]).areas(area);
            (list, side)
        };
        let set = settings(node);
        let facts_of: Vec<&Fact> = set.iter().map(|(f, _)| *f).collect();
        let width = label_width(facts_of.iter().map(|f| f.label.as_str()));
        // one line each, or (narrow) each label above its value, as in the overview
        let stacked = stacks(&facts_of, inner_width(list));
        let mut lines: Vec<Line> = Vec::new();
        let mut selected_end = 0;
        for (i, (fact, edit)) in set.iter().enumerate() {
            lines.extend(self.setting_lines(fact, edit, width, i == self.row, stacked, inner_width(list)));
            if i == self.row {
                selected_end = lines.len();
            }
        }
        // the selected setting stays in sight
        let height = list.height.saturating_sub(2) as usize;
        let scroll = selected_end.saturating_sub(height);
        let total = lines.len();
        let text = Paragraph::new(lines).scroll((scroll as u16, 0));
        f.render_widget(text.block(panel_focus(&t("node-edit-settings"), true)), list);
        more_marks(f, list, scroll, height, total);

        let Some((fact, edit)) = set.get(self.row) else { return };
        let editor = self.editor.as_ref();
        let typing = editor.and_then(|e| e.input.as_ref()).is_some();
        let picking = typing && editor.is_some_and(|e| !e.choices.is_empty());
        let rules = help::find(&edit.id, node.kind == Kind::Companion);

        // the name, its id when that differs, what it does
        let title = rules.map_or_else(|| fact.label.clone(), |r| r.title.clone());
        let mut lines = vec![Line::styled(title.clone(), theme::heading())];
        if title != edit.id {
            lines.push(Line::styled(edit.id.clone(), theme::dim()));
        }
        let summary = rules.map_or_else(|| edit.about.clone(), |r| r.summary.clone());
        if !summary.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::raw(summary));
        }

        // what it takes, what it is, what it will be
        // (`fact` here is the setting: the "label  value" helper is `widgets::fact`)
        let mut pairs: Vec<widgets::Fact> = Vec::new();
        let accepted = match rules {
            Some(r) if !r.choices().is_empty() => r.choices().join(" · "),
            Some(r) if !r.range().is_empty() => r.range(),
            // a text with a limit: the limit, said before it is reached
            Some(r) if r.kind == "text" && r.max_bytes.is_some() => {
                tf("edit-up-to", &[("max", r.max_bytes.unwrap_or_default().into())])
            }
            _ => edit.hint.clone(),
        };
        if !accepted.is_empty() {
            pairs.push(widgets::fact(t("edit-values"), accepted));
        }
        // laid out with the secret hidden (then shown: edit mode shows secrets), so a long one
        // goes under its label alone and the rest does not move
        let now = pairs.len();
        pairs.push((t("edit-now"), Line::from(value_span(fact, false))));
        if let Some(new) = editor.and_then(|e| e.draft.values.get(&edit.id)) {
            pairs.push((t("edit-new"), Line::styled(masked(fact, new, self.shows_secrets()), theme::changed())));
        }
        if let Some(default) = rules.map(|r| r.default.clone()).filter(|d| !d.is_empty()) {
            pairs.push((t("edit-default"), Line::styled(default, theme::dim())));
        }
        if is_radio(&edit.id) {
            let names = presets::names(node);
            let line = if names.is_empty() { Line::styled(t("presets-none"), theme::dim()) } else { Line::raw(names) };
            pairs.push((t("edit-preset"), line));
        }
        lines.push(Line::raw(""));
        let room = inner_width(side);
        let stacked = facts_width(&pairs) > room;
        pairs[now].1 = Line::from(value_span(fact, self.shows_secrets()));
        lines.extend(facts_in(pairs, room, stacked));

        if edit.danger || rules.is_some_and(|r| !r.warning.is_empty()) {
            let warning = rules.map(|r| r.warning.clone()).filter(|w| !w.is_empty()).unwrap_or_else(|| t("edit-danger-long"));
            lines.push(Line::raw(""));
            lines.push(Line::styled(format!("! {warning}"), theme::warn()));
        }

        // why the last key or value was refused, right here, then the keys at the bottom
        let hint = match (picking, typing) {
            (true, _) => "edit-pick-hint",
            (false, true) => "edit-typing-hint",
            _ if is_radio(&edit.id) => "edit-radio-hint",
            _ => "edit-select-hint",
        };
        let block = panel(&t("edit-selected"));
        let inner = block.inner(side);
        f.render_widget(block, side);
        let [text, problem, keys] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(3), Constraint::Length(2)]).areas(inner);
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), text);
        f.render_widget(Paragraph::new(self.problem_line()).wrap(Wrap { trim: false }), problem);
        f.render_widget(Paragraph::new(Line::styled(t(hint), theme::dim())).wrap(Wrap { trim: false }), keys);
    }

    /// Why the last key or value was refused, in red; else, while typing a value with a limit,
    /// how much room is left ("" when neither)
    fn problem_line(&self) -> Line<'static> {
        let editor = self.editor.as_ref();
        match (editor.and_then(|e| e.problem.clone()), editor.and_then(Editor::room)) {
            (Some(problem), _) => Line::styled(format!("× {problem}"), theme::bad()),
            (None, Some((used, max))) => room_line(used, max),
            (None, None) => Line::raw(""),
        }
    }

    /// One setting: "! label   value" (or, `stacked`, the label above its value), "old → new"
    /// when changed, the text being typed. `!` = it changes how the node transmits.
    fn setting_lines(
        &self,
        fact: &Fact,
        edit: &Editable,
        width: usize,
        selected: bool,
        stacked: bool,
        room: usize,
    ) -> Vec<Line<'static>> {
        let mark = Span::styled(if edit.danger { "! " } else { "  " }, theme::warn());
        // the selected setting: its label in full colour (grey on the grey bar would vanish)
        let label = Span::styled(fact.label.clone(), if selected { theme::heading() } else { theme::dim() });
        let mut spans = Vec::new();
        let typing = self
            .editor
            .as_ref()
            .and_then(|e| e.input.as_ref())
            .filter(|(target, _)| *target == Target::Setting(edit.id.clone()));
        let drafted = self.editor.as_ref().and_then(|e| e.draft.values.get(&edit.id));
        let choices = self.editor.as_ref().map_or(&[][..], |e| e.choices.as_slice());
        if let (Some((_, text)), false) = (typing, choices.is_empty()) {
            // the picker: every value, the chosen one marked
            for choice in choices {
                let style = if choice.eq_ignore_ascii_case(text) { theme::edit_badge() } else { theme::field() };
                spans.push(Span::styled(format!(" {choice} "), style));
                spans.push(Span::raw(" "));
            }
        } else if let Some((_, text)) = typing {
            // the box as long as the longest value the setting takes, when that is known
            // (a key: two hex digits per byte)
            let rules = self.editor.as_ref().and_then(|e| e.rules);
            let per_byte = if rules.is_some_and(|r| r.kind == "key") { 2 } else { 1 };
            let room = rules.and_then(|r| r.max_bytes).map_or(NAME_MAX, |b| b * per_byte);
            spans.extend(input(&masked(fact, text, self.shows_secrets()), room + 2));
        } else if let Some(new) = drafted {
            spans.push(value_span(fact, self.shows_secrets()).patch_style(theme::dim()));
            spans.push(Span::styled(" → ", theme::dim()));
            spans.push(Span::styled(masked(fact, new, self.shows_secrets()), theme::changed()));
        } else {
            spans.push(value_span(fact, self.shows_secrets()));
        }
        let lines = pair_lines(mark, label, spans, width, stacked, room);
        if selected && typing.is_none() {
            lines.into_iter().map(|l| l.style(theme::selected())).collect()
        } else {
            lines
        }
    }

    fn draw_contacts(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx, node: &Node) {
        if !node.inspected || node.contacts.is_empty() {
            return not_there(f, area, ctx, node, "node-contacts", "node-no-contacts");
        }
        let shown = self.contacts(node, ctx);
        let mut title = tf("node-contacts-sorted", &[("count", node.contacts.len().into()), ("sort", self.contact_sort.word().into())]);
        // a custom name being typed: said above the list, for which contact, with the text (it
        // is typed in that contact's row too)
        let area = if let Some((key, text)) = &self.own_name {
            let name = node.contacts.iter().find(|c| &c.public_key == key).map(|c| clean(&c.name)).unwrap_or_default();
            let mut spans = vec![Span::styled(format!("{}  ", tf("node-own-name-for", &[("name", name.into())])), theme::heading())];
            spans.extend(input(text, OWN_NAME_MAX + 1));
            let [line, rest] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
            f.render_widget(Paragraph::new(Line::from(spans)), line);
            rest
        } else {
            area
        };
        // the search, above the list while typed or kept
        let area = if self.finding || !self.find.is_empty() {
            title = tf("node-contacts-found", &[("title", title.into()), ("count", shown.len().into())]);
            let mut spans = vec![Span::styled(format!("{}  ", t("key-search")), theme::heading())];
            spans.extend(if self.finding { input(&self.find, 24) } else { vec![Span::styled(self.find.clone(), theme::title())] });
            let [line, rest] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
            f.render_widget(Paragraph::new(Line::from(spans)), line);
            rest
        } else {
            area
        };
        if shown.is_empty() {
            let text = Paragraph::new(t("node-contacts-none-found")).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel_focus(&title, true)), area);
            return;
        }
        // the selected contact below (as high as it needs), the list, the line for a refused value
        let c = shown[self.row.min(shown.len() - 1)];
        let person = ctx.book.find(&c.public_key);
        let position = if c.lat != 0.0 || c.lon != 0.0 { format!("{:.5}, {:.5}", c.lat, c.lon) } else { "-".into() };
        let mut details = vec![
            fact(t("field-public-key"), c.public_key.clone()),
            fact(t("field-heard"), format!("{}  ({})", when(c.last_advert), ago(c.last_advert))),
            fact(t("field-position"), position),
            fact(t("field-route"), route(c)),
        ];
        // what the address book knows more: the other companions that hold it. Always there
        // ("-" when none), so the list does not jump up and down from one contact to the next
        let others: Vec<String> = person
            .map(|p| p.held_by.iter().filter(|h| !h.node.eq_ignore_ascii_case(&node.public_key)).map(|h| clean(&h.node_name)).collect())
            .unwrap_or_default();
        details.push(fact(t("field-also-on"), if others.is_empty() { "-".into() } else { others.join(", ") }));
        let lines = facts(details, inner_width(area));
        let above = details_below(f, area, &clean(&c.name), lines, 5);
        let [list, problem] = Layout::vertical([Constraint::Min(4), Constraint::Length(1)]).areas(above);
        f.render_widget(Paragraph::new(self.problem_line()), problem);
        let editor = self.editor.as_ref();
        let checking = editor.is_some() && !self.checked.is_empty();
        let rows = shown.iter().map(|c| {
            let draft = editor.and_then(|e| e.draft.contacts.get(&c.public_key));
            let favourite = draft.and_then(|d| d.favourite).unwrap_or(c.favourite);
            // ★ a favourite (amber while changed in edit mode); the time, position and route
            // are in the selected contact's details below
            let fav_style = if draft.and_then(|d| d.favourite).is_some() { theme::changed() } else { theme::warn() };
            let removed = draft.is_some_and(|d| d.remove);
            let kind = if removed { Line::styled(t("edit-removed"), theme::bad()) } else { contact_kind(c.kind) };
            // your name, in its own column next to the advertised one (typed here with n)
            let own = match &self.own_name {
                Some((key, text)) if *key == c.public_key => Line::from(widgets::input_cell(text, OWN_NAME_MAX + 1)),
                _ => Line::styled(ctx.book.find(&c.public_key).map(|p| clean(&p.own_name)).unwrap_or_default(), theme::title()),
            };
            // ★, type, key, then the names: what it is first, as the Contacts page has it; the
            // [x] boxes first once one is checked
            let mut cells = vec![
                Line::styled(if favourite { "★" } else { " " }, fav_style),
                kind,
                widgets::key_prefix(&c.public_key),
                name_cell(None, draft.and_then(|d| d.name.clone()), &c.name),
                own,
            ];
            if checking {
                let mark = if self.checked.contains(&c.public_key) { Line::styled("[x]", theme::bad()) } else { Line::styled("[ ]", theme::dim()) };
                cells.insert(0, mark);
            }
            (cells, if removed { theme::removed() } else { Style::new() })
        });
        let mut cols = vec![
            Col::new(String::new()),
            Col::new(t("col-type")),
            Col::new(t("col-key")).drop(1),
            Col::new(t("col-name")).grow(),
            Col::new(t("col-own-name")).grow(),
        ];
        if checking {
            cols.insert(0, Col::new(String::new()));
            title = tf("node-contacts-checked", &[("title", title.into()), ("count", self.checked.len().into())]);
        }
        let grid = Grid::new(cols)
        .styled(rows);
        self.table.select(Some(self.row.min(shown.len() - 1)));
        grid.show(f, list, panel_focus(&title, true), &mut self.table);
    }

    fn draw_channels(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx, node: &Node) {
        if !node.inspected || node.channels.is_empty() {
            return not_there(f, area, ctx, node, "node-channels", "node-no-channels");
        }
        let editor = self.editor.as_ref();
        let rows = node.channels.iter().map(|c| channel_row(c, editor));
        let grid = Grid::new(vec![Col::new(t("col-slot")), Col::new(t("col-name")).grow(), Col::new(String::new())])
            .styled(rows);
        let title = tf("node-channels-title", &[("count", node.channels.len().into())]);
        // a refused name or the room left while typing one, else: channel keys are secrets,
        // only in backups, never on screen
        let busy = self.editor.as_ref().is_some_and(|e| e.problem.is_some() || e.input.is_some());
        let line = if busy { self.problem_line() } else { Line::styled(t("node-channel-keys"), theme::dim()) };
        let list = note_below(f, area, line);
        self.table.select(Some(self.row));
        grid.show(f, list, panel_focus(&title, true), &mut self.table);
    }

    /// The node's regions as a tree: flood allowed or not, its home and default scope, and
    /// whether each is in the user's list (`+` copies it there)
    fn draw_regions(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx, node: &Node) {
        let Some(now) = &node.regions else {
            return not_there(f, area, ctx, node, "node-regions", "node-no-regions");
        };
        let rows = region_rows(now, self.editor.as_ref().map(|e| &e.draft.regions), &ctx.regions);
        let editing = self.editor.is_some();
        // below: a cut reply (the list may be short), else a refused key; the keys themselves
        // are in the bottom line, not repeated here
        let line = if now.cut {
            Some(Line::styled(format!("! {}", t("regions-cut")), theme::warn()))
        } else if self.editor.as_ref().is_some_and(|e| e.problem.is_some()) {
            Some(self.problem_line())
        } else {
            None
        };
        let list = match line {
            Some(line) => note_below(f, area, line),
            None => area,
        };
        let title = tf("node-regions-title", &[("count", now.list.len().into())]);
        if rows.is_empty() {
            let text = Paragraph::new(t("node-no-regions")).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel_focus(&title, true)), list);
        } else {
            self.row = self.row.min(rows.len() - 1);
            let cells = rows.iter().map(|r| {
                let blank = || Line::raw("");
                if r.separator {
                    let line = Line::styled(format!("── {} ──", t("regions-offer-line")), theme::dim());
                    return (vec![line, blank(), blank(), blank()], Style::new());
                }
                let name = format!("{}{}", "  ".repeat(r.depth), r.name);
                // the user's region the node does not have: dimmed, Enter in config adds it
                if r.offer {
                    let how = if editing { Line::styled(t("regions-offer-add"), theme::dim()) } else { blank() };
                    return (vec![Line::styled(name, theme::dim()), blank(), blank(), how], Style::new());
                }
                if r.name == regions::ROOT {
                    let name = Line::from(vec![Span::raw(name), Span::styled(format!("  {}", t("regions-root")), theme::dim())]);
                    let (word, style) = if r.flood { ("regions-flood", theme::good()) } else { ("regions-no-flood", theme::bad()) };
                    let mark = if r.flood_changed { Span::styled("→ ", theme::changed()) } else { Span::raw("  ") };
                    return (vec![name, Line::from(vec![mark, Span::styled(t(word), style)]), blank(), blank()], Style::new());
                }
                let name = if r.added { Line::styled(name, theme::changed()) } else { Line::raw(name) };
                // allowed in green, blocked in red; "→" in amber when config changes it
                let (word, style) = if r.flood { ("regions-flood", theme::good()) } else { ("regions-no-flood", theme::bad()) };
                let mark = if r.flood_changed || r.added { Span::styled("→ ", theme::changed()) } else { Span::raw("  ") };
                let flood = Line::from(vec![mark, Span::styled(t(word), style)]);
                let mut marks = Vec::new();
                if r.home {
                    marks.push(t("regions-home"));
                }
                if r.default {
                    marks.push(t("regions-default"));
                }
                let marks = Line::styled(marks.join(", "), if r.marks_changed { theme::changed() } else { theme::title() });
                // in the user's list (√, after `+`) or not yet
                let state = if r.removed {
                    Line::styled(t("edit-removed"), theme::bad())
                } else if ctx.regions.iter().any(|l| l.name == r.name) {
                    Line::styled(format!("√ {}", t("regions-in-list")), theme::good())
                } else {
                    Line::styled(t("regions-not-in-list"), theme::dim())
                };
                (vec![name, flood, marks, state], if r.removed { theme::removed() } else { Style::new() })
            });
            let grid = Grid::new(vec![
                Col::new(t("col-name")).grow(),
                Col::new(t("col-flood")),
                Col::new(String::new()).drop(2),
                Col::new(String::new()).drop(1),
            ])
            .styled(cells);
            self.table.select(Some(self.row));
            grid.show(f, list, panel_focus(&title, true), &mut self.table);
        }
    }

    /// The access list: each public key with its role, what edit mode changes ("→ admin",
    /// removed, added; added ones are picked from the address book, `a`), and below it what the
    /// list is
    fn draw_access(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx, node: &Node) {
        let Some(now) = &node.acl else {
            return not_there(f, area, ctx, node, "acl-title", "acl-none");
        };
        let editor = self.editor.as_ref();
        let rows = acl_rows(node, editor.map(|e| &e.draft.acl));
        // below: a refused change (the list full), else what the list is
        let mut below = Vec::new();
        if editor.is_some_and(|e| e.problem.is_some()) {
            below.push(self.problem_line());
        } else {
            below.push(Line::styled(tf("acl-about", &[("max", acl::MAX.into())]), theme::dim()));
        }
        let list = details_below(f, area, &t("acl-title"), below, 4);
        let title = tf("acl-count", &[("count", now.len().into()), ("max", acl::MAX.into())]);
        if rows.is_empty() {
            let id = if self.editor.is_some() { "acl-empty-edit" } else { "acl-empty" };
            let text = Paragraph::new(t(id)).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel_focus(&title, true)), list);
            return;
        }
        self.row = self.row.min(rows.len() - 1);
        let cells = rows.iter().map(|r| {
            let role_style = |perms: u8| if acl::role_of(perms) == acl::ADMIN { theme::warn() } else { Style::new() };
            let role = match (r.now, r.after) {
                // out of the list (x)
                (_, None) => Line::styled(t("edit-removed"), theme::bad()),
                (Some(was), Some(is)) if was != is => Line::from(vec![
                    Span::styled(acl::role_name(was), theme::dim()),
                    Span::styled(" → ", theme::dim()),
                    Span::styled(acl::role_name(is), theme::changed()),
                ]),
                (None, Some(is)) => Line::from(vec![
                    Span::styled(acl::role_name(is), theme::changed()),
                    Span::styled(format!("  ({})", t("acl-added")), theme::dim()),
                ]),
                (_, Some(is)) => Line::styled(acl::role_name(is), role_style(is)),
            };
            let key = if r.now.is_none() { Line::styled(r.key.clone(), theme::changed()) } else { Line::raw(r.key.clone()) };
            // who it is, from the address book: your name for it, else the name it advertises
            let who = ctx.book.find(&r.key).map_or_else(String::new, |p| if p.own_name.is_empty() && p.name.is_empty() { String::new() } else { clean(&p.shown_name()) });
            (vec![Line::styled(who, theme::title()), key, role], if r.after.is_none() { theme::removed() } else { Style::new() })
        });
        let grid = Grid::new(vec![Col::new(t("col-name")), Col::new(t("col-key")).grow(), Col::new(t("acl-role"))]).styled(cells);
        self.table.select(Some(self.row));
        grid.show(f, list, panel_focus(&title, true), &mut self.table);
    }

    fn draw_backups(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx, node: &Node) {
        let Some(backups) = own_backups(ctx, node).filter(|b| !b.snapshots.is_empty()) else {
            let text = Paragraph::new(t("node-no-backups")).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel(&t("node-backups"))), area);
            return;
        };
        let manifest = backups.snapshots.get(self.row).and_then(|s| s.manifest.as_ref());
        let list = details_below(f, area, &t("backups-details"), describe(manifest, inner_width(area)), 4);
        let title = tf("node-backups-title", &[("count", backups.snapshots.len().into())]);
        self.table.select(Some(self.row));
        // the [x] boxes once one is checked
        let checked = (!self.checked_backups.is_empty()).then_some(self.checked_backups.as_slice());
        snapshots_table(&backups.snapshots, checked).show(f, list, panel_focus(&title, true), &mut self.table);
    }
}

/// The help of one setting, over the view (`?` in edit mode): what it is, every value it takes
/// and what each does, its range, default, the firmware that added it, the notes, the warning.
/// From locales/<code>/settings_help.toml (src/help.rs); a setting without an entry shows the
/// command list's own line. Returns the scroll as drawn (no farther than the end).
fn draw_help(f: &mut Frame, area: Rect, node: &Node, fact: &Fact, edit: &Editable, scroll: u16, reveal: bool) -> u16 {
    // the text's width first: the box is then as high as the text (up to the page)
    let width = READING.min(area.width.saturating_sub(8));
    let rules = help::find(&edit.id, node.kind == Kind::Companion);
    let mut lines: Vec<Line> = Vec::new();
    let title = rules.map_or_else(|| fact.label.clone(), |r| r.title.clone());
    lines.push(Line::from(vec![
        Span::styled(title, theme::heading()),
        Span::styled(format!("   {}", edit.id), theme::dim()),
    ]));
    lines.push(Line::raw(""));
    let Some(rules) = rules else {
        lines.push(Line::raw(tf("help-none", &[("about", edit.about.as_str().into())])));
        return show_help(f, area, width, lines, scroll);
    };
    if !rules.summary.is_empty() {
        lines.push(Line::raw(rules.summary.clone()));
        lines.push(Line::raw(""));
    }

    // the values: each one, with what it does; the current one marked
    if !rules.values.is_empty() {
        lines.push(Line::styled(t("help-values"), theme::title()));
        let current = rules.editable(&fact.value);
        let width = rules.values.iter().map(|v| v.value.chars().count()).max().unwrap_or(0) + 3;
        for v in &rules.values {
            let now = v.value.eq_ignore_ascii_case(&current);
            let mark = if now { "● " } else { "  " };
            let style = if now { theme::heading() } else { Style::new() };
            lines.push(Line::from(vec![
                Span::styled(mark, theme::title()),
                Span::styled(format!("{:<width$}", v.value), style),
                Span::styled(v.means.clone(), theme::dim()),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // the facts, aligned (`fact` here is the setting: the helper is `widgets::fact`)
    let mut pairs: Vec<widgets::Fact> = Vec::new();
    if !rules.range().is_empty() {
        pairs.push(widgets::fact(t("edit-range"), rules.range()));
    }
    // laid out with a secret hidden, then shown: a long one goes under its label alone
    let now = pairs.len();
    pairs.push((t("edit-now"), Line::from(value_span(fact, false))));
    if !rules.default.is_empty() {
        pairs.push(widgets::fact(t("edit-default"), rules.default.clone()));
    }
    if !rules.since.is_empty() {
        pairs.push(widgets::fact(t("edit-since"), rules.since.clone()));
    }
    let room = width.saturating_sub(PANEL_EDGES) as usize;
    let stacked = facts_width(&pairs) > room;
    pairs[now].1 = Line::from(value_span(fact, reveal));
    lines.extend(facts_in(pairs, room, stacked));

    for note in &rules.notes {
        lines.push(Line::raw(""));
        lines.push(Line::raw(note.clone()));
    }
    if !rules.warning.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(format!("! {}", rules.warning), theme::warn()));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(t("help-source"), theme::dim()));
    show_help(f, area, width, lines, scroll)
}

/// The help box (the shared scrolling sheet)
fn show_help(f: &mut Frame, area: Rect, width: u16, lines: Vec<Line<'static>>, scroll: u16) -> u16 {
    scroll_sheet(f, area, &t("key-help"), width, lines, scroll, &t("help-close"))
}

/// "mark label   value" on one line, the label padded to `width`, a value too long for the line
/// wrapped under itself (`widgets::hanging`); or, `stacked` (a very narrow panel), the mark and
/// label on a line and the value under it, indented past the mark.
pub(super) fn pair_lines(
    mark: Span<'static>,
    label: Span<'static>,
    value: Vec<Span<'static>>,
    width: usize,
    stacked: bool,
    room: usize,
) -> Vec<Line<'static>> {
    if stacked {
        let mut lines = vec![Line::from(vec![mark, label])];
        lines.extend(widgets::hanging(vec![Span::raw("  ")], value, 2, room));
        return lines;
    }
    let padded = Span::styled(format!("{:<width$}", label.content), label.style);
    widgets::hanging(vec![mark, padded], value, 2 + width, room)
}

/// Whether facts must be stacked in `room` cells: the mark and the label column leave the values
/// too little room (`widgets::must_stack`). One long value never decides it: it wraps in its
/// column. All of a panel go the same way, whatever the secrets show.
pub(super) fn stacks(facts: &[&Fact], room: usize) -> bool {
    widgets::must_stack(2 + label_width(facts.iter().map(|f| f.label.as_str())), room)
}

/// A fact's value as shown: a secret as dots until `s` shows it (then in its own colour), one
/// the node cannot tell (the admin password) said so
pub(super) fn value_span(fact: &Fact, reveal: bool) -> Span<'static> {
    match (fact.secret, reveal, fact.value.is_empty()) {
        (false, _, _) if fact.updated => Span::styled(fact.value.clone(), theme::good()),
        (false, _, _) => Span::raw(fact.value.clone()),
        (true, false, _) => Span::styled("••••••", theme::dim()),
        (true, true, true) => Span::styled(t("fact-unreadable"), theme::dim()),
        (true, true, false) => Span::styled(fact.value.clone(), theme::secret()),
    }
}

/// Text typed for a fact as shown: a secret as one dot per character, unless `s` shows them
pub(super) fn masked(fact: &Fact, text: &str, reveal: bool) -> String {
    if fact.secret && !reveal {
        "•".repeat(text.chars().count())
    } else {
        text.to_string()
    }
}

/// One of the radio's settings (a preset sets them)
pub(super) fn is_radio(id: &str) -> bool {
    [node_edit::FREQ, node_edit::BW, node_edit::SF, node_edit::CR, node_edit::CONSOLE_RADIO].contains(&id)
}

/// The radio preset chooser (`p` in edit mode), over the view: the presets as the official apps
/// offer them, narrowed by what was typed, the node's own marked. Choosing one only prepares the
/// changes: they are seen, and sent with the commit.
pub(super) fn draw_presets(f: &mut Frame, area: Rect, node: &Node, picker: &PresetPicker) {
    let shown = picker.shown();
    let now = presets::matching(node);
    let rows = shown.iter().map(|p| {
        let mark = if now.iter().any(|m| m.title == p.title) { "●" } else { " " };
        let ids = p.path_hash_size.map(|s| tf("presets-ids", &[("size", s.into())])).unwrap_or_default();
        vec![
            Line::styled(mark, theme::title()),
            Line::raw(p.title.clone()),
            Line::styled(p.describe(), theme::dim()),
            Line::styled(ids, theme::dim()),
        ]
    });
    let grid = Grid::new(vec![Col::new(String::new()), Col::new(String::new()), Col::new(String::new()).grow(), Col::new(String::new()).drop(1)])
        .rows(rows);
    // as wide as the list (at least a reading width, for the note), as high as it is
    let want = (grid.width() as u16 + PANEL_EDGES).max(READING);
    let rect = sheet(area, want, shown.len() as u16 + 9);
    crate::widgets::lift(f, rect);
    let block = panel_focus(&t("presets-title"), true);
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let [find, _, list, _, note] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(inner);

    // what was typed, to narrow the list: the box across the line
    let label = format!("{}  ", t("presets-find"));
    let mut find_line = vec![Span::styled(label.clone(), theme::dim())];
    find_line.extend(input(&picker.filter, (find.width as usize).saturating_sub(label.chars().count())));
    f.render_widget(Paragraph::new(Line::from(find_line)), find);

    let mut state = TableState::default().with_selected(Some(picker.row.min(shown.len().saturating_sub(1))));
    if shown.is_empty() {
        f.render_widget(Paragraph::new(Line::styled(t("presets-no-match"), theme::dim())), list);
    } else {
        f.render_stateful_widget(grid.table(list.width as usize), list, &mut state);
    }
    f.render_widget(Paragraph::new(Line::styled(t("presets-note"), theme::dim())).wrap(Wrap { trim: false }), note);
}

/// The settings a node lets change, in its order
pub(super) fn settings(node: &Node) -> Vec<(&Fact, &Editable)> {
    node.facts.iter().filter_map(|f| f.edit.as_ref().map(|e| (f, e))).collect()
}

/// How many lines the overview's right panel has (for scrolling): facts and the two rules
fn overview_lines(node: &Node) -> usize {
    node.facts.len() + 2
}

/// What a row of a section is, for edit mode (`contacts`: the rows of the Contacts section)
fn row_target(section: Section, node: &Node, row: usize, contacts: &[&Contact]) -> Option<Target> {
    match section {
        Section::Overview => settings(node).get(row).map(|(_, e)| Target::Setting(e.id.clone())),
        Section::Contacts => contacts.get(row).map(|c| Target::Contact(c.public_key.clone())),
        Section::Channels => node.channels.get(row).map(|c| Target::Channel(c.slot)),
        // regions and the access list have keys of their own (edit_key), no value to type
        Section::Regions | Section::Access | Section::Backups => None,
    }
}

/// One entry of the access list, as it will be
struct AclRow {
    key: String,
    /// Its permissions on the node now (None: added in edit mode)
    now: Option<u8>,
    /// After the commit (None: removed)
    after: Option<u8>,
}

/// The node's access list with edit mode's changes (`changes`: key -> new permissions, 0 =
/// removed), then the keys added there
/// The keys in the access list now, with the ones added in edit mode (not offered again)
fn acl_have(node: &Node, editor: Option<&Editor>) -> Vec<String> {
    let mut have: Vec<String> = node.acl.iter().flatten().map(|e| e.key.clone()).collect();
    if let Some(editor) = editor {
        have.extend(editor.draft.acl.keys().cloned());
    }
    have
}

fn acl_rows(node: &Node, changes: Option<&BTreeMap<String, u8>>) -> Vec<AclRow> {
    let empty = BTreeMap::new();
    let changes = changes.unwrap_or(&empty);
    let now = node.acl.as_deref().unwrap_or(&[]);
    let after = |key: &str, had: Option<u8>| match changes.get(key) {
        Some(0) => None,
        Some(perms) => Some(*perms),
        None => had,
    };
    let mut rows: Vec<AclRow> =
        now.iter().map(|e| AclRow { key: e.key.clone(), now: Some(e.perms), after: after(&e.key, Some(e.perms)) }).collect();
    for (key, perms) in changes.iter().filter(|(k, p)| **p != 0 && !now.iter().any(|e| &e.key == *k)) {
        rows.push(AclRow { key: key.clone(), now: None, after: Some(*perms) });
    }
    rows
}

/// One region in the node's Regions section, with what edit mode does to it
#[derive(Default)]
struct RegionRow {
    /// The line between the node's regions and the ones of the user's list it does not have
    separator: bool,
    /// Below that line: a region of the user's list the node does not have (Enter in config
    /// adds it)
    offer: bool,
    depth: usize,
    name: String,
    /// Flood allowed, after the changes
    flood: bool,
    flood_changed: bool,
    /// Added in edit mode (not on the node yet)
    added: bool,
    /// Removed in edit mode (itself, or a region above it)
    removed: bool,
    home: bool,
    default: bool,
    /// Home / default scope moved to this region (or away from it) in edit mode
    marks_changed: bool,
}

/// The node's regions as they will be (what it has, what is added, each one's state), then,
/// after a separator, the regions of the user's list (`library`) it does not have
fn region_rows(now: &NodeRegions, edits: Option<&node_edit::RegionEdits>, library: &[Region]) -> Vec<RegionRow> {
    let empty = node_edit::RegionEdits::default();
    let edits = edits.unwrap_or(&empty);
    let mut list = now.list.clone();
    list.extend(edits.add.iter().filter(|a| !now.list.iter().any(|r| r.name == a.name)).cloned());
    let gone: Vec<String> = edits.remove.iter().flat_map(|name| regions::with_children(&list, name)).collect();
    let home = edits.home.as_ref().or(now.home.as_ref());
    let default = edits.default.as_ref().map_or(now.default.as_ref(), |d| d.as_ref());
    // first `*`, the root (packets with no region): only its flood can change
    let root_flood = edits.flood.get(regions::ROOT).copied();
    let root = RegionRow {
        name: regions::ROOT.to_string(),
        flood: root_flood.unwrap_or(now.root_flood),
        flood_changed: root_flood.is_some(),
        ..RegionRow::default()
    };
    let tree = regions::ordered(&list).into_iter().map(|(depth, r)| {
            let added = !now.list.iter().any(|n| n.name == r.name);
            let changed = edits.flood.get(&r.name).copied();
            let is = |mark: Option<&String>| mark == Some(&r.name);
            let home_moved = edits.home.is_some() && is(home) != is(now.home.as_ref());
            let default_moved = edits.default.is_some() && is(default) != is(now.default.as_ref());
            RegionRow {
                // under the root
                depth: depth + 1,
                name: r.name.clone(),
                flood: changed.unwrap_or(r.flood),
                flood_changed: changed.is_some(),
                added,
                removed: gone.contains(&r.name),
                home: is(home),
                default: is(default),
                marks_changed: home_moved || default_moved,
                ..RegionRow::default()
            }
        });
    let mut rows: Vec<RegionRow> = std::iter::once(root).chain(tree).collect();
    // the user's regions this node does not have (nor gets in this config): ready to add
    let offered: Vec<RegionRow> = regions::ordered(library)
        .into_iter()
        .filter(|(_, r)| !list.iter().any(|n| n.name == r.name))
        .map(|(depth, r)| RegionRow { depth: depth + 1, name: r.name.clone(), offer: true, ..RegionRow::default() })
        .collect();
    if !offered.is_empty() {
        rows.push(RegionRow { separator: true, ..RegionRow::default() });
        rows.extend(offered);
    }
    rows
}

/// "── Settings ──────────" across `width`
fn rule(title: &str, width: usize) -> Line<'static> {
    let head = format!("── {title} ");
    let rest = width.saturating_sub(head.chars().count());
    Line::styled(format!("{head}{}", "─".repeat(rest)), theme::dim())
}

/// A channel, with what edit mode changes on it: its cells and the row's style
fn channel_row(c: &Channel, editor: Option<&Editor>) -> (Vec<Line<'static>>, Style) {
    let draft = editor.and_then(|e| e.draft.channels.get(&c.slot));
    let typing = editor
        .and_then(|e| e.input.as_ref())
        .filter(|(target, _)| *target == Target::Channel(c.slot))
        .map(|(_, text)| text.clone());
    let removed = draft.is_some_and(|d| d.remove);
    let state = if removed { Line::styled(t("edit-removed"), theme::bad()) } else { Line::raw("") };
    let cells = vec![Line::raw(c.slot.to_string()), name_cell(typing, draft.and_then(|d| d.name.clone()), &c.name), state];
    (cells, if removed { theme::removed() } else { Style::new() })
}

/// A contact's or channel's name: the box being typed in (as long as a name can be), the new
/// name (changed), or the name
fn name_cell(typing: Option<String>, new: Option<String>, name: &str) -> Line<'static> {
    match (typing, new) {
        (Some(text), _) => Line::from(widgets::input_cell(&text, NAME_MAX + 1)),
        (None, Some(new)) => Line::styled(clean(&new), theme::changed()),
        (None, None) => Line::raw(clean(name)),
    }
}

/// The backups of this very board (USB serial number, else public key), whatever its name was
fn own_backups<'a>(ctx: &'a Ctx, node: &Node) -> Option<&'a NodeBackups> {
    ctx.backups.iter().find(|b| b.is_board(&node.port.serial_number, &node.public_key))
}

/// Restoring backup `row` of this node onto it (its own board: as it was). The worker reads both
/// and asks before anything is written.
fn restore_here(node: &Node, backups: Option<&NodeBackups>, row: usize) -> Action {
    let Some(snapshot) = backups.and_then(|b| b.snapshots.get(row)) else { return Action::None };
    let note = |id: &str| Action::Ask(Popup::note(t("restore-title"), vec![t(id)]));
    match &snapshot.manifest {
        Some(_) if node.kind.is_node() && !node.probing => ask_restore(snapshot.path.clone(), node.port.clone(), Mode::Same),
        Some(_) => note("restore-no-target"),
        None => note("backup-unreadable"),
    }
}

/// A list section that has nothing to show: being read (the loader), not read, or empty
fn not_there(f: &mut Frame, area: Rect, ctx: &Ctx, node: &Node, title: &str, empty: &str) {
    if let (Some(job), false) = (ctx.job_on(&node.port), node.inspected) {
        return loading(f, area, &t(title), job);
    }
    let text = t(if node.inspected { empty } else { "node-not-read" });
    f.render_widget(Paragraph::new(text).style(theme::dim()).wrap(Wrap { trim: false }).block(panel(&t(title))), area);
}

/// A panel whose content is on its way: the moving dots, what is being done, in its middle
pub(super) fn loading(f: &mut Frame, area: Rect, title: &str, job: &Job) {
    let block = panel(title);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines = vec![
        Line::styled(spinner(), theme::title()),
        Line::raw(""),
        Line::styled(job_text(job), theme::heading()),
        Line::styled(t("node-reading-hint"), theme::dim()),
    ];
    let top = inner.height.saturating_sub(lines.len() as u16) / 2;
    let rect = Rect { y: inner.y + top, height: inner.height.saturating_sub(top), ..inner };
    f.render_widget(Paragraph::new(lines).centered().wrap(Wrap { trim: false }), rect);
}

/// A contact's type (ADV_TYPE_* in the firmware), as a label and the kind its colour comes from
fn contact_type(kind: u8) -> (String, Kind) {
    match kind {
        1 => (t("contact-chat"), Kind::Companion),
        2 => (t("contact-repeater"), Kind::Repeater),
        3 => (t("contact-room"), Kind::RoomServer),
        4 => (t("contact-sensor"), Kind::Sensor),
        _ => (t("contact-other"), Kind::Unknown),
    }
}

/// "direct", "3 hops", "flood"
fn route(c: &Contact) -> String {
    match c.hops {
        None => t("route-flood"),
        Some(0) => t("route-direct"),
        Some(n) => tf("route-hops", &[("n", n.into())]),
    }
}

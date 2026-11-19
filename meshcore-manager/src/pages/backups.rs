//! Backups: on the left the nodes that have backups, on the right the backups of the selected
//! node by date (newest first), below what the selected backup holds. A node is one board (its
//! USB serial number, else its key): renaming it does not split its backups.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, TableState, Wrap};
use ratatui::Frame;

use std::path::PathBuf;

use super::{step, Only, Page};
use crate::app::{Action, Confirm, Ctx, Popup};
use crate::backup::compare::{Report, Tone};
use crate::backup::diff::Mode;
use crate::backup::restore::{self, Request};
use crate::backup::store::{self, NodeBackups, Snapshot};
use crate::backup::{Manifest, Reason, FORMAT};
use crate::devices;
use crate::i18n::{t, tf};
use crate::node::Node;
use crate::serial::Port;
use crate::theme;
use crate::worker::Cmd;
use crate::widgets::{
    clean, details_below, fact, facts, inner_width, key_prefix, panel, panel_focus, scroll_sheet, Col, Grid, PANEL_EDGES, READING,
};

#[derive(Default)]
pub struct Backups {
    node: usize,
    snapshot: usize,
    /// The keyboard is in the list of backups (right), not in the list of nodes (left)
    in_snapshots: bool,
    node_table: TableState,
    snapshot_table: TableState,
    /// Which kind of node is listed (`f`)
    only: Only,
    /// The restore questions being answered
    pick: Pick,
    /// The backups of the selected node checked (Space): to delete, or two to compare
    checked: Vec<PathBuf>,
    /// The comparison on screen, by its scroll (None: closed)
    compare_scroll: Option<u16>,
    /// The number of the last comparison opened (`Ctx::compared`): a new one opens by itself
    compare_seen: u64,
    /// This page asked for a comparison (`c`, Enter): only then does a new one open here (a
    /// node's view asks for its own)
    compare_asked: bool,
}

impl Backups {
    /// The nodes in the list: those of the kind chosen with `f`
    fn shown<'a>(&self, ctx: &'a Ctx) -> Vec<&'a NodeBackups> {
        ctx.backups.iter().filter(|b| self.only.shows(b.kind)).collect()
    }

    fn selected<'a>(&self, ctx: &'a Ctx) -> Option<&'a Snapshot> {
        self.shown(ctx).get(self.node)?.snapshots.get(self.snapshot)
    }
}

impl Page for Backups {
    fn title(&self) -> String {
        t("tab-backups")
    }

    fn keys(&self) -> Vec<(&'static str, String)> {
        if self.compare_scroll.is_some() {
            return vec![("↑↓", t("key-scroll")), ("Esc", t("key-close"))];
        }
        // only what acts where the focus is: the boards (left) open their backups; a backup
        // (right) is compared, restored, checked
        let mut keys = vec![("↑↓", t("key-select")), ("←→", t("key-panel"))];
        if self.in_snapshots {
            keys.push(("Enter", t("key-compare-node")));
            keys.push(("R", t("key-restore")));
            let compare = if self.checked.len() == 2 { t("key-compare-checked") } else { t("key-compare") };
            keys.push(("c", compare));
            keys.push(("Space", t("key-check")));
            keys.push(("a", t(if self.checked.is_empty() { "key-check-all" } else { "key-uncheck-all" })));
        } else {
            keys.push(("Enter", t("key-its-backups")));
        }
        // "x" is red in the key bar: it deletes. Among the backups it deletes the checked ones
        // only (Space first): shown once one is checked
        match (self.in_snapshots, self.checked.len()) {
            (false, _) => keys.push(("x", t("key-delete-node-backups"))),
            (true, 0) => {}
            (true, count) => keys.push(("x", tf("key-delete-n", &[("count", count.into())]))),
        }
        if !self.checked.is_empty() {
            keys.push(("Esc", t("key-uncheck")));
        }
        keys.push(self.only.key());
        keys
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        let shown = self.shown(ctx);
        if shown.is_empty() {
            let empty = if ctx.backups.is_empty() { "backups-empty" } else { "only-none" };
            let text = Paragraph::new(t(empty)).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel(&self.only.title(t("backups-nodes")))), area);
            return;
        }
        self.node = self.node.min(shown.len() - 1);
        let node = shown[self.node];
        self.snapshot = self.snapshot.min(node.snapshots.len().saturating_sub(1));
        // the nodes as wide as their names (up to two fifths), the backups get the rest
        // every board ever backed up (the backups are on disk): ● the ones in the Fleet now; one
        // that is gone (broken, lost) still has its backups, to restore onto another board
        let rows = shown.iter().map(|n| {
            let kind = match n.kind {
                Some(kind) => Line::styled(kind.label(), theme::kind(kind)),
                None => Line::raw("-"),
            };
            let key = n.id.as_ref().map_or_else(|| Line::raw(""), |id| key_prefix(&id.public_key));
            let here = if board_here(n, ctx) { Line::styled("●", theme::good()) } else { Line::styled("·", theme::dim()) };
            vec![here, key, Line::raw(clean(&n.name)), kind, Line::raw(n.snapshots.len().to_string())]
        });
        let grid = Grid::new(vec![
            Col::new(String::new()),
            Col::new(String::new()),
            Col::new(String::new()).grow(),
            Col::new(String::new()),
            Col::new(String::new()),
        ])
        .rows(rows);
        let want = grid.width() as u16 + PANEL_EDGES;
        let [left, right] = Layout::horizontal([Constraint::Length(want.min(area.width * 2 / 5)), Constraint::Min(0)]).areas(area);
        self.node_table.select(Some(self.node));
        grid.show(f, left, panel_focus(&self.only.title(t("backups-nodes")), !self.in_snapshots), &mut self.node_table);

        let manifest = self.selected(ctx).and_then(|s| s.manifest.as_ref());
        let lines = describe(manifest, inner_width(right));
        let list = details_below(f, right, &t("backups-details"), lines, 4);
        // a backup deleted meanwhile is no longer checked
        self.checked.retain(|p| node.snapshots.iter().any(|s| &s.path == p));
        // a board not here now: its backups can still go onto another one (R)
        let id = if board_here(node, ctx) { "backups-of" } else { "backups-of-gone" };
        let mut title = tf(id, &[("name", clean(&node.name).into())]);
        if !self.checked.is_empty() {
            title = tf("fleet-checked", &[("title", title.into()), ("count", self.checked.len().into())]);
        }
        self.snapshot_table.select(Some(self.snapshot));
        snapshots_table(&node.snapshots, Some(&self.checked)).show(f, list, panel_focus(&title, self.in_snapshots), &mut self.snapshot_table);

        // a comparison this page asked for: it opens by itself when it comes, over the page
        if let Some((number, report)) = &ctx.compared {
            if *number != self.compare_seen {
                self.compare_seen = *number;
                if std::mem::take(&mut self.compare_asked) {
                    self.compare_scroll = Some(0);
                }
            }
            if let Some(scroll) = self.compare_scroll {
                let width = READING.min(area.width.saturating_sub(8));
                let title = t("compare-title");
                self.compare_scroll = Some(scroll_sheet(f, area, &title, width, report_lines(report), scroll, &t("compare-close")));
            }
        }
    }

    /// The comparison is open: its keys only, Esc closes it
    fn owns_keys(&self) -> bool {
        self.compare_scroll.is_some()
    }

    fn on_key(&mut self, key: KeyEvent, ctx: &Ctx) -> Action {
        if let Some(scroll) = self.compare_scroll {
            self.compare_scroll = match key.code {
                KeyCode::Esc | KeyCode::Enter => None,
                KeyCode::Up => Some(scroll.saturating_sub(1)),
                KeyCode::Down => Some(scroll.saturating_add(1)),
                KeyCode::PageUp => Some(scroll.saturating_sub(10)),
                KeyCode::PageDown => Some(scroll.saturating_add(10)),
                KeyCode::Home => Some(0),
                // drawn clamped at the end
                KeyCode::End => Some(u16::MAX),
                _ => Some(scroll),
            };
            return Action::None;
        }
        match key.code {
            KeyCode::Char('c') if self.in_snapshots => {
                let action = self.compare(ctx);
                self.compare_asked |= matches!(action, Action::Send(_));
                return action;
            }
            KeyCode::Left => self.in_snapshots = false,
            KeyCode::Right => self.in_snapshots = true,
            // a board (left): its backups
            KeyCode::Enter if !self.in_snapshots => self.in_snapshots = true,
            // the backup against the node as it is now (its own board, plugged in or near)
            KeyCode::Enter => {
                let action = self.compare_now(ctx);
                self.compare_asked |= matches!(action, Action::Send(_));
                return action;
            }
            // restore: a capital, on purpose (it writes to a node)
            KeyCode::Char('R') if self.in_snapshots => return self.restore(ctx),
            KeyCode::Char('x') => return self.delete(ctx),
            KeyCode::Esc => self.checked.clear(),
            KeyCode::Char(' ') if self.in_snapshots => {
                if let Some(path) = self.selected(ctx).map(|s| s.path.clone()) {
                    match self.checked.iter().position(|p| *p == path) {
                        Some(at) => {
                            self.checked.remove(at);
                        }
                        None => self.checked.push(path),
                    }
                }
            }
            KeyCode::Char('a') if self.in_snapshots => {
                let all: Vec<PathBuf> =
                    self.shown(ctx).get(self.node).map(|n| n.snapshots.iter().map(|s| s.path.clone()).collect()).unwrap_or_default();
                self.checked = if self.checked.len() == all.len() { Vec::new() } else { all };
            }
            KeyCode::Char('f') => {
                self.only = self.only.next();
                self.node = 0;
                self.snapshot = 0;
                self.checked.clear();
            }
            code if self.in_snapshots => {
                let len = self.shown(ctx).get(self.node).map_or(0, |n| n.snapshots.len());
                self.snapshot = step(self.snapshot, len, code);
            }
            code => {
                let node = step(self.node, self.shown(ctx).len(), code);
                if node != self.node {
                    self.node = node;
                    self.snapshot = 0;
                    self.checked.clear();
                }
            }
        }
        Action::None
    }

    /// The deletion was confirmed: nothing is checked any more
    fn confirmed(&mut self) {
        self.checked.clear();
    }

    /// An answer to the restore questions: the node, then (another board) move or copy
    fn chosen(&mut self, choice: usize, _ctx: &Ctx) -> Action {
        match std::mem::take(&mut self.pick) {
            Pick::Target(backup, targets) => {
                let Some((port, own)) = targets.get(choice).cloned() else { return Action::None };
                if own {
                    return ask_restore(backup, port, Mode::Same);
                }
                let lines = vec![t("restore-choose-mode"), t("restore-move-means"), t("restore-copy-means")];
                let mut popup = Popup::note(t("restore-title"), lines);
                popup.buttons = vec![
                    (t("restore-choose-move"), Some(Confirm::Choose(0))),
                    (t("restore-choose-copy"), Some(Confirm::Choose(1))),
                    (t("answer-cancel"), None),
                ];
                // copy is the one that takes nothing from the other board: the focus starts there
                popup.focus = 1;
                self.pick = Pick::Mode(backup, port);
                Action::Ask(popup)
            }
            Pick::Mode(backup, port) => match MODES.get(choice) {
                Some(mode) => ask_restore(backup, port, *mode),
                None => Action::None,
            },
            Pick::None => Action::None,
        }
    }
}

/// The backups of one board by date, newest first (also in a node's own view). `checked`: a
/// [x] column first, for the ones checked to delete (the Backups page only).
pub(super) fn snapshots_table(snapshots: &[Snapshot], checked: Option<&[PathBuf]>) -> Grid {
    let rows = snapshots.iter().map(|s| {
        let mut row = match &s.manifest {
            Some(m) => {
                let items: usize = m.sections.iter().map(|s| s.items).sum();
                // verified (read back from the disk), complete (an older version's), partial
                let state = match (m.verified, m.complete) {
                    // a newer program's layout: kept, shown, not restored here
                    _ if m.format > FORMAT => Line::styled(t("backup-newer"), theme::warn()),
                    (true, _) => Line::styled(format!("√ {}", t("backup-verified")), theme::good()),
                    (false, true) => Line::styled(t("backup-complete"), theme::good()),
                    (false, false) => Line::styled(t("backup-partial"), theme::warn()),
                };
                vec![
                    Line::raw(s.shown()),
                    Line::raw(m.reason.text()),
                    Line::raw(m.node.firmware.clone()),
                    Line::raw(tf("backup-contents", &[("sections", m.sections.len().into()), ("items", items.into())])),
                    state,
                ]
            }
            None => vec![
                Line::raw(s.shown()),
                Line::raw(""),
                Line::raw(""),
                Line::raw(""),
                Line::styled(t("backup-unreadable"), theme::bad()),
            ],
        };
        if let Some(checked) = checked {
            let mark = if checked.contains(&s.path) { Line::styled("[x]", theme::bad()) } else { Line::styled("[ ]", theme::dim()) };
            row.insert(0, mark);
        }
        row
    });
    // narrow: the contents go first, then the firmware, then the reason
    let mut cols = vec![
        Col::new(t("col-date")),
        Col::new(t("col-reason")).drop(1),
        Col::new(t("col-firmware")).drop(2),
        Col::new(t("col-contents")).grow().drop(3),
        Col::new(t("col-state")),
    ];
    if checked.is_some() {
        cols.insert(0, Col::new(String::new()));
    }
    Grid::new(cols).rows(rows)
}

impl Backups {
    /// Compares two backups of the node: the two checked ones, else the selected one with the one
    /// made before it. The worker reads them (each file checked) and the result opens here.
    fn compare(&self, ctx: &Ctx) -> Action {
        let note = |id: &str| Action::Ask(Popup::note(t("compare-title"), vec![t(id)]));
        let Some(node) = self.shown(ctx).get(self.node).copied() else { return Action::None };
        // newest first in the list: the higher index is the older one
        let at = |path: &PathBuf| node.snapshots.iter().position(|s| &s.path == path);
        let (newer, older) = match self.checked.as_slice() {
            [a, b] => match (at(a), at(b)) {
                (Some(i), Some(j)) => (i.min(j), i.max(j)),
                _ => return Action::None,
            },
            [] if self.snapshot + 1 < node.snapshots.len() => (self.snapshot, self.snapshot + 1),
            [] => return note("compare-no-older"),
            _ => return note("compare-pick-two"),
        };
        let path = |i: usize| node.snapshots[i].path.clone();
        Action::Send(vec![Cmd::CompareBackups { older: path(older), newer: path(newer) }])
    }

    /// The selected backup against its own board as it is now (read, nothing written)
    fn compare_now(&self, ctx: &Ctx) -> Action {
        let note = |id: &str| Action::Ask(Popup::note(t("compare-title"), vec![t(id)]));
        let Some(snapshot) = self.selected(ctx) else { return Action::None };
        let Some(manifest) = &snapshot.manifest else { return note("backup-unreadable") };
        match ctx.nodes.iter().find(|n| n.kind.is_node() && !n.probing && restore::is_own(manifest, n)) {
            Some(node) => compare_with(snapshot.path.clone(), node.port.clone()),
            None => note("compare-node-absent"),
        }
    }
}

/// Compares a backup with the node on `port` as it is now (also from a node's own view)
pub(super) fn compare_with(backup: PathBuf, port: Port) -> Action {
    Action::Send(vec![Cmd::CompareNode { backup, port }])
}

/// The comparison as lines: what was compared, then each change with its mark and colour
pub(super) fn report_lines(report: &Report) -> Vec<Line<'static>> {
    let head = tf(
        "compare-head",
        &[("name", clean(&report.name).into()), ("older", report.older.as_str().into()), ("newer", report.newer.as_str().into())],
    );
    let mut lines = vec![Line::styled(head, theme::heading())];
    let changes = &report.comparison;
    lines.push(Line::styled(tf("compare-count", &[("count", changes.changes.into())]), theme::dim()));
    for (tone, text) in &changes.lines {
        let line = match tone {
            Tone::Heading => {
                lines.push(Line::raw(""));
                Line::styled(text.clone(), theme::title())
            }
            Tone::Added => Line::styled(format!("  + {text}"), theme::good()),
            Tone::Removed => Line::styled(format!("  - {text}"), theme::bad()),
            Tone::Changed => Line::styled(format!("  ~ {text}"), theme::changed()),
            Tone::Note => Line::styled(format!("  ! {text}"), theme::dim()),
        };
        lines.push(line);
    }
    lines
}

/// The most backups a delete question names one by one (the rest counted)
const DELETE_SHOWN: usize = 10;

/// Reasons that mark a backup as the way back to before a change
const WAY_BACK: [Reason; 4] = [Reason::BeforeRestore, Reason::BeforeConfig, Reason::BeforeFirmware, Reason::BeforeReset];

impl Backups {
    /// Asks before deleting: the checked backups (none checked: nothing, Space first); in the
    /// nodes panel every backup of the node
    fn delete(&self, ctx: &Ctx) -> Action {
        let Some(node) = self.shown(ctx).get(self.node).copied() else { return Action::None };
        let doomed: Vec<&Snapshot> = if !self.in_snapshots {
            node.snapshots.iter().collect()
        } else {
            node.snapshots.iter().filter(|s| self.checked.contains(&s.path)).collect()
        };
        ask_delete(node, &doomed)
    }
}

/// The board of these backups is in the Fleet now (plugged in, or a Bluetooth companion heard)
pub(super) fn board_here(backups: &NodeBackups, ctx: &Ctx) -> bool {
    ctx.nodes.iter().any(|n| n.kind.is_node() && backups.is_board(&n.port.serial_number, &n.public_key))
}

/// The question before deleting backups of one board (the Backups page, and a node's own
/// Backups section): each one named, what is lost with them. Red, the focus on "cancel"; more
/// than one is asked a second time.
pub(super) fn ask_delete(node: &NodeBackups, doomed: &[&Snapshot]) -> Action {
        if doomed.is_empty() {
            return Action::None;
        }
        let count = doomed.len();
        let name = clean(&node.name);
        let mut lines = vec![tf("delete-ask", &[("count", count.into()), ("name", name.as_str().into())])];
        for s in doomed.iter().take(DELETE_SHOWN) {
            let why = s.manifest.as_ref().map(|m| m.reason.text()).unwrap_or_else(|| t("backup-unreadable"));
            lines.push(format!("  · {}  {why}", s.shown()));
        }
        if count > DELETE_SHOWN {
            lines.push(tf("restore-more", &[("count", (count - DELETE_SHOWN).into())]));
        }
        lines.push(String::new());
        // what is lost with them
        let verified = |s: &&Snapshot| s.manifest.as_ref().is_some_and(|m| m.verified);
        let left = node.snapshots.len() - count;
        if left == 0 {
            lines.push(tf("delete-none-left", &[("name", name.as_str().into())]));
        } else if doomed.iter().any(verified) && !node.snapshots.iter().filter(|s| !doomed.iter().any(|d| d.path == s.path)).any(|s| verified(&s)) {
            lines.push(t("delete-no-verified-left"));
        }
        if doomed.iter().any(|s| s.manifest.as_ref().is_some_and(|m| WAY_BACK.contains(&m.reason))) {
            lines.push(t("delete-way-back"));
        }
        lines.push(t("delete-cannot-undo"));
        let run = Confirm::Send(vec![Cmd::DeleteBackups(doomed.iter().map(|s| s.path.clone()).collect())]);
        let confirm = if count > 1 {
            let sure = tf("delete-sure", &[("count", count.into()), ("name", name.as_str().into())]);
            let again = Popup::ask(t("delete-title"), vec![sure], run, tf("delete-yes", &[("count", count.into())]), t("answer-cancel"));
            Confirm::Ask(Box::new(again.safe()))
        } else {
            run
        };
        Action::Ask(Popup::ask(t("delete-title"), lines, confirm, tf("delete-yes", &[("count", count.into())]), t("answer-cancel")).safe())
}

impl Backups {
    /// Restoring the selected backup, step by step: onto which node (the nodes of its kind that
    /// answer, its own board first), then how when it is another board (move / copy). The
    /// worker then reads both and asks the last question (app.rs `restore_question`).
    fn restore(&mut self, ctx: &Ctx) -> Action {
        let note = |id: &str| Action::Ask(Popup::note(t("restore-title"), vec![t(id)]));
        let Some(snapshot) = self.selected(ctx) else { return Action::None };
        let Some(manifest) = &snapshot.manifest else { return note("backup-unreadable") };
        let mut targets: Vec<&Node> =
            ctx.nodes.iter().filter(|n| n.kind == manifest.node.kind && n.kind.is_node() && !n.probing).collect();
        if targets.is_empty() {
            return note("restore-no-target");
        }
        // its own board first: the usual case
        targets.sort_by_key(|n| !restore::is_own(manifest, n));
        self.pick = Pick::Target(snapshot.path.clone(), targets.iter().map(|n| (n.port.clone(), restore::is_own(manifest, n))).collect());
        if targets.len() == 1 {
            return self.chosen(0, ctx);
        }
        let mut popup = Popup::note(t("restore-title"), vec![t("restore-choose-target")]);
        popup.buttons = targets
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let mut text = clean(&n.name);
                if restore::is_own(manifest, n) {
                    text = tf("restore-target-own", &[("name", text.into())]);
                }
                (text, Some(Confirm::Choose(i)))
            })
            .chain([(t("answer-cancel"), None)])
            .collect();
        Action::Ask(popup)
    }
}

/// Where a restore is in its questions (`Page::chosen` answers them)
#[derive(Default)]
enum Pick {
    #[default]
    None,
    /// The backup's folder, the nodes offered (port, is the backup's own board)
    Target(PathBuf, Vec<(Port, bool)>),
    /// The backup's folder, the other board chosen: move or copy
    Mode(PathBuf, Port),
}

/// The modes offered for another board, in the order of the buttons
const MODES: [Mode; 2] = [Mode::Move, Mode::Copy];

/// Hands the restore to the worker: it reads the backup and the node, then asks (nothing written
/// before that). Also from a node's own view.
pub(super) fn ask_restore(backup: PathBuf, port: Port, mode: Mode) -> Action {
    Action::Send(vec![Cmd::RestorePreview(Request { backup, port, mode, safety: None, putting_back: false })])
}

/// A backup section by what it holds, as the companion's apps call it (the ids are the ones
/// src/backup/read.rs writes); a section of a newer version is shown by its id
fn section_name(id: &str) -> String {
    let text = match id {
        "self" => "section-self",
        "device" => "section-device",
        "tuning" => "section-tuning",
        "autoadd" => "section-autoadd",
        "flood-scope" => "section-flood-scope",
        "custom-vars" => "section-custom-vars",
        "private-key" => "section-private-key",
        "channels" => "section-channels",
        "contacts" => "section-contacts",
        "console" => "section-console",
        "acl" => "section-acl",
        "regions" => "section-regions",
        "address-book" => "section-address-book",
        _ => return id.to_string(),
    };
    t(text)
}

/// What a backup holds, as the lines of a panel `width` wide inside (also in a node's own view);
/// `None`: its manifest could not be read
pub(super) fn describe(m: Option<&Manifest>, width: usize) -> Vec<Line<'static>> {
    let Some(m) = m else { return vec![Line::styled(t("backup-unreadable"), theme::bad())] };
    let or_dash = |s: &str| if s.is_empty() { "-".to_string() } else { s.to_string() };
    let sections: Vec<String> = m
        .sections
        .iter()
        .map(|s| {
            let name = section_name(&s.id);
            // one item: the name alone ("radio tuning"); a list: how many ("contacts 350")
            let text = if s.items == 1 { name } else { format!("{name} {}", s.items) };
            if s.secret {
                format!("{text} *")
            } else {
                text
            }
        })
        .collect();
    let mut lines = facts(
        vec![
            fact(t("field-node"), format!("{}  ({})", clean(&m.node.name), m.node.kind.label())),
            fact(t("field-board"), or_dash(&devices::describe(&m.node.model))),
            fact(t("field-firmware"), or_dash(&m.node.firmware)),
            fact(t("field-public-key"), or_dash(&m.node.public_key)),
            fact(t("field-hardware-id"), or_dash(&m.node.hardware_id)),
            fact(t("field-created"), format!("{}  ({}, format {})", store::created_text(&m.created), m.tool, m.format)),
            fact(t("field-reason"), m.reason.text()),
            fact(t("field-contents"), sections.join("   ")),
        ],
        width,
    );
    if m.sections.iter().any(|s| s.secret) {
        lines.push(Line::styled(t("backup-secret-mark"), theme::dim()));
    }
    // in a verified backup: what this firmware does not have or this version does not read yet
    // (a note); in an older one it may be a failure (a warning)
    if !m.missing.is_empty() {
        let style = if m.verified { theme::dim() } else { theme::warn() };
        lines.push(Line::styled(tf("backup-missing", &[("missing", m.missing.join(", ").into())]), style));
    }
    lines
}

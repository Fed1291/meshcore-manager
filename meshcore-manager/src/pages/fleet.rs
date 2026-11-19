//! Fleet: the nodes on USB and Bluetooth, and what is known about the selected one.
//! Enter opens the node's own view (pages/node.rs); Esc comes back to the list.
//! The list is plain at first. `s` (or Space) opens select mode: a check box per row, Space
//! checks nodes ([x]), `a` all / none; `b` then backs up the checked ones, `e` edits the settings
//! they have in common (pages/group.rs, one kind at a time). Esc unchecks all and leaves it.
//! The title says whether Bluetooth listens; when it cannot, why (under the details).

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, TableState, Wrap};
use ratatui::Frame;

use super::group::GroupView;
use super::node::NodeView;
use super::{step, Only, Page};
use crate::app::{Action, Confirm, Ctx, Popup};
use crate::ble;
use crate::i18n::{t, tf};
use crate::node::{Kind, Node};
use crate::presets;
use crate::serial::{self, Port};
use crate::theme;
use crate::widgets::{self, clean, fact, facts, panel, Col, Fact, Grid};
use crate::worker::Cmd;

#[derive(Default)]
pub struct Fleet {
    selected: usize,
    table: TableState,
    /// The node's own view, when one is open
    view: Option<NodeView>,
    /// Several nodes edited together, when open
    group: Option<GroupView>,
    /// Which kind of node is listed (`f`)
    only: Only,
    /// Select mode (`s`): the check boxes are shown
    selecting: bool,
    /// The PIN being typed for a Bluetooth companion (`p`, or asked after a refusal)
    pin: Option<PinPrompt>,
    /// The last PIN ask of the worker opened (`Ctx::pin_ask`: each one once)
    pin_seen: u64,
    /// The checked nodes (Space), by port: an unplugged one simply drops out
    checked: Vec<Port>,
    /// The groups `e` offered to edit (one per kind), for `chosen`
    choices: Vec<(Kind, Vec<Port>)>,
    /// How the list is ordered (`o`)
    sort: Sort,
    /// The selected node's pairing (`pairing`), noted when drawn: the key bar has no `Ctx`
    pairing: Option<bool>,
    /// Every node shown is checked (`a` then unchecks them all), noted when drawn
    all_checked: bool,
    /// The checked nodes that are paired Bluetooth companions (`u` unpairs them), when drawn
    checked_paired: usize,
    /// Bluetooth companions in the list, and when one more last turned up (or the page was
    /// first drawn): the listening waves calm down once none came for `BLE_CALM`
    ble_count: usize,
    ble_news: Option<Instant>,
}

/// No new Bluetooth companion for this long: the ones nearby are found, the waves go slower
const BLE_CALM: Duration = Duration::from_secs(20);

/// A Bluetooth companion not paired yet (no PIN known for it: it never connected), or whose
/// pairing was lost (its last reading failed). One that answered, or whose PIN is saved, pairs
/// by itself: no PIN to type, no `p` shown.
fn needs_pin(node: &Node) -> bool {
    node.port.is_ble() && !node.inspected && (read_failed(node) || (node.model.is_empty() && !ble::known_pin(&node.port.dev)))
}

/// Its last reading failed (not "not read yet")
fn read_failed(node: &Node) -> bool {
    !node.note.is_empty() && node.note != t("node-ble-not-read")
}

/// A Bluetooth companion that answered: paired with this computer, `u` unpairs it
fn paired(node: &Node) -> bool {
    node.port.is_ble() && !needs_pin(node)
}

/// For the keys: Some(true) pair (`p`), Some(false) unpair (`u`), None not Bluetooth
pub(super) fn pairing(node: &Node) -> Option<bool> {
    node.port.is_ble().then(|| needs_pin(node))
}

/// The question before unpairing one companion or several: which, what it does, how to pair again
fn ask_unpair(nodes: &[&Node]) -> Action {
    let names: Vec<String> = nodes.iter().map(|n| clean(&n.name)).collect();
    let lines = vec![tf("unpair-ask", &[("names", names.join(", ").into()), ("count", nodes.len().into())]), t("unpair-after")];
    let cmds = nodes.iter().map(|n| Cmd::BleUnpair((*n).clone())).collect();
    Action::Ask(Popup::ask(t("unpair-title"), lines, Confirm::Send(cmds), t("unpair-yes"), t("answer-cancel")).safe())
}

/// The order of the list, `o` goes to the next
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Sort {
    /// USB first (by port), then Bluetooth (strongest first)
    #[default]
    Link,
    /// Companions, repeaters, room servers, sensors, then the rest; by name inside each
    Kind,
    Name,
}

impl Sort {
    fn next(self) -> Sort {
        match self {
            Sort::Link => Sort::Kind,
            Sort::Kind => Sort::Name,
            Sort::Name => Sort::Link,
        }
    }

    fn word(self) -> String {
        t(match self {
            Sort::Link => "sort-link",
            Sort::Kind => "sort-kind",
            Sort::Name => "sort-name",
        })
    }
}

/// The place of a kind in the list sorted by kind
fn kind_rank(kind: Kind) -> u8 {
    match kind {
        Kind::Companion => 0,
        Kind::Repeater => 1,
        Kind::RoomServer => 2,
        Kind::Sensor => 3,
        _ => 4,
    }
}

impl Fleet {
    /// Nothing new heard over Bluetooth for `BLE_CALM` (noted at each draw)
    fn ble_calm(&mut self, ctx: &Ctx) -> bool {
        let count = ctx.nodes.iter().filter(|n| n.port.is_ble()).count();
        if count > self.ble_count || self.ble_news.is_none() {
            self.ble_news = Some(Instant::now());
        }
        self.ble_count = count;
        self.ble_news.is_some_and(|at| at.elapsed() >= BLE_CALM)
    }

    /// The nodes in the list: those of the kind chosen with `f` (and any still being identified),
    /// in the order chosen with `o` (the worker's order kept among equals)
    fn shown<'a>(&self, ctx: &'a Ctx) -> Vec<&'a Node> {
        let mut list: Vec<&Node> = ctx.nodes.iter().filter(|n| n.probing || self.only.shows(Some(n.kind))).collect();
        let name = |n: &Node| clean(&n.name).to_lowercase();
        match self.sort {
            Sort::Link => list.sort_by_key(|n| (n.port.is_ble(), std::cmp::Reverse(n.signal.unwrap_or(i16::MIN)))),
            Sort::Kind => list.sort_by_key(|n| (kind_rank(n.kind), name(n))),
            Sort::Name => list.sort_by_key(|n| (n.name.is_empty(), name(n))),
        }
        list
    }

    /// The checked nodes still plugged in
    fn checked_nodes<'a>(&self, ctx: &'a Ctx) -> Vec<&'a Node> {
        ctx.nodes.iter().filter(|n| self.checked.contains(&n.port)).collect()
    }

    /// `e` on checked nodes: their common settings, for nodes of one kind that answer. All of
    /// one kind: opened at once. Mixed kinds, or some that do not answer: a choice, one button
    /// per kind ("edit the 3 repeater"), the others left aside.
    fn edit_checked(&mut self, ctx: &Ctx) -> Action {
        let nodes = self.checked_nodes(ctx);
        if nodes.is_empty() {
            return Action::Ask(Popup::note(t("edit-title"), vec![t("group-need-checks")]));
        }
        // the ones that answer, by kind, in the order of the list
        let mut groups: Vec<(Kind, Vec<Port>)> = Vec::new();
        for node in nodes.iter().filter(|n| n.kind.is_node() && !n.probing) {
            match groups.iter_mut().find(|(kind, _)| *kind == node.kind) {
                Some((_, ports)) => ports.push(node.port.clone()),
                None => groups.push((node.kind, vec![node.port.clone()])),
            }
        }
        let silent = nodes.len() - groups.iter().map(|(_, p)| p.len()).sum::<usize>();
        if groups.is_empty() {
            return Action::Ask(Popup::note(t("edit-title"), vec![t("group-not-nodes")]));
        }
        if groups.len() == 1 && silent == 0 {
            self.choices = groups;
            return self.open_choice(0, ctx);
        }
        let mut lines = vec![t("group-choose")];
        if silent > 0 {
            lines.push(tf("group-silent", &[("count", silent.into())]));
        }
        let mut popup = Popup::note(t("edit-title"), lines);
        popup.buttons = groups
            .iter()
            .enumerate()
            .map(|(i, (kind, ports))| {
                let text = tf("group-edit-these", &[("count", ports.len().into()), ("kind", kind.label().into())]);
                (text, Some(Confirm::Choose(i)))
            })
            .chain([(t("answer-cancel"), None)])
            .collect();
        self.choices = groups;
        Action::Ask(popup)
    }

    /// Opens group `choice` of `choices`: only those stay checked. The ones not read yet are read
    /// first (their settings come from that); the view waits for them.
    fn open_choice(&mut self, choice: usize, ctx: &Ctx) -> Action {
        let Some((kind, ports)) = self.choices.get(choice).cloned() else { return Action::None };
        self.checked = ports.clone();
        let read = ctx
            .nodes
            .iter()
            .filter(|n| ports.contains(&n.port) && !n.inspected)
            .map(|n| Cmd::Inspect(n.port.clone()))
            .collect();
        self.group = Some(GroupView::open(ports, kind));
        Action::Send(read)
    }
}

impl Page for Fleet {
    fn title(&self) -> String {
        t("tab-fleet")
    }

    fn keys(&self) -> Vec<(&'static str, String)> {
        if self.pin.is_some() {
            return vec![("0-9", t("settings-key-digits")), ("Enter", t("pin-key-pair")), ("Esc", t("key-cancel"))];
        }
        if let Some(view) = &self.view {
            return view.keys();
        }
        if let Some(group) = &self.group {
            return group.keys();
        }
        let mut keys = vec![("↑↓", t("key-select")), ("Enter", t("key-open"))];
        let checked = self.checked.len();
        if self.selecting {
            keys.push(("Space", t("key-check")));
            // a toggle: says what it will do now
            keys.push(("a", t(if self.all_checked { "key-uncheck-all" } else { "key-check-all" })));
            if checked > 0 {
                keys.push(("b", tf("key-backup-n", &[("count", checked.into())])));
                keys.push(("e", tf("key-edit-n", &[("count", checked.into())])));
                if self.checked_paired > 0 {
                    keys.push(("u", tf("key-unpair-n", &[("count", self.checked_paired.into())])));
                }
            } else {
                keys.push(("b", t("key-backup")));
            }
            keys.push(("Esc", t("key-select-leave")));
        } else {
            keys.push(("b", t("key-backup")));
            keys.push(("s", t("key-select-mode")));
        }
        // a Bluetooth companion: pair it when it is not (or lost it), else unpair
        match self.pairing {
            Some(true) => keys.push(("p", t("key-pin"))),
            Some(false) => keys.push(("u", t("key-unpair"))),
            None => {}
        }
        keys.extend([("r", t("key-rescan-one")), ("R", t("key-rescan-all")), self.only.key()]);
        keys.push(("o", tf("key-sort", &[("now", self.sort.word().into())])));
        keys
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        self.take_pin_ask(ctx);
        self.draw_page(f, area, ctx);
        if let Some(prompt) = &self.pin {
            draw_pin(f, area, prompt);
        }
    }

    fn typing(&self) -> bool {
        self.pin.is_some() || self.view.as_ref().is_some_and(NodeView::typing) || self.group.as_ref().is_some_and(GroupView::typing)
    }

    /// A node's view (or a group edit, or the PIN being typed) is open: its keys, Esc back
    fn owns_keys(&self) -> bool {
        self.pin.is_some() || self.view.is_some() || self.group.is_some()
    }

    fn shown_boards(&self) -> Vec<Port> {
        match (&self.view, &self.group) {
            (Some(view), _) => vec![view.port().clone()],
            (None, Some(group)) => group.ports().to_vec(),
            (None, None) => Vec::new(),
        }
    }

    /// A node's view: its commit or leave question was answered. A group edit closes after its
    /// commit (each node is read again) or when its changes are dropped.
    fn confirmed(&mut self) {
        if let Some(view) = &mut self.view {
            view.confirmed();
        }
        self.group = None;
    }

    /// A group was chosen to edit (`edit_checked`)
    fn chosen(&mut self, choice: usize, ctx: &Ctx) -> Action {
        self.open_choice(choice, ctx)
    }

    fn on_key(&mut self, key: KeyEvent, ctx: &Ctx) -> Action {
        if self.pin.is_some() {
            return self.pin_key(key, ctx);
        }
        // `p` in a Bluetooth companion's Overview (only looking): its PIN, then pair and read
        // `u` in a paired Bluetooth companion's Overview: unpair it (asked first)
        if let Some(view) = self.view.as_ref().filter(|v| key.code == KeyCode::Char('u') && v.pairs_here()) {
            if let Some(node) = ctx.nodes.iter().find(|n| n.port == *view.port() && paired(n)) {
                return ask_unpair(&[node]);
            }
        }
        let pairable = |v: &NodeView| ctx.nodes.iter().any(|n| n.port == *v.port() && needs_pin(n));
        if let Some(view) = self.view.as_ref().filter(|v| key.code == KeyCode::Char('p') && v.pairs_here() && pairable(v)) {
            let port = view.port().clone();
            let name = ctx.nodes.iter().find(|n| n.port == port).map_or_else(String::new, |n| n.name.clone());
            self.pin = Some(PinPrompt::new(port, &name, false, None));
            return Action::None;
        }
        if let Some(view) = &mut self.view {
            return match view.on_key(key, ctx) {
                Some(action) => action,
                None => {
                    self.view = None;
                    Action::None
                }
            };
        }
        if let Some(group) = &mut self.group {
            return match group.on_key(key, ctx) {
                Some(action) => action,
                None => {
                    self.group = None;
                    Action::None
                }
            };
        }
        let shown = self.shown(ctx);
        let node = shown.get(self.selected).copied();
        match key.code {
            // the next filter; the selected node stays selected when it is still shown
            KeyCode::Char('f') => {
                let port = node.map(|n| n.port.clone());
                self.only = self.only.next();
                let shown = self.shown(ctx);
                self.selected = port.and_then(|p| shown.iter().position(|n| n.port == p)).unwrap_or(0);
                Action::None
            }
            // the next order; the selected node stays selected
            KeyCode::Char('o') => {
                let port = node.map(|n| n.port.clone());
                self.sort = self.sort.next();
                let shown = self.shown(ctx);
                self.selected = port.and_then(|p| shown.iter().position(|n| n.port == p)).unwrap_or(0);
                Action::None
            }
            // select mode on / off (off: nothing checked any more)
            KeyCode::Char('s') => {
                self.selecting = !self.selecting;
                if !self.selecting {
                    self.checked.clear();
                }
                Action::None
            }
            // checks the selected node (and opens select mode when it was not)
            KeyCode::Char(' ') => {
                self.selecting = true;
                if let Some(node) = node {
                    match self.checked.iter().position(|p| *p == node.port) {
                        Some(at) => {
                            self.checked.remove(at);
                        }
                        None => self.checked.push(node.port.clone()),
                    }
                }
                Action::None
            }
            // every node shown checked, or (all were) none
            KeyCode::Char('a') if self.selecting => {
                let all = shown.iter().all(|n| self.checked.contains(&n.port));
                self.checked.retain(|p| !shown.iter().any(|n| n.port == *p));
                if !all {
                    self.checked.extend(shown.iter().map(|n| n.port.clone()));
                }
                Action::None
            }
            KeyCode::Esc => {
                self.checked.clear();
                self.selecting = false;
                Action::None
            }
            KeyCode::Char('e') if self.selecting => self.edit_checked(ctx),
            // the pairing PIN of a Bluetooth companion, before it is opened (or to change it)
            // pairing: only a companion not paired yet, or whose pairing was lost
            KeyCode::Char('p') => match node.filter(|n| needs_pin(n)) {
                Some(node) => {
                    self.pin = Some(PinPrompt::new(node.port.clone(), &node.name, false, None));
                    Action::None
                }
                None => Action::None,
            },
            // the checked paired companions, else the selected one
            KeyCode::Char('u') if !self.checked.is_empty() => {
                let all: Vec<&Node> = ctx.nodes.iter().filter(|n| self.checked.contains(&n.port) && paired(n)).collect();
                if all.is_empty() {
                    return Action::None;
                }
                ask_unpair(&all)
            }
            KeyCode::Char('u') => match node.filter(|n| paired(n)) {
                Some(node) => ask_unpair(&[node]),
                None => Action::None,
            },
            KeyCode::Char('R') => Action::Send(vec![Cmd::Rescan(None)]),
            KeyCode::Char('r') => match node {
                Some(node) => Action::Send(vec![Cmd::Rescan(Some(node.port.clone()))]),
                None => Action::Send(vec![Cmd::Rescan(None)]),
            },
            // the checked nodes, else the selected one
            KeyCode::Char('b') if !self.checked.is_empty() => ask_backup(&self.checked_nodes(ctx)),
            KeyCode::Char('b') => node.map_or(Action::None, |n| ask_backup(&[n])),
            KeyCode::Enter => match node {
                Some(node) => {
                    let (view, action) = NodeView::open(node);
                    self.view = Some(view);
                    action
                }
                None => Action::None,
            },
            code => {
                self.selected = step(self.selected, shown.len(), code);
                Action::None
            }
        }
    }
}

impl Fleet {
    /// A PIN the worker asks for (a companion refused the one tried): opened once, over whatever
    /// the page shows
    fn take_pin_ask(&mut self, ctx: &Ctx) {
        let Some(ask) = ctx.pin_ask.as_ref().filter(|a| a.number > self.pin_seen) else { return };
        self.pin_seen = ask.number;
        let name = ctx.nodes.iter().find(|n| n.port == ask.port).map_or_else(String::new, |n| n.name.clone());
        self.pin = Some(PinPrompt::new(ask.port.clone(), &name, true, Some(ask.number)));
    }

    /// A key while the PIN is typed: 6 digits; Enter gives it (and runs again what was refused);
    /// empty + Enter = the PIN from Settings again; Esc = nothing changes
    fn pin_key(&mut self, key: KeyEvent, ctx: &Ctx) -> Action {
        let Some(prompt) = &mut self.pin else { return Action::None };
        match key.code {
            KeyCode::Esc => self.pin = None,
            KeyCode::Backspace => {
                prompt.typed.pop();
                prompt.problem = None;
            }
            KeyCode::Enter => {
                let address = prompt.port.dev.clone();
                let pin = if prompt.typed.is_empty() && !prompt.refused {
                    None
                } else {
                    match ble::pin(&prompt.typed) {
                        Some(pin) => Some(pin),
                        None => {
                            prompt.problem = Some(t("ble-bad-pin"));
                            return Action::None;
                        }
                    }
                };
                // what was refused, run again with it (a reading, a backup, a restore's check)
                let retry = prompt
                    .ask
                    .and_then(|number| ctx.pin_ask.as_ref().filter(|a| a.number == number))
                    .and_then(|a| a.retry.as_ref())
                    .and_then(Cmd::retry_copy);
                // asked with `p`: pair and read it now (a refusal asks again)
                let pair_now = prompt.ask.is_none().then(|| Cmd::Inspect(prompt.port.clone()));
                self.pin = None;
                let mut cmds = vec![Cmd::BlePin { address, pin }];
                cmds.extend(retry);
                cmds.extend(pair_now);
                return Action::Send(cmds);
            }
            _ => match widgets::typed(&key) {
                Some(c) if c.is_ascii_digit() && prompt.typed.len() < ble::PIN_DIGITS => {
                    prompt.typed.push(c);
                    prompt.problem = None;
                }
                Some(_) => prompt.problem = Some(t("ble-bad-pin")),
                None => {}
            },
        }
        Action::None
    }

    /// The list, a node's view or a group edit
    fn draw_page(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        if let Some(view) = &mut self.view {
            return view.draw(f, area, ctx);
        }
        if let Some(group) = &mut self.group {
            return group.draw(f, area, ctx);
        }
        // the unplugged ones are no longer checked
        self.checked.retain(|port| ctx.nodes.iter().any(|n| &n.port == port));
        let shown = self.shown(ctx);
        let mut title = self.only.title(tf("fleet-title", &[("count", shown.len().into())]));
        if self.selecting {
            title = tf("fleet-checked", &[("title", title.into()), ("count", self.checked.len().into())]);
        }
        // Bluetooth: listening, off in the settings, or why it cannot
        let (bluetooth, why) = match (ctx.config.bluetooth.enabled, &ctx.bluetooth) {
            (false, _) => (t("fleet-ble-disabled"), None),
            (true, Some(why)) => (t("fleet-ble-problem"), Some(why.clone())),
            // waves going out while it listens: slower once nothing new came for a while
            (true, None) => (format!("{} {}", t("fleet-ble-listening"), widgets::listening(self.ble_calm(ctx))), None),
        };
        title = format!("{title}  ·  {bluetooth}");
        self.pairing = None;
        self.all_checked = !shown.is_empty() && shown.iter().all(|n| self.checked.contains(&n.port));
        self.checked_paired = ctx.nodes.iter().filter(|n| self.checked.contains(&n.port) && paired(n)).count();
        if shown.is_empty() {
            let text = t(match (ctx.nodes.is_empty(), ctx.scanned) {
                (false, _) => "only-none",
                (true, true) => "fleet-empty",
                (true, false) => "fleet-looking",
            });
            let mut lines = vec![Line::styled(text, theme::dim())];
            if let Some(why) = &why {
                lines.push(Line::raw(""));
                lines.push(Line::styled(tf("fleet-ble-why", &[("why", why.as_str().into())]), theme::warn()));
            }
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(panel(&title)), area);
            return;
        }
        self.selected = self.selected.min(shown.len() - 1);
        let node = shown[self.selected];
        self.pairing = pairing(node);
        let mut lines = identity_lines(node, identity(node), widgets::inner_width(area));
        lines.push(Line::raw(""));
        lines.push(Line::styled(t("fleet-open-hint"), theme::dim()));
        if let Some(why) = &why {
            lines.push(Line::styled(tf("fleet-ble-why", &[("why", why.as_str().into())]), theme::warn()));
        }
        let list = widgets::details_below(f, area, &t("fleet-details"), lines, 4);

        let selecting = self.selecting;
        let rows = shown.iter().map(|n| {
            let kind = if n.probing {
                // "●∙∙ identifying 3 s": moving, and how long it has taken so far
                let secs = n.seen.elapsed().as_secs();
                Line::from(vec![
                    Span::styled(format!("{} ", widgets::spinner()), theme::title()),
                    Span::styled(tf("fleet-probing", &[("secs", secs.into())]), theme::dim()),
                ])
            } else {
                Line::styled(n.kind.label(), theme::kind(n.kind))
            };
            // a Bluetooth companion not read yet: what is happening, where its board would be
            // (the cells are empty until it is read). It connects by itself, with the PIN it
            // has (yours, the one it last paired with, else the default): "connecting with its
            // saved PIN…". Only when that failed: why, and the key that helps (p a PIN, Enter
            // again).
            let board = if n.port.is_ble() && n.model.is_empty() {
                let with = if ble::given_pin(&n.port.dev).is_some() {
                    t("fleet-pin-yours")
                } else if ble::saved_pin(&n.port.dev).is_some() {
                    t("fleet-pin-saved-one")
                } else {
                    t("fleet-pin-default")
                };
                if ctx.job_on(&n.port).is_some() {
                    Line::from(vec![
                        Span::styled(format!("{} ", widgets::spinner()), theme::title()),
                        Span::styled(tf("fleet-ble-connecting", &[("with", with.into())]), theme::title()),
                    ])
                } else if read_failed(n) {
                    let (key, what) = if needs_pin(n) { ("p", t("key-pin")) } else { ("Enter", t("fleet-ble-try-again")) };
                    Line::from(vec![
                        Span::styled(t("fleet-ble-failed"), theme::warn()),
                        Span::raw("  "),
                        Span::styled(key, theme::key()),
                        Span::styled(format!(" {what}"), theme::dim()),
                    ])
                } else {
                    // heard, its turn comes (one Bluetooth connection at a time)
                    Line::styled(tf("fleet-ble-will-connect", &[("with", with.into())]), theme::dim())
                }
            } else {
                Line::raw(n.board())
            };
            let mut cells = Vec::new();
            // the check box only in select mode
            if selecting {
                cells.push(if self.checked.contains(&n.port) {
                    Line::styled("[x]", theme::title())
                } else {
                    Line::styled("[ ]", theme::dim())
                });
            }
            cells.extend([
                widgets::link_cell(&n.port, n.signal),
                widgets::key_prefix(&n.public_key),
                kind,
                Line::raw(clean(&n.name)),
                board,
                Line::raw(widgets::firmware_short(&n.firmware)),
                last_backup(ctx, n),
            ]);
            cells
        });
        // a narrow window: long cells are shortened first ("…"); narrower still: the firmware
        // goes, then the board, then the last backup
        let mut cols = Vec::new();
        if selecting {
            cols.push(Col::new(String::new()));
        }
        cols.extend([
            Col::new(t("col-link")),
            Col::new(t("col-key")),
            Col::new(t("col-kind")),
            Col::new(t("col-name")).grow(),
            Col::new(t("col-board")).drop(2),
            Col::new(t("col-firmware")).drop(3),
            Col::new(t("col-last-backup")).drop(1),
        ]);
        let grid = Grid::new(cols).rows(rows);
        self.table.select(Some(self.selected));
        grid.show(f, list, widgets::panel_focus(&title, true), &mut self.table);
    }
}

/// The PIN being typed for a Bluetooth companion: `p` in the list, or asked after the companion
/// refused the one tried (`ask`: that ask's number, to run again what was refused)
struct PinPrompt {
    port: Port,
    name: String,
    typed: String,
    refused: bool,
    ask: Option<u64>,
    problem: Option<String>,
}

impl PinPrompt {
    fn new(port: Port, name: &str, refused: bool, ask: Option<u64>) -> PinPrompt {
        let name = if name.trim().is_empty() { port.dev.clone() } else { clean(name) };
        PinPrompt { port, name, typed: String::new(), refused, ask, problem: None }
    }
}

/// The PIN box over the page: why it is asked, where the PIN is, the field. Typed in clear (it
/// is being typed), never kept on screen after.
fn draw_pin(f: &mut Frame, area: Rect, prompt: &PinPrompt) {
    let mut lines = Vec::new();
    if prompt.refused {
        lines.push(Line::styled(tf("pin-refused", &[("name", prompt.name.as_str().into())]), theme::warn()));
    } else {
        lines.push(Line::raw(tf("pin-for", &[("name", prompt.name.as_str().into())])));
    }
    lines.push(Line::styled(t("pin-where"), theme::dim()));
    lines.push(Line::raw(""));
    let mut field = vec![Span::raw(format!("{}  ", t("pin-label")))];
    field.extend(widgets::input(&prompt.typed, ble::PIN_DIGITS + 2));
    lines.push(Line::from(field));
    if !prompt.refused {
        lines.push(Line::styled(t("pin-empty-default"), theme::dim()));
    }
    if let Some(problem) = &prompt.problem {
        lines.push(Line::styled(problem.clone(), theme::bad()));
    }
    let width = 64.min(area.width.saturating_sub(4));
    let height = lines.len() as u16 + 6;
    let rect = widgets::centered(area, width, height);
    widgets::lift(f, rect);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(widgets::panel_focus(&t("pin-title"), true)), rect);
}

/// The question before a backup of one node or several (Fleet and the node's view): the ones
/// that answer are backed up, one after the other; the others are named as left out
pub(super) fn ask_backup(nodes: &[&Node]) -> Action {
    let (usable, left_out): (Vec<&Node>, Vec<&Node>) = nodes.iter().copied().partition(|n| n.kind.is_node() && !n.probing);
    if usable.is_empty() {
        return Action::Ask(Popup::note(t("backup-title"), vec![t("backup-not-a-node")]));
    }
    let mut lines = match usable.as_slice() {
        [node] => vec![tf("backup-ask", &[("name", clean(&node.name).into()), ("board", node.board().into())])],
        _ => {
            let mut lines = vec![tf("backup-ask-many", &[("count", usable.len().into())])];
            lines.extend(usable.iter().map(|n| format!("  · {}  ({})", clean(&n.name), n.board())));
            lines
        }
    };
    if !left_out.is_empty() {
        let names = left_out.iter().map(|n| serial::short(&n.port.dev)).collect::<Vec<_>>().join(", ");
        lines.push(tf("backup-left-out", &[("ports", names.into())]));
    }
    lines.push(t("backup-ask-secrets"));
    Action::Ask(Popup::ask(
        t("backup-title"),
        lines,
        Confirm::Send(usable.iter().map(|n| Cmd::Backup((*n).clone())).collect()),
        tf("backup-yes", &[("count", usable.len().into())]),
        t("answer-cancel"),
    ))
}

/// The date of the board's newest backup, or "never" (for a node that can be backed up); while
/// one is being made (several backed up at once: each row): moving, and how far it is
fn last_backup(ctx: &Ctx, node: &Node) -> Line<'static> {
    let backing_up = tf("job-backup", &[("name", node.name.as_str().into())]);
    if let Some(job) = ctx.job_on(&node.port).filter(|j| j.title == backing_up) {
        let far = if job.total > 0 { format!(" {}/{}", job.done, job.total) } else { String::new() };
        return Line::from(vec![
            Span::styled(format!("{} ", widgets::spinner()), theme::title()),
            Span::styled(format!("{}{far}", t("fleet-backing-up")), theme::title()),
        ]);
    }
    let newest = ctx
        .backups
        .iter()
        .find(|b| b.is_board(&node.port.serial_number, &node.public_key))
        .and_then(|b| b.snapshots.first());
    match newest {
        Some(s) => Line::raw(s.shown()),
        None if node.kind.is_node() && !node.probing => Line::styled(t("fleet-never"), theme::warn()),
        None => Line::raw(""),
    }
}

/// The radio settings, with the preset they are when one has them: "869.618 ...  (EU/UK (Narrow))"
fn radio(node: &Node) -> String {
    let names = presets::names(node);
    match (node.radio.is_empty(), names.is_empty()) {
        (true, _) => "-".into(),
        (false, true) => node.radio.clone(),
        (false, false) => format!("{}   ({names})", node.radio),
    }
}

/// Who a node is (Fleet and the node's view)
pub(super) fn identity(node: &Node) -> Vec<Fact> {
    let or_dash = |s: &str| if s.is_empty() { "-".to_string() } else { s.to_string() };
    // USB: its ids and chip; Bluetooth: the signal, and which PIN pairing uses (never the PIN)
    let link = if node.port.is_ble() {
        let signal = node.signal.map_or_else(|| "-".to_string(), |d| format!("{d} dBm"));
        // which PIN it connects with first (never the PIN itself)
        let pin = if ble::given_pin(&node.port.dev).is_some() {
            t("fleet-pin-given")
        } else if ble::saved_pin(&node.port.dev).is_some() {
            t("fleet-pin-saved")
        } else {
            t("fleet-pin-settings")
        };
        fact(t("field-bluetooth"), format!("{signal}   {pin}"))
    } else {
        let usb = match node.port.board() {
            Some(b) => format!("{}  {}  ({})", node.port.usb_id(), b.chip, b.boards),
            None => format!("{}  {}", node.port.usb_id(), node.port.label()),
        };
        fact(t("field-usb"), usb)
    };
    vec![
        fact(t("field-port"), node.port.name()),
        link,
        fact(t("field-hardware-id"), or_dash(&node.port.serial_number)),
        fact(t("field-board"), or_dash(&node.board())),
        fact(t("field-firmware"), or_dash(&node.firmware)),
        fact(t("field-public-key"), or_dash(&node.public_key)),
        fact(t("field-radio"), radio(node)),
    ]
}

/// A node's facts as the lines of a panel `width` wide inside, then its note (why it does not
/// answer, ...)
pub(super) fn identity_lines(node: &Node, pairs: Vec<Fact>, width: usize) -> Vec<Line<'static>> {
    let mut lines = facts(pairs, width);
    if !node.note.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(node.note.clone(), theme::warn()));
    }
    // a Bluetooth companion that has not answered yet: how to pair it, here in its card
    if needs_pin(node) {
        lines.push(Line::from(vec![Span::styled("p ", theme::key()), Span::styled(t("pin-hint"), theme::dim())]));
    }
    lines
}

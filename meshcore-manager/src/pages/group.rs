//! Group edit: the settings that several checked nodes of one kind have in common, changed once
//! for all of them (Fleet: Space checks nodes, `e` opens this). Each setting shows its value on
//! the nodes ("3 different values" when they differ); the selected one shows, node by node, what
//! the change does ("20 → 24", "no change"). One commit writes every node that changes, each by
//! its own `Cmd::Apply`, backed up first when chosen.
//! Left out: what makes a node itself (its name, its private key, its position: `edit::OWN`).
//! `a` adds a region of the user's list (with the ones above it) to every node that can hold
//! regions; a node that has it already is left as it is. Picking it again takes it back.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, TableState, Wrap};
use ratatui::Frame;

use super::node::edit::{commit_question, Editor, Target};
use super::node::{draw_presets, is_radio, loading, masked, pair_lines, settings, stacks, value_span, PresetKey, PresetPicker};
use super::regions::{Picked, RegionPicker};
use super::step;
use crate::app::{Action, Ctx};
use crate::regions::Region;
use crate::worker::Cmd;
use crate::i18n::{t, tf};
use crate::node::edit::{plan, Draft, OWN};
use crate::node::{Fact, Kind, Node};
use crate::serial::Port;
use crate::theme;
use crate::widgets::{
    clean, ellipsis, inner_width, input, label_width, more_marks, panel, panel_focus, room_line, Col, Grid, READING,
};

/// Group edit is an edit mode: the secrets the nodes told are shown in full, as in one node's
/// edit mode (the layout is still decided with them hidden, `stacks`)
const SHOWN: bool = true;

pub struct GroupView {
    ports: Vec<Port>,
    kind: Kind,
    /// The selected setting
    row: usize,
    editor: Editor,
    /// The region chooser is open (`a`)
    picker: Option<RegionPicker>,
    /// The radio preset chooser is open (`p`): the same radio for all of them
    presets: Option<PresetPicker>,
}

impl GroupView {
    pub fn open(ports: Vec<Port>, kind: Kind) -> GroupView {
        GroupView { ports, kind, row: 0, editor: Editor::for_group(), picker: None, presets: None }
    }

    /// The checked nodes still plugged in
    /// The boards edited together (their jobs show here, not in the bottom line)
    pub fn ports(&self) -> &[Port] {
        &self.ports
    }

    fn nodes<'a>(&self, ctx: &'a Ctx) -> Vec<&'a Node> {
        ctx.nodes.iter().filter(|n| self.ports.contains(&n.port)).collect()
    }

    pub fn typing(&self) -> bool {
        self.editor.input.is_some() || self.picker.is_some() || self.presets.is_some()
    }

    pub fn keys(&self) -> Vec<(&'static str, String)> {
        if self.presets.is_some() {
            return vec![("↑↓", t("key-select")), ("Enter", t("key-use-preset")), ("a-z", t("key-find")), ("Esc", t("key-close"))];
        }
        if self.picker.is_some() {
            return vec![("↑↓", t("key-select")), ("Enter", t("key-region-take")), ("a-z", t("key-find")), ("Esc", t("key-close"))];
        }
        if self.editor.input.is_some() && !self.editor.choices.is_empty() {
            return vec![("←→", t("key-pick")), ("Enter", t("key-ok")), ("Esc", t("key-cancel")), ("a-z", t("key-type-instead"))];
        }
        if self.editor.input.is_some() {
            return vec![("Enter", t("key-ok")), ("Esc", t("key-cancel"))];
        }
        let mut keys = vec![
            ("Esc", t("key-leave-edit")),
            ("↑↓", t("key-select")),
            ("Enter", t("key-change")),
            ("p", t("key-presets")),
            ("a", t("key-region-add")),
        ];
        if self.editor.can_undo() {
            keys.push(("u", t("key-undo")));
        }
        if self.editor.can_redo() {
            keys.push(("U", t("key-redo")));
        }
        keys.push(("c", tf("key-commit", &[("count", self.editor.draft.count().into())])));
        keys
    }

    /// None: close (back to the list). A key that changed the draft is remembered for `u`.
    pub fn on_key(&mut self, key: KeyEvent, ctx: &Ctx) -> Option<Action> {
        let before = self.editor.draft.clone();
        // `u` / `U` themselves move through the history: not a change to remember
        let undoing = matches!(key.code, KeyCode::Char('u' | 'U')) && !self.typing();
        let action = self.key(key, ctx);
        if !undoing {
            self.editor.track(before);
        }
        action
    }

    fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Option<Action> {
        let nodes = self.nodes(ctx);
        // all unplugged: any key goes back; still being read: only Esc
        let Some(reference) = reference(&nodes) else { return None };
        if nodes.iter().any(|n| !n.inspected) {
            return if key.code == KeyCode::Esc { None } else { Some(Action::None) };
        }
        if self.editor.input.is_some() {
            self.editor.typing_key(key, &reference);
            return Some(Action::None);
        }
        // the radio presets: the preset's values for every node (each node then changes only
        // what it does not have already)
        if let Some(picker) = &mut self.presets {
            match picker.on_key(key) {
                PresetKey::Not => {}
                PresetKey::Closed => self.presets = None,
                PresetKey::Chosen(preset) => {
                    self.editor.apply_preset(&reference, preset);
                    self.row = settings(&reference).iter().position(|(_, e)| is_radio(&e.id)).unwrap_or(self.row);
                    self.presets = None;
                }
            }
            return Some(Action::None);
        }
        // the region chooser: a region of the list (again: taken back), or a new name (added
        // to the list too)
        if let Some(picker) = &mut self.picker {
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
            self.picker = None;
            if let Some(name) = name {
                if self.editor.draft.regions.add.iter().any(|r| r.name == name) {
                    self.editor.undo_region(&name);
                } else {
                    self.editor.add_region(&ctx.regions, &name, &[]);
                }
            }
            return Some(save.map_or(Action::None, |list| Action::Send(vec![Cmd::SaveRegions(list)])));
        }
        self.editor.problem = None;
        let set = settings(&reference);
        let id = set.get(self.row).map(|(_, e)| e.id.clone());
        match key.code {
            // nothing changed: just leave; else ask, in the middle, with what would be lost
            KeyCode::Esc => return self.editor.leave(&reference),
            KeyCode::Enter => {
                if let Some(id) = id {
                    self.editor.start(Target::Setting(id), &reference);
                }
            }
            // back / forward through the changes, one at a time
            KeyCode::Char('u') => self.editor.undo_last(),
            KeyCode::Char('U') => self.editor.redo_last(),
            KeyCode::Char('c') => return Some(self.commit(&nodes)),
            KeyCode::Char('p') if set.iter().any(|(_, e)| is_radio(&e.id)) => self.presets = Some(PresetPicker::on(&reference)),
            KeyCode::Char('p') => self.editor.problem = Some(t("group-no-radio")),
            KeyCode::Char('a') if nodes.iter().any(|n| n.regions.is_some()) => self.picker = Some(RegionPicker::default()),
            KeyCode::Char('a') => self.editor.problem = Some(t("group-no-regions")),
            code => self.row = step(self.row, set.len(), code),
        }
        Some(Action::None)
    }

    /// The commit question: node by node, what will be written (a node where nothing changes is
    /// said so, and left alone)
    fn commit(&mut self, nodes: &[&Node]) -> Action {
        let mut lines = Vec::new();
        let mut writes = Vec::new();
        for node in nodes {
            // this node's own changes: a value it already has is none
            let mut draft = Draft::default();
            for (id, value) in &self.editor.draft.values {
                if current(node, id).is_none_or(|f| f.value != *value || f.secret) {
                    draft.values.insert(id.clone(), value.clone());
                }
            }
            // regions: each node adds only what it does not have (`plan` leaves the rest)
            draft.regions = self.editor.draft.regions.clone();
            let steps = match plan(node, &draft) {
                Ok(steps) => steps,
                Err(problem) => {
                    let problem = format!("{}: {problem}", clean(&node.name));
                    self.editor.problem = Some(problem.clone());
                    return Action::None;
                }
            };
            if steps.is_empty() {
                lines.push(tf("group-node-unchanged", &[("name", clean(&node.name).into())]));
                continue;
            }
            lines.push(format!("{}:", clean(&node.name)));
            for step in &steps {
                lines.push(format!("  {} {}", if step.danger { "!" } else { "·" }, step.line));
            }
            writes.push(((*node).clone(), steps));
        }
        if writes.is_empty() {
            return Action::None;
        }
        let count: usize = writes.iter().map(|(_, steps)| steps.len()).sum();
        let mut ask = vec![tf("group-commit-ask", &[("nodes", writes.len().into()), ("count", count.into())]), String::new()];
        ask.extend(lines);
        ask.push(String::new());
        if writes.iter().any(|(_, steps)| steps.iter().any(|s| s.danger)) {
            ask.push(t("commit-danger"));
        }
        ask.push(t("commit-once"));
        Action::Ask(commit_question(ask, writes))
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        self.draw_page(f, area, ctx);
        if let Some(picker) = &self.picker {
            let adding: Vec<String> = self.editor.draft.regions.add.iter().map(|r| r.name.clone()).collect();
            picker.draw(f, area, &ctx.regions, &adding);
        }
        // the presets, the first node's own marked
        if let (Some(picker), Some(first)) = (&self.presets, self.nodes(ctx).first()) {
            draw_presets(f, area, first, picker);
        }
    }

    fn draw_page(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        let nodes = self.nodes(ctx);
        let width = area.width as usize;
        let [head, names, _, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)])
                .areas(area);

        // "3 × repeater   EDIT   2 changes not sent", then the nodes' names
        let count = self.editor.draft.count();
        let title = tf("group-title", &[("count", nodes.len().into()), ("kind", self.kind.label().into())]);
        let pending = format!(" {} ", tf("edit-pending", &[("count", count.into())]));
        let who = Line::from(vec![
            Span::styled(format!("{title}  "), theme::heading()),
            Span::styled(format!(" {} ", t("edit-badge")), theme::edit_badge()),
            Span::styled(pending, if count == 0 { theme::field() } else { theme::edit_count() }),
        ]);
        let all: Vec<String> = nodes.iter().map(|n| clean(&n.name)).collect();
        f.render_widget(Paragraph::new(ellipsis(who, width)), head);
        f.render_widget(Paragraph::new(ellipsis(Line::styled(all.join("  ·  "), theme::dim()), width)), names);

        let Some(reference) = reference(&nodes) else {
            let text = Paragraph::new(t("group-gone")).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel(&t("edit-title"))), body);
            return;
        };
        // their settings come from reading them: wait for every one
        if nodes.iter().any(|n| !n.inspected) {
            // the reading of one of them (they are read side by side)
            match nodes.iter().find_map(|n| ctx.job_on(&n.port)) {
                Some(job) => loading(f, body, &t("node-edit-settings"), job),
                None => f.render_widget(Paragraph::new(t("node-not-read")).style(theme::dim()).block(panel(&t("node-edit-settings"))), body),
            }
            return;
        }
        let set = settings(&reference);
        if set.is_empty() {
            let text = Paragraph::new(t("group-nothing-common")).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel(&t("node-edit-settings"))), body);
            return;
        }
        self.row = self.row.min(set.len() - 1);

        // the settings beside what the selected one does on each node (below when narrow)
        let (list, side) = if area.width >= READING + READING / 2 {
            let [list, side] = Layout::horizontal([Constraint::Min(0), Constraint::Percentage(45)]).areas(body);
            (list, side)
        } else {
            let [list, side] = Layout::vertical([Constraint::Min(5), Constraint::Percentage(45)]).areas(body);
            (list, side)
        };
        let [list, problem] = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(list);
        let facts: Vec<&Fact> = set.iter().map(|(f, _)| *f).collect();
        let label = label_width(facts.iter().map(|f| f.label.as_str()));
        let room = inner_width(list);
        let stacked = stacks(&facts, room);
        let mut lines: Vec<Line> = Vec::new();
        let mut selected_end = 0;
        for (i, (fact, edit)) in set.iter().enumerate() {
            let selected = i == self.row;
            let mark = Span::styled(if edit.danger { "! " } else { "  " }, theme::warn());
            let name = Span::styled(fact.label.clone(), if selected { theme::heading() } else { theme::dim() });
            let typing = self.editor.input.as_ref().filter(|(target, _)| *target == Target::Setting(edit.id.clone()));
            let mut value = Vec::new();
            match (typing, self.editor.draft.values.get(&edit.id)) {
                (Some((_, text)), _) if !self.editor.choices.is_empty() => {
                    for choice in &self.editor.choices {
                        let style = if choice.eq_ignore_ascii_case(text) { theme::edit_badge() } else { theme::field() };
                        value.push(Span::styled(format!(" {choice} "), style));
                        value.push(Span::raw(" "));
                    }
                }
                // an edit mode: secrets in full (`SHOWN`)
                (Some((_, text)), _) => value.extend(input(&masked(fact, text, SHOWN), text.chars().count().max(24) + 2)),
                (None, Some(new)) => {
                    value.push(summary(&nodes, &edit.id).patch_style(theme::dim()));
                    value.push(Span::styled(" → ", theme::dim()));
                    value.push(Span::styled(masked(fact, new, SHOWN), theme::changed()));
                }
                (None, None) => value.push(summary(&nodes, &edit.id)),
            }
            let item = pair_lines(mark, name, value, label, stacked, room);
            if selected && typing.is_none() {
                lines.extend(item.into_iter().map(|l| l.style(theme::selected())));
            } else {
                lines.extend(item);
            }
            if selected {
                selected_end = lines.len();
            }
        }
        let shown = list.height.saturating_sub(2) as usize;
        let scroll = selected_end.saturating_sub(shown);
        let total = lines.len();
        let text = Paragraph::new(lines).scroll((scroll as u16, 0));
        f.render_widget(text.block(panel_focus(&t("node-edit-settings"), true)), list);
        more_marks(f, list, scroll, shown, total);
        // a refused value, else the regions being added to every node
        let adding: Vec<&str> = self.editor.draft.regions.add.iter().map(|r| r.name.as_str()).collect();
        let why = match (&self.editor.problem, self.editor.room()) {
            (Some(problem), _) => Line::styled(format!("× {problem}"), theme::bad()),
            // typing a value with a limit: the room left
            (None, Some((used, max))) => room_line(used, max),
            (None, None) if adding.is_empty() => Line::raw(""),
            (None, None) => Line::styled(tf("group-regions-adding", &[("names", adding.join(", ").into())]), theme::changed()),
        };
        let why = ellipsis(why, problem.width as usize);
        f.render_widget(Paragraph::new(why), problem);

        // the selected setting, node by node: now, and after the commit
        let Some((fact, edit)) = set.get(self.row) else { return };
        let new = self.editor.draft.values.get(&edit.id);
        let rows = nodes.iter().map(|node| {
            let now = current(node, &edit.id);
            let after = match (new, now) {
                (Some(new), Some(now)) if now.value == *new && !now.secret => Line::styled(t("group-no-change"), theme::dim()),
                (Some(new), _) => Line::styled(masked(fact, new, SHOWN), theme::changed()),
                (None, _) => Line::raw(""),
            };
            vec![Line::raw(clean(&node.name)), Line::from(now.map_or(Span::raw("-"), |f| value_span(f, SHOWN))), after]
        });
        let grid = Grid::new(vec![Col::new(t("col-node")).grow(), Col::new(t("col-now")), Col::new(t("col-after"))]).rows(rows);
        let title = tf("group-on-each", &[("what", fact.label.as_str().into())]);
        grid.show(f, side, panel(&title), &mut TableState::default());
    }
}

/// One node standing for all of them, for the editor: the first one with only the settings every
/// node has (what makes a node itself left out). Its rules and limits apply to all (one kind).
fn reference(nodes: &[&Node]) -> Option<Node> {
    let first = nodes.first()?;
    let mut node = (*first).clone();
    node.facts.retain(|f| {
        f.edit.as_ref().is_some_and(|e| {
            !OWN.contains(&e.id.as_str()) && nodes.iter().all(|n| current(n, &e.id).is_some())
        })
    });
    Some(node)
}

/// The setting `id` of `node`
fn current<'a>(node: &'a Node, id: &str) -> Option<&'a Fact> {
    node.facts.iter().find(|f| f.edit.as_ref().is_some_and(|e| e.id == id))
}

/// The value the nodes have: the one value when they agree, else "3 different values"
fn summary(nodes: &[&Node], id: &str) -> Span<'static> {
    let mut values: Vec<&str> = nodes.iter().filter_map(|n| current(n, id)).map(|f| f.value.as_str()).collect();
    values.sort_unstable();
    values.dedup();
    match (values.len(), nodes.first().and_then(|n| current(n, id))) {
        (1, Some(fact)) => value_span(fact, SHOWN),
        (count, _) => Span::styled(tf("group-mixed", &[("count", count.into())]), theme::warn()),
    }
}

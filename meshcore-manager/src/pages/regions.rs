//! Regions: the user's own list of region names (regions.txt, in the firmware's `region`
//! format, src/regions.rs), as a tree. It is what the picker offers when a region is added to a
//! node (one node's view, or several checked nodes together), so names are typed once and spelt
//! the same on every node. `a` adds a region under the selected one, `A` at the top, `x`
//! removes one (with the ones under it, after asking). Regions a node has that are not in the
//! list can be copied in from that node's view (`+`).
//!
//! Also the picker (`RegionPicker`): the list, narrowed by what is typed; Enter takes the
//! selected region, or the typed name when nothing matches (it is then added to the list too).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, TableState, Wrap};
use ratatui::Frame;

use super::{step, Page};
use crate::app::{Action, Confirm, Ctx, Popup};
use crate::help;
use crate::i18n::{t, tf};
use crate::regions::{self, Region, NAME_MAX};
use crate::theme;
use crate::widgets::{
    inner_width, input, note_above, panel, panel_focus, room_line, sheet, text_rows, Col, Grid, PANEL_EDGES, READING,
};
use crate::worker::Cmd;

#[derive(Default)]
pub struct Regions {
    row: usize,
    table: TableState,
    /// A new region being typed: (its parent, the name so far)
    adding: Option<(Option<String>, String)>,
    /// Why the last key or name was refused
    problem: Option<String>,
    /// Asked to remove these (a region and the ones under it): while "remove" has the focus,
    /// they are shown as going
    removing: Vec<String>,
}

impl Page for Regions {
    fn title(&self) -> String {
        t("tab-regions")
    }

    fn keys(&self) -> Vec<(&'static str, String)> {
        if self.adding.is_some() {
            return vec![("Enter", t("key-ok")), ("Esc", t("key-cancel"))];
        }
        vec![
            ("↑↓", t("key-select")),
            ("a", t("key-region-add-under")),
            ("A", t("key-region-add-top")),
            ("x", t("key-remove")),
        ]
    }

    fn typing(&self) -> bool {
        self.adding.is_some()
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        let tree = regions::ordered(&ctx.regions);
        let title = tf("regions-title", &[("count", tree.len().into())]);
        // the question closed: nothing is being removed any more
        if ctx.asking.is_none() {
            self.removing.clear();
        }
        // "remove" (the first button) has the focus: the rows it takes are shown going
        let going = ctx.asking == Some(0);
        // on top: what this list is and why it is here
        let area = note_above(f, area, t("regions-about"));
        // below, only when there is something: the name being typed (with the room left), and
        // why a key or a name was refused
        let mut lines = Vec::new();
        if let Some((parent, text)) = &self.adding {
            let under = parent.as_deref().map_or_else(|| t("regions-top"), |p| tf("regions-under", &[("name", p.into())]));
            let mut spans = vec![Span::styled(format!("{under}  "), theme::dim())];
            spans.extend(input(text, NAME_MAX + 2));
            spans.push(Span::raw("  "));
            spans.extend(room_line(text.len(), NAME_MAX).spans);
            lines.push(Line::from(spans));
        }
        if let Some(problem) = &self.problem {
            lines.push(Line::styled(format!("× {problem}"), theme::bad()));
        }
        // the panel below as high as its text (wrapped in a narrow window), the tree gets the rest
        let high = if lines.is_empty() { 0 } else { text_rows(&lines, inner_width(area)) as u16 + 2 };
        let [list, below] = Layout::vertical([Constraint::Min(3), Constraint::Length(high)]).areas(area);
        if tree.is_empty() && self.adding.is_none() {
            let text = Paragraph::new(t("regions-empty")).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel(&title)), list);
        } else {
            self.row = self.row.min(tree.len().saturating_sub(1));
            let rows = tree.iter().map(|(depth, r)| {
                let name = format!("{}{}", "  ".repeat(*depth), r.name);
                // where it came from: the nodes it was copied from (`+` in a node's view)
                let from = Line::styled(r.from.join(", "), theme::dim());
                if going && self.removing.contains(&r.name) {
                    (vec![Line::raw(name), from, Line::styled(t("edit-removed"), theme::bad())], theme::removed())
                } else {
                    (vec![Line::raw(name), from, Line::raw("")], Style::new())
                }
            });
            let grid = Grid::new(vec![
                Col::new(t("col-name")).grow(),
                Col::new(t("col-from")).drop(1),
                Col::new(String::new()),
            ])
            .styled(rows);
            self.table.select((!tree.is_empty()).then_some(self.row));
            grid.show(f, list, panel_focus(&title, true), &mut self.table);
        }
        if !lines.is_empty() {
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(panel(&t("regions-new"))), below);
        }
    }

    fn on_key(&mut self, key: KeyEvent, ctx: &Ctx) -> Action {
        let tree = regions::ordered(&ctx.regions);
        let selected = tree.get(self.row).map(|(_, r)| r.name.clone());
        if let Some((parent, text)) = &mut self.adding {
            match key.code {
                KeyCode::Esc => self.adding = None,
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Enter => {
                    let Some(name) = regions::clean_name(text) else { return Action::None };
                    if ctx.regions.iter().any(|r| r.name == name) {
                        self.problem = Some(tf("regions-exists", &[("name", name.into())]));
                        return Action::None;
                    }
                    let mut list = ctx.regions.clone();
                    list.push(Region::new(name, parent.clone()));
                    self.adding = None;
                    return Action::Send(vec![Cmd::SaveRegions(list)]);
                }
                KeyCode::Char(c) => match type_name(text, c) {
                    Ok(()) => self.problem = None,
                    Err(problem) => self.problem = Some(problem),
                },
                _ => {}
            }
            return Action::None;
        }
        self.problem = None;
        match key.code {
            KeyCode::Char('a') => self.adding = Some((selected, String::new())),
            KeyCode::Char('A') => self.adding = Some((None, String::new())),
            KeyCode::Char('x') | KeyCode::Delete => {
                let Some(name) = selected else { return Action::None };
                let gone = regions::with_children(&ctx.regions, &name);
                let list: Vec<Region> = ctx.regions.iter().filter(|r| !gone.contains(&r.name)).cloned().collect();
                // the ones under it by name, so the question says exactly what goes
                let under: Vec<&str> = gone.iter().filter(|n| **n != name).map(String::as_str).collect();
                let mut lines = vec![tf("regions-remove-ask", &[("name", name.as_str().into()), ("count", under.len().into())])];
                if !under.is_empty() {
                    lines.push(tf("regions-remove-under", &[("names", under.join(", ").into())]));
                }
                lines.push(t("regions-remove-nodes"));
                self.removing = gone;
                return Action::Ask(
                    Popup::ask(t("tab-regions"), lines, Confirm::Send(vec![Cmd::SaveRegions(list)]), t("regions-remove-yes"), t("answer-cancel"))
                        .safe(),
                );
            }
            code => self.row = step(self.row, tree.len(), code),
        }
        Action::None
    }
}

/// A key typed into a region name: lowercase, only what a name can hold, at most NAME_MAX bytes
fn type_name(text: &mut String, c: char) -> Result<(), String> {
    let c = c.to_lowercase().next().unwrap_or(c);
    if !regions::name_char(c) {
        return Err(help::bad_char(&t("regions-name"), c));
    }
    if text.len() + c.len_utf8() > NAME_MAX {
        return Err(help::too_long(&t("regions-name"), NAME_MAX));
    }
    text.push(c);
    Ok(())
}

/// Choosing a region for a node: the user's list, narrowed by what is typed
#[derive(Default)]
pub struct RegionPicker {
    pub filter: String,
    row: usize,
    problem: Option<String>,
}

/// What a key in the picker did
pub enum Picked {
    /// Still choosing
    Not,
    /// Closed, nothing chosen
    Closed,
    /// This region of the list
    Region(String),
    /// A name typed that is not in the list: to add to the node and to the list
    New(String),
}

impl RegionPicker {
    /// The regions shown: the whole tree, or those whose name holds what was typed
    fn shown<'a>(&self, library: &'a [Region]) -> Vec<(usize, &'a Region)> {
        let tree = regions::ordered(library);
        if self.filter.is_empty() {
            return tree;
        }
        tree.into_iter().filter(|(_, r)| r.name.contains(&self.filter)).map(|(_, r)| (0, r)).collect()
    }

    /// What was typed, as a new name, when it is a valid name and not exactly one of the list:
    /// offered as the last row ("fr" when the list only has "ch-fr")
    fn new_name(&self, library: &[Region]) -> Option<String> {
        regions::clean_name(&self.filter).filter(|name| !library.iter().any(|r| &r.name == name))
    }

    pub fn on_key(&mut self, key: KeyEvent, library: &[Region]) -> Picked {
        let shown = self.shown(library);
        let new = self.new_name(library);
        match key.code {
            KeyCode::Esc => return Picked::Closed,
            KeyCode::Enter => {
                if let Some((_, region)) = shown.get(self.row) {
                    return Picked::Region(region.name.clone());
                }
                if let Some(name) = new {
                    return Picked::New(name);
                }
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.row = 0;
            }
            KeyCode::Char(c) => {
                self.problem = type_name(&mut self.filter, c).err();
                self.row = 0;
            }
            // the new name is one more row, under the list
            code => self.row = step(self.row, shown.len() + new.is_some() as usize, code),
        }
        Picked::Not
    }

    /// Over the page: what was typed, the regions (● = the node has it), how it works
    pub fn draw(&self, f: &mut Frame, area: Rect, library: &[Region], have: &[String]) {
        let shown = self.shown(library);
        let new = self.new_name(library);
        let rows_count = shown.len() + new.is_some() as usize;
        let rect = sheet(area, READING.min(64 + PANEL_EDGES), rows_count as u16 + 10);
        crate::widgets::lift(f, rect);
        let block = panel_focus(&t("regions-pick-title"), true);
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let [find, _, list, _, note] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .areas(inner);
        let label = format!("{}  ", t("presets-find"));
        let mut line = vec![Span::styled(label.clone(), theme::dim())];
        line.extend(input(&self.filter, (find.width as usize).saturating_sub(label.chars().count())));
        f.render_widget(Paragraph::new(Line::from(line)), find);
        if rows_count == 0 {
            let text = if library.is_empty() { t("regions-pick-empty") } else { t("presets-no-match") };
            f.render_widget(Paragraph::new(Line::styled(text, theme::dim())).wrap(Wrap { trim: false }), list);
        } else {
            let mut rows: Vec<Vec<Line>> = shown
                .iter()
                .map(|(depth, r)| {
                    let mark = if have.contains(&r.name) { Line::styled("●", theme::title()) } else { Line::raw(" ") };
                    vec![mark, Line::raw(format!("{}{}", "  ".repeat(*depth), r.name))]
                })
                .collect();
            // the typed name, when it is a new one: added to the node and to the list
            if let Some(name) = new {
                rows.push(vec![
                    Line::styled("+", theme::changed()),
                    Line::styled(tf("regions-pick-new", &[("name", name.into())]), theme::changed()),
                ]);
            }
            let grid = Grid::new(vec![Col::new(String::new()), Col::new(String::new()).grow()]).rows(rows);
            let mut state = TableState::default().with_selected(Some(self.row.min(rows_count - 1)));
            f.render_stateful_widget(grid.table(list.width as usize), list, &mut state);
        }
        let hint = match &self.problem {
            Some(problem) => Line::styled(format!("× {problem}"), theme::bad()),
            // typing: the room left first, then what Enter does
            None if !self.filter.is_empty() => {
                let mut spans = room_line(self.filter.len(), NAME_MAX).spans;
                spans.push(Span::styled(format!("   {}", t("regions-pick-note")), theme::dim()));
                Line::from(spans)
            }
            None => Line::styled(t("regions-pick-note"), theme::dim()),
        };
        f.render_widget(Paragraph::new(hint).wrap(Wrap { trim: false }), note);
    }
}

//! The Access tab's chooser (edit mode, `a`): people from the address book put in the node's
//! access list, a whole role at once or one by one, never by typing a key. Shows the companions
//! (people) of the book and the keys seen in other access lists, not the ones already in this
//! list; opens on your admins when you have some. Letters search a name, ← → shows one role only
//! (the role you gave them on the Contacts page, `r`), Space checks, `+` checks every one shown,
//! Enter adds the checked ones (else the selected one). Each gets the role you gave it in the
//! book; one without a role gets the role shown, else read only (never more rights than asked
//! for). A whole key pasted in the search, in none of your contacts, is the last resort.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, TableState, Wrap};
use ratatui::Frame;

use super::contact_kind;
use crate::book::{Book, Person};
use crate::i18n::{t, tf};
use crate::node::acl;
use crate::theme;
use crate::widgets::{self, clean, input, panel_focus, sheet, Col, Grid, PANEL_EDGES};

#[derive(Default)]
pub struct AclPick {
    row: usize,
    find: String,
    /// One role only (`acl::ROLES`), None = all
    only: Option<u8>,
    /// The keys checked
    checked: Vec<String>,
}

pub enum Picked {
    Not,
    Closed,
    /// (key, role) to add
    Add(Vec<(String, u8)>),
}

impl AclPick {
    /// Opens on your admins when you have some (the usual reason to open it), else on everyone
    pub fn new(book: &Book) -> AclPick {
        let admins = book.people.iter().any(|p| p.role == acl::ADMIN);
        AclPick { only: admins.then_some(acl::ADMIN), ..AclPick::default() }
    }

    /// A whole public key typed or pasted in the search (spaces left out), for a key that is in
    /// no list yet: the last resort, never the way in
    fn pasted(&self) -> Option<String> {
        let key: String = self.find.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_uppercase();
        acl::valid_key(&key).then_some(key)
    }

    /// The keys offered: people (companions) and keys of other lists, not this node, not
    /// already in its list (`have`), searched, one role; A to Z
    fn shown<'a>(&self, book: &'a Book, own: &str, have: &[String]) -> Vec<&'a Person> {
        let find = self.find.to_lowercase();
        let mut list: Vec<&Person> = book
            .people
            .iter()
            .filter(|p| p.kind == 0 || p.kind == 1)
            .filter(|p| !p.key.eq_ignore_ascii_case(own) && !have.iter().any(|k| k.eq_ignore_ascii_case(&p.key)))
            .filter(|p| self.only.is_none_or(|r| p.role == r))
            .filter(|p| {
                find.is_empty()
                    || p.name.to_lowercase().contains(&find)
                    || p.own_name.to_lowercase().contains(&find)
                    || p.key.to_lowercase().starts_with(&find)
            })
            .collect();
        let name = |p: &Person| p.shown_name().trim_start_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        list.sort_by_key(|p| name(p));
        list
    }

    /// The role a key gets: yours from the book, else the one filtered on, else read only
    fn role_for(&self, p: &Person) -> u8 {
        if p.role != 0 {
            p.role
        } else {
            self.only.unwrap_or(acl::ROLES[0])
        }
    }

    pub fn on_key(&mut self, key: KeyEvent, book: &Book, own: &str, have: &[String]) -> Picked {
        let shown = self.shown(book, own, have);
        match key.code {
            KeyCode::Esc => return Picked::Closed,
            KeyCode::Enter => {
                let chosen: Vec<&Person> = if self.checked.is_empty() {
                    shown.get(self.row).copied().into_iter().collect()
                } else {
                    book.people.iter().filter(|p| self.checked.contains(&p.key)).collect()
                };
                if chosen.is_empty() {
                    // nobody in the book has it: a whole key pasted goes in as it is
                    return match self.pasted() {
                        Some(key) => Picked::Add(vec![(key, self.only.unwrap_or(acl::ROLES[0]))]),
                        None => Picked::Not,
                    };
                }
                return Picked::Add(chosen.iter().map(|p| (p.key.clone(), self.role_for(p))).collect());
            }
            KeyCode::Char(' ') => {
                if let Some(p) = shown.get(self.row) {
                    match self.checked.iter().position(|k| *k == p.key) {
                        Some(at) => {
                            self.checked.remove(at);
                        }
                        None => self.checked.push(p.key.clone()),
                    }
                    self.row = (self.row + 1).min(shown.len().saturating_sub(1));
                }
            }
            // every one shown checked, or none
            KeyCode::Char('+') => {
                if shown.iter().all(|p| self.checked.contains(&p.key)) {
                    self.checked.retain(|k| !shown.iter().any(|p| p.key == *k));
                } else {
                    for p in &shown {
                        if !self.checked.contains(&p.key) {
                            self.checked.push(p.key.clone());
                        }
                    }
                }
            }
            // all -> admin -> read-write -> read only -> all (and back)
            KeyCode::Left | KeyCode::Right => {
                let order = [None, Some(acl::ADMIN), Some(2), Some(1)];
                let at = order.iter().position(|o| *o == self.only).unwrap_or(0);
                let next = if key.code == KeyCode::Right { at + 1 } else { at + order.len() - 1 };
                self.only = order[next % order.len()];
                self.row = 0;
            }
            KeyCode::Backspace => {
                self.find.pop();
                self.row = 0;
            }
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End => {
                self.row = crate::pages::step(self.row, shown.len(), key.code);
            }
            _ => {
                if let Some(c) = widgets::typed(&key).filter(|c| !c.is_control()) {
                    self.find.push(c);
                    self.row = 0;
                }
            }
        }
        Picked::Not
    }

    pub fn draw(&self, f: &mut Frame, area: Rect, book: &Book, own: &str, have: &[String]) {
        let shown = self.shown(book, own, have);
        let rect = sheet(area, 110 + PANEL_EDGES, shown.len() as u16 + 10);
        widgets::lift(f, rect);
        let only = self.only.map_or_else(|| t("only-all"), acl::role_name);
        let title = tf("acl-pick-title", &[("count", self.checked.len().into())]);
        let block = panel_focus(&title, true);
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let [find, _, list, _, note] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1), Constraint::Length(1), Constraint::Length(2)])
                .areas(inner);
        // the search, and which role is shown
        let label = format!("{}  ", t("key-search"));
        let mut line = vec![Span::styled(label, theme::heading())];
        line.extend(input(&self.find, 24));
        line.push(Span::styled(format!("    {}  ", t("col-role")), theme::heading()));
        line.push(Span::styled(only, theme::title()));
        f.render_widget(Paragraph::new(Line::from(line)), find);
        if shown.is_empty() {
            let line = match self.pasted() {
                Some(key) => Line::styled(tf("acl-pick-pasted", &[("key", acl::short(&key).into())]), theme::changed()),
                None if book.people.is_empty() => Line::styled(t("acl-pick-empty"), theme::dim()),
                None => Line::styled(t("node-contacts-none-found"), theme::dim()),
            };
            f.render_widget(Paragraph::new(line).wrap(Wrap { trim: false }), list);
        } else {
            let rows = shown.iter().map(|p| {
                let mark = if self.checked.contains(&p.key) { Line::styled("[x]", theme::changed()) } else { Line::styled("[ ]", theme::dim()) };
                let role = if p.role == 0 { Line::styled("-", theme::dim()) } else { Line::styled(acl::role_name(p.role), theme::good()) };
                vec![
                    mark,
                    contact_kind(p.kind),
                    widgets::key_prefix(&p.key),
                    Line::raw(clean(&p.name)),
                    Line::styled(clean(&p.own_name), theme::title()),
                    role,
                ]
            });
            let grid = Grid::new(vec![
                Col::new(String::new()),
                Col::new(t("col-type")),
                Col::new(t("col-key")),
                Col::new(t("col-name")).grow(),
                Col::new(t("col-own-name")).grow(),
                Col::new(t("col-role")),
            ])
            .rows(rows);
            let mut state = TableState::default().with_selected(Some(self.row.min(shown.len() - 1)));
            f.render_stateful_widget(grid.table(list.width as usize), list, &mut state);
        }
        f.render_widget(Paragraph::new(Line::styled(t("acl-pick-note"), theme::dim())).wrap(Wrap { trim: false }), note);
    }
}

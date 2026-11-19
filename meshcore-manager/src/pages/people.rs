//! Contacts: the address book (contacts.toml, src/book.rs). Every contact read from your
//! companions and every key of an access list, kept by public key, with a custom name, an access
//! role and a favourite mark of yours. Filled by itself each time a node is read: nothing to
//! type in. `/` searches, `o` sorts, `f` shows one type / the favourites / one role, `n` a
//! custom name, `r` the role, `*` favourite (written to the companions that hold it, asked
//! first), `x` takes it out of the book (it comes back when a node read again still has it).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, TableState, Wrap};
use ratatui::Frame;

use super::node::contact_kind;
use super::{step, Page};
use crate::app::{Action, Confirm, Ctx, Popup};
use crate::book::{Change, Person};
use crate::i18n::{t, tf};
use crate::node::acl;
use crate::node::edit::{self, Step};
use crate::node::{Kind, Node};
use crate::proto;
use crate::theme;
use crate::widgets::{self, ago, clean, details_below, fact, facts, inner_width, input, input_cell, note_above, panel_focus, Col, Grid};
use crate::worker::Cmd;

/// The longest custom name (a line in a table, not a firmware limit)
const TEXT_MAX: usize = 48;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Sort {
    #[default]
    Name,
    Kind,
    Heard,
}

impl Sort {
    fn next(self) -> Sort {
        match self {
            Sort::Name => Sort::Kind,
            Sort::Kind => Sort::Heard,
            Sort::Heard => Sort::Name,
        }
    }

    fn word(self) -> String {
        t(match self {
            Sort::Name => "sort-name",
            Sort::Kind => "sort-kind",
            Sort::Heard => "sort-heard",
        })
    }
}

/// What `f` shows: all, one type (ADV_TYPE_*), the favourites, one access role
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Only {
    #[default]
    All,
    Kind(u8),
    Favourites,
    Role(u8),
}

impl Only {
    /// all -> companions -> repeaters -> rooms -> sensors -> favourites -> admins -> read-write
    /// -> read only -> all
    fn next(self) -> Only {
        match self {
            Only::All => Only::Kind(1),
            Only::Kind(k) if k < 4 => Only::Kind(k + 1),
            Only::Kind(_) => Only::Favourites,
            Only::Favourites => Only::Role(acl::ADMIN),
            Only::Role(r) if r > 1 => Only::Role(r - 1),
            Only::Role(_) => Only::All,
        }
    }

    fn word(self) -> String {
        match self {
            Only::All => t("only-all"),
            Only::Kind(kind) => contact_word(kind),
            Only::Favourites => t("people-favourites"),
            Only::Role(role) => acl::role_name(role),
        }
    }

    fn keeps(self, p: &Person) -> bool {
        match self {
            Only::All => true,
            Only::Kind(kind) => p.kind == kind,
            Only::Favourites => p.is_favourite(),
            Only::Role(role) => p.has_role(role),
        }
    }
}

/// What is being typed
enum Typing {
    Find,
    /// A custom name for this key
    Name(String),
}

#[derive(Default)]
pub struct People {
    row: usize,
    table: TableState,
    sort: Sort,
    only: Only,
    find: String,
    typing: Option<Typing>,
    /// The custom name being typed
    text: String,
    /// Keys checked (Space, `a`): `x` takes them all out of the book
    checked: Vec<String>,
}

impl People {
    /// The keys shown: searched, filtered, sorted
    fn shown<'a>(&self, ctx: &'a Ctx) -> Vec<&'a Person> {
        let find = self.find.to_lowercase();
        let mut list: Vec<&Person> = ctx
            .book
            .people
            .iter()
            .filter(|p| self.only.keeps(p))
            .filter(|p| {
                find.is_empty()
                    || p.name.to_lowercase().contains(&find)
                    || p.own_name.to_lowercase().contains(&find)
                    || p.key.to_lowercase().starts_with(&find)
                    || (p.role != 0 && acl::role_name(p.role).to_lowercase().contains(&find))
            })
            .collect();
        // A to Z by its letters: an emoji or a sign in front does not move a name to the end
        let name = |p: &Person| p.shown_name().trim_start_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        match self.sort {
            Sort::Name => list.sort_by_key(|p| name(p)),
            Sort::Kind => list.sort_by_key(|p| (if p.kind == 0 { u8::MAX } else { p.kind }, name(p))),
            Sort::Heard => list.sort_by_key(|p| std::cmp::Reverse(p.last_heard)),
        }
        list
    }

    fn type_key(&mut self, key: KeyEvent) -> Action {
        let Some(typing) = &self.typing else { return Action::None };
        let finding = matches!(typing, Typing::Find);
        match key.code {
            KeyCode::Esc => {
                if finding {
                    self.find.clear();
                }
                self.typing = None;
            }
            KeyCode::Enter => {
                let change = match typing {
                    Typing::Find => None,
                    Typing::Name(key) => Some(Change::OwnName { key: key.clone(), name: self.text.trim().to_string() }),
                };
                self.typing = None;
                if let Some(change) = change {
                    return Action::Send(vec![Cmd::Book(change)]);
                }
            }
            KeyCode::Backspace => {
                if finding {
                    self.find.pop();
                } else {
                    self.text.pop();
                }
            }
            _ => {
                if let Some(c) = widgets::typed(&key).filter(|c| !c.is_control()) {
                    if finding {
                        self.find.push(c);
                    } else if self.text.chars().count() < TEXT_MAX {
                        self.text.push(c);
                    }
                }
            }
        }
        if finding {
            self.row = 0;
        }
        Action::None
    }
}

/// `*`: the favourite mark changes in the book, and on every companion that holds the contact and
/// is read in the Fleet now (each by its own commit, asked first, as edit mode does). The ones
/// not read now are named: they keep their mark until `*` is used again with them read.
fn favourite(p: &Person, nodes: &[Node]) -> Action {
    let on = !p.is_favourite();
    let book = Cmd::Book(Change::Favourite { key: p.key.clone(), on });
    let what = t(if on { "edit-favourite-on" } else { "edit-favourite-off" });
    let mut writes: Vec<(Node, Vec<Step>)> = Vec::new();
    let mut away: Vec<String> = Vec::new();
    for h in p.held_by.iter().filter(|h| h.favourite != on) {
        let read = nodes.iter().find(|n| n.kind == Kind::Companion && n.inspected && n.public_key.eq_ignore_ascii_case(&h.node));
        match read.and_then(|n| n.contacts.iter().find(|c| c.public_key.eq_ignore_ascii_case(&p.key)).map(|c| (n, c))) {
            Some((node, c)) => {
                let line = tf("edit-contact", &[("name", c.name.as_str().into()), ("what", what.as_str().into())]);
                let step = Step { line, danger: false, change: edit::Change::Frame(proto::update_contact(c, &c.name, on)) };
                writes.push((node.clone(), vec![step]));
            }
            None => away.push(clean(&h.node_name)),
        }
    }
    if writes.is_empty() {
        // nothing to write now: only the book (the details show, companion by companion, where
        // the ★ is; one read later gets it with `*` again)
        return Action::Send(vec![book]);
    }
    let mut lines = vec![tf("people-favourite-ask", &[("name", clean(&p.shown_name()).into()), ("what", what.as_str().into()), ("count", writes.len().into())]), String::new()];
    for (node, steps) in &writes {
        lines.extend(steps.iter().map(|s| format!("  · {}: {}", clean(&node.name), s.line)));
    }
    if !away.is_empty() {
        lines.push(String::new());
        lines.push(tf("people-favourite-away", &[("nodes", away.join(", ").into())]));
    }
    // the book changes with the commit (cancel: nothing changes, here or on a node)
    let send = |backup_first: bool| {
        let mut cmds = vec![Cmd::Book(Change::Favourite { key: p.key.clone(), on })];
        cmds.extend(writes.iter().map(|(node, steps)| Cmd::Apply { node: node.clone(), steps: steps.clone(), backup_first }));
        Confirm::Send(cmds)
    };
    let count = writes.len();
    let yes = tf("commit-yes", &[("count", count.into())]);
    Action::Ask(Popup::ask(t("commit-title"), lines, send(false), yes, t("commit-no")).first(t("commit-backup-first"), send(true)))
}

impl Page for People {
    fn title(&self) -> String {
        t("tab-contacts")
    }

    fn keys(&self) -> Vec<(&'static str, String)> {
        match &self.typing {
            Some(Typing::Find) => return vec![("a-z", t("key-find")), ("Enter", t("key-ok")), ("Esc", t("key-find-clear"))],
            Some(_) => return vec![("Enter", t("key-save")), ("Esc", t("key-cancel"))],
            None => {}
        }
        vec![
            ("↑↓", t("key-select")),
            ("/", t("key-search")),
            ("n", t("key-own-name")),
            ("*", t("key-favourite")),
            ("r", t("key-role")),
            ("o", tf("key-sort", &[("now", self.sort.word().into())])),
            ("f", tf("key-only", &[("now", self.only.word().into())])),
            ("Space", t("key-check")),
            ("a", t(if self.checked.is_empty() { "key-check-all" } else { "key-uncheck-all" })),
            if self.checked.is_empty() { ("x", t("key-remove")) } else { ("x", tf("key-delete-n", &[("count", self.checked.len().into())])) },
        ]
    }

    fn typing(&self) -> bool {
        self.typing.is_some()
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        let area = note_above(f, area, t("people-about"));
        let shown = self.shown(ctx);
        let mut title = tf("people-title", &[("count", ctx.book.people.len().into()), ("sort", self.sort.word().into())]);
        if self.only != Only::All {
            title = format!("{title} · {}", self.only.word());
        }
        // the search, above the list while typed or kept
        let finding = matches!(self.typing, Some(Typing::Find));
        let area = if finding || !self.find.is_empty() {
            title = tf("node-contacts-found", &[("title", title.into()), ("count", shown.len().into())]);
            let mut spans = vec![Span::styled(format!("{}  ", t("key-search")), theme::heading())];
            spans.extend(if finding { input(&self.find, 24) } else { vec![Span::styled(self.find.clone(), theme::title())] });
            let [line, rest] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
            f.render_widget(Paragraph::new(Line::from(spans)), line);
            rest
        } else {
            area
        };
        if shown.is_empty() {
            let id = if ctx.book.people.is_empty() { "people-empty" } else { "node-contacts-none-found" };
            let text = Paragraph::new(t(id)).style(theme::dim()).wrap(Wrap { trim: false });
            f.render_widget(text.block(panel_focus(&title, true)), area);
            return;
        }
        self.row = self.row.min(shown.len() - 1);
        let p = shown[self.row];
        // below: everything the book knows of the selected key
        // the same lines for every key ("-" when empty): the list does not jump up and down
        let or_dash = |text: String| if text.is_empty() { "-".to_string() } else { text };
        // the name it advertises, then the other names companions hold for it
        let first = clean(&p.name);
        let mut names = vec![first.clone()];
        names.extend(p.other_names().iter().map(|n| clean(n)).filter(|n| *n != first));
        names.retain(|n| !n.is_empty());
        let heard = if p.last_heard == 0 { String::new() } else { format!("{}  ({})", widgets::when(p.last_heard), ago(p.last_heard)) };
        // each companion, ★ where it is a favourite there
        let held = p.held_by.iter().map(|h| format!("{}{}", if h.favourite { "★ " } else { "" }, clean(&h.node_name)));
        let access = p.access.iter().map(|a| format!("{}: {}", clean(&a.node_name), acl::role_name(a.perms)));
        let details = vec![
            fact(t("field-public-key"), p.key.clone()),
            fact(t("field-advertised"), or_dash(names.join(", "))),
            fact(t("field-heard"), or_dash(heard)),
            fact(t("field-held-by"), or_dash(held.collect::<Vec<_>>().join(", "))),
            fact(t("field-role"), if p.role == 0 { "-".into() } else { acl::role_name(p.role) }),
            fact(t("field-access"), or_dash(access.collect::<Vec<_>>().join(", "))),
        ];
        let lines = facts(details, inner_width(area));
        let list = details_below(f, area, &clean(&p.shown_name()), lines, 5);

        let checking = !self.checked.is_empty();
        let rows = shown.iter().map(|p| {
            let own = match &self.typing {
                Some(Typing::Name(key)) if *key == p.key => Line::from(input_cell(&self.text, TEXT_MAX + 1)),
                _ => Line::styled(clean(&p.own_name), theme::title()),
            };
            // ★ amber; dim when the companions holding it do not all have it the same
            let star = if p.is_favourite() {
                let all = p.held_by.iter().all(|h| h.favourite);
                Line::styled("★", if all { theme::warn() } else { theme::dim() })
            } else {
                Line::raw(" ")
            };
            let role = if p.role == 0 { Line::raw("") } else { Line::styled(acl::role_name(p.role), theme::good()) };
            // where it is: "3 companions", "access on 2"
            let mut on = Vec::new();
            if !p.held_by.is_empty() {
                on.push(tf("people-held", &[("count", p.held_by.len().into())]));
            }
            if !p.access.is_empty() {
                on.push(tf("people-access", &[("count", p.access.len().into())]));
            }
            // ★, type, key, then the names, then the role and where it is; [x] first once one is
            // checked
            let mut row = vec![
                star,
                contact_kind(p.kind),
                widgets::key_prefix(&p.key),
                Line::raw(clean(&p.name)),
                own,
                role,
                Line::styled(on.join(", "), theme::dim()),
            ];
            if checking {
                let mark = if self.checked.contains(&p.key) { Line::styled("[x]", theme::bad()) } else { Line::styled("[ ]", theme::dim()) };
                row.insert(0, mark);
            }
            row
        });
        let mut cols = vec![
            Col::new(String::new()),
            Col::new(t("col-type")),
            Col::new(t("col-key")).drop(1),
            Col::new(t("col-name")).grow(),
            Col::new(t("col-own-name")).grow(),
            Col::new(t("col-role")).drop(3),
            Col::new(t("col-on")).drop(2),
        ];
        if checking {
            cols.insert(0, Col::new(String::new()));
            title = tf("node-contacts-checked", &[("title", title.into()), ("count", self.checked.len().into())]);
        }
        let grid = Grid::new(cols).rows(rows);
        self.table.select(Some(self.row));
        grid.show(f, list, panel_focus(&title, true), &mut self.table);
    }

    fn on_key(&mut self, key: KeyEvent, ctx: &Ctx) -> Action {
        if self.typing.is_some() {
            return self.type_key(key);
        }
        let shown = self.shown(ctx);
        let selected = shown.get(self.row).map(|p| (*p).clone());
        match key.code {
            KeyCode::Char('/') => self.typing = Some(Typing::Find),
            KeyCode::Char('n') | KeyCode::Enter => {
                if let Some(p) = selected {
                    self.text = p.own_name.clone();
                    self.typing = Some(Typing::Name(p.key));
                }
            }
            KeyCode::Char('*') => {
                if let Some(p) = selected {
                    return favourite(&p, &ctx.nodes);
                }
            }
            // none -> read only -> read-write -> admin -> none (saved at once: only in the book)
            KeyCode::Char('r') => {
                if let Some(p) = selected {
                    let role = if p.role >= acl::ADMIN { 0 } else { p.role + 1 };
                    return Action::Send(vec![Cmd::Book(Change::Role { key: p.key, role })]);
                }
            }
            KeyCode::Char('o') => {
                self.sort = self.sort.next();
                let shown = self.shown(ctx);
                self.row = selected.and_then(|s| shown.iter().position(|p| p.key == s.key)).unwrap_or(0);
            }
            // the next filter; the selected one stays selected when it is still shown
            KeyCode::Char('f') => {
                self.only = self.only.next();
                let shown = self.shown(ctx);
                self.row = selected.as_ref().and_then(|s| shown.iter().position(|p| p.key == s.key)).unwrap_or(0);
            }
            KeyCode::Char(' ') => {
                if let Some(p) = selected {
                    match self.checked.iter().position(|k| *k == p.key) {
                        Some(at) => {
                            self.checked.remove(at);
                        }
                        None => self.checked.push(p.key),
                    }
                    self.row = (self.row + 1).min(shown.len().saturating_sub(1));
                }
            }
            // every one shown checked, or none
            KeyCode::Char('a') => {
                if shown.iter().all(|p| self.checked.contains(&p.key)) {
                    self.checked.clear();
                } else {
                    self.checked = shown.iter().map(|p| p.key.clone()).collect();
                }
            }
            KeyCode::Esc if !self.checked.is_empty() => self.checked.clear(),
            // the checked ones, else the selected one: out of the book (asked first)
            KeyCode::Char('x') | KeyCode::Delete => {
                let keys: Vec<String> = if self.checked.is_empty() { selected.iter().map(|p| p.key.clone()).collect() } else { self.checked.clone() };
                let Some(first) = keys.first() else { return Action::None };
                let lines = if keys.len() == 1 {
                    let name = ctx.book.find(first).map(|p| clean(&p.shown_name())).unwrap_or_default();
                    vec![tf("people-remove-ask", &[("name", name.into())]), t("people-remove-back")]
                } else {
                    vec![tf("people-remove-ask-n", &[("count", keys.len().into())]), t("people-remove-back")]
                };
                let cmds = keys.into_iter().map(|key| Cmd::Book(Change::Remove { key })).collect();
                self.checked.clear();
                return Action::Ask(Popup::ask(t("tab-contacts"), lines, Confirm::Send(cmds), t("people-remove-yes"), t("answer-cancel")).safe());
            }
            code => self.row = step(self.row, shown.len(), code),
        }
        Action::None
    }
}

/// "repeater" for the `f` filter
fn contact_word(kind: u8) -> String {
    t(match kind {
        1 => "contact-chat",
        2 => "contact-repeater",
        3 => "contact-room",
        _ => "contact-sensor",
    })
}

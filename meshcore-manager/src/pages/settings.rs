//! Settings: the program's own settings (the nodes' are in their own view). Each is used at once
//! and saved to config.toml (config::set: in place, comments kept, the file before kept as .bak).
//! - Language: "automatic" or one of the languages there are (built in, or in the data folder's
//!   `lang` folder, i18n.rs); ← → or Enter goes to the next, the screen follows.
//! - Debug log: on / off (debug.rs: JSON Lines, nothing private).
//! - Backups folder: typed or pasted (Enter; works on any console), or chosen in the system's
//!   folder window (`o`, folder_pick.rs); tested by the worker first (made when missing, a file
//!   written and read back), then the question: copy the backups there, move them, or leave them.
//! - Original files: the English and French files put back in `lang/` as they came (asked
//!   first; a changed file is kept as `.bak`, i18n::restore_originals).
//!
//! Drawn as groups (Display, Diagnostics, Backups, Translation), each setting as it works: the
//! languages as choices, the debug log as an on / off switch, the folder in a field, the original
//! files as a button. What the selected one does: beside the list on a wide screen, below it otherwise.
//!
//! Adding a setting: a `Row`, its group and name in `heading`, its look in `control`, its text in
//! `about`, what its keys do in `on_key`; the setting itself in config.rs (TEMPLATE + Default)
//! and, used live, in `app::Setting`.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use super::{step, Page};
use crate::app::{Action, Confirm, Ctx, Popup, Setting};
use crate::ble;
use crate::debug;
use crate::i18n::{self, t, tf};
use crate::theme;
use crate::widgets::{self, details_below, input, note_above, panel, panel_focus};
use crate::worker::Cmd;

/// From this width the details go beside the list (else below it)
const SIDE_BY_SIDE: u16 = 100;
/// The details panel's width beside the list
const DETAILS_WIDTH: u16 = 46;

/// The longest folder path taken (Windows allows 32 767 with its long-path prefix; Linux 4096)
const PATH_MAX: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Language,
    DebugLog,
    BackupsFolder,
    BluetoothPin,
    Originals,
}

const ROWS: [Row; 5] = [Row::Language, Row::DebugLog, Row::BackupsFolder, Row::BluetoothPin, Row::Originals];


#[derive(Default)]
pub struct Settings {
    row: usize,
    /// The backups folder being typed
    typing: Option<String>,
    /// The Bluetooth PIN being typed
    pin: Option<String>,
    /// It still holds the folder in use, as a selected text: the first character typed or pasted
    /// replaces it (a pasted path is not added to the old one); ← → or Backspace edit it instead
    fresh: bool,
    /// Why the last key was refused
    problem: Option<String>,
}

/// A language's own name ("Français"), else its code
fn name_of(code: &str) -> String {
    i18n::languages().into_iter().find(|(c, _)| c == code).map_or_else(|| code.to_string(), |(_, n)| n)
}

/// "automatic (Français)": automatic says which language it found
fn auto_text() -> String {
    tf("settings-language-auto", &[("name", name_of(&i18n::active()).into())])
}

/// A setting's group (its heading in the list) and name
fn heading(row: Row) -> (&'static str, &'static str) {
    match row {
        Row::Language => ("settings-group-display", "settings-language"),
        Row::DebugLog => ("settings-group-diagnostics", "settings-debug"),
        Row::BackupsFolder => ("settings-group-backups", "settings-backups-folder"),
        Row::BluetoothPin => ("settings-group-bluetooth", "settings-ble-pin"),
        Row::Originals => ("settings-group-translation", "settings-originals"),
    }
}

/// A path cut at its start ("…\mcmanager-data\backups") to fit `room` cells: its end says the most
fn path_fits(path: &str, room: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= room {
        return path.to_string();
    }
    let keep = room.saturating_sub(1);
    format!("…{}", chars[chars.len() - keep..].iter().collect::<String>())
}

/// The language after `now`: automatic, then each language there is, then automatic again
fn next_language(now: &str, back: bool) -> String {
    let mut all: Vec<String> = vec!["auto".into()];
    all.extend(i18n::languages().into_iter().map(|(code, _)| code));
    let now = i18n::normal(now);
    let at = all.iter().position(|c| *c == now).unwrap_or(0);
    let len = all.len();
    let next = if back { (at + len - 1) % len } else { (at + 1) % len };
    all[next].clone()
}

impl Settings {
    fn selected(&self) -> Row {
        ROWS[self.row.min(ROWS.len() - 1)]
    }

    /// A setting's control, `room` cells wide at most, as it looks: the languages as a row of
    /// choices (the one in use filled), the debug log as an on / off switch, the folder in a
    /// field, the original files as a button. `selected`: the arrows that change it are shown.
    fn control(&self, row: Row, ctx: &Ctx, selected: bool, room: usize) -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        match row {
            Row::Language => {
                let now = match i18n::normal(&ctx.config.ui.language).as_str() {
                    "" => "auto".to_string(),
                    other => other.to_string(),
                };
                let mut options = vec![("auto".to_string(), auto_text())];
                options.extend(i18n::languages());
                if selected {
                    spans.push(Span::styled("‹ ", theme::key()));
                }
                for (i, (code, name)) in options.into_iter().enumerate() {
                    if i > 0 {
                        spans.push(Span::raw(" "));
                    }
                    let style = if code == now { theme::tab_open() } else { theme::dim() };
                    spans.push(Span::styled(format!(" {name} "), style));
                }
                if selected {
                    spans.push(Span::styled(" ›", theme::key()));
                }
            }
            Row::DebugLog => {
                let on = ctx.config.debug.log;
                let (on_style, off_style) =
                    if on { (theme::badge(Color::Green), theme::field()) } else { (theme::field(), theme::badge(Color::Gray)) };
                spans.push(Span::styled(format!(" {} ", t("settings-on")), on_style));
                spans.push(Span::styled(format!(" {} ", t("settings-off")), off_style));
                if on {
                    spans.push(Span::styled(format!("  ● {}", t("settings-debug-writing")), theme::good()));
                }
            }
            Row::BackupsFolder => match &self.typing {
                // the folder in use, ready to be replaced: shown selected
                Some(text) if self.fresh => {
                    let path = path_fits(text, room.saturating_sub(2));
                    spans.push(Span::styled(format!(" {path} "), theme::selected()));
                }
                Some(text) => spans.extend(input(&path_fits(text, room.saturating_sub(4)), room.saturating_sub(2))),
                None => {
                    let path = path_fits(&ctx.paths.backups.display().to_string(), room.saturating_sub(2));
                    spans.push(Span::styled(format!(" {path} "), theme::field()));
                }
            },
            // dots until changed: it is the companions' pairing secret
            Row::BluetoothPin => match &self.pin {
                Some(text) => spans.extend(input(text, ble::PIN_DIGITS + 2)),
                None => {
                    let dots = "•".repeat(ctx.config.bluetooth.pin.chars().count().max(4));
                    spans.push(Span::styled(format!(" {dots} "), theme::field()));
                }
            },
            Row::Originals => {
                let style = if selected { theme::tab_open() } else { theme::field() };
                spans.push(Span::styled(format!(" {} ", t("settings-originals-button")), style));
            }
        }
        Line::from(spans)
    }

    /// A key while the Bluetooth PIN is typed: digits only, 6 of them
    fn pin_key(&mut self, key: KeyEvent) -> Action {
        let Some(text) = &mut self.pin else { return Action::None };
        match key.code {
            KeyCode::Esc => self.pin = None,
            KeyCode::Enter => match ble::pin(text) {
                Some(pin) => {
                    self.pin = None;
                    self.problem = None;
                    return Action::Set(Setting::BluetoothPin(pin));
                }
                None => self.problem = Some(t("ble-bad-pin")),
            },
            KeyCode::Backspace => {
                text.pop();
            }
            _ => match widgets::typed(&key) {
                Some(c) if c.is_ascii_digit() && text.len() < ble::PIN_DIGITS => {
                    text.push(c);
                    self.problem = None;
                }
                Some(_) => self.problem = Some(t("ble-bad-pin")),
                None => {}
            },
        }
        Action::None
    }

    /// What the selected row is, for the details panel
    fn about(&self, row: Row, ctx: &Ctx) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        match row {
            Row::Language => {
                lines.push(Line::raw(t("settings-language-about")));
                lines.push(Line::styled(
                    tf("settings-language-folder", &[("folder", ctx.paths.root.join("lang").display().to_string().into())]),
                    theme::dim(),
                ));
            }
            Row::DebugLog => {
                lines.push(Line::raw(t("settings-debug-about")));
                if let Some(file) = debug::today_file() {
                    lines.push(Line::styled(tf("settings-debug-file", &[("file", file.display().to_string().into())]), theme::dim()));
                }
            }
            Row::BackupsFolder => {
                lines.push(Line::raw(t("settings-backups-folder-about")));
                let hint = if self.typing.is_some() { "settings-folder-typing" } else { "settings-folder-change" };
                lines.push(Line::styled(t(hint), theme::dim()));
            }
            Row::BluetoothPin => lines.push(Line::raw(t("settings-ble-pin-about"))),
            Row::Originals => {
                lines.push(Line::raw(t("settings-originals-about")));
                let folder = ctx.paths.root.join("lang");
                lines.push(Line::styled(tf("settings-language-folder", &[("folder", folder.display().to_string().into())]), theme::dim()));
            }
        }
        if let Some(problem) = &self.problem {
            lines.push(Line::styled(format!("× {problem}"), theme::bad()));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(tf("settings-file", &[("file", ctx.paths.config.display().to_string().into())]), theme::dim()));
        lines
    }

    /// A key while the folder is typed
    fn typing_key(&mut self, key: KeyEvent) -> Action {
        let Some(text) = &mut self.typing else { return Action::None };
        let fresh = std::mem::take(&mut self.fresh);
        match key.code {
            KeyCode::Esc => self.typing = None,
            KeyCode::Enter => {
                let folder = text.trim().to_string();
                self.typing = None;
                // tested by the worker, then the question (app.rs `folder_question`)
                return Action::Send(vec![Cmd::CheckFolder(folder)]);
            }
            KeyCode::Backspace => {
                text.pop();
            }
            // Ctrl+U: empty the field (as in most terminals)
            _ if widgets::ctrl(&key, 'u') => text.clear(),
            // any character, AltGr ones too (`\` on many keyboards)
            _ if widgets::typed(&key).is_some() => {
                let c = widgets::typed(&key).unwrap_or_default();
                // the folder in use was still whole: the first character replaces it
                if fresh {
                    text.clear();
                }
                // any character a path can hold, on either system: only control characters are
                // refused (Windows and Linux both forbid them; NUL ends a path everywhere)
                if c.is_control() {
                    self.problem = Some(t("settings-folder-bad-char"));
                } else if text.len() + c.len_utf8() > PATH_MAX {
                    self.problem = Some(tf("edit-key-too-long", &[("what", t("settings-backups-folder").into()), ("max", PATH_MAX.into())]));
                } else {
                    text.push(c);
                    self.problem = None;
                }
            }
            _ => {}
        }
        Action::None
    }
}

impl Page for Settings {
    fn title(&self) -> String {
        t("tab-settings")
    }

    fn keys(&self) -> Vec<(&'static str, String)> {
        if self.typing.is_some() {
            return vec![("Enter", t("settings-key-test")), ("Ctrl+U", t("settings-key-clear")), ("Esc", t("key-cancel"))];
        }
        if self.pin.is_some() {
            return vec![("0-9", t("settings-key-digits")), ("Enter", t("key-ok")), ("Esc", t("key-cancel"))];
        }
        let mut keys = vec![("↑↓", t("key-select"))];
        match self.selected() {
            Row::Language => keys.push(("←→ Enter", t("key-change"))),
            Row::DebugLog => keys.push(("Enter", t("key-change"))),
            Row::BackupsFolder => {
                keys.push(("Enter", t("settings-key-type")));
                keys.push(("o", t("settings-key-browse")));
            }
            Row::BluetoothPin => keys.push(("Enter", t("key-change"))),
            Row::Originals => keys.push(("Enter", t("settings-key-originals"))),
        }
        keys
    }

    fn typing(&self) -> bool {
        self.typing.is_some() || self.pin.is_some()
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        let area = note_above(f, area, t("settings-about-page"));
        let about = self.about(self.selected(), ctx);
        // the details beside the list on a wide screen, below it otherwise
        let list = if area.width >= SIDE_BY_SIDE {
            let [list, details] = Layout::horizontal([Constraint::Min(40), Constraint::Length(DETAILS_WIDTH)]).areas(area);
            let text = Paragraph::new(about).wrap(Wrap { trim: false }).block(panel(&t("settings-selected")));
            f.render_widget(text, details);
            list
        } else {
            details_below(f, area, &t("settings-selected"), about, 12)
        };
        // inside the borders, after the "▸ " / "  " and the indent of the controls
        let room = list.width.saturating_sub(2 + 4) as usize;
        let mut lines: Vec<Line> = Vec::new();
        let mut group = "";
        for (i, row) in ROWS.iter().enumerate() {
            let (its_group, name) = heading(*row);
            if its_group != group {
                if !group.is_empty() {
                    lines.push(Line::raw(""));
                }
                lines.push(Line::styled(format!(" {}", t(its_group).to_uppercase()), theme::title()));
                group = its_group;
            }
            let selected = i == self.row;
            let name_line = if selected {
                Line::from(vec![
                    Span::styled(" » ", theme::key()),
                    Span::styled(t(name), theme::heading().add_modifier(Modifier::BOLD)),
                ])
            } else {
                Line::from(vec![Span::raw("   "), Span::raw(t(name))])
            };
            lines.push(name_line);
            let mut control = vec![Span::raw("     ")];
            control.extend(self.control(*row, ctx, selected, room).spans);
            lines.push(Line::from(control));
        }
        let text = Paragraph::new(lines).block(panel_focus(&t("tab-settings"), true));
        f.render_widget(text, list);
    }

    fn on_key(&mut self, key: KeyEvent, ctx: &Ctx) -> Action {
        if self.typing.is_some() {
            return self.typing_key(key);
        }
        if self.pin.is_some() {
            return self.pin_key(key);
        }
        self.problem = None;
        match (self.selected(), key.code) {
            (Row::Language, KeyCode::Right | KeyCode::Enter) => {
                Action::Set(Setting::Language(next_language(&ctx.config.ui.language, false)))
            }
            (Row::Language, KeyCode::Left) => Action::Set(Setting::Language(next_language(&ctx.config.ui.language, true))),
            (Row::DebugLog, KeyCode::Enter | KeyCode::Char(' ')) => Action::Set(Setting::DebugLog(!ctx.config.debug.log)),
            (Row::BackupsFolder, KeyCode::Enter) => {
                // starts from the folder in use, selected: Backspace edits it, typing or pasting
                // replaces it
                self.typing = Some(ctx.paths.backups.display().to_string());
                self.fresh = true;
                Action::None
            }
            // the system's folder window (where there is one): the folder chosen is tested and
            // asked about as a typed one
            (Row::BackupsFolder, KeyCode::Char('o')) => Action::Send(vec![Cmd::PickFolder]),
            // typed new (the one in use is not shown)
            (Row::BluetoothPin, KeyCode::Enter) => {
                self.pin = Some(String::new());
                Action::None
            }
            // asked first, the focus on "cancel": a changed file is kept, but set aside
            (Row::Originals, KeyCode::Enter) => {
                let lines = vec![t("settings-originals-ask"), t("settings-originals-kept")];
                let run = Confirm::Run(vec![Cmd::RestoreLanguages]);
                Action::Ask(Popup::ask(t("settings-originals"), lines, run, t("settings-originals-yes"), t("answer-cancel")).safe())
            }
            (_, code) => {
                self.row = step(self.row, ROWS.len(), code);
                Action::None
            }
        }
    }
}

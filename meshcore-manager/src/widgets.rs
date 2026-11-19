//! Small drawing helpers every page uses: a titled panel, details as "label  value", the popup.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Padding, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Popup;
use crate::event::Job;
use crate::i18n::{t, tf};
use crate::theme;

/// A box with rounded corners, its title, and a space between the border and the text
pub fn panel(title: &str) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border())
        .padding(Padding::horizontal(1))
        .title(Line::styled(format!(" {title} "), theme::title()))
}

/// The panel that has the keyboard, when a page has several (and every popup): a thick border in
/// the accent, so where the keys go is seen at a glance
pub fn panel_focus(title: &str, focus: bool) -> Block<'static> {
    let block = panel(title);
    if focus {
        block.border_type(BorderType::Thick).border_style(theme::border_focus())
    } else {
        block
    }
}

/// Borders and padding of a `panel`, left and right together
pub const PANEL_EDGES: u16 = 4;

/// A comfortable line length for reading text (help, notes): wider is hard to follow
pub const READING: u16 = 88;

/// One "label  value" of a details panel (see `facts`)
pub type Fact = (String, Line<'static>);

/// A plain "label  value"
pub fn fact(label: String, value: String) -> Fact {
    (label, Line::raw(value))
}

/// The width these facts want side by side: the label column and the longest value
pub fn facts_width(pairs: &[Fact]) -> usize {
    label_width(pairs.iter().map(|(label, _)| label.as_str())) + pairs.iter().map(|(_, v)| v.width()).max().unwrap_or(0)
}

/// The least room a value gets beside its label: below it, labels go above their values
pub const VALUE_MIN: usize = 16;

/// Whether "label  value" pairs with labels `column` cells wide must be stacked in `width` cells:
/// only when the values would get less than VALUE_MIN beside the labels (one long value never
/// decides it: it wraps in its own column, `hanging`)
pub fn must_stack(column: usize, width: usize) -> bool {
    column + VALUE_MIN > width
}

/// `head` (a label, padded to `column` cells) then `value`, cut to lines of `room` cells: the
/// rest of a long value goes on the next lines under itself, `column` cells in (a hanging
/// indent), never under the labels. Cut at a character: keys and numbers have no spaces.
pub fn hanging(head: Vec<Span<'static>>, value: Vec<Span<'static>>, column: usize, room: usize) -> Vec<Line<'static>> {
    let width = room.saturating_sub(column).max(1);
    let mut lines = Vec::new();
    let mut current = head;
    let mut used = 0;
    for span in value {
        let mut piece = String::new();
        for c in span.content.chars() {
            let w = Span::raw(c.to_string()).width();
            if used + w > width && used > 0 {
                if !piece.is_empty() {
                    current.push(Span::styled(std::mem::take(&mut piece), span.style));
                }
                lines.push(Line::from(std::mem::replace(&mut current, vec![Span::raw(" ".repeat(column))])));
                used = 0;
            }
            piece.push(c);
            used += w;
        }
        if !piece.is_empty() {
            current.push(Span::styled(piece, span.style));
        }
    }
    lines.push(Line::from(current));
    lines
}

/// The lines of a details panel `width` cells wide inside (`inner_width`): two columns lined up,
/// a long value wrapped in its own column (`hanging`); when even that leaves the values too
/// little room (`must_stack`), each label above its value.
pub fn facts(pairs: Vec<Fact>, width: usize) -> Vec<Line<'static>> {
    let column = label_width(pairs.iter().map(|(label, _)| label.as_str()));
    facts_in(pairs, width, must_stack(column, width))
}

/// `facts`, with the layout chosen by the caller: two columns, or (`stacked`) each label above
/// its value, indented. No empty lines between them: the labels are dimmed, the values not.
/// A secret once shown wraps in its place: the other lines never move sideways.
pub fn facts_in(pairs: Vec<Fact>, width: usize, stacked: bool) -> Vec<Line<'static>> {
    let column = label_width(pairs.iter().map(|(label, _)| label.as_str()));
    let mut lines = Vec::new();
    for (label, value) in pairs {
        if stacked {
            lines.push(Line::styled(label, theme::dim()));
            lines.extend(hanging(vec![Span::raw("  ")], value.spans, 2, width));
        } else {
            let head = vec![Span::styled(format!("{label:<column$}"), theme::dim())];
            lines.extend(hanging(head, value.spans, column, width));
        }
    }
    lines
}

/// The room for text inside a `panel` this wide
pub fn inner_width(area: Rect) -> usize {
    area.width.saturating_sub(PANEL_EDGES) as usize
}

/// A box over the page `width` wide at most (and a margin narrower than the page), `height` high
/// at most: for the help and choosers
pub fn sheet(area: Rect, width: u16, height: u16) -> Rect {
    centered(area, width.min(area.width.saturating_sub(8)), height.min(area.height.saturating_sub(2)))
}

/// A table column: its heading and how it behaves when the room is short or large
pub struct Col {
    head: String,
    grow: bool,
    drop: u8,
}

impl Col {
    /// As wide as its widest cell (or heading)
    pub fn new(head: String) -> Col {
        Col { head, grow: false, drop: 0 }
    }

    /// Takes the room left over (a name)
    pub fn grow(mut self) -> Col {
        self.grow = true;
        self
    }

    /// Left out when the table does not fit; the highest `order` goes first
    pub fn drop(mut self, order: u8) -> Col {
        self.drop = order;
        self
    }
}

/// A table sized by what it holds, the same everywhere: every column as wide as its widest cell,
/// a `grow` column takes what is left, and in a narrow window the columns marked `drop` go,
/// least useful first, instead of every column being squeezed.
pub struct Grid {
    cols: Vec<Col>,
    rows: Vec<(Vec<Line<'static>>, Style)>,
}

impl Grid {
    pub fn new(cols: Vec<Col>) -> Grid {
        Grid { cols, rows: Vec::new() }
    }

    /// Rows, one cell per column
    pub fn rows(self, rows: impl IntoIterator<Item = Vec<Line<'static>>>) -> Grid {
        self.styled(rows.into_iter().map(|cells| (cells, Style::new())))
    }

    /// Rows that each have a style of their own (a contact to remove)
    pub fn styled(mut self, rows: impl IntoIterator<Item = (Vec<Line<'static>>, Style)>) -> Grid {
        self.rows.extend(rows);
        self
    }

    fn widths(&self) -> Vec<usize> {
        let mut widths: Vec<usize> = self.cols.iter().map(|c| Line::raw(c.head.as_str()).width()).collect();
        for (cells, _) in &self.rows {
            for (width, cell) in widths.iter_mut().zip(cells) {
                *width = (*width).max(cell.width());
            }
        }
        widths
    }

    /// The width it wants: every column and the space between them
    pub fn width(&self) -> usize {
        let widths = self.widths();
        widths.iter().sum::<usize>() + widths.len().saturating_sub(1)
    }

    /// The table for `room` cells (inside its panel: `inner_width`). Too wide: long columns are
    /// shortened first (the widest one, a `grow` one first, down to `SHORT` cells), so every
    /// column stays in sight; still too wide: the `drop` columns go; still too wide: the widest
    /// are narrowed further. A cell cut short ends with "…".
    pub fn table(self, room: usize) -> Table<'static> {
        /// A long column (a board, a firmware) is shortened down to this before any column goes
        const SHORT: usize = 16;
        let mut widths = self.widths();
        let mut keep = vec![true; self.cols.len()];
        let needed = |widths: &[usize], keep: &[bool]| {
            let kept: Vec<usize> = widths.iter().zip(keep).filter(|(_, k)| **k).map(|(w, _)| *w).collect();
            kept.iter().sum::<usize>() + kept.len().saturating_sub(1)
        };
        // never shorter than its heading
        let floor: Vec<usize> = self.cols.iter().map(|c| Line::raw(c.head.as_str()).width().max(SHORT)).collect();
        while needed(&widths, &keep) > room {
            let long = (0..self.cols.len()).filter(|&i| widths[i] > floor[i]).max_by_key(|&i| (widths[i], self.cols[i].grow));
            match long {
                Some(i) => widths[i] -= 1,
                None => break,
            }
        }
        while needed(&widths, &keep) > room {
            let next = (0..self.cols.len()).filter(|&i| keep[i] && self.cols[i].drop > 0).max_by_key(|&i| self.cols[i].drop);
            match next {
                Some(i) => keep[i] = false,
                None => break,
            }
        }
        let mut over = needed(&widths, &keep).saturating_sub(room);
        while over > 0 {
            let widest = (0..self.cols.len()).filter(|&i| keep[i] && widths[i] > 3).max_by_key(|&i| (self.cols[i].grow, widths[i]));
            match widest {
                Some(i) => {
                    widths[i] -= 1;
                    over -= 1;
                }
                None => break,
            }
        }
        let constraints: Vec<Constraint> = (0..self.cols.len())
            .filter(|&i| keep[i])
            .map(|i| if self.cols[i].grow { Constraint::Min(widths[i] as u16) } else { Constraint::Length(widths[i] as u16) })
            .collect();
        let pick = move |cells: Vec<Line<'static>>| -> Vec<Cell<'static>> {
            cells
                .into_iter()
                .zip(keep.iter().zip(&widths))
                .filter(|(_, (k, _))| **k)
                .map(|(c, (_, w))| Cell::from(ellipsis(c, *w)))
                .collect()
        };
        let heads: Vec<Line<'static>> = self.cols.iter().map(|c| Line::raw(c.head.clone())).collect();
        let has_head = self.cols.iter().any(|c| !c.head.is_empty());
        let header = Row::new(pick(heads)).style(theme::dim());
        let rows: Vec<Row<'static>> = self.rows.into_iter().map(|(cells, style)| Row::new(pick(cells)).style(style)).collect();
        let table = Table::new(rows, constraints).row_highlight_style(theme::selected());
        if has_head {
            table.header(header)
        } else {
            table
        }
    }

    /// The table in its panel at `area`, with "▲ more" / "▼ more" on the panel's border when
    /// rows are out of sight above / below (`more_marks`)
    pub fn show(self, f: &mut Frame, area: Rect, block: Block<'static>, state: &mut TableState) {
        let total = self.rows.len();
        let head = usize::from(self.cols.iter().any(|c| !c.head.is_empty()));
        let inner = block.inner(area);
        f.render_stateful_widget(self.table(inner.width as usize).block(block), area, state);
        more_marks(f, area, state.offset(), (inner.height as usize).saturating_sub(head), total);
    }
}

/// On the border of the panel at `area`: "▲ more" at the top when rows are out of sight above,
/// "▼ more" at the bottom when some are below. `first`: the first row shown, `shown`: how many
/// it has room for, `total`: how many there are. A list never ends in silence at the edge.
pub fn more_marks(f: &mut Frame, area: Rect, first: usize, shown: usize, total: usize) {
    if area.height < 3 || area.width < 16 {
        return;
    }
    let mut put = |y: u16, text: String| {
        let text = format!(" {text} ");
        let width = (Line::raw(text.as_str()).width() as u16).min(area.width - 4);
        let rect = Rect { x: area.x + area.width - 2 - width, y, width, height: 1 };
        f.render_widget(Paragraph::new(Line::styled(text, theme::title())), rect);
    };
    if first > 0 {
        put(area.y, t("more-above"));
    }
    if first + shown < total {
        put(area.y + area.height - 1, t("more-below"));
    }
}

/// A line cut to `width` cells: when it was longer, its end is "…", so a cut shows as one
/// (a name, a board, a header line in a narrow window)
pub fn ellipsis(line: Line<'static>, width: usize) -> Line<'static> {
    if line.width() <= width {
        return line;
    }
    let room = width.saturating_sub(1);
    let mut used = 0;
    let mut spans = Vec::new();
    for span in line.spans {
        let mut text = String::new();
        let mut cut = false;
        for c in span.content.chars() {
            let w = Span::raw(c.to_string()).width();
            if used + w > room {
                cut = true;
                break;
            }
            used += w;
            text.push(c);
        }
        spans.push(Span::styled(text, span.style));
        if cut {
            break;
        }
    }
    let style = spans.last().map(|s| s.style).unwrap_or_default();
    spans.push(Span::styled("…", style));
    Line::from(spans).style(line.style)
}

/// How many rows these lines take, wrapped in `width` cells
pub fn text_rows(lines: &[Line], width: usize) -> usize {
    lines.iter().map(|l| l.width().div_ceil(width.max(1)).max(1)).sum()
}

/// The farthest a text of these lines can be scrolled in a box `width` x `height`: its end at
/// the bottom of the box, never empty rows after it
pub fn scroll_max(lines: &[Line], width: usize, height: usize) -> usize {
    text_rows(lines, width).saturating_sub(height)
}

/// A details panel at the bottom of `area`, as high as its lines need (the list above keeps at
/// least `keep` rows); returns the room left above it
pub fn details_below(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line<'static>>, keep: u16) -> Rect {
    let rows = text_rows(&lines, inner_width(area));
    let [above, below] =
        Layout::vertical([Constraint::Min(keep), Constraint::Length(rows as u16 + 2)]).areas(area);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(panel(title)), below);
    above
}

/// A line of text (a hint, why a key was refused) at the bottom of `area`, no border, as high as
/// it wraps (never cut in a narrow window); returns the room left above it
pub fn note_below(f: &mut Frame, area: Rect, line: Line<'static>) -> Rect {
    let rows = text_rows(std::slice::from_ref(&line), area.width as usize) as u16;
    let [above, below] = Layout::vertical([Constraint::Min(3), Constraint::Length(rows)]).areas(area);
    f.render_widget(Paragraph::new(line).wrap(Wrap { trim: false }), below);
    above
}

/// A short explanation at the top of `area` (what a page is for), no border, dimmed, as high as
/// it wraps, with an empty line under it; returns the room left below
pub fn note_above(f: &mut Frame, area: Rect, text: String) -> Rect {
    let line = Line::styled(text, theme::dim());
    let rows = text_rows(std::slice::from_ref(&line), area.width as usize) as u16;
    let [above, _, below] = Layout::vertical([Constraint::Length(rows), Constraint::Length(1), Constraint::Min(3)]).areas(area);
    f.render_widget(Paragraph::new(line).wrap(Wrap { trim: false }), above);
    below
}

/// A text field being typed into: " text▮      " on a filled box at least `width` cells wide, the
/// cursor a cell in the accent colour after the text. (No underline, no thin bar: it reads as a
/// box to type in, the way text inputs look in most terminal apps.)
/// The same box inside a table cell: no space before the text, so the name being typed stays
/// in line with the names above and below it
pub fn input_cell(text: &str, width: usize) -> Vec<Span<'static>> {
    let used = text.chars().count() + 1;
    vec![
        Span::styled(text.to_string(), theme::field()),
        Span::styled(" ", theme::cursor()),
        Span::styled(" ".repeat(width.saturating_sub(used).max(1)), theme::field()),
    ]
}

pub fn input(text: &str, width: usize) -> Vec<Span<'static>> {
    let used = text.chars().count() + 2;
    vec![
        Span::styled(format!(" {text}"), theme::field()),
        Span::styled(" ", theme::cursor()),
        Span::styled(" ".repeat(width.saturating_sub(used).max(1)), theme::field()),
    ]
}

/// A text over the page in a box `width` wide and as high as its lines (up to the page): the help,
/// a comparison. Scrolled no farther than its end, "▲ more" / "▼ more" on its border, its keys on
/// the last line. Returns the scroll as drawn (the caller keeps it: one Up after the end moves).
pub fn scroll_sheet(f: &mut Frame, area: Rect, title: &str, width: u16, lines: Vec<Line<'static>>, scroll: u16, keys: &str) -> u16 {
    let inner = width.saturating_sub(PANEL_EDGES) as usize;
    // borders and the keys line
    let rect = sheet(area, width, text_rows(&lines, inner) as u16 + 3);
    lift(f, rect);
    let block = panel_focus(title, true);
    let room = block.inner(rect);
    f.render_widget(block, rect);
    let [text, bottom] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(room);
    let scroll = scroll.min(scroll_max(&lines, inner, text.height as usize) as u16);
    let rows = text_rows(&lines, inner);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll, 0)), text);
    f.render_widget(Paragraph::new(Line::styled(keys.to_string(), theme::dim())), bottom);
    more_marks(f, rect, scroll as usize, text.height as usize, rows);
    scroll
}

/// A key that is Ctrl+`c` as a shortcut. Windows reports AltGr as Ctrl+Alt (AltGr+C can type a
/// letter on some keyboards): with Alt it is text, not the shortcut.
pub fn ctrl(key: &KeyEvent, c: char) -> bool {
    key.code == KeyCode::Char(c) && key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT)
}

/// The character a key types into a text field: plain, shifted, or through AltGr (which Windows
/// reports as Ctrl+Alt: `\` `@` `{` `|` `€` on many keyboards). Ctrl alone is a shortcut.
pub fn typed(key: &KeyEvent) -> Option<char> {
    let KeyCode::Char(c) = key.code else { return None };
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    (!ctrl || key.modifiers.contains(KeyModifiers::ALT)).then_some(c)
}

/// How many characters of a public key the lists show
const KEY_SHOWN: usize = 4;

/// The first characters of a public key, in their own colour ("4A1F"): finds a node among many
/// at a glance. "" when not known.
pub fn key_prefix(key: &str) -> Line<'static> {
    Line::styled(key.chars().take(KEY_SHOWN).collect::<String>().to_uppercase(), theme::key_prefix())
}

/// How a node is linked: " USB " COM3, or " BLE " AA:BB:CC:DD:EE:FF  -67 dBm (the signal
/// coloured: green near, yellow far, red at the edge)
pub fn link_cell(port: &crate::serial::Port, signal: Option<i16>) -> Line<'static> {
    let ble = port.is_ble();
    // a Bluetooth address by its first 2 bytes ("C1:D5…"): enough to tell them apart in a list,
    // the whole one is in the details
    let name = if ble { format!("{}…", port.name().chars().take(5).collect::<String>()) } else { port.name() };
    let mut spans = vec![Span::styled(if ble { " BLE " } else { " USB " }, theme::link(ble)), Span::raw(format!(" {name}"))];
    if let Some(dbm) = signal {
        let style = match dbm {
            d if d >= -75 => theme::good(),
            d if d >= -88 => theme::warn(),
            _ => theme::bad(),
        };
        spans.push(Span::styled(format!("  {dbm} dBm"), style));
    }
    Line::from(spans)
}

/// Under a text field with a length limit (the firmware's): "3 / 15 characters · 12 left",
/// dimmed, amber when few are left, so the limit is known before a key is refused
pub fn room_line(used: usize, max: usize) -> Line<'static> {
    let left = max.saturating_sub(used);
    let style = if left <= 3 { theme::warn() } else { theme::dim() };
    Line::styled(tf("edit-room", &[("used", used.into()), ("max", max.into()), ("left", left.into())]), style)
}

/// The label column for these labels: the longest one and two spaces
pub fn label_width<'a>(labels: impl Iterator<Item = &'a str>) -> usize {
    labels.map(|l| l.chars().count()).max().unwrap_or(0) + 2
}

/// A running job in words: "Reading MyNode…", "Writing to MyNode: tx 2/3"
pub fn job_text(job: &Job) -> String {
    if job.total == 0 && job.step.is_empty() {
        format!("{}…", job.title)
    } else if job.total == 0 {
        // a step without a count ("connecting…")
        format!("{}: {}", job.title, job.step)
    } else {
        format!("{}: {} {}/{}", job.title, job.step, job.done + 1, job.total)
    }
}

/// Three dots, one lit, moving: "●∙∙" "∙●∙" "∙∙●" "∙●∙". Driven by the clock, so every
/// redraw shows the right frame without keeping any state
pub fn spinner() -> &'static str {
    const FRAMES: [&str; 4] = ["●∙∙", "∙●∙", "∙∙●", "∙●∙"];
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    FRAMES[(ms / 180 % 4) as usize]
}

/// Radio waves leaving an antenna, while Bluetooth listens for companions: "·)  " "·)) "
/// "·)))" " ))" ... Always 4 columns (the text after it does not move); plain characters every
/// console font has. Driven by the clock, as `spinner`. `calm`: nothing new was heard for a
/// while (the companions nearby are found): the waves go slower, it still listens.
pub fn listening(calm: bool) -> &'static str {
    const FRAMES: [&str; 6] = ["·   ", "·)  ", "·)) ", "·)))", "· ))", "·  )"];
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let step = if calm { 900 } else { 220 };
    FRAMES[(ms / step % 6) as usize]
}

/// How long ago a time of a node was (seconds since 1970): "5 min", "3 h", "2 d"; 0 = never
pub fn ago(secs: u32) -> String {
    if secs == 0 {
        return t("age-never");
    }
    // under a minute, or a node clock ahead of this computer's: "now"
    match chrono::Utc::now().timestamp() - secs as i64 {
        d if d < 60 => t("age-now"),
        d if d < 3_600 => tf("age-minutes", &[("n", (d / 60).into())]),
        d if d < 86_400 => tf("age-hours", &[("n", (d / 3_600).into())]),
        d => tf("age-days", &[("n", (d / 86_400).into())]),
    }
}

/// A time of a node (seconds since 1970) in this computer's time zone; 0 = never
pub fn when(secs: u32) -> String {
    match chrono::DateTime::from_timestamp(secs as i64, 0).filter(|_| secs != 0) {
        Some(time) => time.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string(),
        None => t("age-never"),
    }
}

/// A firmware as a list shows it: the version alone ("v1.17.1-d929643"), without the build date
/// the firmware adds in brackets ("(Build: 14-Aug-2026)", "(14 Aug 2026)"): the details have it
pub fn firmware_short(firmware: &str) -> String {
    firmware.split(" (").next().unwrap_or(firmware).trim().to_string()
}

/// Makes room for a box over the page (a question, a note, a sheet, a picker): a shadow down and
/// to the right (what is under it darkened: 2 columns, 1 row, as cells are twice as high as
/// wide), then the box emptied on a slightly lighter ground, so it stands off the page. Call it
/// instead of `Clear`, then draw the box in `rect`.
pub fn lift(f: &mut Frame, rect: Rect) {
    let screen = f.area();
    let buf = f.buffer_mut();
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;
    let mut shade = |x: u16, y: u16| {
        if x < screen.x + screen.width && y < screen.y + screen.height {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(theme::shadow());
            }
        }
    };
    for y in rect.y + 1..=bottom {
        for x in right..right + 2 {
            shade(x, y);
        }
    }
    for x in rect.x + 2..right {
        shade(x, bottom);
    }
    f.render_widget(Clear, rect);
    f.render_widget(Block::new().style(theme::raised()), rect);
}

/// A box of `width` x `height` in the middle of `area` (smaller when the area is)
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h }
}

/// A button: "  write it to the node  ", filled in the accent when it has the focus, a grey box
/// otherwise; red when it removes or throws something away
fn button(text: &str, focused: bool, danger: bool) -> Span<'static> {
    let style = match (danger, focused) {
        (true, _) => theme::danger_button(focused),
        (false, true) => theme::tab_open(),
        (false, false) => theme::field(),
    };
    Span::styled(format!("  {text}  "), style)
}

/// A message over the page: a question (its buttons, ← → / Tab between them, Enter presses the
/// focused one, Esc = cancel) or a note (one OK button)
/// The widest a question or a note gets, in columns of text
const POPUP_MAX: usize = 76;

pub fn popup(f: &mut Frame, area: Rect, popup: &Popup) {
    let mut spans = Vec::new();
    for (i, (text, confirm)) in popup.buttons.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(button(text, i == popup.focus, popup.danger && confirm.is_some()));
    }
    let buttons = Line::from(spans);
    let hint = t(if popup.buttons.len() > 1 { "popup-keys-question" } else { "popup-keys-note" });
    let mut lines: Vec<Line> = popup.lines.iter().map(|l| Line::raw(l.clone())).collect();
    lines.push(Line::raw(""));
    lines.push(buttons.right_aligned());
    lines.push(Line::styled(hint, theme::dim()).right_aligned());
    // as wide as its longest line, but no wider than a comfortable reading width: a long line
    // wraps (the box grows down, not across the whole screen)
    let widest = lines.iter().map(|l| l.width()).max().unwrap_or(0).max(popup.title.chars().count() + 2).min(POPUP_MAX);
    let high = text_rows(&lines, widest) as u16;
    // borders, padding, and a little air
    let rect = centered(area, (widest as u16).saturating_add(6), high.saturating_add(2));
    lift(f, rect);
    let block = panel_focus(&popup.title, true);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(block), rect);
}

/// A node name made safe for table columns: flag emoji (two "regional indicator" letters) become
/// "[CH]". Many terminals (all of Windows) have no flag glyphs and draw them with an unknown
/// width, which breaks the columns.
pub fn clean(s: &str) -> String {
    const A: u32 = 0x1F1E6;
    let letter = |c: char| (A..A + 26).contains(&(c as u32)).then(|| char::from(b'A' + (c as u32 - A) as u8));
    if !s.chars().any(|c| letter(c).is_some()) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match (letter(c), chars.peek().copied().and_then(letter)) {
            (Some(a), Some(b)) => {
                chars.next();
                out.push_str(&format!("[{a}{b}]"));
            }
            (Some(a), None) => out.push(a),
            _ => out.push(c),
        }
    }
    out
}

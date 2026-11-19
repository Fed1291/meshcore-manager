//! Colours and text styles, in one place: pages ask for a meaning ("title", "bad"), never a colour.
//! Only the 16 basic terminal colours: they follow the user's own terminal theme everywhere.
//!
//! One accent (cyan) marks structure: panel titles, the focused border, the open tab, keys. Its
//! complement on the colour wheel is the warm side (red-orange; split-complementary: amber), so
//! "you changed something" is amber (the terminal's Yellow, amber in most themes): the edit
//! badge, the count of changes, a changed value, the chosen choice. Green / red stay good / bad.
//! A colour always means one thing.

use ratatui::style::{Color, Modifier, Style};

use crate::event::Level;
use crate::node::Kind;

const ACCENT: Color = Color::Cyan;

/// Panel titles, the program's name
pub fn title() -> Style {
    Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// A heading inside a panel (a setting's name, a node's name): bold, the normal text colour
pub fn heading() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

/// Labels, column heads, hints
pub fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

/// Panel borders
pub fn border() -> Style {
    Style::new().fg(Color::DarkGray)
}

/// The border of the panel that has the keyboard
pub fn border_focus() -> Style {
    Style::new().fg(ACCENT)
}

/// A key to press, in the bottom line
pub fn key() -> Style {
    Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// The first characters of a public key in lists: a colour of its own (magenta: used for nothing
/// else), so the eye finds a node by it among many
pub fn key_prefix() -> Style {
    // bright white, bold: stands out from the plain text without a colour that means something
    // else here (cyan structure, amber changed, green / red good / bad, the kinds' own)
    Style::new().fg(Color::White).add_modifier(Modifier::BOLD)
}

/// How a node is linked: USB plain, Bluetooth in blue (a label with a background)
pub fn link(ble: bool) -> Style {
    if ble {
        badge(Color::Blue)
    } else {
        badge(Color::Gray)
    }
}

/// A secret shown on request (`s`): a lighter shade of the accent, so it stands out from the
/// other values and from the structure
pub fn secret() -> Style {
    Style::new().fg(Color::LightCyan)
}

/// The open page in the header's row of pages
pub fn tab_open() -> Style {
    Style::new().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
}

/// The open section of a node's view: the accent, bold (the others are dimmed)
pub fn section_open() -> Style {
    Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// A small label with a background: " EDIT ", " repeater "
pub fn badge(color: Color) -> Style {
    Style::new().fg(Color::Black).bg(color).add_modifier(Modifier::BOLD)
}

/// The edit mode badge (amber, the accent's complement: things are being changed)
pub fn edit_badge() -> Style {
    badge(Color::Yellow)
}

/// The second half of the edit badge: how many changes wait ("1 change not sent"), amber on grey
pub fn edit_count() -> Style {
    Style::new().fg(Color::Yellow).bg(Color::DarkGray).add_modifier(Modifier::BOLD)
}

/// The selected row of a table: a blue bar behind it, bold. Only the background changes: each
/// cell keeps its own colour (kinds, ★, emoji), so the row reads as it does unselected
pub fn selected() -> Style {
    Style::new().bg(SELECTED_BAR).add_modifier(Modifier::BOLD)
}

/// A steel blue, lighter than the page and dark enough for white, cyan, amber and red text on
/// it (terminals without 24-bit colour show the nearest of their colours)
const SELECTED_BAR: Color = Color::Rgb(38, 79, 120);

/// The ground of a box over the page (questions, notes, sheets, pickers): a little lighter than
/// the page, so it stands off it
pub fn raised() -> Style {
    Style::new().bg(Color::Rgb(26, 29, 36))
}

/// The shadow of a box over the page: what is under it, darkened (black ground, its text dimmed)
pub fn shadow() -> Style {
    Style::new().bg(Color::Black).fg(Color::Rgb(70, 70, 70)).remove_modifier(Modifier::BOLD)
}

/// A value changed in edit mode, not sent yet
pub fn changed() -> Style {
    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

/// A text field being typed into: a filled box, so it reads as "type here"
pub fn field() -> Style {
    Style::new().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD)
}

/// The cursor in a text field: one cell in the accent colour, where the next letter goes
pub fn cursor() -> Style {
    Style::new().bg(ACCENT).fg(Color::Black)
}

pub fn good() -> Style {
    Style::new().fg(Color::Green)
}

pub fn warn() -> Style {
    Style::new().fg(Color::Yellow)
}

pub fn bad() -> Style {
    Style::new().fg(Color::Red)
}

/// A button that removes or throws something away: red, filled when it has the focus
pub fn danger_button(focused: bool) -> Style {
    if focused {
        Style::new().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::Red).bg(Color::DarkGray).add_modifier(Modifier::BOLD)
    }
}

/// A row that edit mode will remove from the node
pub fn removed() -> Style {
    Style::new().fg(Color::Red).add_modifier(Modifier::CROSSED_OUT)
}

pub fn level(level: Level) -> Style {
    match level {
        Level::Info => Style::new(),
        Level::Good => good(),
        Level::Bad => bad(),
    }
}

/// Each kind of node has its own colour (none of them the accent)
fn kind_color(kind: Kind) -> Color {
    match kind {
        Kind::Companion => Color::LightBlue,
        Kind::Repeater => Color::LightGreen,
        Kind::RoomServer => Color::LightMagenta,
        Kind::Sensor => Color::LightYellow,
        Kind::Bootloader => Color::Yellow,
        Kind::Unknown => Color::DarkGray,
    }
}

pub fn kind(kind: Kind) -> Style {
    Style::new().fg(kind_color(kind))
}

/// The kind as a badge, in a node's header
pub fn kind_badge(kind: Kind) -> Style {
    badge(kind_color(kind))
}

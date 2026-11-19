//! Info: what the program is, its version, and its license, starting with what a person who
//! changes it is asked to do (share the changes, under the same license).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use super::Page;
use crate::app::{Action, Ctx};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::{centered, inner_width, more_marks, panel, scroll_max, text_rows, READING};

#[derive(Default)]
pub struct About {
    scroll: usize,
}

impl Page for About {
    fn title(&self) -> String {
        t("tab-about")
    }

    fn keys(&self) -> Vec<(&'static str, String)> {
        vec![("↑↓", t("key-scroll"))]
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, _ctx: &Ctx) {
        let lines = vec![
            Line::styled(tf("about-name", &[("version", env!("CARGO_PKG_VERSION").into())]), theme::heading()),
            Line::styled(t("app-by"), theme::dim()),
            Line::raw(""),
            Line::styled(t("about-share-title"), theme::title()),
            Line::raw(t("about-share")),
            Line::raw(""),
            Line::styled(t("about-license-title"), theme::title()),
            Line::raw(t("about-license")),
            Line::raw(""),
            Line::styled(t("about-license-link"), theme::dim()),
            Line::raw(""),
            Line::styled(t("about-meshcore-title"), theme::title()),
            Line::raw(t("about-meshcore")),
        ];
        // a reading width, in the middle of the page
        let rect = centered(area, READING.min(area.width), area.height);
        let inner = inner_width(rect);
        let shown = rect.height.saturating_sub(2) as usize;
        self.scroll = self.scroll.min(scroll_max(&lines, inner, shown));
        let rows = text_rows(&lines, inner);
        let text = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((self.scroll as u16, 0));
        f.render_widget(text.block(panel(&t("tab-about"))), rect);
        more_marks(f, rect, self.scroll, shown, rows);
    }

    fn on_key(&mut self, key: KeyEvent, _ctx: &Ctx) -> Action {
        // clamped to the end when drawn
        self.scroll = match key.code {
            KeyCode::Up => self.scroll.saturating_sub(1),
            KeyCode::Down => self.scroll + 1,
            KeyCode::PageUp => self.scroll.saturating_sub(10),
            KeyCode::PageDown => self.scroll + 10,
            KeyCode::Home => 0,
            _ => self.scroll,
        };
        Action::None
    }
}

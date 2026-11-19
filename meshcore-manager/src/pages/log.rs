//! Log: what happened since the program started (nodes found, backups made, what went wrong).

use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::Page;
use crate::app::{Action, Ctx};
use crate::i18n::t;
use crate::theme;
use crate::widgets::panel;

pub struct Log;

impl Page for Log {
    fn title(&self) -> String {
        t("tab-log")
    }

    fn keys(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, ctx: &Ctx) {
        // the newest lines that fit, the latest at the bottom
        let room = area.height.saturating_sub(2) as usize;
        let lines: Vec<Line> = ctx
            .log
            .iter()
            .skip(ctx.log.len().saturating_sub(room))
            .map(|l| {
                Line::from(vec![
                    Span::styled(format!("{}  ", l.time), theme::dim()),
                    Span::styled(l.text.clone(), theme::level(l.level)),
                ])
            })
            .collect();
        let lines = if lines.is_empty() { vec![Line::styled(t("log-empty"), theme::dim())] } else { lines };
        f.render_widget(Paragraph::new(lines).block(panel(&t("tab-log"))), area);
    }

    fn on_key(&mut self, _key: KeyEvent, _ctx: &Ctx) -> Action {
        Action::None
    }
}

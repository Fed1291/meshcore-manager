//! Boards: the hardware the program knows (data/devices.toml plus the user's own devices.toml):
//! the boards by the model their firmware reports, and what each USB id usually is.

use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::TableState;
use ratatui::Frame;

use super::{step, Page};
use crate::app::{Action, Ctx};
use crate::devices;
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::{panel, panel_focus, Col, Grid};

#[derive(Default)]
pub struct Boards {
    selected: usize,
    table: TableState,
}

impl Page for Boards {
    fn title(&self) -> String {
        t("tab-boards")
    }

    fn keys(&self) -> Vec<(&'static str, String)> {
        vec![("↑↓", t("key-scroll"))]
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, _ctx: &Ctx) {
        // the USB ids below, as high as they are (up to half), the boards (scrolled) above
        let ids = devices::all_usb();
        let height = (ids.len() as u16 + 3).min(area.height / 2);
        let [top, bottom] = Layout::vertical([Constraint::Min(0), Constraint::Length(height)]).areas(area);

        let boards = devices::all();
        let rows = boards.iter().map(|d| {
            vec![Line::raw(d.name.clone()), Line::raw(d.maker.clone()), Line::raw(d.chip.clone()), Line::raw(d.model.clone())]
        });
        let grid = Grid::new(vec![
            Col::new(t("col-board")).grow(),
            Col::new(t("col-maker")).drop(2),
            Col::new(t("col-chip")),
            Col::new(t("col-reported")).drop(1),
        ])
        .rows(rows);
        self.selected = self.selected.min(boards.len().saturating_sub(1));
        self.table.select(Some(self.selected));
        grid.show(f, top, panel_focus(&tf("boards-title", &[("count", boards.len().into())]), true), &mut self.table);

        let rows = ids.iter().map(|u| {
            let chip = if u.flash_mode { Line::styled(u.chip.clone(), theme::warn()) } else { Line::raw(u.chip.clone()) };
            vec![Line::raw(u.id.clone()), chip, Line::raw(u.boards.clone())]
        });
        let grid = Grid::new(vec![Col::new(t("col-usb-id")), Col::new(t("col-chip")), Col::new(t("col-boards")).grow()])
            .rows(rows);
        // capped at half the page: what does not fit is marked "▼ more"
        grid.show(f, bottom, panel(&tf("boards-usb-title", &[("count", ids.len().into())])), &mut TableState::default());
    }

    fn on_key(&mut self, key: KeyEvent, _ctx: &Ctx) -> Action {
        self.selected = step(self.selected, devices::all().len(), key.code);
        Action::None
    }
}

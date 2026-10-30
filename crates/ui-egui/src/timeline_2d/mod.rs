//! Two-dimensional (2D) History Browser for SolidCraft.
//!
//! Provides multi-track component layout, two-tier header scrubber, dependency graph
//! visualization, movable brackets corridor, and feature grouping/folding.

pub mod canvas;
pub mod deps;
pub mod state;

use egui::{Rect, Ui, UiBuilder};
use crate::SolveApp;
use crate::theme::Tokens;
use crate::ui_kit::{Container, Edge, Surface};

/// Computes the floating card rectangle for the 2D timeline strip.
pub fn floating_rect(available: Rect) -> Rect {
    Container::floating_card(Surface::Panel).bottom_strip_rect(available, canvas::DOCK_TOTAL_HEIGHT)
}

/// Renders the 2D timeline dock frame and contents.
pub fn timeline(app: &mut SolveApp, ui: &mut Ui) {
    let t = Tokens::get();
    app.shell.history_dock_space = Some(ui.available_rect_before_wrap());
    if Container::for_layout(app.ui.layout, Surface::Panel).floating() {
        let rect = floating_rect(ui.available_rect_before_wrap());
        if !rect.is_finite() || rect.width() < 1.0 || rect.height() < 30.0 {
            app.shell.history = None;
            return;
        }
        app.shell.history = Some(rect);
        crate::scenario::publish_rect("history:card", rect);
        let mut child = ui.new_child(UiBuilder::new().id_salt("sc_timeline_2d").max_rect(rect));
        Container::paint_frame(ui.painter(), Container::new(ui.ctx(), Surface::Panel).frame(ui.ctx(), t.timeline, crate::frame::Side::Left), rect);
        child.set_clip_rect(rect);
        canvas::paint_timeline_2d(app, &mut child, true);
    } else {
        let response = Container::new(ui.ctx(), Surface::Panel)
            .edge("sc_timeline", Edge::Bottom)
            .exact_size(canvas::DOCK_TOTAL_HEIGHT)
            .frame(Container::new(ui.ctx(), Surface::Panel).edge_frame(t.timeline))
            .show(ui, |ui| {
                canvas::paint_timeline_2d(app, ui, false);
            });
        let frame = Container::new(ui.ctx(), Surface::Panel).edge_frame(t.timeline);
        Container::paint_inset(ui.painter(), response.response.rect - frame.outer_margin, frame);
        app.shell.history = Some(response.response.rect);
        crate::scenario::publish_rect("history:strip", response.response.rect);
    }
}

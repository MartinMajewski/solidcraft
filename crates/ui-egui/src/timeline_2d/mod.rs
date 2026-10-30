//! Two-dimensional (2D) History Browser for SolidCraft.
//!
//! Provides multi-track component layout, two-tier header scrubber, dependency graph
//! visualization, movable brackets corridor, and feature grouping/folding.

pub mod canvas;
pub mod deps;
pub mod state;

use egui::{Rect, Ui, UiBuilder, pos2};
use crate::SolveApp;
use crate::theme::Tokens;
use crate::ui_kit::{Container, Edge, Surface};

/// Computes the floating card rectangle for the 2D timeline strip.
pub fn floating_rect(available: Rect) -> Rect {
    Container::floating_card(Surface::Panel).bottom_strip_rect(available, canvas::DOCK_TOTAL_HEIGHT)
}

/// Computes the floating card rectangle with a custom height.
pub fn floating_rect_with_height(available: Rect, height: f32) -> Rect {
    Container::floating_card(Surface::Panel).bottom_strip_rect(available, height)
}

/// Renders the 2D timeline dock frame and contents.
pub fn timeline(app: &mut SolveApp, ui: &mut Ui) {
    let t = Tokens::get();
    app.shell.history_dock_space = Some(ui.available_rect_before_wrap());
    let height_id = egui::Id::new("sc_timeline_2d_height");

    if Container::for_layout(app.ui.layout, Surface::Panel).floating() {
        let remembered_h: f32 = ui.ctx().data_mut(|d| d.get_persisted::<f32>(height_id)).unwrap_or(canvas::DOCK_TOTAL_HEIGHT);
        let rect = floating_rect_with_height(ui.available_rect_before_wrap(), remembered_h);
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
        let available = ui.available_rect_before_wrap();
        let min_h = canvas::HEADER_TOTAL_HEIGHT + canvas::ROW_HEIGHT;
        let max_h = (available.height() * 0.75).max(min_h);

        let remembered_h: f32 = ui.ctx().data_mut(|d| d.get_persisted::<f32>(height_id)).unwrap_or(canvas::DOCK_TOTAL_HEIGHT);
        let current_h = remembered_h.clamp(min_h, max_h);

        let rounding = egui::CornerRadius {
            nw: t.radius_lg,
            ne: t.radius_lg,
            sw: 0,
            se: 0,
        };
        let frame = Container::new(ui.ctx(), Surface::Panel)
            .edge_frame(t.timeline)
            .corner_radius(rounding);

        let response = Container::new(ui.ctx(), Surface::Panel)
            .edge("sc_timeline", Edge::Bottom)
            .exact_size(current_h)
            .frame(frame)
            .show(ui, |ui| {
                canvas::paint_timeline_2d(app, ui, false);
            });

        Container::paint_inset(ui.painter(), response.response.rect - frame.outer_margin, frame);
        app.shell.history = Some(response.response.rect);
        crate::scenario::publish_rect("history:strip", response.response.rect);
        crate::scenario::publish_handle("timeline:top_rail", pos2(response.response.rect.center().x, response.response.rect.top()));

        // Upper edge interactive resize overlay
        let rail_bounds = Rect::from_x_y_ranges(
            response.response.rect.x_range(),
            (response.response.rect.top() - 5.0)..=(response.response.rect.top() + 5.0),
        );
        let overlay = Container::new(ui.ctx(), Surface::Chrome)
            .area(egui::Id::new("sc_timeline_top_resize_overlay"))
            .fixed_pos(rail_bounds.min)
            .movable(false)
            .order(egui::Order::Middle);
        overlay.show(ui.ctx(), |ui| {
            ui.set_min_size(rail_bounds.size());
            let (rect, resp) = ui.allocate_exact_size(rail_bounds.size(), egui::Sense::click_and_drag());
            let resp = resp.on_hover_cursor(egui::CursorIcon::ResizeVertical);
            if resp.hovered() || resp.dragged() {
                ui.painter().line_segment(
                    [pos2(rect.left(), response.response.rect.top()), pos2(rect.right(), response.response.rect.top())],
                    egui::Stroke::new(2.0, t.accent),
                );
            }
            if resp.dragged() {
                let dy = ui.input(|i| i.pointer.delta().y);
                if dy.is_finite() && dy != 0.0 {
                    let new_h = (current_h - dy).clamp(min_h, max_h);
                    ui.ctx().data_mut(|d| d.insert_persisted(height_id, new_h));
                    ui.ctx().request_repaint();
                }
            }
            if resp.double_clicked() {
                ui.ctx().data_mut(|d| d.insert_persisted(height_id, canvas::DOCK_TOTAL_HEIGHT));
                ui.ctx().request_repaint();
            }
        });
    }
}

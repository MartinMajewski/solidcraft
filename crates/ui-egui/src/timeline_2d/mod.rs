//! Two-dimensional (2D) History Browser for SolidCraft.
//!
//! Provides multi-track component layout, two-tier header scrubber, dependency graph
//! visualization, movable brackets corridor, and feature grouping/folding.

pub mod canvas;
pub mod deps;
pub mod state;

use crate::SolveApp;
use crate::theme::Tokens;
use crate::ui_kit::{Container, Edge, Surface};
use egui::{Rect, Ui, UiBuilder, pos2};

/// Computes the floating card rectangle with a custom height.
pub fn floating_rect_with_height(available: Rect, height: f32) -> Rect {
    Container::floating_card(Surface::Panel).bottom_strip_rect(available, height)
}

/// Computes the floating card rectangle for the 2D timeline strip using persisted height.
pub fn floating_rect(ctx: &egui::Context, available: Rect) -> Rect {
    let height_id = egui::Id::new("sc_timeline_2d_height");
    let remembered_h: f32 = ctx.data_mut(|d| d.get_persisted::<f32>(height_id)).unwrap_or(canvas::DOCK_TOTAL_HEIGHT);
    let min_h = canvas::HEADER_TOTAL_HEIGHT + canvas::ROW_HEIGHT;
    let max_h = (available.height() * 0.75).max(min_h);
    let current_h = remembered_h.clamp(min_h, max_h);
    floating_rect_with_height(available, current_h)
}

/// Renders the 2D timeline dock frame and contents.
pub fn timeline(app: &mut SolveApp, ui: &mut Ui) {
    let t = Tokens::get();
    app.shell.history_dock_space = Some(ui.available_rect_before_wrap());
    let height_id = egui::Id::new("sc_timeline_2d_height");
    let available = ui.available_rect_before_wrap();
    let min_h = canvas::HEADER_TOTAL_HEIGHT + canvas::ROW_HEIGHT;
    let max_h = (available.height() * 0.75).max(min_h);

    let remembered_h: f32 = ui.ctx().data_mut(|d| d.get_persisted::<f32>(height_id)).unwrap_or(canvas::DOCK_TOTAL_HEIGHT);
    let current_h = remembered_h.clamp(min_h, max_h);

    let rounding = egui::CornerRadius { nw: t.radius_lg, ne: t.radius_lg, sw: 0, se: 0 };
    let frame = Container::new(ui.ctx(), Surface::Panel).edge_frame(t.timeline).corner_radius(rounding);

    let is_floating = Container::for_layout(app.ui.layout, Surface::Panel).floating();
    let view_rect = if is_floating {
        let rect = floating_rect_with_height(available, current_h);
        if !rect.is_finite() || rect.width() < 1.0 || rect.height() < 30.0 {
            app.shell.history = None;
            return;
        }
        app.shell.history = Some(rect);
        crate::scenario::publish_rect("history:card", rect);
        let mut child = ui.new_child(UiBuilder::new().id_salt("sc_timeline_2d").max_rect(rect));
        Container::paint_frame(ui.painter(), frame, rect);
        child.set_clip_rect(rect);
        canvas::paint_timeline_2d(app, &mut child, true);
        rect
    } else {
        let response = Container::new(ui.ctx(), Surface::Panel).edge("sc_timeline", Edge::Bottom).exact_size(current_h).frame(frame).show(ui, |ui| {
            canvas::paint_timeline_2d(app, ui, false);
        });

        Container::paint_inset(ui.painter(), response.response.rect - frame.outer_margin, frame);
        app.shell.history = Some(response.response.rect);
        crate::scenario::publish_rect("history:strip", response.response.rect);
        response.response.rect
    };

    // Upper edge interactive resize rail (shared by both Floating and Docked layouts)
    let rail_bounds = Rect::from_x_y_ranges(view_rect.x_range(), (view_rect.top() - 4.0)..=(view_rect.top() + 6.0));
    let overlay = Container::new(ui.ctx(), Surface::Chrome)
        .area(egui::Id::new("sc_timeline_2d_resize_rail"))
        .fixed_pos(rail_bounds.min)
        .movable(false)
        .order(egui::Order::Middle);
    overlay.show(ui.ctx(), |ui| {
        ui.set_min_size(rail_bounds.size());
        ui.set_clip_rect(ui.ctx().content_rect());
        let (_, resp) = ui.allocate_exact_size(rail_bounds.size(), egui::Sense::click_and_drag());
        let resp = resp.on_hover_cursor(egui::CursorIcon::ResizeVertical);

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

    crate::scenario::publish_handle("timeline:top_rail", pos2(view_rect.center().x, view_rect.top()));
    crate::scenario::publish_rect("timeline:top_rail", rail_bounds);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Harness;
    use serde_json::json;

    #[test]
    fn test_timeline_2d_hover_and_resize() {
        let mut h = Harness::new();
        h.app.preferences.timeline_2d = true;
        h.call("ui.resize", json!({"width": 1400, "height": 900}));
        h.frames(2);

        // --- 1. Floating layout test ---
        let initial_rect = crate::scenario::rect_of("history:card").unwrap();
        let initial_h = initial_rect.height();

        let rail_handle = crate::scenario::handle_at("timeline:top_rail").unwrap();
        assert!((rail_handle.y - initial_rect.top()).abs() < 1.0);

        // Drag top rail upward by 60px
        h.call(
            "ui.drag",
            json!({
                "x0": rail_handle.x,
                "y0": rail_handle.y,
                "x1": rail_handle.x,
                "y1": rail_handle.y - 60.0,
                "steps": 6,
            }),
        );
        h.frames(2);

        let new_rect = crate::scenario::rect_of("history:card").unwrap();
        assert!(new_rect.height() > initial_h + 30.0, "Timeline height should have increased, was {}, now {}", initial_h, new_rect.height());

        // Double-click rail to reset
        let new_rail = crate::scenario::handle_at("timeline:top_rail").unwrap();
        h.call(
            "ui.click",
            json!({
                "x": new_rail.x,
                "y": new_rail.y,
                "double": true,
            }),
        );
        h.frames(2);
        let reset_rect = crate::scenario::rect_of("history:card").unwrap();
        assert!((reset_rect.height() - canvas::DOCK_TOTAL_HEIGHT).abs() < 2.0);

        // --- 2. Docked layout test ---
        h.app.ui.layout = crate::shell::LayoutMode::Docked;
        h.frames(2);

        let docked_rect = crate::scenario::rect_of("history:strip").unwrap();
        let docked_h = docked_rect.height();
        let docked_rail = crate::scenario::handle_at("timeline:top_rail").unwrap();

        h.call(
            "ui.drag",
            json!({
                "x0": docked_rail.x,
                "y0": docked_rail.y,
                "x1": docked_rail.x,
                "y1": docked_rail.y - 50.0,
                "steps": 5,
            }),
        );
        h.frames(2);

        let new_docked = crate::scenario::rect_of("history:strip").unwrap();
        assert!(new_docked.height() > docked_h + 25.0);
    }

    #[test]
    fn test_rename_group_dialog() {
        let mut h = Harness::new();
        h.app.run("solid.box", json!({"length": 20, "width": 20, "height": 10})).unwrap();
        h.app.run("solid.box", json!({"length": 20, "width": 20, "height": 10, "corner": [30, 0, 0]})).unwrap();
        let f0 = h.app.session.doc.features[0].id;
        let f1 = h.app.session.doc.features[1].id;

        let res = h
            .app
            .run_heavy(
                "timeline.group",
                json!({
                    "name": "Group 1",
                    "features": [f0, f1],
                }),
            )
            .unwrap();
        let gid = res["group"].as_u64().unwrap();

        let mut d = crate::dialogs::Dialog::rename_group(gid, "Group 1");
        assert_eq!(crate::dialogs::report_title(&d), "RENAME GROUP");

        if let crate::dialogs::Kind::RenameGroup { name, .. } = &mut d.kind {
            *name = "Custom Assembly".to_string();
        }
        let cmds = crate::dialogs::apply_commands(&h.app, &d).unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].0, "timeline.rename_group");
        assert_eq!(cmds[0].1["group"], gid);
        assert_eq!(cmds[0].1["name"], "Custom Assembly");

        h.app.dialog = Some(d);
        h.frames(2);
        h.step(&json!({"click": {"handle": "dialog:ok"}})).unwrap();
        h.frames(2);

        assert_eq!(h.app.session.doc.timeline_groups.iter().find(|g| g.id == gid).map(|g| g.name.as_str()), Some("Custom Assembly"));
    }

    #[test]
    fn test_multi_component_chips_and_connector() {
        let mut h = Harness::new();
        h.app.preferences.timeline_2d = true;

        // 1. Box in root component 0
        h.app.run("solid.box", json!({"length": 20, "width": 20, "height": 10})).unwrap();

        // 2. Create Component 1
        let c1 = h.app.run("component.create", json!({"name": "Housing", "activate": true})).unwrap();
        let comp1 = c1["component"].as_u64().unwrap();

        // 3. Box in Component 1
        h.app.run("solid.box", json!({"length": 20, "width": 20, "height": 10, "corner": [5, 0, 0]})).unwrap();

        let b0 = h.app.session.model.state().bodies[0].name.clone();
        let b1 = h.app.session.model.state().bodies[1].name.clone();

        // 4. Combine b0 (in component 0) and b1 (in component 1)
        h.app.run("solid.combine", json!({"target": b0, "tools": [b1], "operation": "join"})).unwrap();

        let combine_feat = h.app.session.doc.features.last().unwrap();
        let affected = canvas::feature_affected_components(&h.app.session.doc, Some(&h.app.session.model), combine_feat);
        assert_eq!(affected, vec![0, comp1]);

        // 5. Chamfer in Component 1 on edge of combined body
        let m = h.app.session.model.state().bodies[0].mesh();
        let p0 = m.edges[0][0];
        let p1 = m.edges[0][1];
        let edge_mid = (p0 + p1) * 0.5;
        h.app
            .run(
                "solid.chamfer",
                json!({
                    "edges": [[edge_mid.x, edge_mid.y, edge_mid.z]],
                    "distance": 1.0,
                }),
            )
            .unwrap();

        let chamfer_feat = h.app.session.doc.features.last().unwrap();
        let chamfer_affected = canvas::feature_affected_components(&h.app.session.doc, Some(&h.app.session.model), chamfer_feat);
        assert!(chamfer_affected.contains(&0) && chamfer_affected.contains(&comp1));

        // Render frames and verify no panics
        h.call("ui.resize", json!({"width": 1400, "height": 900}));
        h.frames(3);
    }

    #[test]
    fn test_chip_drag_horizontal_reorder() {
        let mut h = Harness::new();
        h.app.preferences.timeline_2d = true;

        // 1. Box 1 in root component 0
        h.app.run("solid.box", json!({"name": "BoxA", "length": 20, "width": 20, "height": 10})).unwrap();
        let f0 = h.app.session.doc.features[0].id;

        // 2. Box 2 in root component 0 (independent, non-overlapping)
        h.app.run("solid.box", json!({"name": "BoxB", "length": 15, "width": 15, "height": 10, "corner": [50, 0, 0]})).unwrap();
        let f1 = h.app.session.doc.features[1].id;

        // Verify initial sequence: [BoxA, BoxB]
        assert_eq!(h.app.session.doc.features[0].id, f0);
        assert_eq!(h.app.session.doc.features[1].id, f1);

        // 3. Test horizontal reorder: move BoxB before BoxA
        let res = h.app.run_heavy(
            "timeline.reorder",
            json!({
                "feature": f1,
                "position": 0,
                "active": true,
            }),
        );
        assert!(res.is_ok(), "reorder BoxB to position 0 should succeed");
        assert_eq!(h.app.session.doc.features[0].id, f1);
        assert_eq!(h.app.session.doc.features[1].id, f0);

        // Render frames with drag state active
        h.call("ui.resize", json!({"width": 1400, "height": 900}));
        h.frames(2);
    }

    #[test]
    fn test_suppressed_chips_and_dependency_cascade() {
        let mut h = Harness::new();
        h.app.preferences.timeline_2d = true;

        // 1. Base box in root component
        h.app.run("solid.box", json!({"name": "BaseBox", "length": 30, "width": 30, "height": 15})).unwrap();
        let box_id = h.app.session.doc.features[0].id;

        // 2. Chamfer on an edge of BaseBox
        let m = h.app.session.model.state().bodies[0].mesh();
        let p0 = m.edges[0][0];
        let p1 = m.edges[0][1];
        let edge_mid = (p0 + p1) * 0.5;
        h.app
            .run(
                "solid.chamfer",
                json!({
                    "edges": [[edge_mid.x, edge_mid.y, edge_mid.z]],
                    "distance": 1.0,
                }),
            )
            .unwrap();
        let chamfer_id = h.app.session.doc.features[1].id;

        // 3. Verify dependency graph: chamfer depends on BaseBox
        let dep_graph = deps::DependencyGraph::from_doc(&h.app.session.doc);
        assert!(dep_graph.is_dependent(box_id, chamfer_id));
        assert!(dep_graph.is_prerequisite(chamfer_id, box_id));
        assert!(!dep_graph.is_suppressed(&h.app.session.doc, box_id));
        assert!(!dep_graph.is_suppressed(&h.app.session.doc, chamfer_id));

        // 4. Suppression cascade: suppressing BaseBox must include dependent chamfer
        let cascade = dep_graph.collect_suppression_cascade(box_id);
        assert!(cascade.contains(&box_id));
        assert!(cascade.contains(&chamfer_id));

        let res = h.app.run_heavy(
            "timeline.suppress",
            json!({
                "features": cascade,
                "suppressed": true,
            }),
        );
        assert!(res.is_ok());
        assert!(h.app.session.doc.feature(box_id).unwrap().suppressed);
        assert!(h.app.session.doc.feature(chamfer_id).unwrap().suppressed);
        assert!(dep_graph.is_suppressed(&h.app.session.doc, box_id));
        assert!(dep_graph.is_suppressed(&h.app.session.doc, chamfer_id));

        // 5. Unsuppression cascade: unsuppressing chamfer must restore required BaseBox
        let unsuppress_cascade = dep_graph.collect_unsuppression_cascade(&h.app.session.doc, chamfer_id);
        assert!(unsuppress_cascade.contains(&box_id));
        assert!(unsuppress_cascade.contains(&chamfer_id));

        let res = h.app.run_heavy(
            "timeline.suppress",
            json!({
                "features": unsuppress_cascade,
                "suppressed": false,
            }),
        );
        assert!(res.is_ok());
        assert!(!h.app.session.doc.feature(box_id).unwrap().suppressed);
        assert!(!h.app.session.doc.feature(chamfer_id).unwrap().suppressed);
        assert!(!dep_graph.is_suppressed(&h.app.session.doc, box_id));
        assert!(!dep_graph.is_suppressed(&h.app.session.doc, chamfer_id));

        // 6. Re-suppress BaseBox and render frames to exercise UI chip drawing (dashed border & 50% opacity slashed eye)
        let res = h.app.run_heavy(
            "timeline.suppress",
            json!({
                "features": cascade,
                "suppressed": true,
            }),
        );
        assert!(res.is_ok());

        h.call("ui.resize", json!({"width": 1400, "height": 900}));
        h.frames(3);
    }

    #[test]
    fn test_group_header_truncation_and_tooltip() {
        let mut h = Harness::new();
        h.app.preferences.timeline_2d = true;
        h.call("ui.resize", json!({"width": 1400, "height": 900}));
        h.frames(1);

        let ctx = h.ctx.clone();
        let painter =
            egui::Painter::new(ctx.clone(), egui::LayerId::background(), egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 1000.0)));

        let font = egui::FontId::proportional(10.0);
        let color = egui::Color32::WHITE;

        // 1. Long name in narrow width (48.0 pt for 2 columns): should be truncated with ellipsis
        let long_name = "Foundation Structures";
        let (fitted, is_cut) = canvas::fit_group_banner_title(&painter, long_name, 2, 48.0, font.clone(), color);
        assert!(is_cut, "Long name in 48pt width should be cut");
        assert!(fitted.contains('…'), "Fitted title should contain ellipsis");
        assert!(fitted.ends_with("(2)"), "Fitted title should preserve member count suffix");
        let measured_w = painter.layout_no_wrap(fitted.clone(), font.clone(), color).size().x;
        assert!(measured_w <= 48.0, "Measured width {} must fit within 48.0", measured_w);

        // 2. Wide width (300.0 pt): should NOT be cut
        let (wide_fitted, wide_is_cut) = canvas::fit_group_banner_title(&painter, long_name, 2, 300.0, font.clone(), color);
        assert!(!wide_is_cut, "Long name in 300pt width should not be cut");
        assert_eq!(wide_fitted, "Foundation Structures (2)");

        // 3. Extremely narrow width (10.0 pt): should not crash or exceed width
        let (tiny_fitted, tiny_is_cut) = canvas::fit_group_banner_title(&painter, long_name, 2, 10.0, font.clone(), color);
        assert!(tiny_is_cut);
        let tiny_w = painter.layout_no_wrap(tiny_fitted, font.clone(), color).size().x;
        assert!(tiny_w <= 10.0);

        // 4. Create features and group in a design to exercise rendering with truncation
        h.app.run("solid.box", json!({"length": 20.0, "width": 20.0, "height": 10.0})).unwrap();
        h.app.run("solid.box", json!({"length": 15.0, "width": 15.0, "height": 10.0, "corner": [30, 0, 0]})).unwrap();
        let box1_id = h.app.session.doc.features[0].id;
        let box2_id = h.app.session.doc.features[1].id;

        let res = h.app.run_heavy(
            "timeline.group",
            json!({
                "features": [box1_id, box2_id],
                "name": "Very Long Custom Group Name That Exceeds Two Columns",
            }),
        );
        assert!(res.is_ok());

        h.frames(3);
    }

    #[test]
    fn test_active_component_highlighting_and_activation() {
        let mut h = Harness::new();
        h.app.preferences.timeline_2d = true;
        h.call("ui.resize", json!({"width": 1400, "height": 900}));

        // 1. Initial active component is Root (0)
        assert_eq!(h.app.session.active_component, 0);

        // Feature created while root is active belongs to component 0
        h.app.run("solid.box", json!({"length": 20.0, "width": 20.0, "height": 10.0})).unwrap();
        assert_eq!(h.app.session.doc.features.last().unwrap().component, 0);

        // 2. Create Component 1 "Housing" with activate: false
        let c1 = h.app.run("component.create", json!({"name": "Housing", "activate": false})).unwrap();
        let comp1 = c1["component"].as_u64().unwrap();
        assert_eq!(h.app.session.active_component, 0);

        // 3. Create Component 2 "Shaft" with activate: true
        let c2 = h.app.run("component.create", json!({"name": "Shaft", "activate": true})).unwrap();
        let comp2 = c2["component"].as_u64().unwrap();
        assert_eq!(h.app.session.active_component, comp2);

        // Feature created while comp2 is active belongs to comp2
        h.app.run("solid.box", json!({"length": 15.0, "width": 15.0, "height": 10.0, "corner": [30, 0, 0]})).unwrap();
        assert_eq!(h.app.session.doc.features.last().unwrap().component, comp2);

        // 4. Render frames to exercise paint_pinned_left with active radio and track wash
        h.frames(3);

        // 5. Exclusively activate component 1 "Housing"
        let res = h.app.run("component.activate", json!({"component": comp1}));
        assert!(res.is_ok());
        assert_eq!(h.app.session.active_component, comp1);

        // New feature created now automatically belongs to component 1!
        h.app.run("solid.box", json!({"length": 10.0, "width": 10.0, "height": 5.0, "corner": [60, 0, 0]})).unwrap();
        assert_eq!(h.app.session.doc.features.last().unwrap().component, comp1);

        // 6. Switch back to Root component (0)
        let res = h.app.run("component.activate", json!({"component": 0}));
        assert!(res.is_ok());
        assert_eq!(h.app.session.active_component, 0);

        h.frames(3);
    }
}

//! 2D Grid Canvas, Two-Tier Header, Pinned Component Tracks, and Marker Scrubber.

use std::collections::{BTreeMap, BTreeSet};
use egui::{
    FontId, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use solidcraft_engine::doc::{Document, Feature};
use crate::SolveApp;
use crate::theme::Tokens;
use crate::ui_kit::{HistoryTransport, Menu};
use super::deps::{CorridorSplit, DependencyGraph, MovableCorridor};
use super::state::Timeline2DState;

pub const HEADER_T1_HEIGHT: f32 = 24.0;
pub const HEADER_T2_HEIGHT: f32 = 20.0;
pub const HEADER_TOTAL_HEIGHT: f32 = 44.0;
pub const ROW_HEIGHT: f32 = 38.0;
pub const COL_WIDTH: f32 = 52.0;
pub const PINNED_LEFT_WIDTH: f32 = 160.0;
pub const CHIP_WIDTH: f32 = 38.0;
pub const CHIP_HEIGHT: f32 = 32.0;
pub const VIRTUAL_GROUP_HEIGHT: f32 = 204.0;
pub const DOCK_TOTAL_HEIGHT: f32 = 186.0;

/// Represents a visual column on the timeline grid.
#[derive(Clone, Debug)]
pub enum VisualColumn {
    /// A single feature column (or multiple features in multi-component step).
    Features {
        step_number: usize,
        features: Vec<u64>,
        open_group: Option<u64>,
    },
    /// A folded group column collapsing member features into a single virtual chip.
    FoldedGroup {
        group_id: u64,
        features: Vec<u64>,
    },
}

impl VisualColumn {
    pub fn is_folded_group(&self) -> bool {
        matches!(self, VisualColumn::FoldedGroup { .. })
    }

    pub fn feature_ids(&self) -> &[u64] {
        match self {
            VisualColumn::Features { features, .. } => features,
            VisualColumn::FoldedGroup { features, .. } => features,
        }
    }
}

/// Renders the 2D History Browser dock contents.
pub fn paint_timeline_2d(app: &mut SolveApp, ui: &mut Ui, floating: bool) {
    let t = Tokens::get();
    let accent = crate::theme::TabAccent::for_tab(&t, &app.ui.tab);
    let total_rect = ui.max_rect();
    let painter = ui.painter().clone();

    // Load or initialize state
    let state_id = ui.id().with("sc_timeline_2d_state");
    let mut state: Timeline2DState = ui.data(|d| d.get_temp(state_id)).unwrap_or_default();

    // 1. Build dependency graph
    let dep_graph = DependencyGraph::from_doc(&app.session.doc);
    let total_features = app.session.doc.features.len();
    let marker_slot = app.session.doc.marker.unwrap_or(total_features);

    // 2. Discover component tracks
    let mut component_ids: Vec<u64> = Vec::new();
    let mut component_names: BTreeMap<u64, String> = BTreeMap::new();
    let mut component_colors: BTreeMap<u64, Option<[u8; 3]>> = BTreeMap::new();

    // Root component 0 always first
    component_ids.push(0);
    let root_name = if app.session.doc.name.is_empty() {
        "Root Component".to_string()
    } else {
        app.session.doc.name.clone()
    };
    component_names.insert(0, root_name);

    for comp in &app.session.doc.components {
        if comp.id != 0 && !component_ids.contains(&comp.id) {
            component_ids.push(comp.id);
            component_names.insert(comp.id, comp.name.clone());
            component_colors.insert(comp.id, comp.identification_color);
        }
    }
    for f in &app.session.doc.features {
        if !component_ids.contains(&f.component) {
            component_ids.push(f.component);
            component_names.insert(f.component, format!("Component {}", f.component));
        }
    }

    // 3. Build visual columns (respecting folded groups)
    let visual_columns = build_visual_columns(&app.session.doc, &state.folded_groups);
    let num_columns = visual_columns.len();

    // 4. Compute layout dimensions
    let content_width = num_columns as f32 * COL_WIDTH;
    let grid_origin_x = total_rect.left() + PINNED_LEFT_WIDTH;
    let grid_viewport_width = (total_rect.width() - PINNED_LEFT_WIDTH).max(0.0);
    let max_scroll_x = (content_width - grid_viewport_width).max(0.0);
    state.scroll_x = state.scroll_x.clamp(0.0, max_scroll_x);

    let num_tracks = component_ids.len();
    let content_height = HEADER_TOTAL_HEIGHT + num_tracks as f32 * ROW_HEIGHT;
    let _grid_viewport_height = (total_rect.height() - HEADER_TOTAL_HEIGHT).max(0.0);
    let max_scroll_y = (content_height - total_rect.height()).max(0.0);
    state.scroll_y = state.scroll_y.clamp(0.0, max_scroll_y);

    // 5. Handle mouse wheel scrolling and rubberband overscroll
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
    if ui.rect_contains_pointer(total_rect) {
        if scroll_delta.x.abs() > 0.0 {
            state.scroll_x = (state.scroll_x - scroll_delta.x).clamp(0.0, max_scroll_x);
        }
        if scroll_delta.y.abs() > 0.0 {
            if ui.input(|i| i.modifiers.shift) {
                state.scroll_x = (state.scroll_x - scroll_delta.y).clamp(0.0, max_scroll_x);
            } else {
                let target_y = state.scroll_y - scroll_delta.y;
                if target_y < 0.0 {
                    state.overscroll_y = target_y * 0.35;
                    state.scroll_y = 0.0;
                } else if target_y > max_scroll_y {
                    state.overscroll_y = (target_y - max_scroll_y) * 0.35;
                    state.scroll_y = max_scroll_y;
                } else {
                    state.scroll_y = target_y;
                    state.overscroll_y *= 0.85;
                }
            }
        } else {
            state.overscroll_y *= 0.85;
        }
    }

    // 6. Draw Orthogonal CAD Background Grid
    let grid_rect = Rect::from_min_max(
        pos2(grid_origin_x, total_rect.top() + HEADER_TOTAL_HEIGHT),
        total_rect.right_bottom(),
    );
    let grid_painter = ui.painter().with_clip_rect(grid_rect);
    let grid_line_color = t.border.gamma_multiply(0.25);

    for r in 0..=num_tracks {
        let y = total_rect.top() + HEADER_TOTAL_HEIGHT + r as f32 * ROW_HEIGHT - state.scroll_y + state.overscroll_y;
        if y >= grid_rect.top() && y <= grid_rect.bottom() {
            grid_painter.line_segment([pos2(grid_origin_x, y), pos2(total_rect.right(), y)], Stroke::new(1.0, grid_line_color));
        }
    }
    for c in 0..=num_columns {
        let x = grid_origin_x + c as f32 * COL_WIDTH - state.scroll_x;
        if x >= grid_origin_x && x <= total_rect.right() {
            grid_painter.line_segment([pos2(x, grid_rect.top()), pos2(x, grid_rect.bottom())], Stroke::new(1.0, grid_line_color));
        }
    }

    // 7. Calculate Active Selection and Movable Corridor
    let selected_id = state.selected_features.iter().next().copied();
    let corridor = selected_id.and_then(|id| dep_graph.corridor_for(id));

    // Draw movable corridor background and brackets if selected
    if let (Some(_sel_id), Some(corr)) = (selected_id, corridor) {
        paint_movable_corridor(
            &grid_painter,
            &visual_columns,
            &dep_graph,
            grid_origin_x,
            state.scroll_x,
            grid_rect.top(),
            grid_rect.bottom(),
            corr,
            marker_slot,
            &t,
        );
    }

    // 8. Paint Multi-Component Dotted Connectors & Action Chips
    let mut action_to_edit: Option<u64> = None;
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    for (col_idx, col) in visual_columns.iter().enumerate() {
        let col_x = grid_origin_x + col_idx as f32 * COL_WIDTH - state.scroll_x;
        if col_x + COL_WIDTH < grid_origin_x || col_x > total_rect.right() {
            continue;
        }

        match col {
            VisualColumn::Features { step_number, features, open_group } => {
                // If open group, draw subtle corridor tint in Tier 2
                if let Some(_grp_id) = open_group {
                    let banner_rect = Rect::from_min_size(
                        pos2(col_x, total_rect.top() + HEADER_T1_HEIGHT),
                        vec2(COL_WIDTH, HEADER_T2_HEIGHT),
                    );
                    let tint = t.accent.gamma_multiply(0.06);
                    painter.rect_filled(banner_rect, 0.0, tint);
                }

                // If multi-component, draw dead-centered vertical dotted connector
                if features.len() > 1 {
                    let mut min_y = f32::MAX;
                    let mut max_y = f32::MIN;
                    for &fid in features {
                        if let Some(f) = app.session.doc.feature(fid)
                            && let Some(track_idx) = component_ids.iter().position(|&cid| cid == f.component)
                        {
                            let cy = total_rect.top() + HEADER_TOTAL_HEIGHT + track_idx as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0 - state.scroll_y + state.overscroll_y;
                            min_y = min_y.min(cy);
                            max_y = max_y.max(cy);
                        }
                    }
                    if min_y < max_y && min_y < grid_rect.bottom() && max_y > grid_rect.top() {
                        let cx = col_x + COL_WIDTH / 2.0;
                        let line_color = if *step_number <= marker_slot {
                            accent.color
                        } else {
                            t.text_dim
                        };
                        paint_dotted_vertical_line(&grid_painter, cx, min_y, max_y, line_color);
                    }
                }

                // Paint individual chips
                for &fid in features {
                    if let Some(f) = app.session.doc.feature(fid) {
                        let track_idx = component_ids.iter().position(|&cid| cid == f.component).unwrap_or(0);
                        let chip_top = total_rect.top() + HEADER_TOTAL_HEIGHT + track_idx as f32 * ROW_HEIGHT + (ROW_HEIGHT - CHIP_HEIGHT) / 2.0 - state.scroll_y + state.overscroll_y;
                        let chip_rect = Rect::from_min_size(pos2(col_x + (COL_WIDTH - CHIP_WIDTH) / 2.0, chip_top), vec2(CHIP_WIDTH, CHIP_HEIGHT));

                        if chip_rect.bottom() < grid_rect.top() || chip_rect.top() > grid_rect.bottom() {
                            continue;
                        }

                        let f_slot = dep_graph.id_to_index.get(&fid).copied().unwrap_or(0);
                        let is_active = f_slot < marker_slot;
                        let is_selected = state.is_selected(fid);
                        let is_prereq = selected_id.is_some_and(|sel| dep_graph.is_prerequisite(sel, fid));
                        let is_dependent = selected_id.is_some_and(|sel| dep_graph.is_dependent(sel, fid));

                        let resp = ui.interact(chip_rect, ui.id().with(("chip_2d", fid)), Sense::click_and_drag());
                        if resp.hovered() {
                            state.hovered_feature = Some(fid);
                        }

                        // Attach rich hover tooltip showing action name, kind, status
                        let res = app.session.model.result(fid);
                        let err = res.and_then(|r| r.error.as_deref());
                        let warn = res.and_then(|r| r.warning.as_deref());
                        let mut tip = crate::timeline::status_text(&f.name, f.kind.type_name(), err, warn);
                        if !is_active {
                            let steps_after = f_slot.saturating_sub(marker_slot) + 1;
                            tip.push_str(&format!("\nInactive (rolled back, {} step(s) after marker)", steps_after));
                        }
                        let resp = resp.on_hover_text(tip);

                        // Handle selection clicks
                        if resp.clicked() {
                            let modifiers = ui.input(|i| i.modifiers);
                            if modifiers.shift {
                                state.select_range(&dep_graph.features, fid, modifiers.command || modifiers.ctrl);
                            } else if modifiers.command || modifiers.ctrl {
                                state.toggle_selection(fid);
                            } else {
                                state.select_single(fid);
                            }
                        }

                        if resp.double_clicked() {
                            action_to_edit = Some(fid);
                        }

                        // Paint action chip
                        paint_action_chip(
                            &grid_painter,
                            chip_rect,
                            f,
                            is_active,
                            is_selected,
                            is_prereq,
                            is_dependent,
                            accent,
                            &t,
                        );

                        // Attach context menu
                        let targets = crate::timeline::menu_targets(app, fid);
                        let is_suppressed = f.suppressed;
                        let f_name = f.name.clone();
                        let is_adjacent = state.is_selection_adjacent(&dep_graph.features);
                        let sel_count = state.selected_features.len();

                        Menu::context(&resp, |ui| {
                            if crate::context_menu::button(ui, crate::i18n::tr_label("Edit Feature"), "edit").clicked() {
                                action_to_edit = Some(fid);
                                ui.close();
                            }
                            if is_adjacent && sel_count >= 2 {
                                let label = crate::i18n::format("ui.timeline_2d_group_selection", &[("count", &sel_count.to_string())]);
                                if crate::context_menu::button(ui, &label, "folder").clicked() {
                                    let ordered_selected: Vec<u64> = dep_graph.features.iter().copied().filter(|id| state.selected_features.contains(id)).collect();
                                    let group_name = format!("Group {}", app.session.doc.timeline_groups.len() + 1);
                                    let _ = app.run_heavy("timeline.group", serde_json::json!({
                                        "name": group_name,
                                        "features": ordered_selected,
                                    }));
                                    ui.close();
                                }
                            }
                            if crate::context_menu::button(ui, crate::i18n::tr_label("Rename"), "rename").clicked() {
                                app.dialog = Some(crate::dialogs::Dialog::rename(fid, &f_name));
                                ui.close();
                            }
                            let suppress_key = if is_suppressed { "timeline.unsuppress_features" } else { "timeline.suppress_features" };
                            if crate::context_menu::button(ui, crate::i18n::tr(suppress_key), "suppress").clicked() {
                                let _ = app.run_heavy("timeline.suppress", serde_json::json!({ "feature": fid }));
                                ui.close();
                            }
                            if crate::context_menu::button(ui, crate::i18n::tr_label("Roll History Marker Here"), "rollback").clicked() {
                                let _ = app.run_heavy("timeline.roll_to", serde_json::json!({ "feature": fid }));
                                ui.close();
                            }
                            ui.separator();
                            let delete_label = match targets.len() {
                                1 => crate::i18n::tr_label("Delete").to_string(),
                                n => crate::i18n::format("timeline.delete_many", &[("count", &n.to_string())]),
                            };
                            if crate::context_menu::button(ui, &delete_label, "delete").clicked() {
                                crate::timeline::delete_features(app, targets);
                                ui.close();
                            }
                        });
                    }
                }
            }
            VisualColumn::FoldedGroup { group_id, features } => {
                let card_top = total_rect.top() + HEADER_T1_HEIGHT;
                let card_rect = Rect::from_min_size(
                    pos2(col_x + (COL_WIDTH - CHIP_WIDTH) / 2.0, card_top),
                    vec2(CHIP_WIDTH, VIRTUAL_GROUP_HEIGHT),
                );

                let first_slot = features.first().and_then(|id| dep_graph.id_to_index.get(id)).copied().unwrap_or(0);
                let is_active = first_slot < marker_slot;
                let is_group_selected = features.iter().any(|id| state.is_selected(*id));

                let grp_name = app
                    .session
                    .doc
                    .timeline_groups
                    .iter()
                    .find(|g| g.id == *group_id)
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| format!("Group {group_id}"));

                let resp = ui.interact(card_rect, ui.id().with(("grp_card", *group_id)), Sense::click_and_drag());
                if resp.hovered() {
                    state.hovered_group = Some(*group_id);
                }

                if resp.clicked() {
                    state.selected_features.clear();
                    for &fid in features {
                        state.selected_features.insert(fid);
                    }
                }
                if resp.double_clicked() {
                    state.toggle_group_fold(*group_id);
                }

                // Rich hover tooltip
                let mut tooltip_feats = Vec::new();
                for &fid in features {
                    if let Some(f) = app.session.doc.feature(fid) {
                        let step = dep_graph.id_to_index.get(&fid).copied().unwrap_or(0) + 1;
                        let comp_name = component_names.get(&f.component).map(|s| s.as_str()).unwrap_or("Component");
                        tooltip_feats.push((step, f.name.as_str(), comp_name));
                    }
                }
                let tip = Timeline2DState::format_group_tooltip(&grp_name, &tooltip_feats, is_active);
                let resp = resp.on_hover_text(tip);

                // Group Context Menu
                let gid = *group_id;
                let g_feats = features.clone();
                Menu::context(&resp, |ui| {
                    if crate::context_menu::button(ui, crate::i18n::tr("ui.timeline_2d_open_group"), "folder").clicked() {
                        state.set_group_folded(gid, false);
                        ui.close();
                    }
                    if crate::context_menu::button(ui, crate::i18n::tr("ui.timeline_2d_delete_group_expand"), "folder").clicked() {
                        let _ = app.run_heavy("timeline.ungroup", serde_json::json!({ "group": gid }));
                        ui.close();
                    }
                    if crate::context_menu::button(ui, crate::i18n::tr("ui.timeline_2d_delete_group_all"), "delete").clicked() {
                        crate::timeline::delete_features(app, g_feats);
                        let _ = app.run_heavy("timeline.ungroup", serde_json::json!({ "group": gid }));
                        ui.close();
                    }
                });

                paint_virtual_group_chip(&painter, card_rect, &grp_name, features.len(), is_active, is_group_selected, &t);
            }
        }
    }

    // 9. Paint Two-Tier Header Track
    paint_two_tier_header(
        &painter,
        ui,
        app,
        total_rect,
        grid_origin_x,
        state.scroll_x,
        &visual_columns,
        marker_slot,
        accent,
        &t,
        &mut state,
    );

    // 10. Paint Pinned Left Headers & Transport Controls
    paint_pinned_left(
        &painter,
        ui,
        app,
        total_rect,
        &component_ids,
        &component_names,
        &component_colors,
        state.scroll_y,
        state.overscroll_y,
        floating,
        &t,
    );

    // 11. Paint Extended Rollback Marker
    let marker_col_x = grid_origin_x + marker_slot.min(num_columns) as f32 * COL_WIDTH - state.scroll_x;
    if marker_col_x >= grid_origin_x - 10.0 && marker_col_x <= total_rect.right() + 10.0 {
        let resp = ui.interact(
            Rect::from_min_size(pos2(marker_col_x - 8.0, total_rect.top() + 4.0), vec2(16.0, HEADER_T1_HEIGHT - 4.0)),
            ui.id().with("marker_head_2d"),
            Sense::drag(),
        );
        if resp.dragged()
            && let Some(pos) = pointer_pos
        {
            let target_slot = ((pos.x - grid_origin_x + state.scroll_x) / COL_WIDTH).round().max(0.0) as usize;
            let target_slot = target_slot.min(total_features);
            if target_slot != marker_slot {
                let params = if target_slot < total_features {
                    serde_json::json!({ "position": target_slot })
                } else {
                    serde_json::json!({})
                };
                let _ = app.run_heavy("timeline.roll_to", params);
            }
        }

        let pin_center = pos2(marker_col_x, total_rect.top() + HEADER_T1_HEIGHT / 2.0);
        let line_top = total_rect.top() + HEADER_T1_HEIGHT;
        let line_bottom = total_rect.bottom() - 2.0;

        // Vertical marker line through Tier 2 and component tracks
        painter.line_segment([pos2(marker_col_x, line_top), pos2(marker_col_x, line_bottom)], Stroke::new(2.0, accent.color));

        // Circular pin in Tier 1
        painter.circle(pin_center, 5.0, t.timeline, Stroke::new(2.0, accent.color));
        painter.circle_filled(pin_center, 1.5, accent.color);
    }

    if let Some(edit_id) = action_to_edit {
        app.edit_feature(edit_id);
    }

    // Persist state in egui memory
    ui.data_mut(|d| d.insert_temp(state_id, state));
}

/// Helper: constructs visual columns respecting folded group states.
fn build_visual_columns(doc: &Document, folded_groups: &BTreeSet<u64>) -> Vec<VisualColumn> {
    let mut columns = Vec::new();
    let mut i = 0;
    let n = doc.features.len();

    while i < n {
        let f = &doc.features[i];
        let grp = doc.timeline_groups.iter().find(|g| g.features.contains(&f.id));

        if let Some(g) = grp {
            if folded_groups.contains(&g.id) {
                // Collect contiguous features belonging to this group
                let mut grp_features = Vec::new();
                while i < n && g.features.contains(&doc.features[i].id) {
                    grp_features.push(doc.features[i].id);
                    i += 1;
                }
                columns.push(VisualColumn::FoldedGroup {
                    group_id: g.id,
                    features: grp_features,
                });
                continue;
            } else {
                columns.push(VisualColumn::Features {
                    step_number: i + 1,
                    features: vec![f.id],
                    open_group: Some(g.id),
                });
                i += 1;
                continue;
            }
        }

        columns.push(VisualColumn::Features {
            step_number: i + 1,
            features: vec![f.id],
            open_group: None,
        });
        i += 1;
    }
    columns
}

/// Helper: paints the Two-Tier Header track (Tier 1: Step numbers, Tier 2: Folders/Features).
fn paint_two_tier_header(
    p: &egui::Painter,
    ui: &mut Ui,
    app: &mut SolveApp,
    total_rect: Rect,
    grid_origin_x: f32,
    scroll_x: f32,
    columns: &[VisualColumn],
    marker_slot: usize,
    _accent: crate::theme::TabAccent,
    t: &Tokens,
    state: &mut Timeline2DState,
) {
    let header_rect = Rect::from_min_max(
        pos2(grid_origin_x, total_rect.top()),
        pos2(total_rect.right(), total_rect.top() + HEADER_TOTAL_HEIGHT),
    );
    let header_rounding = egui::CornerRadius { nw: 0, ne: t.radius_lg, sw: 0, se: 0 };
    p.rect_filled(header_rect, header_rounding, t.timeline);

    // Tier 1 / Tier 2 separator line
    p.line_segment(
        [pos2(grid_origin_x, total_rect.top() + HEADER_T1_HEIGHT), pos2(total_rect.right(), total_rect.top() + HEADER_T1_HEIGHT)],
        Stroke::new(1.0, t.border.gamma_multiply(0.4)),
    );
    // Header bottom border
    p.line_segment(
        [pos2(grid_origin_x, total_rect.top() + HEADER_TOTAL_HEIGHT), pos2(total_rect.right(), total_rect.top() + HEADER_TOTAL_HEIGHT)],
        Stroke::new(1.0, t.border),
    );

    let mono_font = FontId::monospace(10.0);

    for (col_idx, col) in columns.iter().enumerate() {
        let col_x = grid_origin_x + col_idx as f32 * COL_WIDTH - scroll_x;
        if col_x + COL_WIDTH < grid_origin_x || col_x > total_rect.right() {
            continue;
        }

        // Notch indicator
        let notch_x = col_x + COL_WIDTH / 2.0;
        p.line_segment(
            [pos2(notch_x, total_rect.top() + 3.0), pos2(notch_x, total_rect.top() + 7.0)],
            Stroke::new(1.0, t.border),
        );

        match col {
            VisualColumn::Features { step_number, .. } => {
                let is_active = *step_number <= marker_slot;
                let text_color = if is_active { t.text } else { t.text_dim };

                // Tier 1: Step number
                p.text(
                    pos2(notch_x, total_rect.top() + 14.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{step_number:02}"),
                    mono_font.clone(),
                    text_color,
                );
            }
            VisualColumn::FoldedGroup { .. } => {
                // Tier 1: Dash (-) in group column
                p.text(
                    pos2(notch_x, total_rect.top() + 14.0),
                    egui::Align2::CENTER_CENTER,
                    "-",
                    mono_font.clone(),
                    t.text_dim,
                );
            }
        }
    }

    // Tier 2: Open group banners spanning contiguous member columns
    let mut i = 0;
    while i < columns.len() {
        if let VisualColumn::Features { open_group: Some(grp_id), step_number, .. } = columns[i] {
            let start_col = i;
            let is_active = step_number <= marker_slot;
            while i + 1 < columns.len() {
                if let VisualColumn::Features { open_group: Some(next_gid), .. } = columns[i + 1]
                    && next_gid == grp_id
                {
                    i += 1;
                    continue;
                }
                break;
            }
            let end_col = i;
            let start_x = grid_origin_x + start_col as f32 * COL_WIDTH - scroll_x;
            let end_x = grid_origin_x + (end_col + 1) as f32 * COL_WIDTH - scroll_x;

            if end_x > grid_origin_x && start_x < total_rect.right() {
                let banner_rect = Rect::from_min_max(
                    pos2(start_x.max(grid_origin_x) + 3.0, total_rect.top() + HEADER_T1_HEIGHT + 2.0),
                    pos2(end_x.min(total_rect.right()) - 3.0, total_rect.top() + HEADER_TOTAL_HEIGHT - 2.0),
                );
                let resp = ui.interact(banner_rect, ui.id().with(("grp_close_pill", grp_id)), Sense::click());
                if resp.clicked() {
                    state.set_group_folded(grp_id, true);
                }
                let pill_color = if is_active { t.accent.gamma_multiply(0.2) } else { t.text_dim.gamma_multiply(0.2) };
                let stroke_color = if is_active { t.accent.gamma_multiply(0.45) } else { t.border };
                let text_color = if is_active { t.accent } else { t.text_dim };

                p.rect_filled(banner_rect, 4.0, pill_color);
                p.rect_stroke(banner_rect, 4.0, Stroke::new(1.0, stroke_color), StrokeKind::Inside);

                let grp_name = app.session.doc.timeline_groups.iter().find(|g| g.id == grp_id).map(|g| g.name.as_str()).unwrap_or("Group");
                let member_count = end_col - start_col + 1;
                let title_text = format!("📁 {} ({})", grp_name, member_count);

                let galley = p.layout_no_wrap(title_text, FontId::proportional(10.0), text_color);
                let title_w = galley.size().x;
                let left_pos = pos2(banner_rect.left() + 6.0, banner_rect.center().y);
                p.galley(pos2(left_pos.x, left_pos.y - galley.size().y / 2.0), galley, text_color);

                let available_right = banner_rect.width() - (title_w + 16.0);
                let right_pos = pos2(banner_rect.right() - 6.0, banner_rect.center().y);
                if available_right > 45.0 {
                    p.text(right_pos, egui::Align2::RIGHT_CENTER, "Close ▴", FontId::proportional(9.0), text_color);
                } else if available_right > 15.0 {
                    p.text(right_pos, egui::Align2::RIGHT_CENTER, "▴", FontId::proportional(9.0), text_color);
                }
            }
        }
        i += 1;
    }
}

/// Helper: paints the stationary pinned left header (Component tracks & Transport controls).
fn paint_pinned_left(
    p: &egui::Painter,
    ui: &mut Ui,
    app: &mut SolveApp,
    total_rect: Rect,
    component_ids: &[u64],
    component_names: &BTreeMap<u64, String>,
    component_colors: &BTreeMap<u64, Option<[u8; 3]>>,
    scroll_y: f32,
    overscroll_y: f32,
    floating: bool,
    t: &Tokens,
) {
    let pinned_rect = Rect::from_min_max(total_rect.left_top(), pos2(total_rect.left() + PINNED_LEFT_WIDTH, total_rect.bottom()));
    let pinned_rounding = egui::CornerRadius {
        nw: t.radius_lg,
        ne: 0,
        sw: if floating { t.radius_lg } else { 0 },
        se: 0,
    };
    p.rect_filled(pinned_rect, pinned_rounding, t.panel);

    // Vertical seam divider
    p.line_segment(
        [pos2(pinned_rect.right(), total_rect.top()), pos2(pinned_rect.right(), total_rect.bottom())],
        Stroke::new(1.0, t.border),
    );

    // Leading Transport Header (top 44px)
    let transport_header_rect = Rect::from_min_size(pinned_rect.left_top(), vec2(PINNED_LEFT_WIDTH, HEADER_TOTAL_HEIGHT));
    p.rect_filled(
        transport_header_rect,
        egui::CornerRadius { nw: t.radius_lg, ne: 0, sw: 0, se: 0 },
        t.timeline,
    );

    // Transport buttons centered and scaled across the header
    let controls = HistoryTransport::shown(app.preferences.history_play_button);
    let btn_size = 26.0;
    let gap = 4.0;
    let num_controls = controls.len();
    let total_width = num_controls as f32 * btn_size + num_controls.saturating_sub(1) as f32 * gap;
    let mut bx = pinned_rect.left() + (PINNED_LEFT_WIDTH - total_width) / 2.0;
    let by = pinned_rect.top() + (HEADER_TOTAL_HEIGHT - btn_size) / 2.0;

    for &control in controls {
        let b_rect = Rect::from_min_size(pos2(bx, by), vec2(btn_size, btn_size));
        let playing = app.timeline_playback.is_some();
        let resp = control.show(ui, b_rect, playing);
        crate::timeline::transport_response(app, control, &resp);
        bx += btn_size + gap;
    }

    // Component rows
    let rows_clip = Rect::from_min_max(pos2(pinned_rect.left(), total_rect.top() + HEADER_TOTAL_HEIGHT), pinned_rect.right_bottom());
    let row_p = p.with_clip_rect(rows_clip);

    for (r_idx, &cid) in component_ids.iter().enumerate() {
        let ry = total_rect.top() + HEADER_TOTAL_HEIGHT + r_idx as f32 * ROW_HEIGHT - scroll_y + overscroll_y;
        let row_rect = Rect::from_min_size(pos2(pinned_rect.left(), ry), vec2(PINNED_LEFT_WIDTH, ROW_HEIGHT));

        if row_rect.bottom() < rows_clip.top() || row_rect.top() > rows_clip.bottom() {
            continue;
        }

        // Row background and bottom line
        row_p.line_segment([row_rect.left_bottom(), row_rect.right_bottom()], Stroke::new(1.0, t.border.gamma_multiply(0.25)));

        // Color badge
        let color = component_colors
            .get(&cid)
            .and_then(|c| c.map(|[r, g, b]| egui::Rgba::from_srgba_unmultiplied(r, g, b, 255).into()))
            .unwrap_or(t.accent);

        let dot_pos = pos2(row_rect.left() + 14.0, row_rect.center().y);
        row_p.circle_filled(dot_pos, 4.0, color);

        // Component name
        let name = component_names.get(&cid).map(|s| s.as_str()).unwrap_or("Component");
        row_p.text(
            pos2(row_rect.left() + 24.0, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            FontId::proportional(11.0),
            t.text,
        );
    }
}

/// Helper: paints an individual action chip on the timeline grid.
fn paint_action_chip(
    p: &egui::Painter,
    rect: Rect,
    f: &Feature,
    is_active: bool,
    is_selected: bool,
    is_prereq: bool,
    is_dependent: bool,
    accent: crate::theme::TabAccent,
    t: &Tokens,
) {
    let stroke_color = if is_selected {
        if is_active { t.text } else { t.text_dim }
    } else if is_prereq {
        if is_active { t.warning } else { t.text_dim }
    } else if is_dependent {
        if is_active { t.accent } else { t.border }
    } else {
        if is_active { t.border } else { t.border.gamma_multiply(0.5) }
    };

    let bg_color = if is_selected {
        if is_active { accent.soft } else { t.panel }
    } else {
        if is_active { t.timeline_item } else { t.panel }
    };

    p.rect_filled(rect, 4.0, bg_color);
    p.rect_stroke(rect, 4.0, Stroke::new(if is_selected || is_prereq || is_dependent { 1.5 } else { 1.0 }, stroke_color), StrokeKind::Inside);

    // Master SVG icon from assets/icons/ui/design/svg
    let icon = crate::timeline::icon_of(&f.kind);
    let icon_rect = Rect::from_center_size(rect.center(), vec2(20.0, 20.0));
    if is_active {
        crate::icons::paint_accented(p, icon_rect, icon, crate::ui_kit::feature_icon_accent(f.kind.type_name(), icon), 1.0);
    } else {
        crate::icons::paint_ink(p, icon_rect, icon, t.text_dim);
    }
}

/// Helper: paints the virtual group chip spanning component tracks.
fn paint_virtual_group_chip(
    p: &egui::Painter,
    rect: Rect,
    group_name: &str,
    count: usize,
    is_active: bool,
    is_selected: bool,
    t: &Tokens,
) {
    let border_color = if is_selected {
        if is_active { t.text } else { t.text_dim }
    } else {
        if is_active { t.accent } else { t.border }
    };

    let bg_color = if is_active {
        t.timeline_item
    } else {
        t.panel
    };

    p.rect_filled(rect, 5.0, bg_color);
    p.rect_stroke(rect, 5.0, Stroke::new(if is_selected { 1.5 } else { 1.0 }, border_color), StrokeKind::Inside);

    // Integrated header cap (expansion chevron indicator)
    let header_cap_rect = Rect::from_min_size(rect.left_top(), vec2(rect.width(), HEADER_T2_HEIGHT));
    p.rect_filled(header_cap_rect, 4.0, if is_active { t.accent.gamma_multiply(0.25) } else { t.text_dim.gamma_multiply(0.2) });
    p.text(
        header_cap_rect.center(),
        egui::Align2::CENTER_CENTER,
        "▶",
        FontId::proportional(9.0),
        if is_active { t.accent } else { t.text_dim },
    );

    // Count badge
    let badge_rect = Rect::from_center_size(pos2(rect.center().x, rect.top() + 38.0), vec2(18.0, 14.0));
    p.rect_filled(badge_rect, 3.0, if is_active { t.accent.gamma_multiply(0.2) } else { t.text_dim.gamma_multiply(0.15) });
    p.text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        count.to_string(),
        FontId::monospace(9.0),
        if is_active { t.accent } else { t.text },
    );

    // Vertical 90-degree rotated title text
    let title_pos = pos2(rect.center().x, rect.top() + 60.0);
    let title_color = if is_active { t.text } else { t.text_dim };
    let mut shape = egui::epaint::TextShape::new(
        title_pos,
        p.layout_no_wrap(group_name.to_string(), FontId::proportional(11.0), title_color),
        title_color,
    );
    shape.angle = std::f32::consts::FRAC_PI_2;
    p.add(shape);
}

/// Helper: paints the movable corridor background tint and brackets [ ... ].
fn paint_movable_corridor(
    p: &egui::Painter,
    columns: &[VisualColumn],
    _dep_graph: &DependencyGraph,
    grid_origin_x: f32,
    scroll_x: f32,
    grid_top: f32,
    grid_bottom: f32,
    corr: MovableCorridor,
    marker_slot: usize,
    t: &Tokens,
) {
    let min_col = corr.min_slot.min(columns.len().saturating_sub(1));
    let max_col = corr.max_slot.min(columns.len().saturating_sub(1));

    let x_start = grid_origin_x + min_col as f32 * COL_WIDTH - scroll_x;
    let x_end = grid_origin_x + (max_col + 1) as f32 * COL_WIDTH - scroll_x;
    let marker_x = grid_origin_x + marker_slot.min(columns.len()) as f32 * COL_WIDTH - scroll_x;

    let split = corr.split_at_marker(marker_slot);

    match split {
        CorridorSplit::AllActive { .. } => {
            let rect = Rect::from_min_max(pos2(x_start, grid_top), pos2(x_end, grid_bottom));
            p.rect_filled(rect, 0.0, t.form_success.gamma_multiply(0.08));
            paint_bracket_open(p, x_start, grid_top, grid_bottom, t.form_success);
            paint_bracket_close(p, x_end, grid_top, grid_bottom, t.form_success);
        }
        CorridorSplit::AllInactive { .. } => {
            let rect = Rect::from_min_max(pos2(x_start, grid_top), pos2(x_end, grid_bottom));
            p.rect_filled(rect, 0.0, t.text_dim.gamma_multiply(0.08));
            paint_bracket_open(p, x_start, grid_top, grid_bottom, t.text_dim);
            paint_bracket_close(p, x_end, grid_top, grid_bottom, t.text_dim);
        }
        CorridorSplit::Spanning { .. } => {
            // Active segment (left of marker)
            let active_rect = Rect::from_min_max(pos2(x_start, grid_top), pos2(marker_x, grid_bottom));
            p.rect_filled(active_rect, 0.0, t.form_success.gamma_multiply(0.08));
            paint_bracket_open(p, x_start, grid_top, grid_bottom, t.form_success);

            // Inactive segment (right of marker)
            let inactive_rect = Rect::from_min_max(pos2(marker_x, grid_top), pos2(x_end, grid_bottom));
            p.rect_filled(inactive_rect, 0.0, t.text_dim.gamma_multiply(0.08));
            paint_bracket_close(p, x_end, grid_top, grid_bottom, t.text_dim);
        }
    }
}

fn paint_bracket_open(p: &egui::Painter, x: f32, top: f32, bottom: f32, color: egui::Color32) {
    let stroke = Stroke::new(2.5, color);
    let tick_len = 10.0;
    p.line_segment([pos2(x + tick_len, top), pos2(x, top)], stroke);
    p.line_segment([pos2(x, top), pos2(x, bottom)], stroke);
    p.line_segment([pos2(x, bottom), pos2(x + tick_len, bottom)], stroke);
}

fn paint_bracket_close(p: &egui::Painter, x: f32, top: f32, bottom: f32, color: egui::Color32) {
    let stroke = Stroke::new(2.5, color);
    let tick_len = 10.0;
    p.line_segment([pos2(x - tick_len, top), pos2(x, top)], stroke);
    p.line_segment([pos2(x, top), pos2(x, bottom)], stroke);
    p.line_segment([pos2(x, bottom), pos2(x - tick_len, bottom)], stroke);
}

fn paint_dotted_vertical_line(p: &egui::Painter, x: f32, y_start: f32, y_end: f32, color: egui::Color32) {
    let dot_spacing = 6.0;
    let mut y = y_start;
    while y <= y_end {
        p.circle_filled(pos2(x, y), 1.5, color);
        y += dot_spacing;
    }
}

//! 2D Grid Canvas, Two-Tier Header, Pinned Component Tracks, and Marker Scrubber.

use std::collections::{BTreeMap, BTreeSet};
use egui::{
    FontId, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use solidcraft_engine::doc::{Document, Feature, FeatureKind, Model};
use crate::SolveApp;
use crate::theme::Tokens;
use crate::ui_kit::{HistoryTransport, Menu};
use super::deps::DependencyGraph;
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

    let mut feature_components_map: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    {
        let model = Some(&app.session.model);
        for f in &app.session.doc.features {
            let affected = feature_affected_components(&app.session.doc, model, f);
            for &cid in &affected {
                if !component_ids.contains(&cid) {
                    let comp_opt = app.session.doc.components.iter().find(|c| c.id == cid);
                    let name = comp_opt.map(|c| c.name.clone()).unwrap_or_else(|| format!("Component {}", cid));
                    let color = comp_opt.and_then(|c| c.identification_color);
                    component_ids.push(cid);
                    component_names.insert(cid, name);
                    component_colors.insert(cid, color);
                }
            }
            feature_components_map.insert(f.id, affected);
        }
    }

    // 3. Build visual columns (respecting folded groups)
    let visual_columns = build_visual_columns(&app.session.doc, &state.folded_groups);
    let num_columns = visual_columns.len();

    // Map marker feature slot to its visual column boundary
    let marker_visual_col = if let Some(marker_idx) = app.session.doc.marker {
        if marker_idx < total_features {
            let f = &app.session.doc.features[marker_idx];
            feature_to_visual_col(&visual_columns, f.id).unwrap_or(num_columns)
        } else {
            num_columns
        }
    } else {
        num_columns
    };

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

    // 7. Calculate Active Selection and Movable Corridor in Visual Columns
    let selected_id = state.selected_features.iter().next().copied();
    let visual_corridor = compute_visual_corridor(&visual_columns, &dep_graph, &state.selected_features);

    // Draw movable corridor background and brackets if selected
    if let Some((min_col, max_col)) = visual_corridor {
        let tracks_height = num_tracks.max(1) as f32 * ROW_HEIGHT;
        let content_tracks_top = grid_rect.top() - state.scroll_y + state.overscroll_y;
        let content_tracks_bottom = content_tracks_top + tracks_height;

        let corridor_top = content_tracks_top.max(grid_rect.top());
        let corridor_bottom = content_tracks_bottom.min(grid_rect.bottom());

        let bracket_top = (content_tracks_top + 4.0).max(grid_rect.top() + 4.0);
        let bracket_bottom = (content_tracks_bottom - 4.0).min(grid_rect.bottom() - 4.0);

        paint_movable_corridor(
            &grid_painter,
            grid_origin_x,
            state.scroll_x,
            corridor_top,
            corridor_bottom,
            bracket_top,
            bracket_bottom,
            min_col,
            max_col,
            marker_visual_col,
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
            VisualColumn::Features { features, open_group, .. } => {
                // If open group, draw subtle corridor tint in Tier 2
                if let Some(_grp_id) = open_group {
                    let banner_rect = Rect::from_min_size(
                        pos2(col_x, total_rect.top() + HEADER_T1_HEIGHT),
                        vec2(COL_WIDTH, HEADER_T2_HEIGHT),
                    );
                    let tint = t.accent.gamma_multiply(0.06);
                    painter.rect_filled(banner_rect, 0.0, tint);
                }

                let is_active = col_idx < marker_visual_col;

                // Collect all chips to paint for this column: (fid, track_idx)
                let mut column_chips: Vec<(u64, usize)> = Vec::new();
                let mut all_track_indices: Vec<usize> = Vec::new();
                let mut any_selected = false;

                for &fid in features {
                    if let Some(f) = app.session.doc.feature(fid) {
                        let affected = feature_components_map.get(&fid).cloned().unwrap_or_else(|| vec![f.component]);
                        let mut tracks: Vec<usize> = affected
                            .iter()
                            .filter_map(|cid| component_ids.iter().position(|id| id == cid))
                            .collect();
                        if tracks.is_empty() {
                            let default_track = component_ids.iter().position(|&cid| cid == f.component).unwrap_or(0);
                            tracks.push(default_track);
                        }
                        tracks.sort_unstable();
                        tracks.dedup();

                        if state.is_selected(fid) {
                            any_selected = true;
                        }

                        for &t_idx in &tracks {
                            column_chips.push((fid, t_idx));
                            all_track_indices.push(t_idx);
                        }
                    }
                }

                all_track_indices.sort_unstable();
                all_track_indices.dedup();

                // If multi-component (or multiple actions across tracks), draw dead-centered vertical dotted connector
                if all_track_indices.len() > 1
                    && let (Some(&min_track), Some(&max_track)) = (all_track_indices.first(), all_track_indices.last())
                {
                    let y1 = total_rect.top() + HEADER_TOTAL_HEIGHT + min_track as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0 - state.scroll_y + state.overscroll_y;
                    let y2 = total_rect.top() + HEADER_TOTAL_HEIGHT + max_track as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0 - state.scroll_y + state.overscroll_y;

                    if y1 < y2 && y1 < grid_rect.bottom() && y2 > grid_rect.top() {
                        let cx = col_x + COL_WIDTH / 2.0;
                        let line_color = if any_selected {
                            if is_active { accent.color } else { t.text_dim }
                        } else {
                            if is_active { t.text_dim } else { t.border.gamma_multiply(0.7) }
                        };
                        let dot_radius = if any_selected { 1.5 } else { 1.25 };
                        paint_dotted_vertical_line(&grid_painter, cx, y1, y2, line_color, dot_radius);
                    }
                }

                // Paint individual chips
                for (fid, track_idx) in column_chips {
                    if let Some(f) = app.session.doc.feature(fid) {
                        let chip_top = total_rect.top() + HEADER_TOTAL_HEIGHT + track_idx as f32 * ROW_HEIGHT + (ROW_HEIGHT - CHIP_HEIGHT) / 2.0 - state.scroll_y + state.overscroll_y;
                        let chip_rect = Rect::from_min_size(pos2(col_x + (COL_WIDTH - CHIP_WIDTH) / 2.0, chip_top), vec2(CHIP_WIDTH, CHIP_HEIGHT));

                        if chip_rect.bottom() < grid_rect.top() || chip_rect.top() > grid_rect.bottom() {
                            continue;
                        }

                        let is_active = col_idx < marker_visual_col;
                        let is_selected = state.is_selected(fid);
                        let is_prereq = selected_id.is_some_and(|sel| dep_graph.is_prerequisite(sel, fid));
                        let is_dependent = selected_id.is_some_and(|sel| dep_graph.is_dependent(sel, fid));

                        let resp = ui.interact(chip_rect, ui.id().with(("chip_2d", fid, track_idx)), Sense::click_and_drag());
                        if resp.hovered() {
                            state.hovered_feature = Some(fid);
                        }

                        // Attach rich hover tooltip showing action name, kind, status
                        let res = app.session.model.result(fid);
                        let err = res.and_then(|r| r.error.as_deref());
                        let warn = res.and_then(|r| r.warning.as_deref());
                        let mut tip = crate::timeline::status_text(&f.name, f.kind.type_name(), err, warn);
                        if !is_active {
                            let steps_after = col_idx.saturating_sub(marker_visual_col) + 1;
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
            VisualColumn::FoldedGroup { .. } => {
                // Handled in Step 9b after header background so the integrated header cap is visible on Tier 2
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
        marker_visual_col,
        accent,
        &t,
        &mut state,
    );

    // 9b. Paint Folded Group Virtual Chips (starts on Tier 2 of Header, spans component tracks)
    for (col_idx, col) in visual_columns.iter().enumerate() {
        let col_x = grid_origin_x + col_idx as f32 * COL_WIDTH - state.scroll_x;
        if col_x + COL_WIDTH < grid_origin_x || col_x > total_rect.right() {
            continue;
        }

        if let VisualColumn::FoldedGroup { group_id, features } = col {
            let card_top = total_rect.top() + HEADER_T1_HEIGHT + 2.0;
            let card_bottom = ((total_rect.top() + HEADER_TOTAL_HEIGHT + component_ids.len().max(1) as f32 * ROW_HEIGHT - 6.0 - state.scroll_y + state.overscroll_y).min(total_rect.bottom() - 6.0)).max(card_top + HEADER_T2_HEIGHT + 24.0);
            let card_height = card_bottom - card_top;
            let card_rect = Rect::from_min_size(
                pos2(col_x + (COL_WIDTH - CHIP_WIDTH) / 2.0, card_top),
                vec2(CHIP_WIDTH, card_height),
            );

            let is_active = col_idx < marker_visual_col;
            let is_group_selected = features.iter().any(|id| state.is_selected(*id));

            let grp_name = app
                .session
                .doc
                .timeline_groups
                .iter()
                .find(|g| g.id == *group_id)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| format!("Group {group_id}"));

            let header_cap_rect = Rect::from_min_size(card_rect.left_top(), vec2(card_rect.width(), HEADER_T2_HEIGHT - 2.0));

            let resp = ui.interact(card_rect, ui.id().with(("grp_card", *group_id)), Sense::click_and_drag());
            if resp.hovered() {
                state.hovered_group = Some(*group_id);
                if let Some(pos) = pointer_pos && header_cap_rect.contains(pos) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }

            if resp.clicked() {
                if let Some(pos) = pointer_pos && header_cap_rect.contains(pos) {
                    state.set_group_folded(*group_id, false);
                } else {
                    state.selected_features.clear();
                    for &fid in features {
                        state.selected_features.insert(fid);
                    }
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
            let g_name = grp_name.clone();
            Menu::context(&resp, |ui| {
                if crate::context_menu::button(ui, crate::i18n::tr("ui.timeline_2d_open_group"), "folder").clicked() {
                    state.set_group_folded(gid, false);
                    ui.close();
                }
                if crate::context_menu::button(ui, crate::i18n::tr("ui.timeline_2d_rename_group"), "rename").clicked() {
                    app.dialog = Some(crate::dialogs::Dialog::rename_group(gid, &g_name));
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
    let marker_col_x = grid_origin_x + marker_visual_col.min(num_columns) as f32 * COL_WIDTH - state.scroll_x;
    if marker_col_x >= grid_origin_x - 10.0 && marker_col_x <= total_rect.right() + 10.0 {
        let resp = ui.interact(
            Rect::from_min_size(pos2(marker_col_x - 8.0, total_rect.top() + 4.0), vec2(16.0, HEADER_T1_HEIGHT - 4.0)),
            ui.id().with("marker_head_2d"),
            Sense::drag(),
        );
        if resp.dragged()
            && let Some(pos) = pointer_pos
        {
            let target_visual_col = ((pos.x - grid_origin_x + state.scroll_x) / COL_WIDTH).round().max(0.0) as usize;
            let target_visual_col = target_visual_col.min(num_columns);
            if target_visual_col != marker_visual_col {
                let target_slot = if target_visual_col < num_columns {
                    visual_columns[target_visual_col]
                        .feature_ids()
                        .first()
                        .and_then(|&fid| dep_graph.id_to_index.get(&fid).copied())
                        .unwrap_or(total_features)
                } else {
                    total_features
                };
                if target_slot != marker_slot {
                    let params = if target_slot < total_features {
                        serde_json::json!({ "position": target_slot })
                    } else {
                        serde_json::json!({})
                    };
                    let _ = app.run_heavy("timeline.roll_to", params);
                }
            }
        }

        let pin_center = pos2(marker_col_x, total_rect.top() + HEADER_T1_HEIGHT / 2.0);
        let line_top = total_rect.top() + HEADER_T1_HEIGHT;
        let line_bottom = total_rect.bottom() - 2.0;

        // Vertical marker line through Tier 2 and component tracks
        painter.line_segment([pos2(marker_col_x, line_top), pos2(marker_col_x, line_bottom)], Stroke::new(2.5, accent.color));

        // Circular pin in Tier 1
        painter.circle(pin_center, 5.0, t.timeline, Stroke::new(2.5, accent.color));
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
    marker_visual_col: usize,
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
                let is_active = col_idx < marker_visual_col;
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
                let is_active = col_idx < marker_visual_col;
                // Tier 1: Dash (-) in group column
                p.text(
                    pos2(notch_x, total_rect.top() + 14.0),
                    egui::Align2::CENTER_CENTER,
                    "-",
                    mono_font.clone(),
                    if is_active { t.text } else { t.text_dim },
                );
            }
        }
    }

    // Tier 2: Open group banners spanning contiguous member columns
    let mut i = 0;
    while i < columns.len() {
        if let VisualColumn::Features { open_group: Some(grp_id), .. } = columns[i] {
            let start_col = i;
            let is_active = start_col < marker_visual_col;
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
                let grp_name = app
                    .session
                    .doc
                    .timeline_groups
                    .iter()
                    .find(|g| g.id == grp_id)
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| "Group".to_string());
                let g_feats = app
                    .session
                    .doc
                    .timeline_groups
                    .iter()
                    .find(|g| g.id == grp_id)
                    .map(|g| g.features.clone())
                    .unwrap_or_default();

                let resp = resp.on_hover_text(format!("Click to fold {}", grp_name));

                let gid = grp_id;
                let g_name = grp_name.clone();
                Menu::context(&resp, |ui| {
                    if crate::context_menu::button(ui, crate::i18n::tr("ui.timeline_2d_close_group"), "folder").clicked() {
                        state.set_group_folded(gid, true);
                        ui.close();
                    }
                    if crate::context_menu::button(ui, crate::i18n::tr("ui.timeline_2d_rename_group"), "rename").clicked() {
                        app.dialog = Some(crate::dialogs::Dialog::rename_group(gid, &g_name));
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
                let pill_color = if is_active { t.accent.gamma_multiply(0.2) } else { t.text_dim.gamma_multiply(0.2) };
                let stroke_color = if is_active { t.accent.gamma_multiply(0.45) } else { t.border };
                let text_color = if is_active { t.accent } else { t.text_dim };

                p.rect_filled(banner_rect, 4.0, pill_color);
                p.rect_stroke(banner_rect, 4.0, Stroke::new(1.0, stroke_color), StrokeKind::Inside);

                let member_count = end_col - start_col + 1;

                // Folder icon
                let icon_rect = Rect::from_center_size(pos2(banner_rect.left() + 10.0, banner_rect.center().y), vec2(11.0, 11.0));
                if is_active {
                    crate::icons::paint_icon(p, icon_rect, "folder", 1.0);
                } else {
                    crate::icons::paint_ink(p, icon_rect, "folder", t.text_dim);
                }

                let title_text = format!("{} ({})", grp_name, member_count);
                let galley = p.layout_no_wrap(title_text, FontId::proportional(10.0), text_color);
                let title_w = galley.size().x;
                let title_pos = pos2(banner_rect.left() + 18.0, banner_rect.center().y - galley.size().y / 2.0);
                p.galley(title_pos, galley, text_color);

                let available_right = banner_rect.width() - (title_w + 24.0);
                let right_pos = pos2(banner_rect.right() - 6.0, banner_rect.center().y);
                if available_right > 50.0 {
                    p.text(right_pos, egui::Align2::RIGHT_CENTER, "Close ◀", FontId::proportional(9.0), text_color);
                } else if available_right > 16.0 {
                    p.text(right_pos, egui::Align2::RIGHT_CENTER, "◀", FontId::proportional(9.0), text_color);
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

    // Integrated header cap (folder icon + expansion chevron indicator)
    let header_cap_rect = Rect::from_min_size(rect.left_top(), vec2(rect.width(), HEADER_T2_HEIGHT - 2.0));
    let cap_bg = if is_active {
        t.accent.gamma_multiply(0.25)
    } else {
        t.text_dim.gamma_multiply(0.2)
    };
    let cap_stroke = if is_active {
        t.accent.gamma_multiply(0.5)
    } else {
        t.border
    };
    let cap_rounding = egui::CornerRadius { nw: 4, ne: 4, sw: 2, se: 2 };
    p.rect_filled(header_cap_rect, cap_rounding, cap_bg);
    p.rect_stroke(header_cap_rect, cap_rounding, Stroke::new(1.0, cap_stroke), StrokeKind::Inside);

    // Folder icon
    let folder_rect = Rect::from_center_size(pos2(header_cap_rect.left() + 11.0, header_cap_rect.center().y), vec2(13.0, 13.0));
    if is_active {
        crate::icons::paint_icon(p, folder_rect, "folder", 1.0);
    } else {
        crate::icons::paint_ink(p, folder_rect, "folder", t.text_dim);
    }

    // Expansion chevron indicator ▶
    p.text(
        pos2(header_cap_rect.right() - 8.0, header_cap_rect.center().y),
        egui::Align2::CENTER_CENTER,
        "▶",
        FontId::proportional(8.0),
        if is_active { t.accent } else { t.text_dim },
    );

    // Count badge
    let badge_rect = Rect::from_center_size(pos2(rect.center().x, rect.top() + HEADER_T2_HEIGHT + 14.0), vec2(18.0, 14.0));
    p.rect_filled(badge_rect, 3.0, if is_active { t.accent.gamma_multiply(0.2) } else { t.text_dim.gamma_multiply(0.15) });
    p.text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        count.to_string(),
        FontId::monospace(9.0),
        if is_active { t.accent } else { t.text },
    );

    // Vertical 90-degree rotated title text
    let title_pos = pos2(rect.center().x, rect.top() + HEADER_T2_HEIGHT + 30.0);
    let title_color = if is_active { t.text } else { t.text_dim };
    let mut shape = egui::epaint::TextShape::new(
        title_pos,
        p.layout_no_wrap(group_name.to_string(), FontId::proportional(11.0), title_color),
        title_color,
    );
    shape.angle = std::f32::consts::FRAC_PI_2;
    p.add(shape);
}

/// Helper: maps a feature ID to its visual column index on the 2D timeline grid.
fn feature_to_visual_col(columns: &[VisualColumn], fid: u64) -> Option<usize> {
    for (idx, col) in columns.iter().enumerate() {
        if col.feature_ids().contains(&fid) {
            return Some(idx);
        }
    }
    None
}

/// Helper: computes the visual corridor `(min_col, max_col)` in visual column indices
/// for the currently selected feature(s), taking into account folded groups.
fn compute_visual_corridor(
    columns: &[VisualColumn],
    dep_graph: &DependencyGraph,
    selected_features: &BTreeSet<u64>,
) -> Option<(usize, usize)> {
    if selected_features.is_empty() {
        return None;
    }

    let selected_vec: Vec<u64> = selected_features.iter().copied().collect();
    let min_curr = selected_vec
        .iter()
        .filter_map(|&id| feature_to_visual_col(columns, id))
        .min()?;
    let max_curr = selected_vec
        .iter()
        .filter_map(|&id| feature_to_visual_col(columns, id))
        .max()?;

    let max_prereq_col = selected_vec
        .iter()
        .filter_map(|id| dep_graph.all_prereqs.get(id))
        .flatten()
        .filter(|p| !selected_features.contains(p))
        .filter_map(|&p| feature_to_visual_col(columns, p))
        .max();

    let min_dependent_col = selected_vec
        .iter()
        .filter_map(|id| dep_graph.all_dependents.get(id))
        .flatten()
        .filter(|d| !selected_features.contains(d))
        .filter_map(|&d| feature_to_visual_col(columns, d))
        .min();

    let min_col = max_prereq_col.map(|c| c + 1).unwrap_or(0).min(min_curr);
    let max_col = min_dependent_col
        .map(|c| c.saturating_sub(1))
        .unwrap_or(columns.len().saturating_sub(1))
        .max(max_curr);

    Some((min_col, max_col))
}

/// Helper: paints the movable corridor background tint and brackets [ ... ].
fn paint_movable_corridor(
    p: &egui::Painter,
    grid_origin_x: f32,
    scroll_x: f32,
    corridor_top: f32,
    corridor_bottom: f32,
    bracket_top: f32,
    bracket_bottom: f32,
    min_col: usize,
    max_col: usize,
    marker_visual_col: usize,
    t: &Tokens,
) {
    if corridor_bottom <= corridor_top || bracket_bottom <= bracket_top {
        return;
    }

    let x_start = grid_origin_x + min_col as f32 * COL_WIDTH - scroll_x;
    let x_end = grid_origin_x + (max_col + 1) as f32 * COL_WIDTH - scroll_x;
    let marker_x = grid_origin_x + marker_visual_col as f32 * COL_WIDTH - scroll_x;

    if max_col < marker_visual_col {
        let rect = Rect::from_min_max(pos2(x_start, corridor_top), pos2(x_end, corridor_bottom));
        p.rect_filled(rect, 0.0, t.form_success.gamma_multiply(0.08));
        if x_start >= grid_origin_x - 1.0 {
            paint_bracket_open(p, x_start, grid_origin_x, bracket_top, bracket_bottom, t.form_success, Some(marker_x));
        }
        if x_end >= grid_origin_x {
            paint_bracket_close(p, x_end, bracket_top, bracket_bottom, t.form_success, Some(marker_x));
        }
    } else if min_col >= marker_visual_col {
        let rect = Rect::from_min_max(pos2(x_start, corridor_top), pos2(x_end, corridor_bottom));
        p.rect_filled(rect, 0.0, t.text_dim.gamma_multiply(0.08));
        if x_start >= grid_origin_x - 1.0 {
            paint_bracket_open(p, x_start, grid_origin_x, bracket_top, bracket_bottom, t.text_dim, Some(marker_x));
        }
        if x_end >= grid_origin_x {
            paint_bracket_close(p, x_end, bracket_top, bracket_bottom, t.text_dim, Some(marker_x));
        }
    } else {
        // Active segment (left of marker)
        let active_rect = Rect::from_min_max(pos2(x_start, corridor_top), pos2(marker_x, corridor_bottom));
        p.rect_filled(active_rect, 0.0, t.form_success.gamma_multiply(0.08));

        // Inactive segment (right of marker)
        let inactive_rect = Rect::from_min_max(pos2(marker_x, corridor_top), pos2(x_end, corridor_bottom));
        p.rect_filled(inactive_rect, 0.0, t.text_dim.gamma_multiply(0.08));

        if x_start >= grid_origin_x - 1.0 {
            paint_bracket_open(p, x_start, grid_origin_x, bracket_top, bracket_bottom, t.form_success, Some(marker_x));
        }
        if x_end >= grid_origin_x {
            paint_bracket_close(p, x_end, bracket_top, bracket_bottom, t.text_dim, Some(marker_x));
        }
    }
}

fn paint_bracket_open(
    p: &egui::Painter,
    x: f32,
    grid_origin_x: f32,
    top: f32,
    bottom: f32,
    color: egui::Color32,
    marker_x: Option<f32>,
) {
    let stroke = Stroke::new(2.0, color);

    // Inset from the pinned component layer's border seam if at the boundary
    let (bx, tick_len) = if (x - grid_origin_x).abs() <= 1.0 {
        (grid_origin_x + 1.5, 6.0)
    } else {
        (x, 7.0)
    };

    let covered_by_marker = marker_x.is_some_and(|mx| (bx - mx).abs() <= 1.5);

    p.line_segment([pos2(bx + tick_len, top), pos2(bx, top)], stroke);
    if !covered_by_marker {
        p.line_segment(
            [pos2(bx, top - stroke.width / 2.0), pos2(bx, bottom + stroke.width / 2.0)],
            stroke,
        );
    }
    p.line_segment([pos2(bx, bottom), pos2(bx + tick_len, bottom)], stroke);
}

fn paint_bracket_close(
    p: &egui::Painter,
    x: f32,
    top: f32,
    bottom: f32,
    color: egui::Color32,
    marker_x: Option<f32>,
) {
    let stroke = Stroke::new(2.0, color);
    let tick_len = 7.0;

    let covered_by_marker = marker_x.is_some_and(|mx| (x - mx).abs() <= 1.5);

    p.line_segment([pos2(x - tick_len, top), pos2(x, top)], stroke);
    if !covered_by_marker {
        p.line_segment(
            [pos2(x, top - stroke.width / 2.0), pos2(x, bottom + stroke.width / 2.0)],
            stroke,
        );
    }
    p.line_segment([pos2(x, bottom), pos2(x - tick_len, bottom)], stroke);
}

fn paint_dotted_vertical_line(p: &egui::Painter, x: f32, y_start: f32, y_end: f32, color: egui::Color32, radius: f32) {
    let dot_spacing = 6.0;
    let mut y = y_start;
    while y <= y_end {
        p.circle_filled(pos2(x, y), radius, color);
        y += dot_spacing;
    }
}

/// Returns the sorted, deduplicated list of component IDs affected by a feature.
pub fn feature_affected_components(
    doc: &Document,
    model: Option<&Model>,
    f: &Feature,
) -> Vec<u64> {
    let mut comps = BTreeSet::new();
    comps.insert(f.component);

    // 1. If evaluation results exist in model, compare state before and after this feature
    if let Some(m) = model
        && let Some(res) = m.result(f.id)
    {
        let before = m.state_before(f.id);
        let after = &res.state;

        for b_before in &before.bodies {
            if let Some(b_after) = after.body(&b_before.name) {
                if b_before.geometry_key() != b_after.geometry_key() || b_before.component != b_after.component {
                    comps.insert(doc.model_body_component(b_before));
                    comps.insert(doc.model_body_component(b_after));
                }
            } else {
                comps.insert(doc.model_body_component(b_before));
            }
        }

        for b_after in &after.bodies {
            if b_after.feature == f.id || before.body(&b_after.name).is_none() {
                comps.insert(doc.model_body_component(b_after));
            }
        }
    }

    // 2. Edge name references (e.g. F1:top|F4:side in fillets and chamfers)
    for name in &f.edge_names {
        for part in name.split(|c: char| !c.is_ascii_alphanumeric()) {
            if let Some(id_str) = part.strip_prefix('F')
                && let Ok(fid) = id_str.parse::<u64>()
                && let Some(feat) = doc.feature(fid)
            {
                comps.insert(feat.component);
            }
        }
    }

    let model_state = model.map(|m| m.state());
    let model_bodies = model_state.as_deref().map(|s| s.bodies.as_slice());

    let resolve_body = |b: &str| -> Option<u64> {
        let trimmed = b.trim();
        if trimmed.is_empty() || trimmed == "*" {
            return None;
        }
        if let Some(bodies) = model_bodies
            && let Some(mb) = bodies.iter().find(|body| body.name.trim() == trimmed)
        {
            return Some(doc.model_body_component(mb));
        }
        if let Some(&cid) = doc.body_components.get(trimmed) {
            return Some(cid);
        }
        for prev in &doc.features {
            if prev.body_names.iter().any(|bn| bn.trim() == trimmed) || prev.name.trim() == trimmed {
                return Some(prev.component);
            }
        }
        Some(doc.body_component(trimmed, f.id))
    };

    for p in &f.participants {
        if let Some(cid) = resolve_body(p) {
            comps.insert(cid);
        }
    }

    match &f.kind {
        FeatureKind::Combine { target, tools, .. } => {
            if let Some(cid) = resolve_body(target.as_ref()) {
                comps.insert(cid);
            }
            for tool in tools {
                if let Some(cid) = resolve_body(tool.as_ref()) {
                    comps.insert(cid);
                }
            }
        }
        FeatureKind::BodyTransfer { from, to, bodies, .. } => {
            comps.insert(*from);
            comps.insert(*to);
            for b in bodies {
                if let Some(cid) = resolve_body(b.as_ref()) {
                    comps.insert(cid);
                }
            }
        }
        FeatureKind::Extrude { targets, .. }
        | FeatureKind::Revolve { targets, .. }
        | FeatureKind::Loft { targets, .. }
        | FeatureKind::Sweep { targets, .. }
        | FeatureKind::Emboss { targets, .. } => {
            for t in targets {
                if let Some(cid) = resolve_body(t.as_ref()) {
                    comps.insert(cid);
                }
            }
        }
        FeatureKind::Fillet { body, edges, .. }
        | FeatureKind::Chamfer { body, edges, .. } => {
            if let Some(b) = body
                && let Some(cid) = resolve_body(b.as_ref())
            {
                comps.insert(cid);
            }
            if let Some(bodies) = model_bodies {
                for p in edges {
                    for b in bodies {
                        let mesh = b.mesh();
                        let is_near = mesh.edges.iter().any(|e| {
                            e.windows(2).any(|w| p.0.dist_to_segment(w[0], w[1]) < 0.05)
                        });
                        if is_near {
                            comps.insert(doc.model_body_component(b));
                        }
                    }
                }
            }
        }
        FeatureKind::Split { body: b, .. } => {
            if let Some(cid) = resolve_body(b.as_ref()) {
                comps.insert(cid);
            }
        }
        FeatureKind::Move { bodies, .. }
        | FeatureKind::Scale { bodies, .. }
        | FeatureKind::Align { bodies, .. }
        | FeatureKind::Remove { bodies, .. } => {
            for b in bodies {
                if let Some(cid) = resolve_body(b.as_ref()) {
                    comps.insert(cid);
                }
            }
        }
        FeatureKind::BoundaryFill { tools, .. } => {
            for t in tools {
                if let Some(cid) = resolve_body(t.as_ref()) {
                    comps.insert(cid);
                }
            }
        }
        FeatureKind::Pattern { bodies, features, .. } => {
            for b in bodies {
                if let Some(cid) = resolve_body(b.as_ref()) {
                    comps.insert(cid);
                }
            }
            for &fid in features {
                if let Some(feat) = doc.feature(fid.0) {
                    comps.insert(feat.component);
                }
            }
        }
        FeatureKind::Mirror { bodies, features, placements, .. } => {
            for b in bodies {
                if let Some(cid) = resolve_body(b.as_ref()) {
                    comps.insert(cid);
                }
            }
            for &fid in features {
                if let Some(feat) = doc.feature(fid.0) {
                    comps.insert(feat.component);
                }
            }
            for path in placements {
                if let Some(&cid) = path.last() {
                    comps.insert(cid);
                }
            }
        }
        _ => {}
    }

    comps.into_iter().collect()
}

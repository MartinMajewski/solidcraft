//! Interactive state, scrolling, multi-selection, and drag models for the 2D History Browser.

use egui::Pos2;
use std::collections::BTreeSet;

/// Dragging state for interactive timeline gestures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragTarget {
    /// Scrubbing the rollback marker head pin across timeline columns.
    Marker,
    /// Dragging a feature action chip horizontally for ripple push reordering.
    Feature {
        id: u64,
        start_col: usize,
        track_idx: usize,
        current_pos: Pos2,
    },
    /// Dragging a virtual group chip.
    Group { id: u64, start_slot: usize },
}

/// Persistent interaction and layout state for the 2D History Browser.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline2DState {
    /// Horizontal scroll offset in pixels.
    pub scroll_x: f32,
    /// Vertical scroll offset in pixels.
    pub scroll_y: f32,
    /// Elastic vertical overscroll displacement for rubberband spring physics.
    pub overscroll_y: f32,
    /// IDs of timeline groups that are currently collapsed/folded.
    pub folded_groups: BTreeSet<u64>,
    /// Set of selected feature IDs.
    pub selected_features: BTreeSet<u64>,
    /// Anchor feature ID for Shift+Click range selections.
    pub selection_anchor: Option<u64>,
    /// Active drag gesture, if any.
    pub drag: Option<DragTarget>,
    /// Last hovered feature ID.
    pub hovered_feature: Option<u64>,
    /// Last hovered group ID.
    pub hovered_group: Option<u64>,
}

impl Timeline2DState {
    /// Checks if a group is folded (closed).
    pub fn is_group_folded(&self, group_id: u64) -> bool {
        self.folded_groups.contains(&group_id)
    }

    /// Toggles the folded state of a group.
    pub fn toggle_group_fold(&mut self, group_id: u64) {
        if self.folded_groups.contains(&group_id) {
            self.folded_groups.remove(&group_id);
        } else {
            self.folded_groups.insert(group_id);
        }
    }

    /// Sets the folded state of a group.
    pub fn set_group_folded(&mut self, group_id: u64, folded: bool) {
        if folded {
            self.folded_groups.insert(group_id);
        } else {
            self.folded_groups.remove(&group_id);
        }
    }

    /// Returns true if `id` is currently selected.
    pub fn is_selected(&self, id: u64) -> bool {
        self.selected_features.contains(&id)
    }

    /// Clears selection and selects a single feature.
    pub fn select_single(&mut self, id: u64) {
        self.selected_features.clear();
        self.selected_features.insert(id);
        self.selection_anchor = Some(id);
    }

    /// Toggles selection of a single feature (Cmd/Ctrl+Click).
    pub fn toggle_selection(&mut self, id: u64) {
        if self.selected_features.contains(&id) {
            self.selected_features.remove(&id);
        } else {
            self.selected_features.insert(id);
        }
        self.selection_anchor = Some(id);
    }

    /// Extends selection across a range in timeline order (Shift+Click).
    pub fn select_range(&mut self, timeline_order: &[u64], to_id: u64, add: bool) {
        let anchor = self.selection_anchor.unwrap_or(to_id);
        let from_idx = timeline_order.iter().position(|&f| f == anchor).unwrap_or(0);
        let to_idx = timeline_order.iter().position(|&f| f == to_id).unwrap_or(0);
        let min_idx = from_idx.min(to_idx);
        let max_idx = from_idx.max(to_idx);

        if !add {
            self.selected_features.clear();
        }
        for &id in &timeline_order[min_idx..=max_idx] {
            self.selected_features.insert(id);
        }
    }

    /// Checks if the current selection set is contiguous in timeline order.
    pub fn is_selection_adjacent(&self, timeline_order: &[u64]) -> bool {
        if self.selected_features.len() < 2 {
            return false;
        }
        let mut indices: Vec<usize> = self
            .selected_features
            .iter()
            .filter_map(|id| timeline_order.iter().position(|f| f == id))
            .collect();
        indices.sort_unstable();

        for w in indices.windows(2) {
            if w[1] != w[0] + 1 {
                return false;
            }
        }
        true
    }

    /// Formats a rich hover tooltip for a virtual group chip.
    pub fn format_group_tooltip(
        group_name: &str,
        features: &[(usize, &str, &str)], // (step_1_based, name, component_name)
        is_active: bool,
    ) -> String {
        let status = if is_active {
            "Active (Evaluated as solid bodies)"
        } else {
            "Inactive (Rolled back, 100% opacity greyscale)"
        };

        let count = features.len();
        let step_range = if count == 0 {
            "No steps".to_string()
        } else if count == 1 {
            format!("Step {:02}", features[0].0)
        } else {
            format!("Steps {:02} – {:02}", features[0].0, features[count - 1].0)
        };

        let mut tip = format!(
            "{group_name} ({count} Actions)\n{step_range} • {status}\n\nIncluded Features:\n"
        );

        for (step, name, comp) in features {
            tip.push_str(&format!("  • [{step:02}] {name} ({comp})\n"));
        }

        tip.push_str("\nDouble-click to open/close\nRight-click for group options");
        tip
    }
}


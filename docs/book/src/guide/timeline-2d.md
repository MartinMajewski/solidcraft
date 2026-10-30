# 2D History Browser

SolidCraft's **2D History Browser** is an advanced two-dimensional, multi-track parametric
timeline. In complex multi-body assemblies, a traditional linear timeline obscures which component
owns each feature and hides the relationships behind cross-component boolean operations. The 2D
History Browser organizes parametric history along two axes: **horizontal** for chronological
rebuild sequence, and **vertical** for component ownership.

![The SolidCraft window with the 2D History Browser enabled](../generated/timeline-2d-overview.png)

## Enabling the 2D History Browser

The 2D History Browser can be activated in two ways:

1. **User Interface**: Open **Preferences › Interface** and check **2D History Browser**
   (`prefs.timeline_2d`).
2. **Control Protocol / Agent Tools**: Send the preference change through the control channel:
   ```json
   {"cmd": "ui.prefs", "params": {"timeline_2d": true}}
   ```

When enabled, the bottom timeline dock transforms into the multi-track canvas. A grab rail along the
top edge of the dock allows vertical resizing between a compact single-track strip and an expanded
multi-track matrix.

## Multi-Track Component Matrix

In parametric assemblies, bodies and operations belong to specific components within the design
hierarchy. The 2D History Browser represents this relationship directly:

- **Rows (Tracks)**: The top track represents the root design (`Root (Design)`), followed by each
  active or inactive component (`Housing`, `Shaft`, `Bearing`, etc.) listed in hierarchy order.
  Each track header features a colored indicator dot matching its component tree appearance.
- **Columns (Global Steps)**: Features are arranged from left to right in absolute chronological
  creation order. Unused track cells display a subtle placeholder dot (`·`), making step alignment
  immediately evident across all components.
- **Multi-Component Actions**: Operations that bridge multiple bodies or components (such as a
  `solid.combine` boolean union, cross-part cut, or assembly joint) render interconnected chips
  across the affected tracks, joined by a vertical dotted connector.

![Topological corridor selection, multi-track connector, and dependency highlighting](../generated/timeline-2d-corridor-selection.png)

## Two-Tier Header

The timeline header is structured into two distinct tiers:

1. **Tier 1 (Scrubber & Steps)**:
   - Displays global step numbers (`01`, `02`, `03`, ...), tick marks, and status markers.
   - Hosts the interactive **Rollback Pin** (a circular scrubber head). Dragging the pin or
     clicking any step column immediately rolls the model's history back to that point in time.
2. **Tier 2 (Groups & Movable Corridor)**:
   - Displays visual brackets and labels for active **Feature Groups**.
   - Dynamically displays the **Movable Corridor** bounds (`[ ... ]`) when an action is selected,
     showing the safe range for reordering.

## Topological Dependencies & Safe Reordering

Parametric modelling relies on a strict directed acyclic graph (DAG) of dependencies: a feature
cannot exist before the sketch it is extruded from, nor can a fillet precede the solid body it
rounds. The 2D History Browser surfaces these relationships visually whenever an action chip is
selected:

- **Active Selection**: Selected feature chips receive a crisp white boundary.
- **Prerequisites (Ancestors)**: All preceding steps that the selected action directly or
  transitively depends upon are outlined in amber/orange (`#f59e0b`). The action cannot be moved
  before the highest prerequisite.
- **Downstream Dependents (Descendants)**: All succeeding steps that depend on the selected action
  are outlined in electric blue (`#38bdf8`). The action cannot be moved after the lowest dependent.
- **Movable Corridor (`[ ... ]`)**: Vibrant green boundary brackets appear on the Tier 2 header and
  across the column grid. This corridor marks every step slot where the selected feature can be
  safely moved without breaking the model.
- **Ripple Push Reordering**: Dragging an action chip horizontally within its corridor shifts
  intermediate features in the opposite direction, automatically executing a valid timeline reorder.
  Vertical dragging across component tracks is restricted to prevent unintended component ownership
  reassignments.

## Feature Grouping & Folding

Features can be organized into named groups to keep extensive timelines clean and manageable:

1. **Multi-Selection**:
   - `Shift + Click`: Selects a contiguous range of features.
   - `Cmd + Click` (macOS) or `Ctrl + Click` (Windows/Linux): Adds or removes individual features
     to/from the selection set.
2. **Group Creation**: Right-click any contiguous multi-selection and choose **Group Selection**.
3. **Folding & Expansion**:
   - In its expanded state, the group spans its member steps under a labeled bracket in the Tier 2
     header.
   - Clicking the fold icon or double-clicking the group header folds the features into a unified
     **Virtual Group Chip**.
   - The virtual group chip features an integrated header cap, a member count badge (e.g. `4`), an
     expansion chevron, and an encompassing background card that spans across all affected tracks.
4. **Group Operations**: Right-clicking a group chip provides actions to **Open/Close Group**,
   **Rename Group**, **Delete Group (Keep Features)**, or **Delete Group and Contents**.

## Transitive Suppression Cascade

Suppressing a feature temporarily removes its geometry from the solid kernel evaluation without
deleting it:

- **Visual Indicator**: Suppressed chips are marked with a slashed eye icon overlay
  (`assets/icons/ui/design/svg/flat/suppress.svg`) rendered at 50% opacity, allowing the underlying
  feature artwork to remain clearly identifiable, paired with a dashed border outline.
- **Automatic Cascading**: Because downstream features depend on parent geometry, suppressing an
  action automatically cascades to all succeeding dependent features, marking them suppressed with
  matching slashed eye overlays.
- **Unsuppressing**: Unsuppressing a feature restores it and its prerequisites to the active rebuild
  sequence.

## Rollback & Greyscale Inactive State

The blue rollback pin determines the active history boundary:

- Steps to the left of the marker are fully evaluated in the 3D solid model.
- Steps to the right of the marker are inactive: their feature chips are rendered in 100% opacity
  greyscale with matched luminance. This maintains full readability of the icons and labels while
  providing an unambiguous visual distinction between active geometry and future steps.

![Timeline dock closeup with folded group card, combine connector, and inactive steps](../generated/timeline-2d-dock-closeup.png)

## The Reference Demo Project

SolidCraft includes a reference model demonstrating all 2D History Browser capabilities and edge
cases at `docs/2d-timeline-demo.sc`.

To open the demo project:
```sh
cargo run -- docs/2d-timeline-demo.sc
```

The demo model exercises:
- Four component tracks: Root (`2d-timeline-demo`), `Main Housing`, `Drive Shaft`, and
  `Bearing & Cap`.
- A folded group (`Foundation Structures`) consolidating the initial housing and shaft extrusions.
- An expanded group (`Shaft Mechanics`) with Tier 2 header brackets.
- A multi-component boolean operation (`Combine1`) connecting the Root and Main Housing tracks with
  a vertical connector.
- Cascading suppression on auxiliary mounting tabs (steps 11 and 12) featuring 50% opacity slashed
  eye overlays.
- A rollback marker positioned at step 10, demonstrating the active-to-inactive luminance transition.


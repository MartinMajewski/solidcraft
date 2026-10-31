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
  active or inactive component (`Left Handle`, `Right Handle`, `Pivot Bolt`, etc.) listed in hierarchy order.
  Each track header features a circular **activation radio indicator** (`○` inactive, `⊙` active)
  mirroring the component tree in the browser. Clicking any component track header immediately
  activates that component (`component.activate`), seamlessly switching the modeling focus.
- **Active Track Highlighting**: The currently active component track is illuminated with a subtle
  accent background wash and accented grid dividers. This provides continuous visual feedback during
  modeling, preventing actions from being inadvertently performed in the wrong component.
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
     header. If the group name is long or the member span is narrow, the title gracefully truncates
     with an ellipsis (`...`) to preserve dock layout, revealing the full name and member count in a
     helpful tooltip on hover.
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

## Practical Design Walkthrough: Multi-Component Pliers Assembly

SolidCraft includes a practical assembly model demonstrating realistic multi-component CAD modeling,
in-process design alterations, and chronological timeline restructuring at `docs/pliers-demo.sc`.

The assembly models a functional mechanical tool consisting of three separate components:
- **Root Component (`pliers-demo`)**: Houses only shared common reference entities (e.g. `Pivot Alignment Ref` sketch defining the central pivot axis on the XY plane), without owning part bodies.
- **`Left Handle` Component**: Models the left gripping jaw, the lower half of the stepped lap pivot boss, the long ergonomic lever arm, and the central pivot bore.
- **`Right Handle` Component**: Models the opposing gripping jaw, the complementary upper half of the stepped lap pivot boss, the opposing lever arm, and the mating pivot bore.
- **`Pivot Bolt` Component**: Models the central connecting bolt with its cylindrical shank, flanged head cap, and lower retaining washer that mechanically unites both handle halves.

To open the pliers project:
```sh
cargo run -- docs/pliers-demo.sc
```

### 1. Exploratory "Vibe-Motivated" Modeling

During early concept modeling, a designer naturally works iteratively across parts—sketching a jaw,
placing a pivot boss on one handle, roughing out the pin, jumping back to add lever arms, and then
cutting bores.

In a traditional 1D linear timeline, these interleaved operations form an unreadable jumble of mixed
components. In the 2D History Browser, the multi-track matrix immediately makes component ownership
crystal clear:

![Initial unorganized exploratory modeling sequence with multi-track component lanes](../generated/timeline-2d-pliers-unorganized.png)

- Each component track clearly separates the operations belonging to `Left Handle`, `Right Handle`,
  and `Pivot Bolt`.
- The active component radio indicator (`⊙ Pivot Bolt`) and track row wash highlight exactly where new
  features are currently being authored.

### 2. In-Place Alteration & DAG Corridor Inspection

As design requirements evolve, the designer alters features directly on the timeline using `timeline.edit`
(for instance, lengthening both lever arms from 110 mm to 135 mm for increased grip mechanical advantage,
and enlarging the bolt head cap):

![Altered pliers assembly with selected Left Lever Arm and its topological Movable Corridor](../generated/timeline-2d-pliers-alteration.png)

Clicking any feature chip immediately surfaces its place in the parametric Directed Acyclic Graph (DAG):
- The selected feature (`Left Lever Arm`) receives a crisp white border.
- Topological prerequisites (ancestors) light up in amber.
- Downstream dependents light up in electric blue.
- The **Movable Corridor (`[ ... ]`)** brackets appear across Tier 2 and the column grid, showing every
  step where this operation can safely be reordered without invalidating the assembly.

### 3. Afterthought Cleanup, Grouping & Folding

Once the mechanical concepts are proven, the designer restructures the timeline into a clean, professional
architecture:

1. **Corridor Reordering**: Features are dragged horizontally within their allowed corridors to group
   all `Left Handle` operations into contiguous steps 02–05, all `Right Handle` operations into steps 06–09,
   and all `Pivot Bolt` operations into steps 10–12.
2. **Feature Grouping**: Operations are grouped into named folders:
   - `Left Handle & Jaw Mechanics (4)`
   - `Right Handle & Jaw Mechanics (4)`
   - `Connecting Pivot Hardware (3)`
3. **Virtual Group Folding**: The pivot hardware group is folded into a compact **Virtual Group Card**
   featuring an integrated header cap and member count badge `3`.
4. **Active Component Switching**: Clicking the radio button next to `Right Handle` immediately activates
   the component, displaying the active radio indicator `⊙ Right Handle` and illuminating the entire track
   row with an accent wash.

![Organized, grouped, and folded timeline with active component highlighting](../generated/timeline-2d-pliers-organized.png)

## The Reference Demo Project

SolidCraft also includes a comprehensive edge-case test model at `docs/2d-timeline-demo.sc`.

To open the reference demo project:
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


# Flow: layout engine and connection paths

In-repo documentation for the SVG flow graph: layout engine flow, subtree math, and connection waypoints.

---

## Layout engine flow

1. **Dimensions first.** Before layout, every node must have a size. Call `setNodeDimension(id, { width, height })` for each node (typically from `NODE_SIZES` or measured content). The engine does not measure; it only computes positions from known dimensions.

2. **Calculate.** Call `calculate(root)`. The engine:
   - Ensures `hasAllDimensions(root)` (throws if any node is missing dimensions).
   - Runs **depth-first** `buildNodeLayout(root, level=0, parentPosition)` to assign a position to every node.
   - Builds connection paths between each parent and its children via `buildConnections(positioned)`.

3. **Result.** `calculate` returns `{ nodes: PositionedNode[], connections: Connection[] }`. Each `PositionedNode` has `node`, `position` (x, y), and `level`. Each `Connection` has `parent`, `child`, and `path` (array of waypoints for the line).

4. **Per-node direction.** Each node can override layout direction with `direction: 'vertical' | 'horizontal'`. Children of a **vertical** parent are stacked vertically (siblings one under the other). Children of a **horizontal** parent are stacked horizontally (siblings side by side). Default direction comes from `LayoutConfig.direction`.

---

## Subtree width and height

Used to center children under/beside the parent and to space siblings.

### Subtree width (`calculateSubtreeWidth`)

- **Leaf (no children):** subtree width = node width.
- **Horizontal parent:** subtree width = max(node width, sum of children subtree widths + spacing between siblings).
- **Vertical parent:** subtree width = node width + spacing + (max child subtree width) × `subtreeOverlap` (children can overlap horizontally to save space).

### Subtree height (`calculateSubtreeHeight`)

- **Leaf (no children):** subtree height = node height.
- **Vertical parent:** subtree height = max(node height, sum of children subtree heights + spacing between siblings).
- **Horizontal parent:** subtree height = node height + spacing + max(child subtree height).

---

## Horizontal stacking (children under a horizontal parent)

- **Single child:** child X = parent X (centered).
- **Multiple children:** children are centered as a block. Total block width = sum of subtree widths + (n−1) × spacing. Start X = parent X − total width / 2. Each child’s X is placed by walking from start X, adding subtree widths and spacing, then adding half of that child’s subtree width so the child is centered in its slot.

Formula (from `calculateChildXPosition`):  
`startX = parentX - (totalSubtreeWidth + (n-1)*spacing.x) / 2`, then for child at index `i`, X = startX + sum of widths and spacing for indices 0..i−1 + subtreeWidth[i] / 2.

---

## Vertical positioning (children beside a vertical parent)

Children are stacked vertically. Start Y = parent bottom + spacing. Each child’s Y is computed so that vertical centers are spaced by `spacing.y * verticalSiblingSpacing`, with each child centered in “half” of its subtree height (from `calculateChildYPosition`).

---

## Connection waypoints

Connections are polylines (or rounded paths in the UI) through a small set of waypoints. Two patterns:

### Trunk-and-branch (vertical parent)

When the parent’s direction is **vertical**, the line uses a vertical “trunk” from the parent down, then branches to each child.

- Trunk X = parent left − `trunkOffset` + `trunkAdjust` (so the trunk is to the left of the parent).
- Waypoints: `(trunkX, parentY)` → `(trunkX, childY)` → `(childLeft, childY)` (then into the child’s left edge).

So: straight down from a point near the parent, then horizontal into the child.

### Z-shape (horizontal parent)

When the parent’s direction is **horizontal**, the line forms a Z: down from parent, across at mid-height, then down into the child.

- Mid Y = halfway between parent bottom and child top.
- Waypoints: `(parentX, parentBottom)` → `(parentX, midY)` → `(childX, midY)` → `(childX, childTop)`.

So: vertical segment, horizontal segment at mid-height, then vertical into the child.

---

## Path commands and rendering

Waypoints are turned into SVG path `d` in `TreeConnectionLine` via `path-commands.ts`: `move`, `line`, `curve`. Rounded corners are applied at right-angle turns (e.g. trunk-and-branch and Z corners) using a fixed corner radius so the stroke doesn’t have sharp corners.

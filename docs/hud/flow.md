# OpenAgents Flow Editor (Agent Factory View) – Spec v1

**Target stack:** TypeScript + Effect
**Host:** OpenAgents Desktop (Bun/Electrobun or other UI host)
**Core requirement:** Core engine is *pure TypeScript* and mostly UI-agnostic. No React in the core.

The Flow Editor is a **Factorio-style agent map** for OpenAgents Desktop. It visualizes MechaCoder and other agents as a factory:

- Nodes = agents, repos, workflows, jobs
- Edges = data/jog flow
- Canvas = infinite grid you can pan & zoom
- State = read-only in v1 (no user editing), driven by MechaCoder / task events

This spec is heavily inspired by the Unkey network map and our old HUD Panes system, but refactored as a **pure TS/Effect core** plus a thin host UI.

---

## 1. Goals & Non-Goals

### 1.1 Goals

- Show what MechaCoder is doing in a **glanceable, game-like** way.
- Provide a **flow map** of:
  - Projects/repos MechaCoder is working on
  - Tasks (from `.openagents/tasks.jsonl` and/or beads)
  - MechaCoder internal loop (read → plan → edit → test → commit → close)
- Use a **portable, pure layout engine** (like Unkey’s `layout-engine.ts`) that:
  - Takes a tree of nodes (+ dimensions),
  - Produces positioned nodes + connection paths,
  - Is reusable from any UI host (desktop, web, etc).
- Provide an **infinite SVG canvas** spec (pan/zoom, grid) without binding to React.
- Make it easy to extend later (drag to rearrange, interactive editing, blueprints).

### 1.2 Non-Goals (v1)

- No user graph editing in v1 (no drag-to-rewire workflows, no node creation).
- No generic graph editor (only tree + simple cross edges; we can extend to DAG later).
- No dependency on React / React Flow / heavy graph libraries in the core.
- No persistence of layout back to tasks (v1 layout is derived from data, not user-authored).

---

## 2. High-Level Architecture

The Flow Editor is split into **four layers**:

1. **Model layer** (`flow-model`): Pure types for nodes and edges.
2. **Layout engine** (`flow-layout`): Pure functions to compute positions and connection paths.
3. **Canvas state** (`flow-canvas`): Pan/zoom state machine, independent of UI.
4. **Host integration layer** (`flow-host-*`): Renders SVG / UI using whatever environment we’re in (e.g. webview).

### 2.1 Module overview (suggested)

```text
src/flow/
  model.ts          # Node/edge types, statuses, dimensions
  layout.ts         # LayoutEngine: tree → positioned nodes + waypoints
  path.ts           # Path helpers: waypoints → SVG-ish path (rounded corners)
  canvas.ts         # CanvasState: pan/zoom state machine
  sample-data.ts    # Example trees (including MechaCoder factory view)
  mechacoder-map.ts # Mapping from MechaCoder events → FlowModel

# Host-specific (can be React, Svelte, raw DOM, etc.)
src/flow-host-svg/
  render.ts         # Functions to render FlowEngine output into SVG
````

---

## 3. Data Model

### 3.1 Core types

```ts
// Unique node identifier
export type NodeId = string;

export type Direction = "horizontal" | "vertical";

export interface FlowNode {
  id: NodeId;
  type: string;            // e.g. "mechacoder", "repo", "task", "agent", "job"
  label: string;           // display name
  direction?: Direction;   // how this node's children are laid out
  children: FlowNode[];

  // Arbitrary metadata for host UIs
  metadata?: Record<string, unknown>;
}

export interface NodeSize {
  width: number;
  height: number;
}

export interface PositionedNode extends FlowNode {
  x: number; // center x
  y: number; // center y
  size: NodeSize;
}

export interface Connection {
  parentId: NodeId;
  childId: NodeId;
  waypoints: Point[];  // in canvas coordinates
}

export interface Point {
  x: number;
  y: number;
}
```

### 3.2 Node categories for MechaCoder

Initial node types (can be extended later):

* `root` – “OpenAgents Desktop” / or “MechaCoder Factory”
* `repo` – each repo MechaCoder is configured to work on
* `task` – `.openagents` task or bead (ready/working/completed)
* `agent` – MechaCoder process instance
* `job` – individual bead/task execution runs (optional in v1)
* `event` – important events like test failures, type errors, etc. (optional in v1)

You can specialize `FlowNode` with `metadata.type` or by discriminated unions if you like, but v1 can get away with string types and metadata.

---

## 4. Layout Engine Spec (pure TS, no UI)

This is a direct conceptual port of Unkey’s `layout-engine.ts`, refactored for our naming.

### 4.1 API surface

```ts
export interface LayoutConfig {
  spacing: { x: number; y: number };   // base grid spacing
  direction: Direction;                // default child layout direction

  // Fine-tuning
  horizontalIndent: number;            // how far children sit below parent in horizontal mode
  verticalOffset: number;              // minor y-offset tweaks if needed
  subtreeOverlap: number;              // 0..1 overlap factor for vertical reuse of horizontal space
  verticalSiblingSpacing: number;      // extra spacing between vertically-stacked siblings

  verticalTrunkOffset: number;         // how far trunk is from parent (for vertical connections)
  verticalTrunkAdjust: number;         // fine-tuning trunk position
}

export interface LayoutInput {
  root: FlowNode;
  sizes: Record<NodeId, NodeSize>;
  config: LayoutConfig;
}

export interface LayoutOutput {
  nodes: PositionedNode[];
  connections: Connection[];
}

export function calculateLayout(input: LayoutInput): LayoutOutput;
```

### 4.2 Behavior

* **Tree assumption**: Input is a **tree** rooted at `input.root`.

  * Multiple roots can be represented by a virtual invisible `root` node with children.

* **Dimensions**:

  * Every node ID in the tree **must** have a size in `sizes` or the engine throws (fail-fast).
  * For our initial MechaCoder map we can define a static `NODE_SIZES` table by type:

    * e.g. `mechacoder` 282×100, `repo` 240×80, `task` 240×80, `root` 160×40.

* **Direction**:

  * For each node, children layout direction is:

    ```ts
    const dir = node.direction ?? config.direction;
    ```
  * This allows mixing horizontal and vertical groups:

    * e.g., root horizontally to repos, each repo vertically to tasks.

### 4.3 Algorithm

The layout engine works in **three stages**:

1. **Flatten + dimension**:

   * Traverse the tree, collect all nodes, and ensure there is a `NodeSize` for each.
   * Build maps:

     * `id → FlowNode`
     * `id → NodeSize`

2. **Measure subtrees** (bottom-up):

   * For each node, compute a “subtree footprint”:

     * `subtreeWidth(id)` and/or `subtreeHeight(id)`
   * Horizontal parents:

     * Subtree width = sum of child subtree widths plus spacing.
     * Subtree height = parent height + spacing.y + max child subtree height.
   * Vertical parents:

     * Subtree height = sum of child subtree heights plus `verticalSiblingSpacing`.
     * Subtree width = max(child subtree widths) + indent.
   * `subtreeOverlap` allows vertical layouts to reuse horizontal space (see Unkey).

3. **Position children** (top-down DFS):

   * Root starts at `{x: 0, y: 0}` (world origin).
   * For each node:

     * If `direction === "horizontal"`:

       * Children share a common `y` aligned below the parent:

         ```ts
         childY = parentY + parentHeight / 2 + spacing.y;
         ```
       * Children’s `x` computed by distributing subtree widths left→right so that siblings are centered under the parent.
     * If `direction === "vertical"`:

       * Children share:

         ```ts
         childX = parentX - parentWidth / 2 + horizontalIndent;
         ```

       * Children’s `y` computed by stacking them vertically according to subtree heights and `verticalSiblingSpacing`.

4. **Build connections**:

   * For each parent-child pair, compute `waypoints: Point[]` in canvas coordinates:

     * Horizontal:

       * Z-shaped path: parent bottom → midY → child top.
     * Vertical:

       * Trunk-and-branch: branch out to a vertical “bus” (trunk) then over to child.

### 4.4 Fail-fast invariants

If any of these occur, `calculateLayout` **throws** (or returns a typed error in Effect):

* Node dimension missing (`sizes[node.id]` undefined).
* Node ID not found in node map.
* Cycles detected (optional — can assume no cycles in v1).
* Empty tree / null root.

This is in the spirit of the Unkey engine: break fast in dev instead of silently overlapping.

---

## 5. Connection Path & Styling

Path math is separate from layout:

```ts
export interface PathConfig {
  cornerRadius: number;   // how rounded corners are
}

export function buildRoundedPath(points: Point[], config: PathConfig): string;
```

* Input: `waypoints` from the layout engine.
* Behavior:

  * Detect direction changes (H→V, V→H).
  * Replace sharp corners with arcs / quadratic curves based on `cornerRadius`.
  * Return an SVG path string (e.g. `"M x y L x2 y2 Q cx cy x3 y3 ..."`) or an AST-like representation that host can choose to serialize.

Animation is **host-level**:

* Recommended CSS/SVG pattern:

  * `stroke-dasharray` + `stroke-dashoffset` + `<animate>` (as in Unkey).
* Host chooses:

  * stroke color based on connection status (idle/active/error),
  * animation style (dots vs dashed vs none).

---

## 6. Canvas State & Interaction (UI-agnostic)

Core canvas state is also pure TS:

```ts
export interface CanvasState {
  scale: number;    // zoom factor
  offset: Point;    // translate in world space
}

export type CanvasEvent =
  | { type: "PAN_START"; pointer: Point }
  | { type: "PAN_MOVE"; pointer: Point }
  | { type: "PAN_END" }
  | { type: "ZOOM"; pointer: Point; delta: number } // wheel delta
  | { type: "RESET" };

export interface CanvasConfig {
  minZoom: number;
  maxZoom: number;
  friction: number;      // for kinetic panning
  minVelocity: number;
}

export interface CanvasStateWithVelocity extends CanvasState {
  velocity: Point;
}
```

Then a reducer:

```ts
export function reduceCanvasState(
  state: CanvasStateWithVelocity,
  event: CanvasEvent,
  config: CanvasConfig
): CanvasStateWithVelocity;
```

**Behavior:**

* **Pan**:

  * On `PAN_START` store initial pointer and offset.
  * On `PAN_MOVE` update offset based on pointer delta.
  * On `PAN_END` store velocity (last few deltas / dt) to drive inertial motion.
* **Zoom**:

  * Adjust `scale` within `[minZoom, maxZoom]`.
  * Recenter `offset` so zoom is around the mouse pointer (not the origin).
* **Friction**:

  * If `velocity` magnitude > `minVelocity`, apply `offset += velocity * dt` and `velocity *= friction` in a per-frame tick (host calls this).
* **Transform usage**:

  * Host uses: `transform = translate(offset.x, offset.y) scale(scale)` on an inner `<g>` or equivalent.

Host responsibility:

* Wiring real pointer/mouse events into `CanvasEvent`s.
* Providing a render loop or hooks to apply friction over time.

---

## 7. Rendering Contract (Host-side)

The core engine does **not** know about DOM/React. The host UI must:

1. Build a `FlowNode` tree and `sizes` (by type).
2. Call `calculateLayout` to get `PositionedNode[]` + `Connection[]`.
3. Maintain `CanvasState` and pass it as a transform.
4. For each `Connection`:

   * Call `buildRoundedPath(connection.waypoints, pathConfig)` to get a path string (or equivalent).
   * Render it as `<path d="...">` (or your render system’s equivalent).
5. For each `PositionedNode`:

   * Render the corresponding node at its center `{x, y}`.
   * Node wrapper should expose:

     * `data-node-id` or equivalent for hit testing,
     * metadata for status, metrics, etc.

The host is free to:

* Use React, Svelte, raw DOM, canvas, WebGPU, etc.
* Add overlays (details panels, metrics) layered above the SVG.

---

## 8. MechaCoder Integration

### 8.1 Data → FlowNode mapping

For the **MechaCoder Factory** view in the `openagents` repo, we want a tree that looks roughly like:

```text
(root) OpenAgents Desktop
  ├─ MechaCoder Agent
  │   ├─ Repo: openagents
  │   │   ├─ Task: openagents-5bb (Epic)
  │   │   ├─ Task: openagents-5bb.1
  │   │   ├─ ...
  │   ├─ Repo: nostr-effect
  │   │   ├─ Task: nostr-effect-997.1
  │   │   ├─ ...
  │   └─ Internal Loop
  │       ├─ Phase: read
  │       ├─ Phase: plan
  │       ├─ Phase: edit
  │       ├─ Phase: test
  │       └─ Phase: commit/close
  └─ (future) Other agents, workflows, etc.
```

A mapping function:

```ts
export interface MechaCoderState {
  // from .openagents/tasks.jsonl, bd, logs, etc.
}

export function buildMechaCoderFlowTree(state: MechaCoderState): FlowNode;
```

This function:

* Reads `.openagents/project.json` and `tasks.jsonl`.
* Optionally queries `bd` (for epics like `openagents-42j`, `openagents-5bb`).
* Reads MechaCoder logs to derive “phase” nodes and statuses (e.g. which phase is active).
* Produces a single root `FlowNode` tree.

### 8.2 Status & animation

Node metadata should include status fields for host-specific styling:

```ts
export type Status = "idle" | "busy" | "error" | "blocked" | "completed";

export interface FlowNodeMetadata {
  status?: Status;
  metrics?: {
    jobsPerHour?: number;
    lastJobAt?: string;
    successRate?: number;
    // etc.
  };
}
```

Host UI can then:

* Color nodes based on `status` (green/yellow/red, etc.).
* Animate connections when jobs are flowing (e.g. recent job between parent & child).
* Show metric badges on nodes (jobs/hour, last run, etc.).

---

## 9. Implementation Phases

This is where your `.openagents` tasks for this project would come from. Rough phases:

### Phase 1 – Core engine

* [ ] Implement `flow-model` types.
* [ ] Implement `flow-layout.calculateLayout` with:

  * Horizontal + vertical directions,
  * Subtree width/height,
  * Fail-fast invariants.
* [ ] Implement `path.buildRoundedPath`.
* [ ] Implement unit tests:

  * Small sample trees,
  * Asserts on positions & path corner behavior.

### Phase 2 – Canvas state

* [ ] Implement `CanvasState` + `CanvasEvent` + reducer.
* [ ] Provide a tiny host demo (even just SVG in a static HTML) to validate pan/zoom.
* [ ] Tests for pan/zoom math (especially mouse-centered zoom).

### Phase 3 – MechaCoder map mapping

* [ ] Implement `MechaCoderState` extraction:

  * from `.openagents/tasks.jsonl`,
  * from last N logs in `docs/logs/YYYYMMDD/`.
* [ ] Implement `buildMechaCoderFlowTree(state)`.
* [ ] Add sample fixtures & snapshot tests for the tree.

### Phase 4 – Host UI (SVG)

* [ ] Implement minimal SVG host that:

  * Calls `calculateLayout` + `buildRoundedPath`,
  * Renders nodes as simple boxes with labels,
  * Renders connections as paths with basic animation.
* [ ] Add click handling to show node details (can be textual for now).

### Phase 5 – Factorio polish (optional)

* [ ] Integrate with your existing **visual language**:

  * Grid, colors, status badges, animations.
* [ ] Add production metrics overlays, mini dashboard.
* [ ] Expose this as an “Agent Factory” or “Flow” pane in OpenAgents Desktop.

---

## 10. Summary

* The **core** of the Flow Editor is:

  * A **tree-based layout engine** (Unkey-style, but ours),
  * A **canvas state machine**,
  * A **data mapping** from MechaCoder state to `FlowNode`.
* Everything is pure TS/Effect-friendly and host-agnostic.
* Hosts (OpenAgents Desktop UI) render the layout into SVG or any other drawing surface, layering overlays and animations on top.
* This gives you a **game-like, Factorio-inspired view** of MechaCoder and the broader agent network, without dragging React/ReactFlow into your engine.

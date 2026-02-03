# Flow UI: SVG graph system (and adoption roadmap)

## 1. Unkey flow — full reference

**Repo path:** `web/apps/dashboard/app/(app)/[workspaceSlug]/projects/[projectId]/(overview)/deployments/[deploymentId]/network/unkey-flow/`

### 1.1 Layout and engine (pure logic)

| File | Contents |
|------|----------|
| `layout-engine.ts` | `Point`, `TreeNode`, `PositionedNode`, `LayoutConfig`, `LayoutEngine<T>`. `setNodeDimension`, `hasAllDimensions`, `calculate(root)`, `flattenTree`. Per-node `direction` (vertical/horizontal). `buildNodeLayout` (depth-first), `calculateSubtreeWidth` / `calculateSubtreeHeight`, `calculateChildXPosition` / `calculateChildYPosition`, `buildConnections` / `buildConnectionPath` (Z-shape vs trunk-and-branch). `getNodeEdges`, `invariant`. |
| `docs.ts` | In-repo documentation: flow of layout engine, subtree width/height, horizontal stacking, vertical positioning, connection waypoints (trunk+branch, Z-shape). |

### 1.2 Canvas

| File | Contents |
|------|----------|
| `components/canvas/infinite-canvas.tsx` | SVG ref, state `{ scale, offset }`, `transform` on `<g>`. Pan: mousedown/move/up, startPanRef gap, momentum (velocityRef, FRICTION, requestAnimationFrame). Wheel: preventDefault, zoom toward cursor, scaleRatio. Renders `GridPattern` + `CanvasBoundary` + children; optional `overlay` in sibling div (`pointer-events-none`). Props: minZoom, maxZoom, defaultZoom, zoomSpeed, gridSize, dotRadius, dotClassName, showGrid, children, overlay. Defaults: gridSize 15, dotRadius 1.5, dotClassName "fill-grayA-4". |
| `components/canvas/grid-pattern.tsx` | `<defs><pattern id="dot-grid">` with one circle per cell; `<animate>` on `r` and `opacity` (ANIMATION_DURATION, MAX_RADIUS_MULTIPLIER, RANDOM_DELAY_MAX). Large rect (GRID_OFFSET -5000, GRID_DIMENSION 10000) fill="url(#dot-grid)". Props: gridSize, dotRadius, dotClassName. |
| `components/canvas/canvas-boundary.tsx` | Wraps children in `ErrorBoundary`. Fallback: `TreeElementNode` at (0, 250) rendering a card (icon, "Failed to render network tree", error.message, "Retry Layout" button). Uses Unkey UI (Button, icons) and inline keyframes for breathe-ring. |

### 1.3 Tree UI

| File | Contents |
|------|----------|
| `components/tree/tree-layout.tsx` | Props: data (DeploymentNode), nodeSpacing, layoutConfig (direction, layout, connections), **connectionAnimation** (AnimationConfig), onNodeClick, renderNode, renderConnection. Creates LayoutEngine, parentMap, allNodes; sets dimensions from NODE_SIZES; handleClick delegates to data-node-id; layout = calculate(data). Renders connections (renderConnection or TreeConnectionLine with animation), then TreeElementNode per positioned node. |
| `components/tree/tree-element-node.tsx` | `foreignObject` at position, width=1 height=1 overflow=visible; inner div position absolute, transform translate(-50%, -50%), data-node-id={id}. |
| `components/tree/tree-connection-line.tsx` | Props: path, **animation** (AnimationConfig). ANIMATION_PRESETS: dots, dashes, dots-slow, dashes-fast, pulse (dashLength, gapLength, speed, strokeWidth, color). AnimationConfig = { preset } or { custom: Partial<...> }. buildPath (rounded corners via path-commands), pathD, dashArray, `<path>` + `<animate attributeName="stroke-dashoffset">`. CORNER_RADIUS 32. |
| `components/tree/tree-path-command.ts` | move, line, curve; PathCommand; renderPath(commands) → SVG d string. |

### 1.4 Node types and components

| File | Contents |
|------|----------|
| `components/nodes/types.ts` | OriginNode, SentinelNode, InstanceNode, SkeletonNode; DeploymentNode union. HealthStatus, BaseMetrics, RegionInfo. isOriginNode, isSentinelNode, isInstanceNode, isSkeletonNode. NODE_SIZES (origin 70×20, sentinel/instance/skeleton 230×70). REGION_INFO, DEFAULT_NODE_WIDTH. |
| `components/nodes/index.ts` | Re-exports node components. |
| `components/nodes/origin-node.tsx` | Renders origin node (label "INTERNET" etc.). |
| `components/nodes/sentinel-node.tsx` | Renders gateway/region card (flagCode, metrics, health). |
| `components/nodes/instance-node.tsx` | Renders instance card (description, metrics, health). |
| `components/nodes/default-node.tsx` | Fallback node. |
| `components/nodes/skeleton-node.tsx` | Loading skeleton. |
| `components/nodes/node-wrapper/` | node-wrapper.tsx, health-banner.tsx. |
| `components/nodes/status/` | status-config.ts, status-dot.tsx, status-indicator.tsx. |
| `components/nodes/components/` | card-footer, card-header, metric-pill. |

### 1.5 Overlay and simulate

| File | Contents |
|------|----------|
| `components/overlay/node-details-panel.tsx` | Panel for selected node (header, metrics, chart, settings). Uses node-details-panel/ (header, metrics, chart with logs-chart-*, settings-row), region-node/sentinel-instances. |
| `components/overlay/project-details.tsx` | Project-level details in overlay. |
| `components/overlay/live.tsx` | LiveIndicator (e.g. "Live" badge). |
| `components/overlay/dev-tree-generator.tsx` | InternalDevTreeGenerator: dev-only UI to generate mock tree (presets: small/medium/large/stress), onGenerate, onReset. Uses trpc, Unkey icons/Button. |
| `components/simulate/tree-generate.tsx` | tree-generate: builds DeploymentNode tree from GeneratorConfig (regions, instancesPerRegion, healthDistribution, regionDirection, instanceDirection). PRESETS (small, medium, large, stress). Used by dev-tree-generator. |

### 1.6 Public API (index)

Exports: nodes, overlay (dev-tree-generator, live, node-details-panel, project-details), canvas (infinite-canvas), simulate (tree-generate), tree (tree-layout, tree-connection-line).

### 1.7 Integration (deployment-network-view.tsx)

- State: generatedTree, selectedNode.
- trpc.deploy.network.get for defaultTree; currentTree = generatedTree ?? defaultTree ?? SKELETON_TREE.
- InfiniteCanvas with overlay: NodeDetailsPanel(selectedNode), ProjectDetails(projectId), LiveIndicator, InternalDevTreeGenerator (dev only).
- TreeLayout: data=currentTree, nodeSpacing { x: 10, y: 75 }, onNodeClick→setSelectedNode, renderNode→renderDeploymentNode (type guards → OriginNode, SentinelNode, InstanceNode, SkeletonNode), renderConnection→TreeConnectionLine.

---

## 2. OpenAgents flow — what we implemented

**Path:** `apps/web/src/components/flow/`

### 2.1 Layout and engine

| File | Contents |
|------|----------|
| `layout-engine.ts` | Same algorithm as Unkey: Point, TreeNode, PositionedNode, LayoutConfig, LayoutEngine<T>. setNodeDimension, hasAllDimensions, calculate, flattenTree. buildNodeLayout, calculateSubtreeWidth/Height, calculateChildXPosition/YPosition, buildConnections, buildConnectionPath (Z-shape, trunk-and-branch). getNodeEdges, invariant. |
| `path-commands.ts` | move, line, curve; PathCommand; renderPath(commands). (No docs.ts.) |

### 2.2 Canvas

| File | Contents |
|------|----------|
| `InfiniteCanvas.tsx` | Same behavior: SVG ref, scale/offset, pan (startPanRef, momentum), wheel zoom. Renders GridPattern + children (no CanvasBoundary); optional overlay. Props: minZoom, maxZoom, defaultZoom, zoomSpeed, gridSize, dotRadius, dotClassName, showGrid, children, overlay. Defaults: gridSize 24, dotRadius 0.8, dotClassName "fill-muted-foreground/40". |
| `GridPattern.tsx` | Same idea: pattern id="flow-dot-grid", animated circle, large rect. Props: gridSize, dotRadius, dotClassName. |

### 2.3 Tree UI

| File | Contents |
|------|----------|
| `TreeLayout.tsx` | Props: data (FlowNode), nodeSpacing, layoutConfig, onNodeClick, renderNode, renderConnection. No connectionAnimation prop. Sets dimensions from NODE_SIZES, calculate(data), click delegation, renders connections then TreeElementNode. |
| `TreeElementNode.tsx` | Same as Unkey: foreignObject at position, div translate(-50%, -50%), data-node-id. |
| `TreeConnectionLine.tsx` | path only; single fixed animation (dashLength 0.1, gapLength 8, dur 2s, stroke-muted-foreground/60). No animation prop or presets. buildPath with rounded corners (CORNER_RADIUS 32). |

### 2.4 Types

| File | Contents |
|------|----------|
| `types.ts` | FlowNode (TreeNode + label, metadata?: { type: FlowNodeType }). FlowNodeType = 'root' \| 'leaf'. NODE_SIZES: root 140×36, leaf 180×56. No domain types (no Origin/Sentinel/Instance), no health/region helpers. |

### 2.5 Public API (index.ts)

Exports: InfiniteCanvas, GridPattern, TreeLayout, TreeElementNode, TreeConnectionLine; layout-engine types + LayoutEngine + invariant; FlowNode, FlowNodeType, NODE_SIZES.

### 2.6 Homepage integration (routes/_app/index.tsx)

- Static HOME_TREE: root "OpenAgents" (direction horizontal), children Runtime, Agents, Protocol (metadata type leaf).
- InfiniteCanvas (no overlay). TreeLayout: nodeSpacing { x: 24, y: 60 }, layoutConfig { direction: 'vertical' }, renderNode → simple card (rounded border, bg-card, label). No onNodeClick, no renderConnection (default TreeConnectionLine).

---

## 3. Gap analysis: Unkey has it, we don’t

| Area | Unkey | OpenAgents | Gap |
|------|--------|------------|-----|
| **Canvas boundary** | CanvasBoundary wraps children; ErrorBoundary fallback renders TreeElementNode at (0,250) with error card + Retry button (Unkey UI + icons). | No error boundary around tree; any layout/render error bubbles. | **Missing:** Error boundary with in-canvas fallback and retry. |
| **Connection animation** | TreeConnectionLine accepts `animation?: AnimationConfig`. Presets: dots, dashes, dots-slow, dashes-fast, pulse (dashLength, gapLength, speed, strokeWidth, color). Custom partial config. TreeLayout passes `connectionAnimation` to TreeConnectionLine. | TreeConnectionLine has no animation prop; single hardcoded style (dash 0.1, gap 8, 2s, one color). TreeLayout has no connectionAnimation prop. | **Missing:** AnimationConfig type, presets, and TreeLayout/TreeConnectionLine support. |
| **Overlay** | Overlay used for: NodeDetailsPanel(selectedNode), ProjectDetails(projectId), LiveIndicator, InternalDevTreeGenerator (dev). | InfiniteCanvas supports overlay prop but homepage does not pass one. | **Missing:** No overlay content (no node details panel, no project details, no live indicator, no dev tree generator). |
| **Node selection** | onNodeClick → setSelectedNode; selected node passed to NodeDetailsPanel. | No onNodeClick on homepage; no selection state. | **Missing:** Optional selection + details panel (or other overlay). |
| **Data source** | Tree from API (trpc.deploy.network.get) or dev-generated (InternalDevTreeGenerator); SKELETON_TREE while loading. | Static HOME_TREE only. | **Missing:** API-backed or generated tree; loading/skeleton state. |
| **Node types** | Four domain node types (Origin, Sentinel, Instance, Skeleton) with type guards, NODE_SIZES per type, dedicated components (OriginNode, SentinelNode, InstanceNode, SkeletonNode), node-wrapper, status (health dot, etc.), card header/footer, metric pill. | Two generic types (root, leaf), one NODE_SIZES map, renderNode returns a single card style; no domain components. | **Missing:** Domain-specific node types and dedicated node components (only if we need deployment/health-style UI). |
| **Simulate / dev tree** | tree-generate.tsx builds DeploymentNode from config (regions, instances, health distribution, directions). Dev-tree-generator UI with presets (small/medium/large/stress), onGenerate/onReset. | None. | **Missing:** Dev-only tree generator and/or config-driven tree builder (only if we want dev/testing trees). |
| **Docs** | docs.ts: layout engine flow, subtree math, connection waypoints. | No in-repo flow docs. | **Missing:** Optional docs.ts or equivalent (this file covers high-level and gaps). |

---

## 4. What we have that matches Unkey

- **Layout engine:** Same algorithm (depth-first, per-node direction, subtree width/height, child X/Y, Z-shape and trunk-and-branch paths), same API surface (setNodeDimension, calculate, flattenTree), same invariant behavior.
- **Path commands:** Same move/line/curve and renderPath → SVG d.
- **InfiniteCanvas:** Same pan/zoom/momentum, same transform and grid; we have overlay prop but no CanvasBoundary.
- **GridPattern:** Same pattern + animated dot idea; we use different default grid size and dot radius and pattern id.
- **TreeLayout:** Same structure (engine, dimensions, calculate, connections then nodes, click delegation); we lack connectionAnimation.
- **TreeElementNode:** Same foreignObject + centered div + data-node-id.
- **TreeConnectionLine:** Same path construction (rounded corners); we have one fixed animation, no presets or config.

---

## 5. Implementation summary (what we did — Steps 1–11 complete)

### Files added

- `flow/layout-engine.ts` — LayoutEngine, TreeNode, Point, LayoutConfig, invariant.
- `flow/path-commands.ts` — move, line, curve, renderPath.
- `flow/GridPattern.tsx` — Animated dot pattern (id flow-dot-grid).
- `flow/InfiniteCanvas.tsx` — Pan/zoom canvas, grid, CanvasBoundary wrapping children, overlay prop (defaults grid 24, dot 0.8).
- `flow/CanvasBoundary.tsx` — ErrorBoundary; fallback TreeElementNode at (0, 250) with error card + Retry (Step 1).
- `flow/TreeElementNode.tsx` — foreignObject + data-node-id.
- `flow/TreeConnectionLine.tsx` — Rounded path, AnimationConfig, ANIMATION_PRESETS (dots, dashes, dots-slow, dashes-fast, pulse), default preset "dots" (Step 2).
- `flow/TreeLayout.tsx` — Engine, NODE_SIZES, connectionAnimation prop, renderNode/renderConnection, click delegation.
- `flow/types.ts` — FlowNode, FlowNodeType (root | leaf | skeleton), NODE_SIZES, isRootNode, isLeafNode, isSkeletonNode, SKELETON_TREE (Steps 5–6).
- `flow/README.md` — In-repo docs: layout engine flow, subtree math, connection waypoints (Step 3).
- `flow/NodeDetailsPanel.tsx` — Panel with header (label, close), body (id, direction, child count), metrics placeholder (Steps 4, 9).
- `flow/ProjectDetails.tsx` — Optional projectId; minimal "Project" badge when set (Step 10).
- `flow/LiveIndicator.tsx` — Small "Live" status badge (Step 10).
- `flow/DevTreeGenerator.tsx` — Dev-only; Small/Medium/Large + Reset, buildFlowTree(PRESETS) (Step 8).
- `flow/tree-generate.ts` — buildFlowTree(config), PRESETS (small, medium, large), TreeGenerateConfig (Step 7).
- `flow/nodes/RootNode.tsx`, `flow/nodes/LeafNode.tsx`, `flow/nodes/SkeletonNode.tsx` — Dedicated node components (Step 6).
- `flow/nodes/index.ts` — Re-exports RootNode, LeafNode, SkeletonNode.
- `flow/index.ts` — Public API: canvas, overlay, tree, layout engine, types/guards/nodes, tree-generate; doc comment + grouped exports (Steps 2, 11).

### Files changed

- `routes/_app/index.tsx` — HOME_TREE, currentTree = generatedTree ?? apiTree ?? SKELETON_TREE, selectedNode, overlay (NodeDetailsPanel, ProjectDetails, LiveIndicator, DevTreeGenerator), renderFlowNode (type guards → RootNode, LeafNode, SkeletonNode), onNodeClick when not skeleton.
- `package.json` — Removed @react-three/fiber, @react-three/drei, three.
- `bun.lock` — Updated.

### Files removed

- `components/three/NodeCanvas.tsx`, `Nodes.tsx`.

### Other

- Grid defaults: gridSize 24, dotRadius 0.8.
- Deploy: `apps/web` deploys via `bun run deploy` to the `openagents-web-app` Cloudflare Worker.
- Steps 1–11: all implemented; no further parity steps in this plan.

---

## 6. Steps to 100% parity with Unkey flow

Execute in order. Each step closes one or more gaps from section 3.

**Step 1 — Canvas boundary**

- Add `flow/CanvasBoundary.tsx`: wrap children in an ErrorBoundary. Fallback: render `TreeElementNode` at position `(0, 250)` with a card containing "Failed to render network tree", `error.message`, and a "Retry" button that calls `reset`. Use existing UI (e.g. Button from `@/components/ui`) and avoid Unkey-specific deps.
- In `InfiniteCanvas.tsx`, wrap `children` in `CanvasBoundary` (so the tree content is inside the boundary; grid stays outside).

**Step 2 — Connection animation API**

- In `TreeConnectionLine.tsx`: define `AnimationConfig` as `{ preset: PresetName; color?: string } | { custom: Partial<{ dashLength, gapLength, speed, strokeWidth, color }> }`. Add `ANIMATION_PRESETS` (dots, dashes, dots-slow, dashes-fast, pulse) with dashLength, gapLength, speed, strokeWidth, color. Accept `animation?: AnimationConfig`; default to preset `"dots"`. Compute pathD and dash animation from config.
- In `TreeLayout.tsx`: add prop `connectionAnimation?: AnimationConfig`. When rendering default connection lines, pass `animation={connectionAnimation}` to `TreeConnectionLine`.
- Export `AnimationConfig` (and optionally preset names) from `flow/index.ts`.

**Step 3 — In-repo flow docs**

- Add `flow/docs.ts` (or `flow/README.md`): document layout engine flow (dimensions → calculate → buildNodeLayout depth-first), subtree width/height, horizontal stacking and vertical positioning formulas, and connection waypoints (trunk-and-branch vs Z-shape). Can adapt content from Unkey's `docs.ts`.

**Step 4 — Node selection and overlay slot**

- Homepage (`routes/_app/index.tsx`): add state `selectedNode: FlowNode | null`, pass `onNodeClick={(node) => setSelectedNode(node)}` to `TreeLayout`.
- Add a minimal `flow/NodeDetailsPanel.tsx`: props `node: FlowNode | null`, `onClose: () => void`. When `node` is set, render a panel (e.g. slide-out or overlay card) with node `label` and `id`; include a close button that calls `onClose`. When `node` is null, render nothing.
- Homepage: pass `overlay={<><NodeDetailsPanel node={selectedNode} onClose={() => setSelectedNode(null)} /></>}` to `InfiniteCanvas` (and extend overlay in later steps).

**Step 5 — Data source and loading/skeleton**

- In `flow/types.ts`: add a `SkeletonNode` type (e.g. `metadata.type: 'skeleton'`) and extend `FlowNode` so it can be root | leaf | skeleton. Add `NODE_SIZES.skeleton` and type guard `isSkeletonNode`. Define `SKELETON_TREE`: a small tree (e.g. root + 2–3 skeleton children) for loading state.
- Homepage: introduce `currentTree` logic: `currentTree = generatedTree ?? apiTree ?? SKELETON_TREE`. For now `apiTree` can be `null` or a static tree; later replace with API. When using `SKELETON_TREE`, consider disabling `onNodeClick` (parity with Unkey's `isShowingSkeleton`).
- In `renderNode`, handle skeleton nodes (e.g. render a skeleton placeholder card or reuse existing card with loading style).

**Step 6 — Domain node type guards and dedicated components (parity structure)**

- In `flow/types.ts`: add type guards `isRootNode`, `isLeafNode` (and `isSkeletonNode` from step 5). Ensure `NODE_SIZES` and `FlowNode` union support root, leaf, skeleton.
- Add `flow/nodes/RootNode.tsx` and `flow/nodes/LeafNode.tsx` (and optionally `flow/nodes/SkeletonNode.tsx`): each accepts `node` and renders the same card style currently in homepage's `renderNode`, so we have dedicated components per type like Unkey's OriginNode, SentinelNode, InstanceNode.
- Add `flow/nodes/index.ts` re-exporting these. Homepage (or a shared `renderFlowNode`): use type guards and render RootNode, LeafNode, SkeletonNode accordingly. This matches Unkey's `renderDeploymentNode` pattern.

**Step 7 — Tree-generate (simulate)**

- Add `flow/tree-generate.ts` (or `flow/simulate/tree-generate.ts`): export a function `buildFlowTree(config)` where config includes e.g. `rootLabel`, `childLabels` or `childCount`, `direction` (vertical/horizontal). Return a `FlowNode` tree. Optionally export `PRESETS` (small, medium, large) mapping to configs (e.g. 3 children, 5 children, 7 children). No health/region logic; keep it generic.

**Step 8 — Dev tree generator overlay**

- Add `flow/DevTreeGenerator.tsx` (or `flow/overlay/DevTreeGenerator.tsx`): dev-only component (guard with `import.meta.env.DEV` or similar). UI: preset buttons (Small, Medium, Large) that call `onGenerate(buildFlowTree(presetConfig))`, plus Reset that calls `onReset()`. Props: `onGenerate: (tree: FlowNode) => void`, `onReset: () => void`.
- Homepage: add state `generatedTree: FlowNode | null`. In overlay, when dev, render `<DevTreeGenerator onGenerate={setGeneratedTree} onReset={() => setGeneratedTree(null)} />`. Ensure `currentTree = generatedTree ?? apiTree ?? SKELETON_TREE` (step 5).

**Step 9 — Node details panel content**

- Flesh out `flow/NodeDetailsPanel.tsx`: for the selected node, show label, id, and optional fields (e.g. direction, child count). Add a simple layout (header with close, body with details) so it matches the *role* of Unkey's node-details-panel. Optional: placeholder area for future metrics/charts.

**Step 10 — Project details and Live indicator (overlay composition parity)**

- Add `flow/ProjectDetails.tsx` (or `flow/overlay/ProjectDetails.tsx`): accept optional `projectId`; render nothing or a minimal "Project" badge so overlay can include it. No backend required for parity.
- Add `flow/LiveIndicator.tsx` (or `flow/overlay/LiveIndicator.tsx`): render a small "Live" or status badge. No backend required.
- Homepage overlay: compose `NodeDetailsPanel`, `ProjectDetails`, `LiveIndicator`, and (in dev) `DevTreeGenerator` so the overlay structure matches Unkey's deployment-network-view (NodeDetailsPanel, ProjectDetails, LiveIndicator, InternalDevTreeGenerator).

**Step 11 — Public API and exports**

- Update `flow/index.ts`: export CanvasBoundary, AnimationConfig (and presets if desired), NodeDetailsPanel, DevTreeGenerator, ProjectDetails, LiveIndicator, tree-generate (buildFlowTree, PRESETS), SKELETON_TREE, and node type guards / node components. Ensure consumers can build a view that matches Unkey's deployment-network-view using only flow exports.

**Completion**

- After step 11 we have: canvas boundary, connection animation config, in-repo docs, node selection + details panel, data source + skeleton + loading, domain node types + dedicated components, tree-generate, dev tree generator, overlay composition (details + project + live + dev generator), and full public API. That is 100% structural and behavioral parity with Unkey's flow; domain content (e.g. health, metrics, API shape) remains OpenAgents-specific.

**Status: Steps 1–11 implemented.** There are no further parity steps in this plan. Section 5 below is updated to reflect the full implementation.

---

## 7. OpenAgents adoption roadmap (Flow-first UI everywhere)

The goal is to make the **Flow canvas** the primary UI across the app:
- center = flow graph (default)
- right = inspector/details/community
- left = quick nav + recents (optional)

### Phase 1 — Homepage demo (fake but believable)

- [ ] Replace the tiny 3-node tree with a representative “OpenAgents on Cloudflare + OpenClaw” graph.
- [ ] Add node selection + overlay inspector (click node → details).
- [ ] Add realistic “live” cues: statuses, timestamps, and “needs approval” badges.

### Phase 2 — Hatchery becomes the primary navigation surface

- [ ] Make the `/hatchery` link real and route it to the flow canvas.
- [ ] Show “Your workspace graph”: Chats, Projects, OpenClaw Cloud, Community.
- [ ] Persist focus/selection in the URL (e.g. `?focus=thread:...`) so links are shareable.

### Phase 3 — Flow-native assistant view (chat as a subgraph)

- [ ] Introduce a “thread graph”: message turns + tool calls + approvals as nodes.
- [ ] Keep the existing composer UX, but surface execution/progress as graph edges/nodes.
- [ ] Make the durable DO thread id the canonical thread identity (agent-worker thread ⇄ UI thread).

### Phase 4 — OpenClaw subgraph

- [ ] OpenClaw Cloud node with status/provision/restart/backup/devices.
- [ ] Sessions list/history as nodes; click → transcript viewer + send.
- [ ] Approvals unify: device pairing + risky actions through the same approval UI.

### Phase 5 — Real data (Convex-backed graphs)

- [ ] Replace fake demo data with Convex graph queries (threads/projects/community/OpenClaw).
- [ ] Introduce node type system + sizing rules (avoid page-local hardcoding).
- [ ] Add UX telemetry hooks: node clicks, time-to-answer, approval conversion rates.

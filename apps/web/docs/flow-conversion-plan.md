# Homepage flow: Three.js to SVG graph conversion

## Where Unkey's flow lives (reference)

Unkey's flow implementation is under a single feature subtree:

**Path:** `web/apps/dashboard/app/(app)/[workspaceSlug]/projects/[projectId]/(overview)/deployments/[deploymentId]/network/unkey-flow/`

**Layout and engine (pure logic, no React):**

- `layout-engine.ts` — `LayoutEngine<T>`, `TreeNode`, `Point`, `LayoutConfig`. Tree flattening, per-node `direction` (horizontal/vertical), subtree width/height, child X/Y positioning, connection path building (Z-shape for horizontal, trunk-and-branch for vertical). Fail-fast `invariant()` for missing dimensions.

**Canvas (SVG pan/zoom):**

- `components/canvas/infinite-canvas.tsx` — State `{ scale, offset }`, `transform` on `<g>`, mouse pan (startPanRef gap), wheel zoom (prevent default), optional momentum. Renders `GridPattern` + `CanvasBoundary` + `children`; optional `overlay` in a sibling div.
- `components/canvas/grid-pattern.tsx` — SVG `<pattern>` with animated circles (radius + opacity), large rect filled with pattern.
- `components/canvas/canvas-boundary.tsx` — Error boundary wrapping children (Unkey-specific UI; we omitted it).

**Tree UI:**

- `components/tree/tree-layout.tsx` — Creates `LayoutEngine`, sets dimensions from a `NODE_SIZES`-like map, calls `calculate(data)`, renders connections then `TreeElementNode` for each positioned node; `renderNode(node, parent)` and optional `renderConnection(path, parent, child)`.
- `components/tree/tree-element-node.tsx` — `<foreignObject>` at `position`, inner div `transform: translate(-50%, -50%)`, `data-node-id` for click delegation.
- `components/tree/tree-connection-line.tsx` — Builds SVG path from `Point[]` with rounded corners via `tree-path-command.ts` (move/line/curve, `renderPath`). Animated stroke via `stroke-dasharray` + `<animate attributeName="stroke-dashoffset">`.
- `components/tree/tree-path-command.ts` — Types and helpers: `MoveTo`, `LineTo`, `QuadraticCurve`, `renderPath(commands)` → SVG `d` string.

**Node types and data:** Unkey uses `components/nodes/types.ts` (e.g. `OriginNode`, `SentinelNode`, `InstanceNode`) and `NODE_SIZES` by `metadata.type`. The view is wired in `deployment-network-view.tsx`: `InfiniteCanvas` → `TreeLayout` with `data`, `renderNode`, `renderConnection`, overlay (details panel, dev tree generator).

---

## What was implemented (OpenAgents)

### Files added

| File | Purpose |
|------|--------|
| `apps/web/src/components/flow/layout-engine.ts` | Generic `LayoutEngine<T>`, `TreeNode`, `Point`, `LayoutConfig`, `PositionedNode`, `invariant`. Per-node direction, subtree width/height, child X/Y, Z-shape and trunk-and-branch connection paths. |
| `apps/web/src/components/flow/path-commands.ts` | `Point`, `move` / `line` / `curve`, `PathCommand`, `renderPath(commands)` → SVG `d` string. |
| `apps/web/src/components/flow/GridPattern.tsx` | SVG `<defs><pattern id="flow-dot-grid">` with one circle per cell; animated `r` and `opacity`. Large rect filled with pattern. Props: `gridSize`, `dotRadius`, `dotClassName`. |
| `apps/web/src/components/flow/InfiniteCanvas.tsx` | SVG ref, state `{ scale, offset }`, pan (mousedown/move/up, startPanRef), wheel zoom (preventDefault, zoom toward cursor), momentum. Renders `<svg>` → `<g transform>`, `GridPattern`, children; optional `overlay` div. Defaults: `gridSize=24`, `dotRadius=0.8`. |
| `apps/web/src/components/flow/TreeElementNode.tsx` | `foreignObject` at `position`, inner div centered with `transform: translate(-50%, -50%)`, `data-node-id={id}`. |
| `apps/web/src/components/flow/TreeConnectionLine.tsx` | Build path from `Point[]` with rounded corners (path-commands); `<path>` with `stroke-dasharray` + `<animate stroke-dashoffset>` for flowing dots. |
| `apps/web/src/components/flow/TreeLayout.tsx` | Accepts `data: FlowNode`, `renderNode`, optional `renderConnection`, `nodeSpacing`, `layoutConfig`. Creates `LayoutEngine`, sets dimensions from `NODE_SIZES`, calls `calculate(data)`, renders connections then nodes; click delegation via `data-node-id`. |
| `apps/web/src/components/flow/types.ts` | `FlowNode` (extends TreeNode with `label`, `metadata?: { type: FlowNodeType }`), `FlowNodeType` ('root' \| 'leaf'), `NODE_SIZES` (root: 140×36, leaf: 180×56). |
| `apps/web/src/components/flow/index.ts` | Re-exports: InfiniteCanvas, GridPattern, TreeLayout, TreeElementNode, TreeConnectionLine, layout-engine types + LayoutEngine + invariant, FlowNode/FlowNodeType/NODE_SIZES. |

### Files changed

| File | Change |
|------|--------|
| `apps/web/src/routes/_app/index.tsx` | Removed lazy `NodeCanvas` and Suspense. Now renders `InfiniteCanvas` → `TreeLayout` with static `HOME_TREE` (root "OpenAgents", children "Runtime", "Agents", "Protocol"), `nodeSpacing={{ x: 24, y: 60 }}`, `layoutConfig={{ direction: 'vertical' }}`, `renderNode` returning a simple card (rounded border, bg-card, label). |
| `apps/web/package.json` | Removed `@react-three/fiber`, `@react-three/drei`, `three`. |
| `apps/web/bun.lock` | Updated after dependency removal. |

### Files removed

| File | Reason |
|------|--------|
| `apps/web/src/components/three/NodeCanvas.tsx` | Replaced by flow (InfiniteCanvas + TreeLayout). |
| `apps/web/src/components/three/Nodes.tsx` | Replaced by flow tree + connection lines. |

### Grid defaults (post-tweak)

- **Grid size:** 24px (dots further apart).
- **Dot radius:** 0.8 (smaller dots).

### Deploy and push

- **Deploy:** Ran `bun run deploy` from `apps/web` (Vite build + `wrangler deploy`). Deployed to Cloudflare Workers at `https://openagents-web-app.openagents.workers.dev`.
- **Commit and push:** All changes committed on branch `flow` and pushed to `origin/flow` (PR link: `https://github.com/OpenAgentsInc/openagents/pull/new/flow`).

---

## Current OpenAgents setup

- **Homepage:** `apps/web/src/routes/_app/index.tsx` — Renders `InfiniteCanvas` → `TreeLayout` with static tree and card-style `renderNode`.
- **Flow module:** `apps/web/src/components/flow/` — Layout engine, path commands, InfiniteCanvas, GridPattern, TreeLayout, TreeElementNode, TreeConnectionLine, types, index. No CanvasBoundary (error boundary omitted).
- **Removed:** `apps/web/src/components/three/NodeCanvas.tsx`, `Nodes.tsx`; deps `@react-three/fiber`, `@react-three/drei`, `three`.

---

## Architecture (high level)

- **Data flow:** Static tree (root + children) → `TreeLayout` → `LayoutEngine.calculate()` → positioned nodes + connection paths → render connections, then nodes via `renderNode`; canvas provides pan/zoom and grid.
- **No Unkey code copy:** Same contracts (layout engine API, canvas state, path commands, tree layout props); OpenAgents-specific node types and styling in `flow/types.ts`.

---

## Implementation order (as executed)

1. Added `flow/layout-engine.ts` and `flow/path-commands.ts` (pure TS).
2. Added `flow/GridPattern.tsx`, `flow/InfiniteCanvas.tsx` (SVG canvas; no CanvasBoundary).
3. Added `flow/TreeElementNode.tsx`, `flow/TreeConnectionLine.tsx`, `flow/TreeLayout.tsx`, `flow/types.ts` with `FlowNode` and `NODE_SIZES`.
4. Wired homepage: static tree (root + Runtime, Agents, Protocol) + `InfiniteCanvas` + `TreeLayout` + simple card `renderNode`.
5. Removed `components/three/*` and Three.js deps; added this doc.
6. Adjusted grid: smaller dots (radius 0.8), more spacing (grid 24).
7. Committed all and pushed branch `flow`.

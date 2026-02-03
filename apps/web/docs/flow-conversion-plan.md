# Homepage flow: Three.js to SVG graph conversion

## Where Unkey's flow lives

Unkey's flow implementation is under a single feature subtree:

**Path:** `web/apps/dashboard/app/(app)/[workspaceSlug]/projects/[projectId]/(overview)/deployments/[deploymentId]/network/unkey-flow/`

**Layout and engine (pure logic, no React):**

- `layout-engine.ts` — `LayoutEngine<T>`, `TreeNode`, `Point`, `LayoutConfig`. Tree flattening, per-node `direction` (horizontal/vertical), subtree width/height, child X/Y positioning, connection path building (Z-shape for horizontal, trunk-and-branch for vertical). Fail-fast `invariant()` for missing dimensions.

**Canvas (SVG pan/zoom):**

- `components/canvas/infinite-canvas.tsx` — State `{ scale, offset }`, `transform` on `<g>`, mouse pan (startPanRef gap), wheel zoom (prevent default), optional momentum. Renders `GridPattern` + `CanvasBoundary` + `children`; optional `overlay` in a sibling div.
- `components/canvas/grid-pattern.tsx` — SVG `<pattern>` with animated circles (radius + opacity), large rect filled with pattern.
- `components/canvas/canvas-boundary.tsx` — Error boundary wrapping children (Unkey-specific UI; we use a simple error boundary or omit initially).

**Tree UI:**

- `components/tree/tree-layout.tsx` — Creates `LayoutEngine`, sets dimensions from a `NODE_SIZES`-like map, calls `calculate(data)`, renders connections then `TreeElementNode` for each positioned node; `renderNode(node, parent)` and optional `renderConnection(path, parent, child)`.
- `components/tree/tree-element-node.tsx` — `<foreignObject>` at `position`, inner div `transform: translate(-50%, -50%)`, `data-node-id` for click delegation.
- `components/tree/tree-connection-line.tsx` — Builds SVG path from `Point[]` with rounded corners via `tree-path-command.ts` (move/line/curve, `renderPath`). Animated stroke via `stroke-dasharray` + `<animate attributeName="stroke-dashoffset">`.
- `components/tree/tree-path-command.ts` — Types and helpers: `MoveTo`, `LineTo`, `QuadraticCurve`, `renderPath(commands)` → SVG `d` string.

**Node types and data:** Unkey uses `components/nodes/types.ts` (e.g. `OriginNode`, `SentinelNode`, `InstanceNode`) and `NODE_SIZES` by `metadata.type`. The view is wired in `deployment-network-view.tsx`: `InfiniteCanvas` → `TreeLayout` with `data`, `renderNode`, `renderConnection`, overlay (details panel, dev tree generator).

---

## Current OpenAgents setup (post-conversion)

- **Homepage:** `apps/web/src/routes/_app/index.tsx` — Renders `InfiniteCanvas` → `TreeLayout` with static tree and `renderNode`.
- **Flow module:** `apps/web/src/components/flow/` — Layout engine, path commands, InfiniteCanvas, GridPattern, TreeLayout, TreeElementNode, TreeConnectionLine, types, index.
- **Removed:** `apps/web/src/components/three/NodeCanvas.tsx` and `Nodes.tsx`; deps `@react-three/fiber`, `@react-three/drei`, `three`.

---

## Architecture (high level)

- **Data flow:** Static tree (root + children) → `TreeLayout` → `LayoutEngine.calculate()` → positioned nodes + connection paths → render connections, then nodes via `renderNode`; canvas provides pan/zoom and grid.
- **No Unkey code copy:** Same contracts (layout engine API, canvas state, path commands, tree layout props); OpenAgents-specific node types and styling in `flow/types.ts`.

---

## Implementation summary

1. Added `flow/layout-engine.ts` and `flow/path-commands.ts` (pure TS).
2. Added `flow/GridPattern.tsx`, `flow/InfiniteCanvas.tsx` (SVG canvas; no CanvasBoundary).
3. Added `flow/TreeElementNode.tsx`, `flow/TreeConnectionLine.tsx`, `flow/TreeLayout.tsx`, `flow/types.ts` with `FlowNode` and `NODE_SIZES`.
4. Wired homepage: static tree (root + Runtime, Agents, Protocol) + `InfiniteCanvas` + `TreeLayout` + simple card `renderNode`.
5. Removed `components/three/*` and Three.js deps; added this doc.

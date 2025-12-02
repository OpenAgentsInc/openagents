# 1314 HUD Flow Tasks Log

## New HUD Tasks

Created 6 tasks for the HUD Flow Editor based on `docs/hud/flow.md` spec:

| ID | Title | Priority | Labels |
|---|---|---|---|
| oa-b78d3f | HUD-1: Flow model types + sample data | P1 | hud, flow, model |
| oa-138548 | HUD-2: Flow layout engine (calculateLayout) | P1 | hud, flow, layout |
| oa-91a779 | HUD-3: Flow connection path builder | P2 | hud, flow, path |
| oa-5fc569 | HUD-4: Canvas state reducer (pan/zoom) | P2 | hud, flow, canvas |
| oa-6ee331 | HUD-5: MechaCoder state to FlowNode tree | P2 | hud, flow, mechacoder |
| oa-4e4f76 | HUD-6: Minimal SVG host renderer for Flow layout | P3 | hud, flow, host, svg |

## Task Descriptions

### HUD-1: Flow model types + sample data (P1)
Define FlowNode/PositionedNode/Connection/Point and related types in src/flow/model.ts. Add src/flow/sample-data.ts with a MechaCoder factory tree matching the spec.

### HUD-2: Flow layout engine (P1)
Implement src/flow/layout.ts with calculateLayout(LayoutInput): LayoutOutput. Compute subtree sizes and positions, produce nodes + connections. Fail-fast on missing sizes or invalid trees.

### HUD-3: Flow connection path builder (P2)
Implement src/flow/path.ts with buildRoundedPath(points, config) -> SVG path string. Detect corners and apply cornerRadius.

### HUD-4: Canvas state reducer (P2)
Implement src/flow/canvas.ts with CanvasState/CanvasEvent/CanvasConfig and reduceCanvasState. Support PAN/ZOOM/RESET and basic inertial panning.

### HUD-5: MechaCoder state to FlowNode tree (P2)
Implement src/flow/mechacoder-map.ts. Define MechaCoderState and buildMechaCoderFlowTree(state): FlowNode.

### HUD-6: Minimal SVG host renderer (P3)
Implement src/flow-host-svg/render.ts that takes LayoutOutput + CanvasState and returns a minimal SVG representation.

## Notes

- Source spec: `docs/hud/flow.md`
- These tasks are intended for MechaCoder / future agent runs to implement
- Implementation order: HUD-1 -> HUD-2 -> (HUD-3, HUD-4, HUD-5 in parallel) -> HUD-6
- All modules should be pure TypeScript with Effect, no React in core

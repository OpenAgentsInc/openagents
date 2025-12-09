# Plan: Replace Three.js with Unit-Based Agent Graph

## Goal

Replace the Three.js visualization in `src/effuse/components/three-background.ts` with an SVG-based graph component that harvests rendering primitives from Unit (`~/code/unit`) and adapts them to Effuse Effect-native patterns.

**Current State**: 8 ATIF nodes rendered in 3D orthographic view with animated connections
**Target State**: SVG-based agent node graph using Unit's layout/rendering code, fully Effuse-native

## Discovery: Existing Flow Framework vs Unit

**Important Finding**: OpenAgents already has a graph rendering framework:
- `/Users/christopherdavid/code/openagents/src/flow/` - Layout, canvas, path utilities
- `/Users/christopherdavid/code/openagents/src/flow-host-svg/` - SVG rendering (610 lines)

**Question for User**: Should I harvest from:
- **A**: Unit framework at `~/code/unit` (as originally requested)
- **B**: Existing Flow framework in OpenAgents codebase
- **C**: Hybrid approach (some from each)

**Recommendation**: Option B (Flow) is faster, already integrated, but Option A (Unit) may have better features.

## Current Three.js Implementation

**File**: `src/effuse/components/three-background.ts`
- Renders 8 ATIF type nodes (Trajectory, Step, Agent, ToolCall, Observation, Metrics, Checkpoint, SubagentRef)
- Fixed grid positions, animated dotted connections
- Interactive: hover, click, cursor changes
- Uses Three.js orthographic camera, HTML label overlays
- Stateless Effuse component with cleanup

## Key Decisions Needed

### 1. Source Framework Choice
- **Flow** (existing in codebase): Faster, integrated, proven
- **Unit** (~/code/unit): More features, physics simulation, more complex

### 2. Node Data
- **Static ATIF nodes**: Keep current 8 educational nodes
- **Dynamic agents**: Show actual runtime agent instances
- **Generic**: Configurable node data source

### 3. Layout Strategy
- **Fixed positions**: Preserve current grid (simplest)
- **Force-directed**: Use Unit's physics simulation
- **Hierarchical**: Use Flow's tree layout
- **Hybrid**: Toggle between layouts

### 4. Visual Style
- **Preserve Factorio aesthetic**: Dark squares, dotted lines, industrial look
- **Or adopt Unit/Flow styles**: Their existing visual language

## Proposed Architecture

### Component Structure

```
src/effuse/components/agent-graph/
├── index.ts                    # Exports
├── agent-graph-component.ts    # Main Effuse component
├── types.ts                    # State, events, node data
├── render.ts                   # SVG rendering (harvested code)
├── layout.ts                   # Position calculation
├── geometry.ts                 # Shapes, connections (from Unit)
├── styles.ts                   # Factorio theme
└── agent-graph-component.test.ts
```

### State Shape

```typescript
interface AgentGraphState {
  nodes: GraphNode[]              // 8 ATIF nodes or dynamic agents
  connections: GraphConnection[]  // Data flow edges
  layout: "fixed" | "force" | "tree"
  canvas: { pan: Point; zoom: number; viewport: Rect }
  hoveredNodeId: string | null
  selectedNodeId: string | null
  animationFrame: number          // For flowing dash animation
}
```

### Event Types

```typescript
type AgentGraphEvent =
  | { type: "nodeClick"; nodeId: string }
  | { type: "nodeHover"; nodeId: string | null }
  | { type: "nodeDragStart"; nodeId: string; point: Point }
  | { type: "nodeDragMove"; point: Point }
  | { type: "nodeDragEnd" }
  | { type: "canvasPan"; delta: Point }
  | { type: "canvasZoom"; delta: number; pointer: Point }
  | { type: "layoutChange"; layout: LayoutType }
  | { type: "animationTick" }
```

## Implementation Approach

### Phase 1: Foundation (Replace Three.js)

**Goal**: SVG-based graph with exact current behavior

**Tasks**:
1. Create agent-graph directory structure
2. Harvest geometry utilities from Unit:
   - `~/code/unit/src/client/util/geometry/` - Point math, shapes, connections
   - `~/code/unit/src/system/platform/component/svg/` - SVG element types
3. Create Effuse component:
   - `initialState()`: 8 ATIF nodes, 7 connections, fixed positions
   - `render()`: Generate SVG string with nodes + connections
   - `setupEvents()`: Animation loop, mouse hover/click
   - `handleEvent()`: Update state on interactions
4. Apply Factorio styling:
   - Dark node backgrounds (#050505)
   - White borders (0.9 opacity)
   - Dotted gray connections with animation
   - Grid background pattern
5. Replace `three-background.ts` in `new-main.ts`

**Files to Harvest from Unit**:
- `~/code/unit/src/client/util/geometry/describeRect.ts` - Node shapes
- `~/code/unit/src/client/util/geometry/unitVector.ts` - Connection math
- `~/code/unit/src/client/util/geometry/pointInNode.ts` - Hit detection
- `~/code/unit/src/system/platform/component/svg/Path/Component.ts` - SVG paths
- `~/code/unit/src/system/platform/component/svg/Group/Component.ts` - SVG groups

**Adaptation Strategy**:
- Copy utility functions as-is (pure math)
- Adapt SVG components to Effuse `html` template strings
- Replace Unit's event emitters with Effect Streams
- Use StateCell for reactive state updates

**Validation**:
- ✓ 8 nodes render in same positions as Three.js
- ✓ Dotted connections animate smoothly
- ✓ Hover highlights nodes
- ✓ Click detection works
- ✓ No Three.js dependencies remain

### Phase 2: Canvas Interactions

**Goal**: Add pan/zoom capabilities

**Tasks**:
1. Harvest canvas state from Unit or Flow
2. Add mouse handlers for pan (drag background)
3. Add wheel handler for zoom
4. Apply SVG transform for viewport
5. Add minimap (optional, like Unit's Minimap component)

**Files to Consider**:
- `~/code/unit/src/client/simulation.ts` - If using physics
- `src/flow/canvas.ts` - If using existing Flow
- `~/code/unit/src/system/platform/component/app/Minimap/Component.ts` - For minimap

### Phase 3: Dynamic Data (If Needed)

**Goal**: Show actual agent instances, not static ATIF nodes

**Tasks**:
1. Define agent data schema
2. Subscribe to socket messages (agent events)
3. Update nodes on agent state changes
4. Handle node add/remove dynamically

**Dependencies**:
- Agent registry/tracking system
- HudMessage types for agent events
- Socket subscription pattern (like apm-widget.ts)

### Phase 4: Advanced Layouts (Optional)

**Goal**: Toggle between fixed/force-directed/tree layouts

**Tasks**:
1. Integrate Unit's force-directed simulation:
   - `~/code/unit/src/client/simulation.ts` (Simulation class)
   - `~/code/unit/src/system/platform/component/app/Minigraph/Component.ts` (force function)
2. Or use Flow's hierarchical layout:
   - `src/flow/layout.ts` (tree positioning)
3. Add layout toggle UI
4. Animate transitions between layouts

## Code Harvest Inventory

### From Unit Framework (`~/code/unit`)

| File | Purpose | Lines | Harvest Strategy |
|------|---------|-------|------------------|
| `src/client/util/geometry/describeRect.ts` | SVG rect paths | ~20 | Copy unchanged |
| `src/client/util/geometry/unitVector.ts` | Direction vectors | ~10 | Copy unchanged |
| `src/client/util/geometry/pointInNode.ts` | Hit testing | ~30 | Copy unchanged |
| `src/client/util/geometry/surfaceDistance.ts` | Connection routing | ~40 | Copy unchanged |
| `src/system/platform/component/svg/Path/Component.ts` | SVG path rendering | ~150 | Adapt to Effuse html template |
| `src/system/platform/component/svg/Group/Component.ts` | SVG groups | ~100 | Adapt to Effuse html template |
| `src/client/simulation.ts` | Force-directed layout | ~400 | Phase 4 only, adapt physics |
| `src/system/platform/component/app/Minigraph/Component.ts` | Graph visualization | ~300 | Reference for layout logic |

### From Existing Flow (`src/flow`)

| File | Purpose | Lines | Alternative to Unit |
|------|---------|-------|---------------------|
| `src/flow/path.ts` | Rounded paths | 74 | Use instead of Unit's path utils |
| `src/flow/canvas.ts` | Pan/zoom state | 185 | Use instead of Unit's Minimap |
| `src/flow-host-svg/render.ts` | SVG rendering | 610 | Use SVG type definitions |
| `src/flow/layout.ts` | Tree layout | 197 | Use for hierarchical mode |

## Critical Files

### New Files to Create

```
src/effuse/components/agent-graph/
├── index.ts                           # Public exports
├── agent-graph-component.ts           # Main Effuse component
├── types.ts                           # State/Event/Node types
├── render.ts                          # SVG rendering logic
├── layout.ts                          # Position calculations
├── geometry.ts                        # Unit geometry utilities
├── styles.ts                          # Factorio theme constants
└── agent-graph-component.test.ts      # Unit + integration tests
```

### Files to Modify

```
src/effuse/
├── index.ts                           # Add AgentGraphComponent export
└── components/
    └── three-background.ts            # DELETE or archive

src/mainview/
├── new.html                           # Update container ID if needed
└── new-main.ts                        # Import AgentGraphComponent
```

## Document Structure

## Testing Strategy

### Unit Tests
- Geometry utilities (pure functions)
- SVG rendering (string output)
- State updates (Effect tests)

### Integration Tests
- Component mounting with makeHappyDomLayer()
- Node hover/click interactions
- Animation loop lifecycle

### Visual Regression
- Screenshot comparison (e2e)
- Animation smoke tests
- Layout consistency

## Migration Strategy

### Parallel Development
1. Keep three-background.ts unchanged
2. Build agent-graph alongside
3. Test in isolation
4. Use feature flag or separate HTML page

### Cutover
1. Update import in `new-main.ts`
2. Update exports in `effuse/index.ts`
3. Archive three-background.ts
4. Remove Three.js if unused elsewhere

## Risk Assessment

**Risks**:
1. **Visual Regression**: New graph looks different from Three.js
   - Mitigation: Match positions/colors exactly in Phase 1
2. **Performance**: SVG slower than WebGL for many nodes
   - Mitigation: Start with 8 nodes, optimize if needed
3. **Animation Jank**: 60fps SVG updates challenging
   - Mitigation: Update transform only, not full re-render
4. **Complex Unit Code**: Hard to adapt event-driven code to Effect
   - Mitigation: Start with utilities only, skip complex components

**Success Criteria**:
- ✓ Visual parity with Three.js version
- ✓ Smooth 60fps animation
- ✓ No Three.js dependencies
- ✓ Fully Effuse-native (StateCell, Effect, proper cleanup)
- ✓ Interactive (hover, click)
- ✓ Tested (unit + integration)

## Finalized Design Decisions

### 1. Source Framework: Unit Framework
- Harvest from `~/code/unit`
- Use physics simulation for force-directed layout
- Adapt Unit's SVG rendering to Effuse patterns

### 2. Node Data: Static ATIF Nodes
- Keep current 8 educational nodes (Trajectory, Step, Agent, ToolCall, Observation, Metrics, Checkpoint, SubagentRef)
- No backend integration needed
- Simplifies initial implementation

### 3. Layout: Force-Directed Physics
- Use Unit's Simulation class (Runge-Kutta integration)
- Nodes will auto-arrange with physics
- More visually interesting than fixed grid

### 4. Scope: Phase 1+2 (Interactive)
- Phase 1: Replace Three.js with SVG + physics layout (~6 hours)
- Phase 2: Add pan/zoom interactions (~3 hours)
- Total: ~9 hours implementation
- Skip Phase 3 (dynamic data) and Phase 4 (layout toggle)

### 5. Visual Style: Factorio Aesthetic
- Preserve dark squares (#050505)
- White borders (0.9 opacity)
- Dotted gray connections with animation
- Industrial, technical look

## Final Implementation Plan

### Phase 1: SVG Graph with Physics Layout (6 hours)

**Step 1.1: Create Directory Structure**
```bash
mkdir -p src/effuse/components/agent-graph
touch src/effuse/components/agent-graph/{index.ts,agent-graph-component.ts,types.ts,render.ts,simulation.ts,geometry.ts,styles.ts}
```

**Step 1.2: Harvest Unit Geometry Utilities**

Copy from `~/code/unit/src/client/util/geometry/`:
- `describeRect.ts` → `geometry.ts` (node shapes)
- `unitVector.ts` → `geometry.ts` (direction vectors)
- `pointInNode.ts` → `geometry.ts` (hit detection)
- `surfaceDistance.ts` → `geometry.ts` (connection endpoints)

These are pure math functions - copy unchanged.

**Step 1.3: Adapt Unit's Simulation Class**

Harvest from `~/code/unit/src/client/simulation.ts`:
- Copy `Simulation<N, L>` class structure
- Adapt to use Effect instead of EventEmitter
- Simplify to RK1 or RK2 (RK4 is overkill for 8 nodes)
- Remove unnecessary features (just need basic force-directed)

Key adaptations:
```typescript
// Unit's EventEmitter pattern
this.emit('tick')

// Becomes Effect Stream pattern
yield* queue.offer({ type: "tick" })
```

**Step 1.4: Create SimNode State**

```typescript
interface SimNode {
  id: string
  label: string
  x: number          // Position
  y: number
  vx: number         // Velocity
  vy: number
  ax: number         // Acceleration
  ay: number
  fx?: number        // Fixed position (if constrained)
  fy?: number
  shape: "rect"
  width: number      // 120
  height: number     // 80
}
```

**Step 1.5: Implement Force Function**

Harvest force logic from `~/code/unit/src/system/platform/component/app/Minigraph/Component.ts` lines 81-155:
- Repulsive forces between nodes (inversely proportional to distance)
- Attractive link forces for connected nodes
- Damping toward center

**Step 1.6: Create SVG Renderer**

```typescript
const renderNode = (node: SimNode, hovered: boolean): string => {
  const fill = hovered ? "#0a0a0a" : "#050505"
  const strokeOpacity = hovered ? 1.0 : 0.9

  return `
    <rect
      x="${node.x - node.width/2}"
      y="${node.y - node.height/2}"
      width="${node.width}"
      height="${node.height}"
      fill="${fill}"
      stroke="white"
      stroke-width="2"
      stroke-opacity="${strokeOpacity}"
      data-node-id="${node.id}"
      style="cursor: pointer;"
    />
    <text
      x="${node.x}"
      y="${node.y}"
      text-anchor="middle"
      dominant-baseline="middle"
      fill="white"
      font-family="Berkeley Mono"
      font-size="11"
      pointer-events="none"
    >
      ${node.label}
    </text>
  `
}

const renderConnection = (from: SimNode, to: SimNode, dashOffset: number): string => {
  const fx = from.x + from.width/2
  const fy = from.y
  const tx = to.x - to.width/2
  const ty = to.y

  return `
    <line
      x1="${fx}" y1="${fy}"
      x2="${tx}" y2="${ty}"
      stroke="#999999"
      stroke-width="2"
      stroke-opacity="0.6"
      stroke-dasharray="8 6"
      stroke-dashoffset="${dashOffset}"
    />
  `
}
```

**Step 1.7: Effuse Component Structure**

```typescript
export const AgentGraphComponent: Component<AgentGraphState, AgentGraphEvent> = {
  id: "agent-graph",

  initialState: () => ({
    nodes: createATIFNodes(),        // 8 nodes with initial positions
    connections: createATIFConnections(), // 7 connections
    hoveredNodeId: null,
    animationFrame: 0,
    simulationRunning: true,
  }),

  render: (ctx) => Effect.gen(function* () {
    const state = yield* ctx.state.get

    return html`
      <svg
        id="agent-graph-svg"
        width="100%"
        height="100%"
        style="position: absolute; inset: 0;"
      >
        <!-- Grid pattern -->
        <defs>
          <pattern id="grid" width="24" height="24" patternUnits="userSpaceOnUse">
            <circle cx="12" cy="12" r="1.2" fill="white" opacity="0.08"/>
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#grid)"/>

        <!-- Connections -->
        ${state.connections.map(conn => {
          const from = state.nodes.find(n => n.id === conn.from)!
          const to = state.nodes.find(n => n.id === conn.to)!
          return renderConnection(from, to, state.animationFrame)
        }).join('')}

        <!-- Nodes -->
        ${state.nodes.map(node =>
          renderNode(node, node.id === state.hoveredNodeId)
        ).join('')}
      </svg>
    `
  }),

  setupEvents: (ctx) => Effect.gen(function* () {
    const svg = yield* ctx.dom.queryId<SVGSVGElement>("agent-graph-svg")

    // Start physics simulation
    const simulation = new Simulation(
      ctx.state.nodes,
      ctx.state.connections,
      forceFunction
    )

    // Animation loop
    let animationId: number
    const animate = () => {
      simulation.tick()  // Update physics

      Effect.runFork(
        ctx.state.update(s => ({
          ...s,
          nodes: simulation.nodes,  // Updated positions
          animationFrame: (s.animationFrame + 1) % 1000,
        }))
      )

      animationId = requestAnimationFrame(animate)
    }
    animate()

    // Mouse hover detection
    svg.addEventListener("mousemove", (e) => {
      const rect = svg.getBoundingClientRect()
      const x = e.clientX - rect.left
      const y = e.clientY - rect.top

      const hoveredNode = findNodeAtPoint(ctx.state.nodes, x, y)
      Effect.runFork(
        ctx.emit({ type: "nodeHover", nodeId: hoveredNode?.id || null })
      )
    })

    // Click detection
    yield* ctx.dom.delegate(svg, "[data-node-id]", "click", (_e, target) => {
      const nodeId = (target as SVGElement).dataset.nodeId!
      Effect.runFork(ctx.emit({ type: "nodeClick", nodeId }))
    })

    // Cleanup
    return Effect.sync(() => {
      cancelAnimationFrame(animationId)
      simulation.stop()
    })
  }),

  handleEvent: (event, ctx) => Effect.gen(function* () {
    switch (event.type) {
      case "nodeHover":
        yield* ctx.state.update(s => ({ ...s, hoveredNodeId: event.nodeId }))
        break

      case "nodeClick":
        console.log("Clicked node:", event.nodeId)
        // TODO: Emit to parent or show details
        break
    }
  }),
}
```

**Step 1.8: Initial Node Data**

```typescript
const createATIFNodes = (): SimNode[] => [
  { id: "trajectory", label: "Trajectory", x: 100, y: 100, vx: 0, vy: 0, ax: 0, ay: 0, shape: "rect", width: 120, height: 80 },
  { id: "step", label: "Step", x: 300, y: 100, vx: 0, vy: 0, ax: 0, ay: 0, shape: "rect", width: 120, height: 80 },
  { id: "agent", label: "Agent", x: 500, y: 100, vx: 0, vy: 0, ax: 0, ay: 0, shape: "rect", width: 120, height: 80 },
  // ... 5 more nodes
]

const createATIFConnections = (): GraphConnection[] => [
  { from: "trajectory", to: "step" },
  { from: "step", to: "agent" },
  // ... 5 more connections
]
```

**Validation Checklist**:
- ✓ 8 nodes render as dark rectangles
- ✓ Nodes auto-arrange with physics
- ✓ 7 dotted connections animate
- ✓ Hover highlights nodes
- ✓ Click logs node ID
- ✓ Simulation stops on unmount (no memory leaks)

---

### Phase 2: Pan & Zoom Interactions (3 hours)

**Step 2.1: Add Canvas State**

```typescript
interface AgentGraphState {
  // ... existing fields
  canvas: {
    pan: { x: number; y: number }
    zoom: number
    viewport: { width: number; height: number }
  }
}
```

**Step 2.2: Harvest Canvas Logic from Unit**

Option A: Copy `~/code/unit/src/flow/canvas.ts` event reducers
Option B: Implement from scratch (simpler for MVP)

Going with Option B for simplicity:

```typescript
// In setupEvents
let isPanning = false
let lastMouse = { x: 0, y: 0 }

svg.addEventListener("mousedown", (e) => {
  if (e.button === 0 && !e.target.dataset.nodeId) {  // Left click on background
    isPanning = true
    lastMouse = { x: e.clientX, y: e.clientY }
    svg.style.cursor = "grabbing"
  }
})

document.addEventListener("mousemove", (e) => {
  if (isPanning) {
    const dx = e.clientX - lastMouse.x
    const dy = e.clientY - lastMouse.y

    Effect.runFork(ctx.emit({ type: "canvasPan", delta: { x: dx, y: dy } }))

    lastMouse = { x: e.clientX, y: e.clientY }
  }
})

document.addEventListener("mouseup", () => {
  isPanning = false
  svg.style.cursor = "default"
})

svg.addEventListener("wheel", (e) => {
  e.preventDefault()
  const delta = e.deltaY > 0 ? 0.9 : 1.1  // Zoom out/in
  const pointer = { x: e.clientX, y: e.clientY }

  Effect.runFork(ctx.emit({ type: "canvasZoom", delta, pointer }))
})
```

**Step 2.3: Handle Canvas Events**

```typescript
handleEvent: (event, ctx) => Effect.gen(function* () {
  switch (event.type) {
    case "canvasPan":
      yield* ctx.state.update(s => ({
        ...s,
        canvas: {
          ...s.canvas,
          pan: {
            x: s.canvas.pan.x + event.delta.x / s.canvas.zoom,
            y: s.canvas.pan.y + event.delta.y / s.canvas.zoom,
          }
        }
      }))
      break

    case "canvasZoom":
      yield* ctx.state.update(s => {
        const newZoom = Math.max(0.25, Math.min(4, s.canvas.zoom * event.delta))
        // TODO: Zoom toward pointer position (not implemented in MVP)
        return {
          ...s,
          canvas: { ...s.canvas, zoom: newZoom }
        }
      })
      break
  }
})
```

**Step 2.4: Apply SVG Transform**

```typescript
render: (ctx) => Effect.gen(function* () {
  const state = yield* ctx.state.get
  const { pan, zoom } = state.canvas

  return html`
    <svg ...>
      <g transform="translate(${pan.x},${pan.y}) scale(${zoom})">
        <!-- All nodes and connections here -->
      </g>
    </svg>
  `
})
```

**Validation Checklist**:
- ✓ Click-drag background pans viewport
- ✓ Scroll wheel zooms in/out
- ✓ Zoom respects min (0.25x) and max (4x) bounds
- ✓ Pan updates smoothly at 60fps
- ✓ Node interactions still work after pan/zoom

---

### Migration from Three.js

**Step 1: Update new-main.ts**

```typescript
// Before
import { ThreeBackgroundComponent } from "../effuse/index.js"
import { IntroCardComponent } from "../effuse/index.js"

const threeContainer = document.getElementById("three-background-container")
const introContainer = document.getElementById("intro-card-container")

Effect.runPromise(
  Effect.all([
    mountComponent(ThreeBackgroundComponent, threeContainer!),
    mountComponent(IntroCardComponent, introContainer!),
  ]).pipe(Effect.provide(EffuseLive))
)

// After
import { AgentGraphComponent } from "../effuse/index.js"
import { IntroCardComponent } from "../effuse/index.js"

const graphContainer = document.getElementById("three-background-container")  // Reuse container
const introContainer = document.getElementById("intro-card-container")

Effect.runPromise(
  Effect.all([
    mountComponent(AgentGraphComponent, graphContainer!),
    mountComponent(IntroCardComponent, introContainer!),
  ]).pipe(Effect.provide(EffuseLive))
)
```

**Step 2: Update effuse/index.ts**

```typescript
// Add export
export { AgentGraphComponent } from "./components/agent-graph/index.js"
```

**Step 3: Archive or Delete**

```bash
git mv src/effuse/components/three-background.ts src/effuse/components/_archived/
# Or just delete if we're confident
```

**Step 4: Remove Three.js Dependency**

Check if Three.js is used elsewhere:
```bash
grep -r "three" src/ --include="*.ts" --include="*.tsx"
```

If only three-background.ts, remove from package.json:
```bash
bun remove three @types/three
```

---

## Summary: What Gets Built

### New Component Structure
```
src/effuse/components/agent-graph/
├── index.ts                           # Exports AgentGraphComponent
├── agent-graph-component.ts           # Main component (State, Events, Render, Setup)
├── types.ts                           # AgentGraphState, AgentGraphEvent, SimNode, etc.
├── simulation.ts                      # Adapted from Unit's Simulation class
├── geometry.ts                        # Harvested from Unit (describeRect, unitVector, pointInNode, surfaceDistance)
├── render.ts                          # renderNode(), renderConnection(), renderGraph()
└── styles.ts                          # Factorio theme constants (colors, sizes)
```

### Features Delivered
- ✅ 8 ATIF nodes with force-directed physics layout
- ✅ Animated dotted connection lines
- ✅ Hover highlighting
- ✅ Click detection (logs to console)
- ✅ Pan (click-drag background)
- ✅ Zoom (scroll wheel, 0.25x to 4x)
- ✅ Factorio aesthetic (dark nodes, white borders, industrial look)
- ✅ Smooth 60fps animation
- ✅ Proper cleanup (no memory leaks)
- ✅ Fully Effuse-native (Effect, StateCell, html templates)

### Time Estimate
- Phase 1 (SVG + Physics): ~6 hours
- Phase 2 (Pan/Zoom): ~3 hours
- **Total**: ~9 hours

### Not Included (Future Work)
- ❌ Dynamic agent data (Phase 3)
- ❌ Layout toggle (Phase 4)
- ❌ Drag-drop nodes
- ❌ Minimap component
- ❌ Node details panel
- ❌ Connection editing

---

## Appendix: Unit Files to Harvest

### Confirmed Files from ~/code/unit

**Geometry Utilities** (copy unchanged):
- `src/client/util/geometry/describeRect.ts`
- `src/client/util/geometry/unitVector.ts`
- `src/client/util/geometry/pointInNode.ts`
- `src/client/util/geometry/surfaceDistance.ts`

**Physics Simulation** (adapt to Effect):
- `src/client/simulation.ts` (~400 lines, Simulation class)

**Force Function Reference** (logic only, don't copy component):
- `src/system/platform/component/app/Minigraph/Component.ts` (lines 81-155)

**SVG Types** (reference only):
- `src/system/platform/component/svg/Path/Component.ts`
- `src/system/platform/component/svg/Group/Component.ts`

---

## Next Steps After Plan Approval

1. Create agent-graph directory
2. Copy geometry utilities from Unit
3. Adapt Simulation class
4. Implement Effuse component
5. Test in isolation
6. Update new-main.ts
7. Remove Three.js
8. Validate visual parity

## Not Needed in Plan (Reference)

#### 2.1 Pin System
**Source:** `~/code/unit/src/Pin.ts` (360 lines)

Analyze:
- Pin as reactive data conduit (not just a value container)
- Three pin types: data (copied), ref (shared), constant (persistent)
- Pin lifecycle: idle → start → data → drop → end
- Event system: 'data', 'drop', 'invalid', 'start', 'end'
- Snapshot/restore for state preservation
- `take()` vs `pull()` vs `peak()` semantics

Code examples:
```typescript
// Current Unit implementation
const pin = new Pin({ data: 42, constant: true }, system)
pin.push(100)
const value = pin.take() // Consumes data
```

**Valuable for Effuse:**
- Constant pins for persistent widget configuration
- Ref pins for shared mutable state (theme, global config)
- Snapshot/restore for HMR state preservation
- Event-driven reactivity (can adapt to Effect Streams)

#### 2.2 Graph Composition
**Source:** `~/code/unit/src/Class/Graph/index.ts` (thousands of lines)

Analyze:
- Graph extends Component (units contain units)
- `_unit: Dict<Unit>` - child unit registry
- `_merge: Record<string, Merge>` - connections between unit pins
- Graph spec manipulation (addUnit, removeUnit, addMerge, plugPin, etc.)
- Parent-child relationships via spec
- Slot system for named content regions

Code examples:
```typescript
// Unit Graph composition
const graph = new Graph(spec, {}, system)
graph.addUnit('apm', APMUnitSpec)
graph.addUnit('tbControls', TBControlsSpec)
graph.addMerge('apm_to_tb', {
  input: { 'apm/sessionAPM': {} },
  output: { 'tbControls/apmData': {} }
})
```

**Valuable for Effuse:**
- Native parent-child widget composition (fixes current manual patterns)
- Pin connections for widget communication (better than emit/subscribe)
- Spec-based composition (declarative vs imperative)
- Graph manipulation API (runtime composition changes)

#### 2.3 Spec Serialization
**Source:** `~/code/unit/src/spec/fromSpec.ts`, `~/code/unit/src/spec/stringify.ts`

Analyze:
- GraphSpec as JSON representation of Graph
- Spec → Class → Instance pattern
- All state serializable (pins, merges, component tree)
- fromSpec() generates Graph classes dynamically
- Bundle specs include dependencies

Code examples:
```typescript
// Unit spec deserialization
const spec: GraphSpec = {
  id: "dashboard",
  units: {
    apm: { id: "apm-unit", input: { expanded: { data: false } } },
    tb: { id: "tb-controls" }
  },
  merges: { /* connections */ }
}
const DashboardClass = fromSpec(spec, specs)
const dashboard = new DashboardClass(system)
```

**Valuable for Effuse:**
- Widget configuration persistence (save/load widget states)
- Visual programming foundation (JSON specs → visual graph editor)
- HMR with spec snapshots (better than current state cloning)
- Runtime spec manipulation (dynamic widget composition)

### Section 3: Effuse Visual Language Design (6-8 pages)

#### 3.1 Architecture Vision

**Core Principle:** Keep Effect-native foundation, add visual primitives on top.

```
Effuse Visual Language Architecture:

Widget<S, E, R>                    [Current Effuse - KEEP]
    ↓
EffuseGraph<S, E, R>              [NEW - Graph composition]
    ├─ EffusePin<A>               [NEW - Adapted Pin with StateCell]
    ├─ WidgetSpec                 [NEW - Serializable widget config]
    └─ Effect Services            [Current - DomService, SocketService, etc.]

Implementation:
- EffusePin wraps StateCell (Effect.Ref + Queue → Stream)
- EffuseGraph composes Widgets via specs
- WidgetSpec is JSON-serializable Widget configuration
- All reactivity via Effect Streams (NOT event emitters)
```

#### 3.2 EffusePin - Effect-Native Pin Adapter

**Harvest:** Pin concept (data conduit, constant/ref/data types, snapshot/restore)
**Adapt:** Replace event emitters with Effect Streams

```typescript
// New implementation (Effect-native)
interface EffusePin<A> {
  readonly get: Effect.Effect<A | undefined, never>
  readonly set: (value: A) => Effect.Effect<void, never>
  readonly take: () => Effect.Effect<A | undefined, never>  // Consume
  readonly pull: () => Effect.Effect<A | undefined, never>  // Read without consuming
  readonly invalidate: () => Effect.Effect<void, never>
  readonly changes: Stream.Stream<A, never>  // Effect Stream, not event emitter
  readonly constant: boolean  // Persists during backpropagation
  readonly ref: boolean       // Reference vs value semantics
  readonly snapshot: () => Effect.Effect<PinSnapshot<A>, never>
  readonly restore: (state: PinSnapshot<A>) => Effect.Effect<void, never>
}

// Implementation uses StateCell underneath
const makeEffusePin = <A>(
  initial: A | undefined,
  opts: { constant?: boolean; ref?: boolean }
): Effect.Effect<EffusePin<A>, never, Scope.Scope> =>
  Effect.gen(function* () {
    const cell = yield* makeCell(initial)

    return {
      get: cell.get,
      set: cell.set,
      take: Effect.gen(function* () {
        const value = yield* cell.get
        yield* cell.set(undefined)
        return value
      }),
      pull: cell.get,  // Non-destructive read
      changes: cell.changes,
      constant: opts.constant ?? false,
      ref: opts.ref ?? false,
      // ... snapshot/restore
    }
  })
```

**Benefits:**
- Type-safe Effect composition
- Stream-based reactivity (fits current Effuse patterns)
- Constant pins for widget configuration
- Ref pins for shared state
- HMR via snapshot/restore

#### 3.3 EffuseGraph - Composable Widget Trees

**Harvest:** Graph composition, parent-child relationships, merge connections
**Adapt:** Use Effect scoping for lifecycle, Widgets instead of Units

```typescript
interface EffuseGraph<S, E, R> extends Widget<S, E, R> {
  readonly children: Record<string, Widget<any, any, any>>
  readonly pins: Record<string, EffusePin<any>>
  readonly merges: Record<string, GraphMerge>  // Pin connections
  readonly spec: EffuseGraphSpec  // Serializable
}

interface EffuseGraphSpec {
  id: string
  children: Record<string, WidgetSpec>
  pins: Record<string, PinSpec>
  merges: Record<string, MergeSpec>
  component: {
    children: string[]  // Render order
    subComponents?: Record<string, { children: string[] }>  // Slots
  }
}

// Usage example
const DashboardSpec: EffuseGraphSpec = {
  id: "dashboard",
  children: {
    apm: APMWidgetSpec,
    tb: TBControlsSpec,
    mc: MCTasksSpec
  },
  pins: {
    activeTab: { constant: true, data: "dashboard" }
  },
  merges: {
    tab_to_visibility: {
      input: { "activeTab": {} },
      output: { "apm/visible": {}, "tb/visible": {}, "mc/visible": {} }
    }
  },
  component: {
    children: ["apm", "tb", "mc"]
  }
}

const DashboardGraph = makeEffuseGraph(DashboardSpec)
```

**Benefits:**
- Native parent-child composition (no more manual patterns)
- Declarative widget trees (spec-based)
- Pin-based widget communication (better than custom events)
- Runtime composition changes (addChild, removeChild, etc.)

#### 3.4 WidgetSpec - Serializable Widget Configuration

**Harvest:** Spec serialization system, snapshot/restore
**Adapt:** Widget interface → JSON spec, Effect services in layer

```typescript
interface WidgetSpec {
  id: string  // Widget type identifier
  initialState?: unknown  // JSON-serializable initial state
  pins?: Record<string, PinSpec>
  children?: Record<string, WidgetSpec>
  metadata?: {
    position?: { x: number; y: number }
    size?: { width: number; height: number }
    [key: string]: any
  }
}

// Registry of Widget constructors
const WidgetRegistry: Record<string, Widget<any, any, any>> = {
  "apm-widget": APMWidget,
  "tb-controls": TBControlsWidget,
  // etc
}

// Deserialization
const widgetFromSpec = (spec: WidgetSpec): Widget<any, any, any> => {
  const widget = WidgetRegistry[spec.id]
  if (!widget) throw new Error(`Widget not found: ${spec.id}`)

  // Override initialState if provided in spec
  if (spec.initialState) {
    return { ...widget, initialState: () => spec.initialState }
  }
  return widget
}

// Serialization
const widgetToSpec = (widget: Widget<S, E, R>, state: S): WidgetSpec => ({
  id: widget.id,
  initialState: state,
  // ... pins, children, metadata
})
```

**Benefits:**
- Save/load widget configurations (user preferences)
- HMR with full widget tree state
- Visual editor foundation (JSON ↔ UI)
- Runtime widget instantiation from specs

### Section 4: Migration Strategy (4-6 pages)

#### 4.1 Phased Implementation Plan

**Phase 1: Core Primitives (Week 1-2)**
- [ ] Implement EffusePin in `src/effuse/pin/effuse-pin.ts`
  - Wrap StateCell with Pin interface
  - Add constant, ref, take, pull, invalidate
  - Implement snapshot/restore
  - Add comprehensive tests

- [ ] Implement PinSpec serialization in `src/effuse/spec/pin-spec.ts`
  - Pin → JSON spec
  - JSON spec → Pin

**Phase 2: Graph Composition (Week 3-4)**
- [ ] Implement EffuseGraph in `src/effuse/graph/effuse-graph.ts`
  - Parent-child widget composition
  - Pin connections (merges)
  - Graph lifecycle with Effect.scoped
  - Spec-based instantiation

- [ ] Implement GraphSpec system in `src/effuse/spec/graph-spec.ts`
  - EffuseGraphSpec type definitions
  - graphFromSpec() deserialization
  - graphToSpec() serialization

- [ ] Update mount system in `src/effuse/widget/mount.ts`
  - Add mountGraph() for EffuseGraph widgets
  - Handle child widget scoping
  - Preserve existing mountWidget() for simple widgets

**Phase 3: Widget Registry (Week 5)**
- [ ] Create widget registry in `src/effuse/registry/widget-registry.ts`
  - Map widget IDs to Widget constructors
  - widgetFromSpec() deserialization
  - widgetToSpec() serialization

- [ ] Migrate existing widgets to registry
  - Register APMWidget, TBControlsWidget, etc.
  - Add WidgetSpec metadata (position, size, etc.)

**Phase 4: Example Migration (Week 6)**
- [ ] Migrate TB Command Center to EffuseGraph
  - Create TBCCShellGraphSpec with children
  - Replace manual tab switching with pin merges
  - Test composition, HMR, serialization

- [ ] Document new patterns in `docs/effuse/VISUAL-LANGUAGE.md`
  - EffusePin usage examples
  - EffuseGraph composition patterns
  - WidgetSpec serialization guide

**Phase 5: Visual Editor Foundation (Week 7-8)**
- [ ] Create spec editor widget in `src/effuse/widgets/spec-editor.ts`
  - JSON editor for WidgetSpec
  - Live preview of spec changes
  - Save/load specs to localStorage

- [ ] Add graph visualization in `src/effuse/widgets/graph-visualizer.ts`
  - Render EffuseGraph as node graph
  - Show pin connections
  - Click to edit widget specs

#### 4.2 Backward Compatibility

**Strategy:** Keep existing Effuse widgets working unchanged.

- Simple widgets (APM, TB Output, etc.) continue using Widget<S, E, R>
- Complex layouts migrate to EffuseGraph
- Both approaches supported simultaneously
- Gradual migration over time

#### 4.3 Testing Strategy

- **Unit tests**: EffusePin, EffuseGraph, spec serialization
- **Integration tests**: Graph composition, mount lifecycle, HMR
- **Migration tests**: Ensure existing widgets still work
- **Example apps**: TB Command Center as EffuseGraph, showcase composition

### Section 5: Code Harvest Inventory (3-4 pages)

**Files to harvest from Unit framework:**

| Unit Source | Purpose | Lines | Harvest Strategy |
|-------------|---------|-------|------------------|
| `src/Pin.ts` | Pin implementation | 360 | Adapt to EffusePin with StateCell |
| `src/Class/Graph/index.ts` | Graph composition | ~3000 | Adapt core composition logic, skip DOM-specific |
| `src/spec/fromSpec.ts` | Spec deserialization | 150 | Adapt for EffuseGraphSpec |
| `src/spec/stringify.ts` | Spec serialization | ~200 | Adapt for Widget serialization |
| `src/types/GraphSpec.ts` | Spec type definitions | ~100 | Adapt type names, structure |
| `src/EventEmitter.ts` | Event system | ~100 | SKIP - use Effect Streams instead |

**Adaptation checklist for each harvested file:**
- [ ] Replace event emitters with Effect Streams
- [ ] Replace System object with Context.Tag services
- [ ] Add Effect error handling
- [ ] Add TypeScript strict types (S, E, R generics)
- [ ] Add Effect-native tests

### Section 6: Critical Files & Changes (2-3 pages)

#### New Files to Create

```
src/effuse/
├── pin/
│   ├── effuse-pin.ts                 [NEW] EffusePin implementation
│   ├── types.ts                      [NEW] EffusePin types
│   └── effuse-pin.test.ts            [NEW] Pin tests
├── graph/
│   ├── effuse-graph.ts               [NEW] EffuseGraph implementation
│   ├── merge.ts                      [NEW] Pin merge system
│   ├── types.ts                      [NEW] Graph types
│   └── effuse-graph.test.ts          [NEW] Graph tests
├── spec/
│   ├── pin-spec.ts                   [NEW] Pin serialization
│   ├── graph-spec.ts                 [NEW] Graph spec system
│   ├── widget-spec.ts                [NEW] Widget spec system
│   └── types.ts                      [NEW] Spec type definitions
├── registry/
│   ├── widget-registry.ts            [NEW] Widget ID → constructor mapping
│   └── widget-registry.test.ts       [NEW] Registry tests
└── widgets/
    ├── spec-editor.ts                [NEW] Visual spec editor
    └── graph-visualizer.ts           [NEW] Graph visualization

docs/
└── research/
    └── unit.md                       [NEW] This research document
```

#### Modified Files

```
src/effuse/
├── widget/
│   ├── mount.ts                      [MODIFY] Add mountGraph() for EffuseGraph
│   └── types.ts                      [MODIFY] Add EffuseGraph to Widget union
├── layers/
│   └── test.ts                       [MODIFY] Mock EffusePin, EffuseGraph
└── hmr/
    └── registry.ts                   [MODIFY] Handle EffuseGraph state preservation

docs/effuse/
├── README.md                         [MODIFY] Add visual language overview
└── ARCHITECTURE.md                   [MODIFY] Add EffuseGraph lifecycle docs
```

### Section 7: Risk Assessment & Mitigation (1-2 pages)

**Risks:**

1. **Complexity creep** - Adding graph layer increases cognitive load
   - Mitigation: Keep simple widgets unchanged, only use graphs for complex layouts

2. **Performance overhead** - Pin abstraction + StateCell could be slow
   - Mitigation: Benchmark early, optimize hot paths, lazy graph instantiation

3. **Breaking HMR** - Graph state preservation more complex than simple widgets
   - Mitigation: Comprehensive HMR tests, snapshot/restore for all graph state

4. **Team learning curve** - New concepts (pins, merges, specs)
   - Mitigation: Thorough documentation, examples, migrate one widget together as team

**Success Criteria:**

- [ ] TB Command Center migrated to EffuseGraph without functional regression
- [ ] HMR works with graph widgets (preserve full state across hot reloads)
- [ ] Specs can be saved/loaded from JSON
- [ ] Performance parity or better vs current Effuse
- [ ] All existing widgets continue working unchanged

### Section 8: Future Vision (1-2 pages)

**What Effuse Visual Language Enables:**

1. **Visual Widget Editor** - Drag-drop graph editor for composing widgets
2. **User-Configurable Dashboards** - Save/load custom layouts
3. **Widget Marketplace** - Share WidgetSpecs as JSON
4. **AI-Generated UIs** - LLM outputs WidgetSpec, renders instantly
5. **Time Travel Debugging** - Snapshot graph state at any point
6. **Multi-User Collaboration** - Sync WidgetSpec changes via CRDT

**Next Steps After Initial Implementation:**

- Implement visual graph editor (drag-drop nodes, edit specs)
- Add CRDT support for multi-user widget editing
- Create widget template library (reusable specs)
- Build AI assistant for spec generation

## Implementation Checklist Summary

**Week 1-2: Core Primitives**
- [ ] Implement EffusePin (src/effuse/pin/effuse-pin.ts)
- [ ] Implement PinSpec serialization
- [ ] Write comprehensive tests

**Week 3-4: Graph Composition**
- [ ] Implement EffuseGraph (src/effuse/graph/effuse-graph.ts)
- [ ] Implement GraphSpec system
- [ ] Update mount system for graphs

**Week 5: Widget Registry**
- [ ] Create widget registry
- [ ] Register all existing widgets
- [ ] Add spec serialization

**Week 6: Example Migration**
- [ ] Migrate TB Command Center to EffuseGraph
- [ ] Document patterns in VISUAL-LANGUAGE.md
- [ ] Validate HMR, serialization

**Week 7-8: Visual Editor**
- [ ] Build spec editor widget
- [ ] Build graph visualizer
- [ ] Create example dashboard

## Document Deliverable

**File:** `docs/research/unit.md`
**Length:** ~25-30 pages
**Sections:** 8 sections as outlined above
**Code Examples:** Extensive (show Unit source, Effuse adaptation, usage patterns)
**Diagrams:** Architecture diagrams (ASCII art or mermaid)
**Tone:** Implementation-ready (assumes we're doing this)

**Audience:**
- Primary: Development team implementing the migration
- Secondary: Future maintainers understanding the design decisions

**Style:**
- Technical depth: Implementation-ready with file paths and code
- Practical examples: Show before/after for current widgets
- Reference links: Line numbers from Unit source code
- Clear trade-offs: Explain why each adaptation decision was made

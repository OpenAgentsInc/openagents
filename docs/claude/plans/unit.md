# Plan: Unit Framework Research & Effuse Visual Language Design

## Goal

Create a comprehensive research document at `docs/research/unit.md` that:
1. Deep-dives the Unit framework's visual primitives
2. Analyzes which primitives are valuable for Effuse
3. Designs an "Effuse Visual Language" by harvesting and adapting Unit concepts
4. Provides implementation-ready migration plan with code examples

**Key Insight:** We're NOT replacing Effuse. We're **harvesting Unit's visual primitives** (Pin, Graph, Spec system) and **adapting them to be Effect-native** to create a unique visual composition language for Effuse.

## User Requirements

- **Integration approach**: Harvest Unit code and adapt it (not wholesale replacement)
- **Core insight**: Unit primitives as the visual foundation, Effuse Effect-native architecture preserved
- **Deliverable stance**: Assume we're integrating (implementation-ready, not neutral analysis)
- **Depth**: Implementation-ready with detailed technical analysis, code examples, file paths

## Document Structure

### Section 1: Executive Summary (2-3 pages)
- What Unit is (MIMO FSM visual programming framework)
- What we're harvesting (Pin, Graph, Spec serialization)
- Why this enhances Effuse (composition, serialization, visual programming)
- High-level architecture vision (Effuse Visual Language)
- Key benefits vs current Effuse (native composition, JSON specs, visual editing potential)

### Section 2: Unit Framework Deep-Dive (5-7 pages)

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

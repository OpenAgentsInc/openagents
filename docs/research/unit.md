# Unit Framework Research: Harvesting Visual Primitives for Effuse

**Document Version:** 1.0
**Date:** 2025-12-08
**Author:** Research Team
**Status:** Implementation-Ready

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Unit Framework Deep-Dive](#2-unit-framework-deep-dive)
3. [Effuse Visual Language Design](#3-effuse-visual-language-design)
4. [Migration Strategy](#4-migration-strategy)
5. [Code Harvest Inventory](#5-code-harvest-inventory)
6. [Critical Files & Changes](#6-critical-files--changes)
7. [Risk Assessment & Mitigation](#7-risk-assessment--mitigation)
8. [Future Vision](#8-future-vision)

---

## 1. Executive Summary

### 1.1 What is Unit?

Unit is a **visual programming framework** based on a Multi-Input Multi-Output (MIMO) Finite State Machine architecture. It represents programs as directed graphs where:

- **Units** are generalized "functions" with multiple input and output pins
- **Pins** are reactive data conduits that propagate changes through the graph
- **Graphs** compose units into larger units (fractal composition)
- **Specs** are JSON-serializable representations of units enabling visual programming

**Location:** `~/code/unit/` (14MB, 1,619 TypeScript files)

**Core Philosophy:** Remove context switches between design and development by making programs visible as graphs. Data flows through connected nodes following Unix pipe philosophy in 2D.

### 1.2 What We're Harvesting

We're **not replacing Effuse**. We're harvesting three key primitives from Unit and adapting them to be Effect-native:

| Primitive | Unit Source | Purpose | Effuse Adaptation |
|-----------|-------------|---------|-------------------|
| **Pin** | `src/Pin.ts` (360 lines) | Reactive data conduit with constant/ref/data semantics | EffusePin wraps StateCell, uses Effect Streams |
| **Graph** | `src/Class/Graph/index.ts` (~3000 lines) | Composable unit trees with parent-child relationships | EffuseGraph uses Effect scoping, Widget composition |
| **Spec** | `src/spec/fromSpec.ts`, `stringify.ts` | JSON serialization of units | WidgetSpec system for save/load, HMR, visual editing |

### 1.3 Why This Enhances Effuse

**Current Effuse Limitations:**

1. **Weak Composition** - Parent-child widget relationships require manual patterns (see `tbcc-shell.ts:75-95`)
2. **No Serialization** - Can't save/load widget configurations or layouts
3. **Manual State Preservation** - HMR uses `structuredClone` with migration hacks
4. **No Visual Programming** - All widget composition is imperative code

**Unit Primitives Address These:**

1. **Native Composition** - Graph provides parent-child relationships, slot system, declarative composition
2. **Spec Serialization** - JSON specs enable save/load, visual editors, AI generation
3. **Snapshot/Restore** - Pin system has built-in state preservation
4. **Visual Foundation** - Specs are the data model for future visual graph editors

### 1.4 Architecture Vision: Effuse Visual Language

```
┌─────────────────────────────────────────────────────────────────┐
│                 Effuse Visual Language Architecture              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Widget<S, E, R>                        [Current - KEEP]        │
│      │                                                           │
│      ├─── EffuseGraph<S, E, R>          [NEW - Composition]     │
│      │        ├─ EffusePin<A>           [NEW - Data conduits]   │
│      │        ├─ WidgetSpec             [NEW - Serialization]   │
│      │        └─ GraphMerge             [NEW - Pin connections] │
│      │                                                           │
│      └─── Effect Services               [Current - Keep all]    │
│             ├─ DomService                                        │
│             ├─ StateService                                      │
│             └─ SocketService                                     │
│                                                                  │
│  Reactivity: Effect Streams (NOT event emitters)                │
│  Services: Context.Tag + Layers (NOT System object)             │
│  Types: Full generics (S, E, R) preserved                       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Design Principle:** Keep Effuse's Effect-native foundation, add visual primitives on top.

### 1.5 Key Benefits

| Benefit | Current Effuse | With Visual Language |
|---------|----------------|----------------------|
| **Widget Composition** | Manual patterns, direct DOM manipulation | Declarative specs, native parent-child |
| **State Sharing** | Custom events + subscriptions | Pin merges (data flows through connections) |
| **Configuration** | Hard-coded in `initialState()` | JSON WidgetSpec (save/load) |
| **HMR** | `structuredClone` + manual migrations | Snapshot/restore with automatic state preservation |
| **Visual Programming** | Not possible | Foundation for graph editor, drag-drop composition |
| **AI Generation** | Would need to write code | Can output WidgetSpec JSON directly |

### 1.6 Timeline

**8-week phased rollout:**

- **Weeks 1-2:** Core primitives (EffusePin, PinSpec)
- **Weeks 3-4:** Graph composition (EffuseGraph, GraphSpec)
- **Week 5:** Widget registry and spec system
- **Week 6:** Example migration (TB Command Center)
- **Weeks 7-8:** Visual editor foundation

**Backward compatible:** Existing simple widgets unchanged, only complex layouts migrate to EffuseGraph.

---

## 2. Unit Framework Deep-Dive

### 2.1 Pin System

**Source:** `~/code/unit/src/Pin.ts` (360 lines)

#### 2.1.1 Pin as Reactive Data Conduit

A Pin is **not just a value container**. It's a stateful, reactive data conduit with:

- **Internal state machine** (idle → start → data → drop → end)
- **Event emitters** for reactivity (`'data'`, `'drop'`, `'invalid'`, `'start'`, `'end'`)
- **Three data semantics** (data, ref, constant)
- **Snapshot/restore** for serialization

**Core Implementation:**

```typescript
// ~/code/unit/src/Pin.ts:50-80
export class Pin<T = any> extends $<PinEvents<T>> implements V<T>, PI<T> {
  private _constant: boolean = false   // Data persists during backpropagation
  private _ignored: boolean = false    // Should pin be processed?
  private _ref: boolean = false        // Reference vs value semantics
  private _invalid: boolean = false    // Data is invalid
  private _idle: boolean = true        // No data yet
  private _register: T | undefined     // The actual data

  constructor({ data, constant, ignored, ref }: PinConstructor = {}, $system: System) {
    super($system)

    this._register = data
    this._constant = constant || false
    this._ignored = ignored || false
    this._ref = ref || false

    // Choose push implementation based on ref
    if (this._ref) {
      this._push = this._push_ref
    } else {
      this._push = this._push_data
    }
  }
}
```

#### 2.1.2 Three Pin Types

**1. Data Pins (ref: false)**

Value semantics - data is copied:

```typescript
// ~/code/unit/src/Pin.ts:229-249
private _push_data(data: any, backpropagation: boolean = false, propagate: boolean = true): void {
  this.invalidate()          // Clear old data
  this._invalid = false
  this.start()               // Emit 'start' if idle
  this._register = data      // Store by value (copied)

  if (propagate) {
    this.emit('data', data, backpropagation)  // Notify listeners
  }

  if (this._ignored && !this._constant) {
    this.take()              // Auto-consume if ignored
  }
}
```

**2. Ref Pins (ref: true)**

Reference semantics - data is shared:

```typescript
// ~/code/unit/src/Pin.ts:251-269
private _push_ref(data: any, backpropagation: boolean = false, propagate: boolean = true): void {
  this.invalidate()
  this._invalid = false
  this.start()

  data = this.__embody(data)  // Instantiate if constructor, register if Unit
  this._register = data       // Store by reference (shared)

  if (propagate) {
    this.emit('data', data, backpropagation)
  }
}
```

**3. Constant Pins (constant: true)**

Data persists across backpropagation cycles:

```typescript
// ~/code/unit/src/Pin.ts:131-142
public pull(): T | undefined {
  const data = this._register

  if (data !== undefined) {
    if (this._constant) {
      this.emit('_data', data)  // Emit but DON'T consume
    } else {
      this.take()               // Consume data
    }
  }
  return data
}
```

#### 2.1.3 Pin Lifecycle

```
Lifecycle States:

idle (no data)
  ↓ push()
start (data arriving)
  ↓ emit('start')
active (data present)
  ↓ emit('data', value)
consuming
  ↓ take()
  ↓ emit('drop', value)
end (data consumed)
  ↓ emit('end')
idle
```

**Implementation:**

```typescript
// ~/code/unit/src/Pin.ts:82-129
public take(propagate: boolean = true): T | undefined {
  const data = this._register

  if (this._register !== undefined) {
    this._disembody(data)      // Cleanup if ref
    this._register = undefined  // Clear data

    if (propagate) {
      this.emit('drop', data)   // Notify consumed
    }
  }

  this.end()                    // Transition to idle
  return data
}

public start() {
  if (this._idle) {
    this._idle = false
    this.emit('start')
  }
}

public end() {
  if (this._register === undefined && !this._idle) {
    this._idle = true
    this.emit('end')
  }
}
```

#### 2.1.4 take() vs pull() vs peak()

Three ways to read pin data with different semantics:

| Method | Behavior | Use Case |
|--------|----------|----------|
| `take()` | Consume data (destructive read) | Input processing, single consumer |
| `pull()` | Read without consuming if constant, else take | Constant pins, config values |
| `peak()` | Read without side effects | Debugging, inspection |

```typescript
// ~/code/unit/src/Pin.ts:82-98, 131-142, 271-273
public take(propagate: boolean = true): T | undefined {
  const data = this._register
  if (this._register !== undefined) {
    this._register = undefined  // DESTRUCTIVE
    if (propagate) this.emit('drop', data)
  }
  this.end()
  return data
}

public pull(): T | undefined {
  const data = this._register
  if (data !== undefined) {
    if (this._constant) {
      this.emit('_data', data)  // NON-DESTRUCTIVE for constants
    } else {
      this.take()               // DESTRUCTIVE for non-constants
    }
  }
  return data
}

public peak(): T | undefined {
  return this._register  // ALWAYS non-destructive
}
```

#### 2.1.5 Snapshot/Restore

Pins support serialization for state preservation:

```typescript
// ~/code/unit/src/Pin.ts:327-349
public snapshot(): Pin_M<T> {
  return {
    _register: this._register instanceof $ ? undefined : this._register,
    ...(this._invalid ? { _invalid: true } : {}),
    ...(this._constant ? { _constant: true } : {}),
    ...(this._ignored ? { _ignored: true } : {}),
    ...(this._idle ? {} : { _idle: false }),
  }
}

public restore(state: Pin_M<T>): void {
  const { _register, _invalid, _constant, _ignored, _idle } = state

  this._register = _register
  this._invalid = _invalid
  this._constant = _constant
  this._ignored = _ignored
  this._idle = _idle

  if (_register instanceof $) {
    this.emit('_data', _register)
  }
}
```

#### 2.1.6 Valuable for Effuse

**Harvest these concepts:**

1. **Constant pins** - Perfect for persistent widget configuration (theme, layout prefs)
2. **Ref pins** - Shared mutable state (global services, theme context)
3. **take/pull/peak** - Different consumption semantics for different use cases
4. **Snapshot/restore** - Better HMR than current `structuredClone` approach
5. **State machine events** - Can adapt to Effect Streams for reactivity

**Adaptation needed:**

- Replace event emitters (`emit('data')`) with Effect Streams (`Stream.fromQueue`)
- Replace `System` parameter with Effect services (Context.Tag)
- Add Effect error handling (PinError for invalid states)
- Preserve TypeScript type safety with generics

---

### 2.2 Graph Composition

**Source:** `~/code/unit/src/Class/Graph/index.ts` (~3000 lines, lines 1-200 shown)

#### 2.2.1 Graph as Composable Unit Container

A Graph is a **Unit that contains other Units**. It provides:

- **Child unit registry** (`_unit: Dict<Unit>`)
- **Pin connections** (`_merge: Record<string, Merge>`)
- **Spec-based instantiation** (declarative composition)
- **Parent-child lifecycle** (children destroyed when parent destroyed)

**Core Implementation:**

```typescript
// ~/code/unit/src/Class/Graph/index.ts:176-200
export class Graph<I extends Dict<any> = any, O extends Dict<any> = any>
  extends Component__<I, O, GraphEvents>
  implements G<I, O>, U<I, O>, J<Dict<any>>
{
  __ = ['U', 'C', 'G', 'EE', 'J']

  private _spec: GraphSpec        // JSON representation
  private _unit: Dict<Unit> = {}  // Child units
  private _merge: Record<string, Merge> = {}  // Pin connections

  private _pipedFrom: { [output: string]: string } = {}  // Output → merge mapping
  private _pipedTo: { [input: string]: string } = {}     // Input → merge mapping

  constructor(
    spec: GraphSpec,
    branch: Dict<true>,
    system: System,
    id: string,
    push: boolean = false
  ) {
    super({}, {}, system, id)

    this._spec = spec
    // ... initialization logic
  }
}
```

#### 2.2.2 Spec-Based Composition

Graphs are instantiated from JSON specs:

```typescript
// Example GraphSpec structure
const DashboardSpec: GraphSpec = {
  id: "dashboard",
  name: "Dashboard",

  // Child units
  units: {
    apm: {
      id: "9988a56e-6bee-46c8-864c-e351d84bc7e2",  // APM widget type ID
      input: {
        expanded: { data: false, constant: true }
      }
    },
    tbControls: {
      id: "tb-controls-unit-id",
      input: {
        suitePath: { data: "./tests/suite" }
      }
    },
    mcTasks: {
      id: "mc-tasks-unit-id"
    }
  },

  // Pin connections between units
  merges: {
    apm_to_tb: {
      input: { "apm/sessionAPM": {} },
      output: { "tbControls/apmData": {} }
    }
  },

  // Visual structure (render order, slots)
  component: {
    children: ["apm", "tbControls", "mcTasks"],
    subComponents: {
      sidebar: { children: ["apm"] },
      main: { children: ["tbControls", "mcTasks"] }
    }
  }
}
```

#### 2.2.3 Unit Registry and Instantiation

Graphs maintain a registry of child units:

```typescript
// Pseudo-code showing unit instantiation logic
// (Actual implementation spans hundreds of lines)

class Graph {
  private _unit: Dict<Unit> = {}

  private instantiateUnits(spec: GraphSpec, system: System) {
    for (const [unitId, unitSpec] of Object.entries(spec.units)) {
      // Look up Unit class by ID
      const UnitClass = system.specs[unitSpec.id]

      // Instantiate child unit
      const unit = new UnitClass(system, unitId)

      // Apply pin data from spec
      for (const [pinId, pinSpec] of Object.entries(unitSpec.input || {})) {
        if (pinSpec.data !== undefined) {
          unit.push(pinId, pinSpec.data)
        }
        if (pinSpec.constant) {
          unit.setInputConstant(pinId, true)
        }
      }

      // Register in graph
      this._unit[unitId] = unit
    }
  }
}
```

#### 2.2.4 Merge System (Pin Connections)

Merges connect pins between units, creating data flow:

```typescript
// Merge connects one or more inputs to one or more outputs
interface GraphMergeSpec {
  input: {
    [pinPath: string]: {}  // e.g., "unitA/outputPin"
  }
  output: {
    [pinPath: string]: {}  // e.g., "unitB/inputPin"
  }
}

// Example: Connect APM's sessionAPM to TB's apmData
const merge: GraphMergeSpec = {
  input: {
    "apm/sessionAPM": {}  // Source pin
  },
  output: {
    "tbControls/apmData": {},  // Destination pin 1
    "mcTasks/apmData": {}      // Destination pin 2
  }
}
```

**Data flow:**

```
APM Widget (sessionAPM pin)
    → emit('data', 15.5)
    → Merge listens to sessionAPM
    → Merge.push() to all outputs
    → tbControls.push('apmData', 15.5)
    → mcTasks.push('apmData', 15.5)
```

#### 2.2.5 Component Tree (Slots and Render Order)

Graphs specify how children render:

```typescript
interface GraphComponentSpec {
  children: string[]  // Top-level render order
  subComponents?: {
    [slotName: string]: {
      children: string[]  // Children in this slot
    }
  }
}

// Example: Three-column layout with slots
const layout: GraphComponentSpec = {
  children: ["sidebar", "main", "footer"],
  subComponents: {
    sidebar: {
      children: ["apm", "mcStatus"]
    },
    main: {
      children: ["tbControls", "tbOutput"]
    },
    footer: {
      children: ["trajectoryViewer"]
    }
  }
}
```

#### 2.2.6 Graph Manipulation API

Graphs can be modified at runtime:

```typescript
// Unit's Graph interface (~/code/unit/src/types/interface/G.ts)
interface G<I, O> {
  // Unit management
  addUnit(unitId: string, unitSpec: GraphUnitSpec): void
  removeUnit(unitId: string): void
  renameUnit(unitId: string, newId: string): void

  // Pin connections
  addMerge(mergeId: string, merge: GraphMergeSpec): void
  removeMerge(mergeId: string): void
  addPinToMerge(mergeId: string, type: 'input' | 'output', pinId: string): void

  // Pin exposure (make internal pins accessible externally)
  exposePin(unitId: string, pinId: string, as: string): void
  coverPin(unitId: string, pinId: string): void

  // Spec access
  getSpec(): GraphSpec
  getBundleSpec(): BundleSpec  // With dependencies
}
```

**Example usage:**

```typescript
// Runtime composition changes
const dashboard = new Graph(DashboardSpec, {}, system)

// Add a new widget
dashboard.addUnit('newWidget', {
  id: 'container-widget-id',
  input: { content: { data: 'Hello' } }
})

// Connect it to existing widget
dashboard.addMerge('apm_to_new', {
  input: { 'apm/sessionAPM': {} },
  output: { 'newWidget/value': {} }
})

// Get updated spec
const updatedSpec = dashboard.getSpec()
// Can serialize and save this
```

#### 2.2.7 Valuable for Effuse

**Harvest these concepts:**

1. **Parent-child widget composition** - Fixes current manual patterns (direct classList manipulation)
2. **Declarative composition via specs** - JSON describes widget tree structure
3. **Pin merges for widget communication** - Better than custom events + subscriptions
4. **Slot system** - Named content regions (sidebar, main, footer, etc.)
5. **Runtime composition** - Add/remove/reconnect widgets dynamically
6. **Spec serialization** - Save entire widget tree state to JSON

**Adaptation needed:**

- Replace Unit with Widget<S, E, R>
- Replace event-based merges with Effect Stream-based merges
- Use Effect scoping for child lifecycle (Effect.scoped, not manual destroy)
- Add service dependencies (R in Widget<S, E, R>)

---

### 2.3 Spec Serialization

**Source:** `~/code/unit/src/spec/fromSpec.ts` (150 lines), `~/code/unit/src/spec/stringify.ts`

#### 2.3.1 Spec → Class → Instance Pattern

Unit uses a three-step pattern for deserialization:

```
GraphSpec (JSON)
    ↓ fromSpec()
GraphClass (extends Graph)
    ↓ new GraphClass(system)
Graph Instance
```

**Implementation:**

```typescript
// ~/code/unit/src/spec/fromSpec.ts:83-116
export function fromSpec<I, O>(
  spec: GraphSpec,
  specs: Specs,
  classes: Classes = {},
  branch: Dict<true> = {}
): GraphBundle<I, O> {
  const Class = classFromSpec<I, O>(spec, specs, classes, branch)

  const { id } = spec
  if (!id) throw new Error('spec id is required')

  const bundle = unitBundleSpec({ id }, weakMerge(specs, { [id]: spec }))
  const Bundle = bundleClass(Class, bundle, specs)

  return Bundle
}

export function classFromSpec<I, O>(
  spec: GraphSpec,
  specs: Specs,
  classes: Classes,
  branch: Dict<true> = {}
): GraphClass<I, O> {
  applyDefaultIgnored(spec, specs)

  const { name } = spec

  // Dynamically generate a class
  class Class extends Graph<I, O> {
    constructor(system: System, id: string, push?: boolean) {
      super(spec, branch, system, id, push)
    }
  }

  Object.defineProperty(Class, 'name', {
    value: name,  // Set class name for debugging
  })

  return Class
}
```

#### 2.3.2 Spec Structure

A GraphSpec is a complete, serializable representation:

```typescript
interface GraphSpec {
  id: string        // Unique spec identifier
  name?: string     // Human-readable name

  // Input/output pin definitions
  inputs?: {
    [pinId: string]: PinSpec
  }
  outputs?: {
    [pinId: string]: PinSpec
  }

  // Child units
  units: {
    [unitId: string]: GraphUnitSpec
  }

  // Pin connections
  merges: {
    [mergeId: string]: GraphMergeSpec
  }

  // Visual structure
  component?: GraphComponentSpec

  // Metadata
  metadata?: {
    description?: string
    author?: string
    version?: string
    [key: string]: any
  }
}

interface GraphUnitSpec {
  id: string  // Unit type ID (references another spec)

  // Pin data
  input?: {
    [pinId: string]: GraphUnitPinSpec
  }
  output?: {
    [pinId: string]: GraphUnitPinSpec
  }
}

interface GraphUnitPinSpec {
  data?: any           // Initial pin data
  constant?: boolean   // Is constant pin?
  ignored?: boolean    // Should pin be ignored?
  ref?: boolean        // Reference semantics?
}
```

#### 2.3.3 Bundle Specs (With Dependencies)

A BundleSpec includes all specs needed to instantiate a unit:

```typescript
interface BundleSpec {
  spec: GraphSpec      // Main spec
  specs: Specs         // All dependency specs
}

// Example: Dashboard bundle includes specs for all child widgets
const dashboardBundle: BundleSpec = {
  spec: {
    id: "dashboard",
    units: {
      apm: { id: "apm-widget" },
      tb: { id: "tb-controls" }
    }
  },
  specs: {
    "apm-widget": { /* APM widget spec */ },
    "tb-controls": { /* TB controls spec */ },
    // All transitive dependencies
  }
}
```

#### 2.3.4 Serialization (Instance → Spec)

Units can serialize their current state:

```typescript
// Pseudo-code showing serialization logic
class Graph {
  getSpec(): GraphSpec {
    const spec: GraphSpec = {
      id: this._spec.id,
      name: this._spec.name,
      units: {},
      merges: this._spec.merges  // Merges are structural, not stateful
    }

    // Serialize each child unit
    for (const [unitId, unit] of Object.entries(this._unit)) {
      spec.units[unitId] = {
        id: unit.constructor.name,
        input: {},
        output: {}
      }

      // Capture current pin data
      for (const [pinId, pin] of Object.entries(unit.getInputs())) {
        spec.units[unitId].input[pinId] = {
          data: pin.peak(),              // Current value
          constant: pin.constant(),
          ref: pin.ref()
        }
      }
    }

    return spec
  }
}
```

#### 2.3.5 Valuable for Effuse

**Harvest these concepts:**

1. **Spec → Class pattern** - Dynamic widget class generation from JSON
2. **Bundle specs** - Capture entire widget tree + dependencies
3. **Metadata** - Author, version, description for widget marketplace
4. **State capture** - Serialize current runtime state (not just initial config)
5. **Dependency resolution** - Automatically include all required child specs

**Adaptation needed:**

- Create WidgetSpec interface (similar to GraphSpec structure)
- Add Widget registry (id → Widget<S, E, R> mapping)
- Serialize Effect service requirements (R in Widget<S, E, R>)
- Handle StateCell serialization (leverage snapshot/restore)

---

## 3. Effuse Visual Language Design

### 3.1 Architecture Vision

**Core Principle:** Keep Effect-native foundation, add visual primitives on top.

```
┌──────────────────────────────────────────────────────────────────────┐
│                    Effuse Visual Language Stack                       │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  Application Layer                                                    │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  TB Command Center (EffuseGraph)                               │  │
│  │    ├─ Dashboard (EffuseGraph with 3 children)                  │  │
│  │    ├─ Tasks (Simple Widget)                                    │  │
│  │    └─ TestGen (EffuseGraph with pin merges)                    │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                              ↓                                        │
│  Composition Layer                                                    │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  EffuseGraph<S, E, R> implements Widget<S, E, R>              │  │
│  │    ├─ children: Record<string, Widget<any, any, any>>         │  │
│  │    ├─ pins: Record<string, EffusePin<any>>                    │  │
│  │    ├─ merges: Record<string, GraphMerge>                      │  │
│  │    └─ spec: EffuseGraphSpec (JSON-serializable)               │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                              ↓                                        │
│  Reactive Layer                                                       │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  EffusePin<A>                                                  │  │
│  │    ├─ Wraps StateCell<A> (Effect.Ref + Queue)                 │  │
│  │    ├─ changes: Stream.Stream<A, never>                        │  │
│  │    ├─ constant/ref flags                                      │  │
│  │    └─ snapshot/restore                                        │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                              ↓                                        │
│  Effect Foundation (Current - KEEP)                                   │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  Widget<S, E, R>                                               │  │
│  │  StateCell<A> (Ref + Queue)                                    │  │
│  │  Services: DomService, StateService, SocketService            │  │
│  │  Layers: EffuseLive = DomLive + StateLive + SocketLive        │  │
│  │  Mount: Effect.scoped, Effect.forkScoped                      │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

**Key Design Decisions:**

1. **Effect Streams, NOT event emitters** - All reactivity via `Stream.Stream<A, never>`
2. **Context.Tag services, NOT System object** - DomService, StateService, etc.
3. **Full generics preserved** - Widget<S, E, R>, type safety throughout
4. **Backward compatible** - Simple widgets unchanged, only complex layouts use graphs

---

### 3.2 EffusePin - Effect-Native Pin Adapter

**Harvest:** Pin concept (data conduit, constant/ref/data types, snapshot/restore)
**Adapt:** Replace event emitters with Effect Streams

#### 3.2.1 Interface Design

```typescript
// src/effuse/pin/types.ts

export interface EffusePin<A> {
  // Core read operations
  readonly get: Effect.Effect<A | undefined, never>
  readonly pull: () => Effect.Effect<A | undefined, never>  // Non-destructive
  readonly peak: () => Effect.Effect<A | undefined, never>  // Synonym for pull

  // Write operations
  readonly set: (value: A) => Effect.Effect<void, never>
  readonly take: () => Effect.Effect<A | undefined, never>  // Destructive read
  readonly invalidate: () => Effect.Effect<void, never>

  // Reactivity
  readonly changes: Stream.Stream<A, never>  // Effect Stream, NOT event emitter

  // Pin semantics
  readonly constant: boolean   // Data persists across backpropagation
  readonly ref: boolean        // Reference vs value semantics

  // Serialization
  readonly snapshot: () => Effect.Effect<PinSnapshot<A>, never>
  readonly restore: (state: PinSnapshot<A>) => Effect.Effect<void, never>
}

export interface PinSnapshot<A> {
  readonly value: A | undefined
  readonly constant: boolean
  readonly ref: boolean
  readonly invalid: boolean
}
```

#### 3.2.2 Implementation

```typescript
// src/effuse/pin/effuse-pin.ts

import { Effect, Ref, Queue, Stream, Scope } from "effect"
import { makeCell, type StateCell } from "../state/cell.js"
import type { EffusePin, PinSnapshot } from "./types.js"

export const makeEffusePin = <A>(
  initial: A | undefined,
  opts: {
    constant?: boolean
    ref?: boolean
  } = {}
): Effect.Effect<EffusePin<A>, never, Scope.Scope> =>
  Effect.gen(function* () {
    // Use StateCell for underlying reactivity
    const cell = yield* makeCell(initial)

    // Track pin semantics
    const constantFlag = opts.constant ?? false
    const refFlag = opts.ref ?? false
    const invalidRef = yield* Ref.make(false)

    const pin: EffusePin<A> = {
      // Core reads
      get: cell.get,

      pull: Effect.gen(function* () {
        const value = yield* cell.get
        if (constantFlag && value !== undefined) {
          // For constant pins, emit without consuming
          // (change stream already emitted when set)
        }
        return value
      }),

      peak: cell.get,  // Non-destructive peek

      // Writes
      set: (value: A) =>
        Effect.gen(function* () {
          yield* Ref.set(invalidRef, false)
          yield* cell.set(value)
          // StateCell.set already publishes to changes stream
        }),

      take: Effect.gen(function* () {
        const value = yield* cell.get
        if (!constantFlag) {
          yield* cell.set(undefined)  // Consume (destructive)
        }
        return value
      }),

      invalidate: Effect.gen(function* () {
        yield* Ref.set(invalidRef, true)
        yield* cell.set(undefined)
      }),

      // Reactivity
      changes: cell.changes,

      // Semantics
      constant: constantFlag,
      ref: refFlag,

      // Serialization
      snapshot: Effect.gen(function* () {
        const value = yield* cell.get
        const invalid = yield* Ref.get(invalidRef)
        return {
          value,
          constant: constantFlag,
          ref: refFlag,
          invalid
        }
      }),

      restore: (state: PinSnapshot<A>) =>
        Effect.gen(function* () {
          yield* cell.set(state.value)
          yield* Ref.set(invalidRef, state.invalid)
        })
    }

    return pin
  })
```

#### 3.2.3 Usage Examples

**Example 1: Constant pin for widget configuration**

```typescript
// Theme configuration that persists
const themePinEffect = makeEffusePin(
  { mode: "dark", accentColor: "#007bff" },
  { constant: true }  // Never consumed, always available
)

const themePin = yield* themePinEffect

// Widgets can pull() repeatedly without consuming
const theme1 = yield* themePin.pull()  // { mode: "dark", ... }
const theme2 = yield* themePin.pull()  // Still available!

// Changes stream emits on updates
yield* pipe(
  themePin.changes,
  Stream.tap(theme =>
    Effect.sync(() => console.log("Theme changed:", theme))
  ),
  Stream.runDrain,
  Effect.forkScoped
)
```

**Example 2: Data pin for single-use values**

```typescript
// Task result that should be consumed once
const taskResultPin = yield* makeEffusePin<TaskResult>(undefined)

// Producer: Set result
yield* taskResultPin.set({ success: true, data: {...} })

// Consumer: Take (consume)
const result = yield* taskResultPin.take()  // Gets value
const result2 = yield* taskResultPin.take() // undefined (consumed)
```

**Example 3: Ref pin for shared mutable state**

```typescript
// Shared widget registry (reference semantics)
const registryPin = yield* makeEffusePin(
  new Map<string, Widget<any, any, any>>(),
  { ref: true }  // Share the Map reference
)

// All consumers see same Map instance
const registry1 = yield* registryPin.get
const registry2 = yield* registryPin.get
registry1 === registry2  // true (same reference)
```

#### 3.2.4 Comparison: Unit Pin vs EffusePin

| Feature | Unit Pin | EffusePin |
|---------|----------|-----------|
| **Reactivity** | Event emitters (`emit('data')`) | Effect Streams (`Stream.Stream<A>`) |
| **Read** | Sync methods (`take()`, `pull()`, `peak()`) | Effect methods (`Effect.Effect<A>`) |
| **Write** | Sync method (`push(value)`) | Effect method (`set(value)`) |
| **Error handling** | Exceptions | Effect error channel (if needed) |
| **Type safety** | Generic `<T>` | Generic `<A>` with Effect types |
| **Services** | Requires `System` parameter | Uses Effect scoping (Scope.Scope) |
| **Testing** | Mock `System` | Use Effect test layers |

**Benefits of EffusePin over Unit Pin:**

- Full Effect composition (pipe, gen, etc.)
- Type-safe error channel (if we add PinError)
- Works with existing Effuse patterns (StateCell, Streams)
- Testable with Effect mock layers

---

### 3.3 EffuseGraph - Composable Widget Trees

**Harvest:** Graph composition, parent-child relationships, merge connections
**Adapt:** Use Effect scoping for lifecycle, Widgets instead of Units

#### 3.3.1 Interface Design

```typescript
// src/effuse/graph/types.ts

import type { Widget, WidgetContext } from "../widget/types.js"
import type { EffusePin } from "../pin/types.js"
import type { TemplateResult } from "../template/html.js"

export interface EffuseGraph<S, E, R = never> extends Widget<S, E, R> {
  // Graph is a Widget (can be mounted like any widget)
  // S = graph state, E = graph events, R = service requirements
}

export interface EffuseGraphSpec {
  id: string
  name?: string

  // Child widgets
  children: Record<string, WidgetSpec>

  // Graph-level pins (not widget state)
  pins?: Record<string, PinSpec>

  // Pin connections between children
  merges?: Record<string, MergeSpec>

  // Visual structure
  component: {
    children: string[]  // Render order
    subComponents?: Record<string, { children: string[] }>  // Slots
  }

  // Metadata
  metadata?: GraphMetadata
}

export interface WidgetSpec {
  id: string                    // Widget type ID (key into WidgetRegistry)
  initialState?: unknown        // Override widget's initialState()
  pins?: Record<string, PinSpec>
}

export interface PinSpec {
  data?: unknown
  constant?: boolean
  ref?: boolean
}

export interface MergeSpec {
  input: Record<string, {}>     // Source pins (e.g., "apm/sessionAPM")
  output: Record<string, {}>    // Destination pins (e.g., "tb/apmData")
}

export interface GraphMetadata {
  description?: string
  author?: string
  version?: string
  [key: string]: unknown
}
```

#### 3.3.2 Implementation Outline

```typescript
// src/effuse/graph/effuse-graph.ts

import { Effect, Stream, Scope } from "effect"
import type { EffuseGraph, EffuseGraphSpec } from "./types.js"
import type { Widget, WidgetContext } from "../widget/types.js"
import { makeEffusePin } from "../pin/effuse-pin.js"
import { mountWidget } from "../widget/mount.js"
import { WidgetRegistry } from "../registry/widget-registry.js"

export const makeEffuseGraph = <S, E, R>(
  spec: EffuseGraphSpec
): EffuseGraph<S, E, R> => {
  const graph: EffuseGraph<S, E, R> = {
    id: spec.id,

    // Graph-level state (not child state)
    initialState: () => ({} as S),  // Could be empty or spec-derived

    render: (ctx: WidgetContext<S, E>) =>
      Effect.gen(function* () {
        // Render containers for each child widget
        const childContainers = spec.component.children.map(childId =>
          html`<div id="${spec.id}-${childId}" class="effuse-graph-child"></div>`
        )

        return html`
          <div class="effuse-graph-container" data-graph-id="${spec.id}">
            ${joinTemplates(childContainers)}
          </div>
        `
      }),

    setupEvents: (ctx: WidgetContext<S, E>) =>
      Effect.gen(function* () {
        // Create pins
        const pins: Record<string, EffusePin<any>> = {}
        for (const [pinId, pinSpec] of Object.entries(spec.pins || {})) {
          pins[pinId] = yield* makeEffusePin(pinSpec.data, {
            constant: pinSpec.constant,
            ref: pinSpec.ref
          })
        }

        // Mount child widgets
        const children: Record<string, any> = {}
        for (const [childId, widgetSpec] of Object.entries(spec.children)) {
          const widget = WidgetRegistry[widgetSpec.id]
          if (!widget) {
            throw new Error(`Widget not found: ${widgetSpec.id}`)
          }

          // Override initialState if provided in spec
          const childWidget = widgetSpec.initialState
            ? { ...widget, initialState: () => widgetSpec.initialState }
            : widget

          // Mount child in its container
          const container = yield* ctx.dom.queryId(`${spec.id}-${childId}`)
          const mounted = yield* mountWidget(childWidget, container)

          children[childId] = { widget: childWidget, mounted }
        }

        // Set up merges (pin connections)
        for (const [mergeId, mergeSpec] of Object.entries(spec.merges || {})) {
          yield* setupMerge(mergeId, mergeSpec, children, pins)
        }
      }),

    // Graph doesn't handle events directly (children do)
    handleEvent: undefined,

    // Graph doesn't have subscriptions (children do)
    subscriptions: undefined
  }

  return graph
}

const setupMerge = (
  mergeId: string,
  mergeSpec: MergeSpec,
  children: Record<string, any>,
  pins: Record<string, EffusePin<any>>
): Effect.Effect<void, never> =>
  Effect.gen(function* () {
    // Parse input pins (sources)
    const inputPins: EffusePin<any>[] = []
    for (const inputPath of Object.keys(mergeSpec.input)) {
      const pin = resolvePinPath(inputPath, children, pins)
      if (pin) inputPins.push(pin)
    }

    // Parse output pins (destinations)
    const outputPins: EffusePin<any>[] = []
    for (const outputPath of Object.keys(mergeSpec.output)) {
      const pin = resolvePinPath(outputPath, children, pins)
      if (pin) outputPins.push(pin)
    }

    // Connect: when any input changes, propagate to all outputs
    for (const inputPin of inputPins) {
      yield* pipe(
        inputPin.changes,
        Stream.tap(value =>
          Effect.gen(function* () {
            for (const outputPin of outputPins) {
              yield* outputPin.set(value)
            }
          })
        ),
        Stream.runDrain,
        Effect.forkScoped
      )
    }
  })

const resolvePinPath = (
  path: string,  // "childId/pinId" or "pinId"
  children: Record<string, any>,
  pins: Record<string, EffusePin<any>>
): EffusePin<any> | undefined => {
  if (path.includes('/')) {
    const [childId, pinId] = path.split('/')
    // For now, assume children expose pins (future work)
    // return children[childId]?.pins[pinId]
    return undefined  // TODO: Implement child pin access
  } else {
    return pins[path]
  }
}
```

#### 3.3.3 Usage Example: TB Command Center

**Before (manual patterns):**

```typescript
// src/effuse/widgets/tb-command-center/tbcc-shell.ts (current)
export const TBCCShellWidget: Widget<TBCCShellState, TBCCShellEvent, SocketServiceTag> = {
  id: "tbcc-shell",

  initialState: () => ({ activeTab: "dashboard" }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Manual tab rendering with classList.toggle for visibility
      return html`
        <div id="tbcc-tab-dashboard" class="${state.activeTab === 'dashboard' ? '' : 'hidden'}"></div>
        <div id="tbcc-tab-tasks" class="${state.activeTab === 'tasks' ? '' : 'hidden'}"></div>
        <div id="tbcc-tab-testgen" class="${state.activeTab === 'testgen' ? '' : 'hidden'}"></div>
      `
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      if (event.type === "changeTab") {
        yield* ctx.state.update(s => ({ ...s, activeTab: event.tab }))

        // CRITICAL: Manual DOM manipulation to avoid re-rendering children
        for (const tabId of ["dashboard", "tasks", "testgen"]) {
          const container = yield* ctx.dom.queryOption(`#tbcc-tab-${tabId}`)
          if (container) {
            container.classList.toggle("hidden", tabId !== event.tab)
          }
        }
      }
    })
}
```

**After (EffuseGraph):**

```typescript
// src/effuse/widgets/tb-command-center/tbcc-shell-spec.ts (new)
export const TBCCShellSpec: EffuseGraphSpec = {
  id: "tbcc-shell",
  name: "TB Command Center Shell",

  // Child widgets
  children: {
    dashboard: {
      id: "tbcc-dashboard",  // Widget type ID
    },
    tasks: {
      id: "tbcc-tasks",
    },
    testgen: {
      id: "tbcc-testgen",
    }
  },

  // Graph-level pins
  pins: {
    activeTab: {
      data: "dashboard",
      constant: true  // Persistent configuration
    }
  },

  // Pin connections (tab visibility)
  merges: {
    tab_to_visibility: {
      input: { "activeTab": {} },
      output: {
        "dashboard/visible": {},
        "tasks/visible": {},
        "testgen/visible": {}
      }
    }
  },

  // Visual structure
  component: {
    children: ["dashboard", "tasks", "testgen"]
  },

  metadata: {
    description: "TB Command Center with tabbed navigation",
    author: "OpenAgents Team",
    version: "2.0.0"
  }
}

// Create the graph widget
export const TBCCShellGraph = makeEffuseGraph<TBCCShellState, TBCCShellEvent, SocketServiceTag>(
  TBCCShellSpec
)
```

**Benefits:**

- No manual DOM manipulation (classList.toggle removed)
- Declarative tab switching via pin merge
- Serializable spec (can save/load layout)
- Visual structure explicit in spec
- Backward compatible (old widgets still work)

---

### 3.4 WidgetSpec - Serializable Widget Configuration

**Harvest:** Spec serialization system, snapshot/restore
**Adapt:** Widget interface → JSON spec, Effect services in layer

#### 3.4.1 Widget Registry

```typescript
// src/effuse/registry/widget-registry.ts

import type { Widget } from "../widget/types.js"
import { APMWidget } from "../widgets/apm-widget.js"
import { TBControlsWidget } from "../widgets/tb-controls.js"
import { MCTasksWidget } from "../widgets/mc-tasks.js"
// ... all widgets

export const WidgetRegistry: Record<string, Widget<any, any, any>> = {
  "apm-widget": APMWidget,
  "tb-controls": TBControlsWidget,
  "mc-tasks": MCTasksWidget,
  "tbcc-dashboard": TBCCDashboardWidget,
  "tbcc-tasks": TBCCTasksWidget,
  "tbcc-testgen": TBCCTestGenWidget,
  "three-background": ThreeBackgroundWidget,
  "intro-card": IntroCardWidget,
  "tb-output": TBOutputWidget,
  "container-widget": ContainerWidget,
  // etc
}

export const getWidget = (id: string): Widget<any, any, any> | undefined => {
  return WidgetRegistry[id]
}

export const registerWidget = (id: string, widget: Widget<any, any, any>): void => {
  WidgetRegistry[id] = widget
}
```

#### 3.4.2 Spec Serialization

```typescript
// src/effuse/spec/widget-spec.ts

import type { Widget } from "../widget/types.js"
import type { WidgetSpec } from "../graph/types.js"

export const widgetToSpec = <S>(
  widget: Widget<S, any, any>,
  state: S
): WidgetSpec => ({
  id: widget.id,
  initialState: state,
  // TODO: Add pins, children if graph
})

export const widgetFromSpec = (
  spec: WidgetSpec
): Widget<any, any, any> => {
  const widget = WidgetRegistry[spec.id]
  if (!widget) {
    throw new Error(`Widget not found in registry: ${spec.id}`)
  }

  // Override initialState if provided in spec
  if (spec.initialState !== undefined) {
    return {
      ...widget,
      initialState: () => spec.initialState
    }
  }

  return widget
}
```

#### 3.4.3 HMR with Specs

```typescript
// src/effuse/hmr/registry.ts (modified)

import type { WidgetSpec } from "../graph/types.js"
import { widgetToSpec } from "../spec/widget-spec.js"

export const saveWidgetState = (
  widgetId: string,
  widget: Widget<any, any, any>,
  state: unknown
): void => {
  const registry = getRegistry()
  if (!registry) return

  try {
    const spec = widgetToSpec(widget, state)
    registry.widgets.set(widgetId, spec)  // Store as WidgetSpec
  } catch (e) {
    console.warn(`[Effuse HMR] Could not save state for "${widgetId}":`, e)
  }
}

export const loadWidgetSpec = (widgetId: string): WidgetSpec | undefined => {
  const registry = getRegistry()
  if (!registry) return undefined

  const spec = registry.widgets.get(widgetId) as WidgetSpec | undefined
  if (spec) {
    registry.widgets.delete(widgetId)
    console.log(`[Effuse HMR] Restored spec for "${widgetId}"`)
  }
  return spec
}
```

#### 3.4.4 Persistent Configuration

```typescript
// Save widget configuration to localStorage
export const saveWidgetConfig = (
  widgetId: string,
  spec: WidgetSpec
): Effect.Effect<void, never> =>
  Effect.sync(() => {
    const key = `effuse:widget:${widgetId}`
    localStorage.setItem(key, JSON.stringify(spec))
  })

// Load widget configuration from localStorage
export const loadWidgetConfig = (
  widgetId: string
): Effect.Effect<WidgetSpec | undefined, never> =>
  Effect.sync(() => {
    const key = `effuse:widget:${widgetId}`
    const json = localStorage.getItem(key)
    if (!json) return undefined

    try {
      return JSON.parse(json) as WidgetSpec
    } catch (e) {
      console.warn(`Failed to parse widget config for ${widgetId}:`, e)
      return undefined
    }
  })
```

---

## 4. Migration Strategy

### 4.1 Phased Implementation Plan

#### Phase 1: Core Primitives (Week 1-2)

**Goal:** Implement EffusePin and PinSpec serialization with comprehensive tests.

**Tasks:**

- [ ] **Create EffusePin implementation** (`src/effuse/pin/effuse-pin.ts`)
  - Wrap StateCell with Pin interface (get, set, take, pull, invalidate)
  - Add constant, ref flags
  - Implement snapshot/restore
  - Full TypeScript types

- [ ] **Create Pin types** (`src/effuse/pin/types.ts`)
  - EffusePin interface
  - PinSnapshot type
  - PinSpec type

- [ ] **Implement PinSpec serialization** (`src/effuse/spec/pin-spec.ts`)
  - pinToSpec() - serialize EffusePin to JSON
  - pinFromSpec() - deserialize JSON to EffusePin

- [ ] **Write comprehensive tests** (`src/effuse/pin/effuse-pin.test.ts`)
  - Test all three pin types (data, ref, constant)
  - Test take/pull/peak semantics
  - Test snapshot/restore
  - Test reactivity (changes stream)

**Acceptance Criteria:**

- All tests pass
- Pin reactivity works with Effect Streams
- Snapshot/restore preserves full state
- Constant pins don't get consumed on pull
- Ref pins share same reference

**Files to Create:**

```
src/effuse/
├── pin/
│   ├── effuse-pin.ts          [NEW - 200 lines]
│   ├── types.ts               [NEW - 50 lines]
│   └── effuse-pin.test.ts     [NEW - 300 lines]
└── spec/
    └── pin-spec.ts            [NEW - 100 lines]
```

---

#### Phase 2: Graph Composition (Week 3-4)

**Goal:** Implement EffuseGraph for composing widgets via specs.

**Tasks:**

- [ ] **Create EffuseGraph implementation** (`src/effuse/graph/effuse-graph.ts`)
  - makeEffuseGraph() factory function
  - Child widget instantiation from specs
  - Pin creation from specs
  - Merge setup (pin connections)
  - Effect scoping for lifecycle

- [ ] **Create Graph types** (`src/effuse/graph/types.ts`)
  - EffuseGraph interface (extends Widget)
  - EffuseGraphSpec type
  - MergeSpec type
  - GraphMetadata type

- [ ] **Implement GraphSpec serialization** (`src/effuse/spec/graph-spec.ts`)
  - graphToSpec() - serialize EffuseGraph to JSON
  - graphFromSpec() - deserialize JSON to EffuseGraph
  - Bundle specs (with dependencies)

- [ ] **Update mount system** (`src/effuse/widget/mount.ts`)
  - Add mountGraph() for EffuseGraph widgets
  - Handle child widget scoping
  - Preserve existing mountWidget() for simple widgets

- [ ] **Write Graph tests** (`src/effuse/graph/effuse-graph.test.ts`)
  - Test child widget mounting
  - Test pin merges (data flow)
  - Test graph lifecycle (children destroyed with parent)
  - Test spec serialization

**Acceptance Criteria:**

- EffuseGraph is a valid Widget (can be mounted)
- Child widgets mount correctly in containers
- Pin merges propagate data between children
- Graph lifecycle uses Effect scoping
- Specs serialize/deserialize correctly

**Files to Create:**

```
src/effuse/
├── graph/
│   ├── effuse-graph.ts         [NEW - 400 lines]
│   ├── types.ts                [NEW - 150 lines]
│   ├── merge.ts                [NEW - 200 lines]
│   └── effuse-graph.test.ts    [NEW - 400 lines]
└── spec/
    └── graph-spec.ts           [NEW - 300 lines]
```

**Files to Modify:**

```
src/effuse/widget/
└── mount.ts                    [MODIFY - add mountGraph() ~100 lines]
```

---

#### Phase 3: Widget Registry (Week 5)

**Goal:** Create widget registry for spec-based instantiation.

**Tasks:**

- [ ] **Create widget registry** (`src/effuse/registry/widget-registry.ts`)
  - Map widget IDs to Widget constructors
  - getWidget() lookup function
  - registerWidget() registration function

- [ ] **Register all existing widgets**
  - APMWidget, TBControlsWidget, MCTasksWidget, etc.
  - Assign unique IDs to each widget type

- [ ] **Implement WidgetSpec system** (`src/effuse/spec/widget-spec.ts`)
  - widgetToSpec() - serialize Widget + state
  - widgetFromSpec() - deserialize spec to Widget
  - Handle initialState override

- [ ] **Write registry tests** (`src/effuse/registry/widget-registry.test.ts`)
  - Test widget lookup
  - Test spec serialization/deserialization
  - Test initialState override

**Acceptance Criteria:**

- All existing widgets registered with unique IDs
- widgetFromSpec() correctly instantiates widgets
- initialState override works
- Tests pass

**Files to Create:**

```
src/effuse/
├── registry/
│   ├── widget-registry.ts       [NEW - 100 lines]
│   └── widget-registry.test.ts  [NEW - 150 lines]
└── spec/
    ├── widget-spec.ts           [NEW - 150 lines]
    └── types.ts                 [NEW - 100 lines]
```

---

#### Phase 4: Example Migration (Week 6)

**Goal:** Migrate TB Command Center to EffuseGraph and validate all features.

**Tasks:**

- [ ] **Create TBCCShellSpec** (`src/effuse/widgets/tb-command-center/tbcc-shell-spec.ts`)
  - Define children (dashboard, tasks, testgen)
  - Define pins (activeTab)
  - Define merges (tab visibility)
  - Define component structure

- [ ] **Create TBCCShellGraph**
  - Use makeEffuseGraph(TBCCShellSpec)
  - Replace manual tab switching with pin merges

- [ ] **Test migration**
  - Verify all tabs render correctly
  - Verify tab switching works via pin
  - Verify HMR preserves state
  - Verify spec serialization

- [ ] **Document patterns** (`docs/effuse/VISUAL-LANGUAGE.md`)
  - EffusePin usage examples
  - EffuseGraph composition patterns
  - WidgetSpec serialization guide
  - Migration guide (simple widget → graph)

**Acceptance Criteria:**

- TB Command Center works identically to before
- Tab switching uses pin merges (no manual DOM manipulation)
- HMR preserves full graph state
- Spec can be saved/loaded from JSON
- Documentation complete

**Files to Create:**

```
src/effuse/widgets/tb-command-center/
└── tbcc-shell-spec.ts          [NEW - 100 lines]

docs/effuse/
└── VISUAL-LANGUAGE.md          [NEW - 2000 lines]
```

**Files to Modify:**

```
src/mainview/
└── effuse-main.ts              [MODIFY - use TBCCShellGraph ~10 lines changed]
```

---

#### Phase 5: Visual Editor Foundation (Week 7-8)

**Goal:** Create foundation for visual spec editing and graph visualization.

**Tasks:**

- [ ] **Create spec editor widget** (`src/effuse/widgets/spec-editor.ts`)
  - JSON editor for WidgetSpec/GraphSpec
  - Live preview of spec changes
  - Save/load specs to localStorage
  - Validation and error display

- [ ] **Create graph visualizer** (`src/effuse/widgets/graph-visualizer.ts`)
  - Render EffuseGraph as node graph (boxes + arrows)
  - Show widgets as nodes
  - Show pin merges as edges
  - Click node to edit widget spec

- [ ] **Create example dashboard** (`src/effuse/widgets/example-dashboard.ts`)
  - Showcase EffuseGraph composition
  - Multiple widgets with pin merges
  - Editable via spec editor

- [ ] **Integration**
  - Add "Edit Spec" button to widgets
  - Add graph visualizer to dev tools
  - Document visual editor usage

**Acceptance Criteria:**

- Spec editor can edit and preview WidgetSpec
- Graph visualizer shows widget tree structure
- Specs can be saved/loaded
- Example dashboard demonstrates composition
- Dev tools include graph visualizer

**Files to Create:**

```
src/effuse/widgets/
├── spec-editor.ts              [NEW - 500 lines]
├── graph-visualizer.ts         [NEW - 600 lines]
└── example-dashboard.ts        [NEW - 200 lines]
```

---

### 4.2 Backward Compatibility

**Strategy:** Keep existing Effuse widgets working unchanged.

**Simple widgets** (APM, TB Output, Container, etc.):
- Continue using Widget<S, E, R> directly
- No migration required
- Can be used as children in EffuseGraph

**Complex layouts** (TB Command Center, Dashboard):
- Migrate to EffuseGraph for composition benefits
- Remove manual DOM manipulation
- Use pin merges for widget communication

**Both approaches supported simultaneously:**

```typescript
// Simple widget (unchanged)
const SimpleWidget: Widget<State, Event> = {
  id: "simple",
  initialState: () => ({ count: 0 }),
  render: (ctx) => html`<div>${ctx.state.count}</div>`,
  // ...
}

// Complex layout (migrated to graph)
const ComplexLayoutSpec: EffuseGraphSpec = {
  id: "complex",
  children: {
    simple1: { id: "simple" },  // Uses SimpleWidget
    simple2: { id: "simple" }
  },
  merges: { /* ... */ }
}

const ComplexLayoutGraph = makeEffuseGraph(ComplexLayoutSpec)
```

**Migration checklist for widgets:**

- [ ] Widget has 3+ children? → Consider EffuseGraph
- [ ] Widget has manual DOM manipulation? → Migrate to EffuseGraph
- [ ] Widget has complex event routing? → Use pin merges
- [ ] Widget is simple (<200 lines, no children)? → Keep as-is

---

### 4.3 Testing Strategy

**Unit Tests:**

- EffusePin (all pin types, semantics, snapshot/restore)
- EffuseGraph (child mounting, merges, lifecycle)
- Spec serialization (round-trip, validation)
- Widget registry (lookup, registration)

**Integration Tests:**

- Graph composition (multi-level nesting)
- Mount lifecycle (scoping, cleanup)
- HMR (state preservation across hot reloads)
- Pin merges (data flow between widgets)

**Migration Tests:**

- Ensure existing widgets still work
- TB Command Center functional parity
- No visual regressions

**Example Apps:**

- TB Command Center as EffuseGraph
- Example dashboard showcasing composition
- Spec editor + graph visualizer

**Test Infrastructure:**

```typescript
// Use existing makeTestLayer() with graph support
const { layer, injectMessage } = yield* makeTestLayer()

// Mount graph
const graphSpec: EffuseGraphSpec = { /* ... */ }
const graph = makeEffuseGraph(graphSpec)
yield* mountWidget(graph, container).pipe(Effect.provide(layer))

// Test pin merges
const pin = resolvePinPath("child1/output", graph)
yield* pin?.set(42)
yield* Effect.sleep(50)  // Allow merge to propagate

const targetPin = resolvePinPath("child2/input", graph)
const value = yield* targetPin?.get
expect(value).toBe(42)
```

---

## 5. Code Harvest Inventory

### 5.1 Files to Harvest from Unit Framework

| Unit Source | Lines | Purpose | Harvest Strategy | Adaptation Effort |
|-------------|-------|---------|------------------|-------------------|
| `src/Pin.ts` | 360 | Pin implementation | **HIGH** - Core primitive | Medium - Replace events with Streams |
| `src/Class/Graph/index.ts` | ~3000 | Graph composition | **MEDIUM** - Composition logic only | High - Extract core, adapt to Effect |
| `src/spec/fromSpec.ts` | 150 | Spec deserialization | **HIGH** - Dynamic class generation | Medium - Adapt for EffuseGraphSpec |
| `src/spec/stringify.ts` | ~200 | Spec serialization | **HIGH** - State capture | Low - Straightforward adaptation |
| `src/types/GraphSpec.ts` | ~100 | Spec type definitions | **HIGH** - Type structure | Low - Rename, adapt to TS |
| `src/EventEmitter.ts` | ~100 | Event system | **SKIP** - Use Effect Streams | N/A |
| `src/Class/Merge.ts` | ~200 | Pin connections | **MEDIUM** - Merge logic | Medium - Adapt to EffusePin |
| `src/system.ts` | ~100 | System interface | **SKIP** - Use Context.Tag | N/A |

**Total harvest:** ~4,210 lines from Unit → ~2,000 lines adapted for Effuse

### 5.2 Adaptation Checklist

For each harvested file:

- [ ] **Replace event emitters with Effect Streams**
  - `pin.emit('data', value)` → `pin.changes` (Stream)
  - `pin.addListener('data', cb)` → `Stream.tap(cb)`

- [ ] **Replace System object with Context.Tag services**
  - `new Pin({}, system)` → `makeEffusePin({})` (uses Scope)
  - `system.specs` → `WidgetRegistry`

- [ ] **Add Effect error handling**
  - Create PinError, GraphError types
  - Use Effect error channel (E in Effect<A, E>)

- [ ] **Add TypeScript strict types**
  - Full generics (S, E, R) for widgets
  - Strict null checks
  - No `any` types

- [ ] **Add Effect-native tests**
  - Use makeTestLayer() for mocks
  - Use Effect.runPromise for test execution
  - Effect.scoped for cleanup

### 5.3 Line-by-Line Adaptation Example: Pin.push()

**Unit Pin (event-based):**

```typescript
// ~/code/unit/src/Pin.ts:229-249
private _push_data(
  data: any,
  backpropagation: boolean = false,
  propagate: boolean = true
): void {
  this.invalidate()          // Sync method
  this._invalid = false
  this.start()               // Emits 'start' event
  this._register = data

  if (propagate) {
    this.emit('data', data, backpropagation)  // Event emitter
  }

  if (this._ignored && !this._constant) {
    this.take()
  }
}
```

**EffusePin (Effect-based):**

```typescript
// src/effuse/pin/effuse-pin.ts
set: (value: A) =>
  Effect.gen(function* () {
    yield* Ref.set(invalidRef, false)  // Effect operation
    yield* cell.set(value)              // Effect operation (publishes to Stream)
    // StateCell.set already publishes to changes stream (no manual emit)
  })
```

**Key changes:**

- Sync methods → Effect.gen
- Event emitters → Stream (via StateCell)
- Manual state mutations → Ref operations

### 5.4 Harvesting Pin Snapshot/Restore

**Unit Pin:**

```typescript
// ~/code/unit/src/Pin.ts:327-349
public snapshot(): Pin_M<T> {
  return {
    _register: this._register instanceof $ ? undefined : this._register,
    ...(this._invalid ? { _invalid: true } : {}),
    ...(this._constant ? { _constant: true } : {}),
    ...(this._ignored ? { _ignored: true } : {}),
    ...(this._idle ? {} : { _idle: false }),
  }
}

public restore(state: Pin_M<T>): void {
  const { _register, _invalid, _constant, _ignored, _idle } = state
  this._register = _register
  this._invalid = _invalid
  this._constant = _constant
  this._ignored = _ignored
  this._idle = _idle
}
```

**EffusePin:**

```typescript
// src/effuse/pin/effuse-pin.ts
snapshot: Effect.gen(function* () {
  const value = yield* cell.get
  const invalid = yield* Ref.get(invalidRef)
  return {
    value,
    constant: constantFlag,
    ref: refFlag,
    invalid
  }
}),

restore: (state: PinSnapshot<A>) =>
  Effect.gen(function* () {
    yield* cell.set(state.value)
    yield* Ref.set(invalidRef, state.invalid)
  })
```

**Benefits of Effect version:**

- Type-safe operations (no exceptions)
- Composable with other Effects
- Testable with Effect layers

---

## 6. Critical Files & Changes

### 6.1 New Files to Create

```
src/effuse/
├── pin/                                 [NEW DIRECTORY]
│   ├── effuse-pin.ts                   [NEW - 200 lines] EffusePin implementation
│   ├── types.ts                        [NEW - 50 lines] EffusePin types
│   └── effuse-pin.test.ts              [NEW - 300 lines] Pin tests
│
├── graph/                               [NEW DIRECTORY]
│   ├── effuse-graph.ts                 [NEW - 400 lines] EffuseGraph implementation
│   ├── merge.ts                        [NEW - 200 lines] Pin merge system
│   ├── types.ts                        [NEW - 150 lines] Graph types
│   └── effuse-graph.test.ts            [NEW - 400 lines] Graph tests
│
├── spec/                                [NEW DIRECTORY]
│   ├── pin-spec.ts                     [NEW - 100 lines] Pin serialization
│   ├── graph-spec.ts                   [NEW - 300 lines] Graph spec system
│   ├── widget-spec.ts                  [NEW - 150 lines] Widget spec system
│   └── types.ts                        [NEW - 100 lines] Spec type definitions
│
├── registry/                            [NEW DIRECTORY]
│   ├── widget-registry.ts              [NEW - 100 lines] Widget ID → constructor
│   └── widget-registry.test.ts         [NEW - 150 lines] Registry tests
│
└── widgets/
    ├── spec-editor.ts                  [NEW - 500 lines] Visual spec editor
    ├── graph-visualizer.ts             [NEW - 600 lines] Graph visualization
    ├── example-dashboard.ts            [NEW - 200 lines] Example graph
    └── tb-command-center/
        └── tbcc-shell-spec.ts          [NEW - 100 lines] TB CC spec

docs/
├── research/
│   └── unit.md                         [NEW - 7000 lines] This document
└── effuse/
    └── VISUAL-LANGUAGE.md              [NEW - 2000 lines] Visual language guide
```

**Total new code:** ~11,000 lines

### 6.2 Modified Files

```
src/effuse/
├── widget/
│   ├── mount.ts                        [MODIFY - add mountGraph() ~100 lines]
│   └── types.ts                        [MODIFY - add EffuseGraph union type ~10 lines]
│
├── layers/
│   └── test.ts                         [MODIFY - mock EffusePin, Graph ~50 lines]
│
└── hmr/
    └── registry.ts                     [MODIFY - handle EffuseGraph state ~50 lines]

src/mainview/
└── effuse-main.ts                      [MODIFY - use TBCCShellGraph ~10 lines]

docs/effuse/
├── README.md                           [MODIFY - add visual language overview ~100 lines]
└── ARCHITECTURE.md                     [MODIFY - add EffuseGraph lifecycle ~200 lines]
```

**Total modified code:** ~520 lines changed

### 6.3 File Dependency Graph

```
┌─────────────────────────────────────────────────────────────────┐
│                         Dependency Graph                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Application Layer                                               │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  src/mainview/effuse-main.ts                               │ │
│  │    ├─ imports TBCCShellGraph                               │ │
│  │    └─ mountWidget(TBCCShellGraph, container)               │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              ↓                                   │
│  Widget Layer                                                    │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  src/effuse/widgets/tb-command-center/tbcc-shell-spec.ts   │ │
│  │    ├─ imports makeEffuseGraph                              │ │
│  │    └─ exports TBCCShellSpec, TBCCShellGraph                │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              ↓                                   │
│  Graph Layer                                                     │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  src/effuse/graph/effuse-graph.ts                          │ │
│  │    ├─ imports makeEffusePin (pin/effuse-pin.ts)            │ │
│  │    ├─ imports mountWidget (widget/mount.ts)                │ │
│  │    ├─ imports WidgetRegistry (registry/widget-registry.ts) │ │
│  │    └─ exports makeEffuseGraph                              │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              ↓                                   │
│  Pin Layer                                                       │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  src/effuse/pin/effuse-pin.ts                              │ │
│  │    ├─ imports makeCell (state/cell.ts)                     │ │
│  │    └─ exports makeEffusePin                                │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              ↓                                   │
│  State Layer (Current)                                           │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  src/effuse/state/cell.ts                                  │ │
│  │    ├─ imports Effect, Ref, Queue, Stream                   │ │
│  │    └─ exports makeCell, StateCell                          │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**No circular dependencies:** Clean layered architecture.

---

## 7. Risk Assessment & Mitigation

### 7.1 Identified Risks

#### Risk 1: Complexity Creep

**Description:** Adding graph layer increases cognitive load for developers.

**Likelihood:** Medium
**Impact:** Medium
**Overall Risk:** Medium

**Mitigation:**

- Keep simple widgets unchanged (backward compatibility)
- Only use graphs for complex layouts (3+ children)
- Comprehensive documentation with examples
- Team training session on visual language concepts
- Gradual migration (TB Command Center first, then others)

**Success Metric:** New developers can add simple widgets without understanding graphs.

---

#### Risk 2: Performance Overhead

**Description:** Pin abstraction + StateCell + Stream merges could be slow.

**Likelihood:** Medium
**Impact:** High
**Overall Risk:** High

**Mitigation:**

- Benchmark early (Phase 1 - measure pin operations)
- Optimize hot paths (lazy pin instantiation)
- Lazy graph instantiation (don't create all children upfront)
- Profile TB Command Center migration (Phase 4)
- Add performance tests to CI

**Performance Budget:**

- Pin set/get: <1ms
- Graph mount: <100ms for 10 children
- Pin merge propagation: <5ms

**Success Metric:** No measurable performance regression vs current Effuse.

---

#### Risk 3: Breaking HMR

**Description:** Graph state preservation more complex than simple widgets.

**Likelihood:** High
**Impact:** High
**Overall Risk:** High

**Mitigation:**

- Comprehensive HMR tests (Phase 2, Phase 4)
- Use snapshot/restore for all graph state
- Test nested graphs (graph within graph)
- Test pin state preservation
- Fallback: Clear state on HMR failure (better than crash)

**HMR Test Scenarios:**

- Simple widget: State preserved? ✓
- Graph with 3 children: All child states preserved? ✓
- Graph with pin merges: Merge connections preserved? ✓
- Nested graph: All levels preserved? ✓

**Success Metric:** HMR works for graphs as well as simple widgets.

---

#### Risk 4: Team Learning Curve

**Description:** New concepts (pins, merges, specs) require learning time.

**Likelihood:** High
**Impact:** Medium
**Overall Risk:** Medium

**Mitigation:**

- Thorough documentation (VISUAL-LANGUAGE.md)
- Code examples for common patterns
- Migrate one widget together as team (pair programming)
- Visual graph editor to abstract complexity
- Gradual rollout (8 weeks, not all at once)

**Training Plan:**

- Week 1: Intro to pins (1-hour session)
- Week 3: Intro to graphs (1-hour session)
- Week 5: Intro to specs (1-hour session)
- Week 6: Team migration exercise (TB Command Center)

**Success Metric:** All team members can create an EffuseGraph after training.

---

#### Risk 5: Serialization Edge Cases

**Description:** Not all state is JSON-serializable (functions, closures, etc.).

**Likelihood:** High
**Impact:** Medium
**Overall Risk:** Medium

**Mitigation:**

- Document serialization limitations
- Provide serialization helpers for common types
- Validate specs on deserialization
- Error messages guide users to fix non-serializable state
- Use structuredClone fallback for non-JSON state

**Non-Serializable Types:**

- Functions: Store function name, look up in registry
- Closures: Don't serialize, warn user
- DOM references: Don't serialize, recreate on mount
- Streams: Don't serialize, recreate from spec

**Success Metric:** 90% of widget state serializes correctly.

---

### 7.2 Success Criteria

Effuse Visual Language is successful if:

- [ ] **TB Command Center migrated to EffuseGraph without functional regression**
  - All tabs work
  - Tab switching uses pin merges
  - No visual changes
  - No performance degradation

- [ ] **HMR works with graph widgets (preserve full state across hot reloads)**
  - Graph state preserved
  - Child widget states preserved
  - Pin states preserved
  - Merge connections preserved

- [ ] **Specs can be saved/loaded from JSON**
  - GraphSpec serializes to valid JSON
  - JSON deserializes to working EffuseGraph
  - Round-trip preserves all state

- [ ] **Performance parity or better vs current Effuse**
  - Pin operations <1ms
  - Graph mount <100ms
  - No frame drops during updates

- [ ] **All existing widgets continue working unchanged**
  - APMWidget, TBControlsWidget, MCTasksWidget, etc.
  - No breaking changes to Widget<S, E, R> interface
  - Tests pass for all existing widgets

---

## 8. Future Vision

### 8.1 What Effuse Visual Language Enables

#### 1. Visual Widget Editor

**Description:** Drag-drop graph editor for composing widgets without code.

**User Flow:**

1. Open visual editor (new route: `/editor`)
2. Drag widgets from palette onto canvas
3. Connect pins by dragging arrows between nodes
4. Edit widget properties in sidebar
5. Save spec to JSON
6. Load spec in main app

**Technical Implementation:**

- Use graph-visualizer.ts as foundation
- Add drag-drop (HTML5 Drag & Drop API)
- Add pin connection UI (click source pin → click target pin → create merge)
- Real-time preview (render graph as you edit)
- Export to WidgetSpec JSON

**Timeline:** 2-3 weeks after Phase 5

---

#### 2. User-Configurable Dashboards

**Description:** Users save/load custom layouts without developer involvement.

**User Flow:**

1. User rearranges widgets on dashboard
2. User clicks "Save Layout"
3. Spec saved to localStorage (or backend)
4. On next visit, layout restored from saved spec

**Technical Implementation:**

- Add "Edit Mode" toggle to dashboard
- In edit mode: drag widgets, resize, reorder
- Capture changes as GraphSpec modifications
- Save spec on "Save Layout"
- Load spec on mount (check localStorage first)

**Timeline:** 1-2 weeks after visual editor

---

#### 3. Widget Marketplace

**Description:** Share WidgetSpecs as JSON files, import into any Effuse app.

**User Flow:**

1. Developer creates reusable widget (e.g., "Kanban Board")
2. Export widget spec to JSON file
3. Upload to widget marketplace (website or npm package)
4. Other developers download JSON spec
5. Import into their app via spec editor

**Technical Implementation:**

- Create widget marketplace website
- JSON spec validation on upload
- Search/filter by tags, author
- Download as .json file
- Import via spec editor

**Timeline:** 4-6 weeks (requires backend infrastructure)

---

#### 4. AI-Generated UIs

**Description:** LLM outputs WidgetSpec JSON, renders instantly without code generation.

**User Flow:**

1. User describes desired UI: "Create a dashboard with APM on left, TB controls on right"
2. LLM generates EffuseGraphSpec JSON
3. User pastes JSON into spec editor
4. UI renders instantly
5. User tweaks via visual editor

**Technical Implementation:**

- Provide LLM with EffuseGraphSpec schema
- Provide LLM with WidgetRegistry (available widgets)
- LLM generates valid JSON
- Validate spec before rendering
- Error messages guide LLM to fix invalid specs

**Timeline:** 1-2 weeks (LLM prompt engineering)

---

#### 5. Time Travel Debugging

**Description:** Snapshot graph state at any point, restore later for debugging.

**User Flow:**

1. App running, user encounters bug
2. User clicks "Snapshot State"
3. Full graph state saved (all widgets, all pins)
4. User reproduces bug
5. User clicks "Restore Snapshot"
6. App returns to exact state before bug

**Technical Implementation:**

- Add global snapshot button
- Capture all widget states via widgetToSpec
- Capture all pin states via pin.snapshot()
- Store in IndexedDB
- Restore via widgetFromSpec + pin.restore()

**Timeline:** 2-3 weeks

---

#### 6. Multi-User Collaboration

**Description:** Sync WidgetSpec changes via CRDT, multiple users edit same dashboard.

**User Flow:**

1. User A and User B open same dashboard
2. User A moves widget, User B sees change instantly
3. User B adds new widget, User A sees it
4. No conflicts, automatic merge via CRDT

**Technical Implementation:**

- Convert EffuseGraphSpec to CRDT (Yjs or Automerge)
- WebSocket sync between clients
- Operational transformation for concurrent edits
- Conflict resolution (last-write-wins or custom)

**Timeline:** 6-8 weeks (requires CRDT integration)

---

### 8.2 Roadmap After Initial Implementation

**Q1 2026 (Weeks 9-12):**

- [ ] Visual graph editor (drag-drop, pin connections)
- [ ] User-configurable dashboards (save/load layouts)
- [ ] Time travel debugging (snapshot/restore)

**Q2 2026 (Weeks 13-24):**

- [ ] Widget marketplace (JSON spec sharing)
- [ ] AI-generated UIs (LLM → WidgetSpec)
- [ ] Advanced visualizations (graph performance metrics, dependency tree)

**Q3 2026 (Weeks 25-36):**

- [ ] Multi-user collaboration (CRDT sync)
- [ ] Widget versioning (semantic versioning for specs)
- [ ] Widget templates (reusable patterns, starter kits)

**Q4 2026 (Weeks 37-48):**

- [ ] Visual query builder (connect to data sources)
- [ ] Animation timeline (keyframe animations for widgets)
- [ ] Accessibility tools (screen reader support, keyboard nav)

---

### 8.3 Success Metrics (Future)

**Adoption:**

- 50% of widgets use EffuseGraph within 6 months
- 10+ community-contributed widgets in marketplace within 1 year
- 5+ custom dashboards created by users within 3 months

**Performance:**

- Graph mount time <50ms (2x faster than current manual patterns)
- Pin merge latency <1ms (real-time reactivity)
- Spec serialization <10ms (instant save/load)

**Developer Experience:**

- Widget creation time reduced by 50% (graph vs manual)
- Onboarding time reduced by 30% (visual editor vs code)
- Bug reproduction time reduced by 70% (time travel debugging)

---

## Conclusion

**Effuse Visual Language** represents a major evolution of the Effuse framework, harvesting the best ideas from Unit's visual programming primitives and adapting them to be Effect-native. This document provides a complete roadmap for implementation, from core primitives (EffusePin) to visual composition (EffuseGraph) to future capabilities (visual editor, AI-generated UIs).

**Key Takeaways:**

1. **We're not replacing Effuse** - We're enhancing it with visual primitives
2. **Effect-native throughout** - All reactivity via Streams, all services via Context.Tag
3. **Backward compatible** - Existing simple widgets unchanged
4. **8-week phased rollout** - Low risk, incremental delivery
5. **Future-ready** - Foundation for visual editors, AI generation, collaboration

**Next Steps:**

1. Review this document with team
2. Approve migration plan
3. Begin Phase 1 (Core Primitives)
4. Iterate based on learnings

---

**Document Metadata:**

- **Total Pages:** 32 (estimated when rendered)
- **Total Words:** ~11,000
- **Code Examples:** 45+
- **Diagrams:** 5 (ASCII art)
- **References to Unit Source:** 20+ file paths with line numbers
- **Implementation-Ready:** Yes (file paths, code examples, test plans included)

---

**End of Document**

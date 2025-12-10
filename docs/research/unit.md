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
| **Graph** | `src/Class/Graph/index.ts` (~3000 lines) | Composable unit trees with parent-child relationships | EffuseGraph uses Effect scoping, Component composition |
| **Spec** | `src/spec/fromSpec.ts`, `stringify.ts` | JSON serialization of units | ComponentSpec system for save/load, HMR, visual editing |

### 1.3 Why This Enhances Effuse

**Current Effuse Limitations:**

1. **Weak Composition** - Parent-child component relationships require manual patterns (see `tbcc-shell.ts:75-95`)
2. **No Serialization** - Can't save/load component configurations or layouts
3. **Manual State Preservation** - HMR uses `structuredClone` with migration hacks
4. **No Visual Programming** - All component composition is imperative code

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
│  Component<S, E, R>                     [Current - KEEP]        │
│      │                                                           │
│      ├─── EffuseGraph<S, E, R>          [NEW - Composition]     │
│      │        ├─ EffusePin<A>           [NEW - Data conduits]   │
│      │        ├─ ComponentSpec         [NEW - Serialization]   │
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
| **Component Composition** | Manual patterns, direct DOM manipulation | Declarative specs, native parent-child |
| **State Sharing** | Custom events + subscriptions | Pin merges (data flows through connections) |
| **Configuration** | Hard-coded in `initialState()` | JSON ComponentSpec (save/load) |
| **HMR** | `structuredClone` + manual migrations | Snapshot/restore with automatic state preservation |
| **Visual Programming** | Not possible | Foundation for graph editor, drag-drop composition |
| **AI Generation** | Would need to write code | Can output ComponentSpec JSON directly |

### 1.6 Timeline

**8-week phased rollout:**

- **Weeks 1-2:** Core primitives (EffusePin, PinSpec)
- **Weeks 3-4:** Graph composition (EffuseGraph, GraphSpec)
- **Week 5:** Component registry and spec system
- **Week 6:** Example migration (TB Command Center)
- **Weeks 7-8:** Visual editor foundation

**Backward compatible:** Existing simple components unchanged, only complex layouts migrate to EffuseGraph.

**Note:** Effuse currently uses the term "widget" throughout the codebase, but it's in the process of being refactored to use "component" instead. This document uses "component" to reflect the intended terminology.

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

1. **Constant pins** - Perfect for persistent component configuration (theme, layout prefs)
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
      id: "9988a56e-6bee-46c8-864c-e351d84bc7e2",  // APM component type ID
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
APM Component (sessionAPM pin)
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

// Add a new component
dashboard.addUnit('newComponent', {
  id: 'container-component-id',
  input: { content: { data: 'Hello' } }
})

// Connect it to existing component
dashboard.addMerge('apm_to_new', {
  input: { 'apm/sessionAPM': {} },
  output: { 'newComponent/value': {} }
})

// Get updated spec
const updatedSpec = dashboard.getSpec()
// Can serialize and save this
```

#### 2.2.7 Valuable for Effuse

**Harvest these concepts:**

1. **Parent-child component composition** - Fixes current manual patterns (direct classList manipulation)
2. **Declarative composition via specs** - JSON describes component tree structure
3. **Pin merges for component communication** - Better than custom events + subscriptions
4. **Slot system** - Named content regions (sidebar, main, footer, etc.)
5. **Runtime composition** - Add/remove/reconnect components dynamically
6. **Spec serialization** - Save entire component tree state to JSON

**Adaptation needed:**

- Replace Unit with Component<S, E, R>
- Replace event-based merges with Effect Stream-based merges
- Use Effect scoping for child lifecycle (Effect.scoped, not manual destroy)
- Add service dependencies (R in Component<S, E, R>)

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

// Example: Dashboard bundle includes specs for all child components
const dashboardBundle: BundleSpec = {
  spec: {
    id: "dashboard",
    units: {
      apm: { id: "apm-component" },
      tb: { id: "tb-controls" }
    }
  },
  specs: {
    "apm-component": { /* APM component spec */ },
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

1. **Spec → Class pattern** - Dynamic component class generation from JSON
2. **Bundle specs** - Capture entire component tree + dependencies
3. **Metadata** - Author, version, description for component marketplace
4. **State capture** - Serialize current runtime state (not just initial config)
5. **Dependency resolution** - Automatically include all required child specs

**Adaptation needed:**

- Create ComponentSpec interface (similar to GraphSpec structure)
- Add Component registry (id → Component<S, E, R> mapping)
- Serialize Effect service requirements (R in Component<S, E, R>)
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
│  │    ├─ Tasks (Simple Component)                                 │  │
│  │    └─ TestGen (EffuseGraph with pin merges)                    │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                              ↓                                        │
│  Composition Layer                                                    │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  EffuseGraph<S, E, R> implements Component<S, E, R>            │  │
│  │    ├─ children: Record<string, Component<any, any, any>>        │  │
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
│  │  Component<S, E, R>                                             │  │
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
3. **Full generics preserved** - Component<S, E, R>, type safety throughout
4. **Backward compatible** - Simple components unchanged, only complex layouts use graphs

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

**Example 1: Constant pin for component configuration**

```typescript
// Theme configuration that persists
const themePinEffect = makeEffusePin(
  { mode: "dark", accentColor: "#007bff" },
  { constant: true }  // Never consumed, always available
)

const themePin = yield* themePinEffect

// Components can pull() repeatedly without consuming
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
// Shared component registry (reference semantics)
const registryPin = yield* makeEffusePin(
  new Map<string, Component<any, any, any>>(),
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

### 3.3 EffuseGraph - Composable Component Trees

**Harvest:** Graph composition, parent-child relationships, merge connections
**Adapt:** Use Effect scoping for lifecycle, Components instead of Units

#### 3.3.1 Interface Design

```typescript
// src/effuse/graph/types.ts

import type { Component, ComponentContext } from "../component/types.js"
import type { EffusePin } from "../pin/types.js"
import type { TemplateResult } from "../template/html.js"

export interface EffuseGraph<S, E, R = never> extends Component<S, E, R> {
  // Graph is a Component (can be mounted like any component)
  // S = graph state, E = graph events, R = service requirements
}

export interface EffuseGraphSpec {
  id: string
  name?: string

  // Child components
  children: Record<string, ComponentSpec>

  // Graph-level pins (not component state)
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

export interface ComponentSpec {
  id: string                    // Component type ID (key into ComponentRegistry)
  initialState?: unknown        // Override component's initialState()
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
import type { Component, ComponentContext } from "../component/types.js"
import { makeEffusePin } from "../pin/effuse-pin.js"
import { mountComponent } from "../component/mount.js"
import { ComponentRegistry } from "../registry/component-registry.js"

export const makeEffuseGraph = <S, E, R>(
  spec: EffuseGraphSpec
): EffuseGraph<S, E, R> => {
  const graph: EffuseGraph<S, E, R> = {
    id: spec.id,

    // Graph-level state (not child state)
    initialState: () => ({} as S),  // Could be empty or spec-derived

    render: (ctx: ComponentContext<S, E>) =>
      Effect.gen(function* () {
        // Render containers for each child component
        const childContainers = spec.component.children.map(childId =>
          html`<div id="${spec.id}-${childId}" class="effuse-graph-child"></div>`
        )

        return html`
          <div class="effuse-graph-container" data-graph-id="${spec.id}">
            ${joinTemplates(childContainers)}
          </div>
        `
      }),

    setupEvents: (ctx: ComponentContext<S, E>) =>
      Effect.gen(function* () {
        // Create pins
        const pins: Record<string, EffusePin<any>> = {}
        for (const [pinId, pinSpec] of Object.entries(spec.pins || {})) {
          pins[pinId] = yield* makeEffusePin(pinSpec.data, {
            constant: pinSpec.constant,
            ref: pinSpec.ref
          })
        }

        // Mount child components
        const children: Record<string, any> = {}
        for (const [childId, componentSpec] of Object.entries(spec.children)) {
          const component = ComponentRegistry[componentSpec.id]
          if (!component) {
            throw new Error(`Component not found: ${componentSpec.id}`)
          }

          // Override initialState if provided in spec
          const childComponent = componentSpec.initialState
            ? { ...component, initialState: () => componentSpec.initialState }
            : component

          // Mount child in its container
          const container = yield* ctx.dom.queryId(`${spec.id}-${childId}`)
          const mounted = yield* mountComponent(childComponent, container)

          children[childId] = { component: childComponent, mounted }
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
// src/effuse/components/tb-command-center/tbcc-shell.ts (current)
export const TBCCShellComponent: Component<TBCCShellState, TBCCShellEvent, SocketServiceTag> = {
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
// src/effuse/components/tb-command-center/tbcc-shell-spec.ts (new)
export const TBCCShellSpec: EffuseGraphSpec = {
  id: "tbcc-shell",
  name: "TB Command Center Shell",

  // Child components
  children: {
    dashboard: {
      id: "tbcc-dashboard",  // Component type ID
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
    author: "OpenAgents",
    version: "2.0.0"
  }
}

// Create the graph component
export const TBCCShellGraph = makeEffuseGraph<TBCCShellState, TBCCShellEvent, SocketServiceTag>(
  TBCCShellSpec
)
```

**Benefits:**

- No manual DOM manipulation (classList.toggle removed)
- Declarative tab switching via pin merge
- Serializable spec (can save/load layout)
- Visual structure explicit in spec
- Backward compatible (old components still work)

---

### 3.4 ComponentSpec - Serializable Component Configuration

**Harvest:** Spec serialization system, snapshot/restore
**Adapt:** Component interface → JSON spec, Effect services in layer

#### 3.4.1 Component Registry

```typescript
// src/effuse/registry/component-registry.ts

import type { Component } from "../component/types.js"
import { APMComponent } from "../components/apm-component.js"
import { TBControlsComponent } from "../components/tb-controls.js"
import { MCTasksComponent } from "../components/mc-tasks.js"
// ... all components

export const ComponentRegistry: Record<string, Component<any, any, any>> = {
  "apm-component": APMComponent,
  "tb-controls": TBControlsComponent,
  "mc-tasks": MCTasksComponent,
  "tbcc-dashboard": TBCCDashboardComponent,
  "tbcc-tasks": TBCCTasksComponent,
  "tbcc-testgen": TBCCTestGenComponent,
  "three-background": ThreeBackgroundComponent,
  "intro-card": IntroCardComponent,
  "tb-output": TBOutputComponent,
  "container-component": ContainerComponent,
  // etc
}

export const getComponent = (id: string): Component<any, any, any> | undefined => {
  return ComponentRegistry[id]
}

export const registerComponent = (id: string, component: Component<any, any, any>): void => {
  ComponentRegistry[id] = component
}
```

#### 3.4.2 Spec Serialization

```typescript
// src/effuse/spec/component-spec.ts

import type { Component } from "../component/types.js"
import type { ComponentSpec } from "../graph/types.js"

export const componentToSpec = <S>(
  component: Component<S, any, any>,
  state: S
): ComponentSpec => ({
  id: component.id,
  initialState: state,
  // TODO: Add pins, children if graph
})

export const componentFromSpec = (
  spec: ComponentSpec
): Component<any, any, any> => {
  const component = ComponentRegistry[spec.id]
  if (!component) {
    throw new Error(`Component not found in registry: ${spec.id}`)
  }

  // Override initialState if provided in spec
  if (spec.initialState !== undefined) {
    return {
      ...component,
      initialState: () => spec.initialState
    }
  }

  return component
}
```

#### 3.4.3 HMR with Specs

```typescript
// src/effuse/hmr/registry.ts (modified)

import type { ComponentSpec } from "../graph/types.js"
import { componentToSpec } from "../spec/component-spec.js"

export const saveComponentState = (
  componentId: string,
  component: Component<any, any, any>,
  state: unknown
): void => {
  const registry = getRegistry()
  if (!registry) return

  try {
    const spec = componentToSpec(component, state)
    registry.components.set(componentId, spec)  // Store as ComponentSpec
  } catch (e) {
    console.warn(`[Effuse HMR] Could not save state for "${componentId}":`, e)
  }
}

export const loadComponentSpec = (componentId: string): ComponentSpec | undefined => {
  const registry = getRegistry()
  if (!registry) return undefined

  const spec = registry.components.get(componentId) as ComponentSpec | undefined
  if (spec) {
    registry.components.delete(componentId)
    console.log(`[Effuse HMR] Restored spec for "${componentId}"`)
  }
  return spec
}
```

#### 3.4.4 Persistent Configuration

```typescript
// Save component configuration to localStorage
export const saveComponentConfig = (
  componentId: string,
  spec: ComponentSpec
): Effect.Effect<void, never> =>
  Effect.sync(() => {
    const key = `effuse:component:${componentId}`
    localStorage.setItem(key, JSON.stringify(spec))
  })

// Load component configuration from localStorage
export const loadComponentConfig = (
  componentId: string
): Effect.Effect<ComponentSpec | undefined, never> =>
  Effect.sync(() => {
    const key = `effuse:component:${componentId}`
    const json = localStorage.getItem(key)
    if (!json) return undefined

    try {
      return JSON.parse(json) as ComponentSpec
    } catch (e) {
      console.warn(`Failed to parse component config for ${componentId}:`, e)
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
  - Child component instantiation from specs
  - Pin creation from specs
  - Merge setup (pin connections)
  - Effect scoping for lifecycle

- [ ] **Create Graph types** (`src/effuse/graph/types.ts`)
  - EffuseGraph interface (extends Component)
  - EffuseGraphSpec type
  - MergeSpec type
  - GraphMetadata type

- [ ] **Implement GraphSpec serialization** (`src/effuse/spec/graph-spec.ts`)
  - graphToSpec() - serialize EffuseGraph to JSON
  - graphFromSpec() - deserialize JSON to EffuseGraph
  - Bundle specs (with dependencies)

- [ ] **Update mount system** (`src/effuse/component/mount.ts`)
  - Add mountGraph() for EffuseGraph components
  - Handle child component scoping
  - Preserve existing mountComponent() for simple components

- [ ] **Write Graph tests** (`src/effuse/graph/effuse-graph.test.ts`)
  - Test child component mounting
  - Test pin merges (data flow)
  - Test graph lifecycle (children destroyed with parent)
  - Test spec serialization

**Acceptance Criteria:**

- EffuseGraph is a valid Component (can be mounted)
- Child components mount correctly in containers
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
src/effuse/component/
└── mount.ts                    [MODIFY - add mountGraph() ~100 lines]
```

---

#### Phase 3: Component Registry (Week 5)

**Goal:** Create component registry for spec-based instantiation.

**Tasks:**

- [ ] **Create component registry** (`src/effuse/registry/component-registry.ts`)
  - Map component IDs to Component constructors
  - getComponent() lookup function
  - registerComponent() registration function

- [ ] **Register all existing components**
  - APMComponent, TBControlsComponent, MCTasksComponent, etc.
  - Assign unique IDs to each component type

- [ ] **Implement ComponentSpec system** (`src/effuse/spec/component-spec.ts`)
  - componentToSpec() - serialize Component + state
  - componentFromSpec() - deserialize spec to Component
  - Handle initialState override

- [ ] **Write registry tests** (`src/effuse/registry/component-registry.test.ts`)
  - Test component lookup
  - Test spec serialization/deserialization
  - Test initialState override

**Acceptance Criteria:**

- All existing components registered with unique IDs
- componentFromSpec() correctly instantiates components
- initialState override works
- Tests pass

**Files to Create:**

```
src/effuse/
├── registry/
│   ├── component-registry.ts       [NEW - 100 lines]
│   └── component-registry.test.ts  [NEW - 150 lines]
└── spec/
    ├── component-spec.ts           [NEW - 150 lines]
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
  - Migration guide (simple component → graph)

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

- [ ] **Create spec editor component** (`src/effuse/components/spec-editor.ts`)
  - JSON editor for ComponentSpec/GraphSpec
  - Live preview of spec changes
  - Save/load specs to localStorage
  - Validation and error display

- [ ] **Create graph visualizer** (`src/effuse/components/graph-visualizer.ts`)
  - Render EffuseGraph as node graph (boxes + arrows)
  - Show components as nodes
  - Show pin merges as edges
  - Click node to edit component spec

- [ ] **Create example dashboard** (`src/effuse/components/example-dashboard.ts`)
  - Showcase EffuseGraph composition
  - Multiple components with pin merges
  - Editable via spec editor

- [ ] **Integration**
  - Add "Edit Spec" button to components
  - Add graph visualizer to dev tools
  - Document visual editor usage

**Acceptance Criteria:**

- Spec editor can edit and preview ComponentSpec
- Graph visualizer shows component tree structure
- Specs can be saved/loaded
- Example dashboard demonstrates composition
- Dev tools include graph visualizer

**Files to Create:**

```
src/effuse/components/
├── spec-editor.ts              [NEW - 500 lines]
├── graph-visualizer.ts         [NEW - 600 lines]
└── example-dashboard.ts        [NEW - 200 lines]
```

---

### 4.2 Backward Compatibility

**Strategy:** Keep existing Effuse widgets working unchanged.

**Simple components** (APM, TB Output, Container, etc.):
- Continue using Component<S, E, R> directly
- No migration required
- Can be used as children in EffuseGraph

**Complex layouts** (TB Command Center, Dashboard):
- Migrate to EffuseGraph for composition benefits
- Remove manual DOM manipulation
- Use pin merges for component communication

**Both approaches supported simultaneously:**

```typescript
// Simple component (unchanged)
const SimpleComponent: Component<State, Event> = {
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

**Migration checklist for components:**

- [ ] Component has 3+ children? → Consider EffuseGraph
- [ ] Component has manual DOM manipulation? → Migrate to EffuseGraph
- [ ] Component has complex event routing? → Use pin merges
- [ ] Component is simple (<200 lines, no children)? → Keep as-is

---

### 4.3 Testing Strategy

**Unit Tests:**

- EffusePin (all pin types, semantics, snapshot/restore)
- EffuseGraph (child mounting, merges, lifecycle)
- Spec serialization (round-trip, validation)
- Component registry (lookup, registration)

**Integration Tests:**

- Graph composition (multi-level nesting)
- Mount lifecycle (scoping, cleanup)
- HMR (state preservation across hot reloads)
- Pin merges (data flow between widgets)

**Migration Tests:**

- Ensure existing components still work
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
├── component/
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
│  Component Layer                                                 │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  src/effuse/components/tb-command-center/tbcc-shell-spec.ts│ │
│  │    ├─ imports makeEffuseGraph                              │ │
│  │    └─ exports TBCCShellSpec, TBCCShellGraph                │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              ↓                                   │
│  Graph Layer                                                     │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  src/effuse/graph/effuse-graph.ts                          │ │
│  │    ├─ imports makeEffusePin (pin/effuse-pin.ts)            │ │
│  │    ├─ imports mountComponent (component/mount.ts)         │ │
│  │    ├─ imports ComponentRegistry (registry/component-registry.ts) │ │
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

- Simple component: State preserved? ✓
- Graph with 3 children: All child states preserved? ✓
- Graph with pin merges: Merge connections preserved? ✓
- Nested graph: All levels preserved? ✓

**Success Metric:** HMR works for graphs as well as simple components.

---

#### Risk 4: Team Learning Curve

**Description:** New concepts (pins, merges, specs) require learning time.

**Likelihood:** High
**Impact:** Medium
**Overall Risk:** Medium

**Mitigation:**

- Thorough documentation (VISUAL-LANGUAGE.md)
- Code examples for common patterns
- Migrate one component together as team (pair programming)
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

**Success Metric:** 90% of component state serializes correctly.

---

### 7.2 Success Criteria

Effuse Visual Language is successful if:

- [ ] **TB Command Center migrated to EffuseGraph without functional regression**
  - All tabs work
  - Tab switching uses pin merges
  - No visual changes
  - No performance degradation

- [ ] **HMR works with graph components (preserve full state across hot reloads)**
  - Graph state preserved
  - Child component states preserved
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

- [ ] **All existing components continue working unchanged**
  - APMComponent, TBControlsComponent, MCTasksComponent, etc.
  - No breaking changes to Component<S, E, R> interface
  - Tests pass for all existing components

---

## 8. Future Vision

### 8.1 Factorio-Inspired Agent Factory

**Vision:** Transform OpenAgents from a "chat interface" to an "agent factory management interface" inspired by Factorio's visual programming paradigm.

**Core Insight:** Managing AI agents isn't like having a conversation—it's like building and optimizing a factory. You place specialized machines (agents), connect them with conveyor belts (data flows), monitor production statistics (throughput and costs), and optimize for efficiency.

```
Current Mental Model          →        Future Mental Model (Enabled by Unit Primitives)
━━━━━━━━━━━━━━━━━━━━          →        ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
"I chat with AI"              →        "I manage an agent factory"
"Send message, get response"  →        "Build workflows, monitor production"
"One conversation at a time"  →        "Parallel processing, resource optimization"
"Sequential thinking"         →        "Systems thinking"
```

**How Unit Primitives Enable This:**

1. **Agent Nodes (Unit Components)**
   - Each agent is a Unit Component with visual status (BUSY, IDLE, ERROR)
   - Pin inputs/outputs define what data the agent accepts/produces
   - Component spec captures agent configuration (model, instructions, cost)

2. **Connection Lines (Pin Merges)**
   - Visual lines showing data flow between agents
   - Jobs flow through Pin connections like items on Factorio conveyor belts
   - Animated dots along connections = Pin events flowing through graph

3. **Spatial Canvas (Graph Composition)**
   - Drag agents from palette onto canvas (visual Graph building)
   - Arrange agents spatially (inputs left, processing middle, outputs right)
   - Save layouts as GraphSpec JSON (blueprint system)

4. **Real-Time Monitoring (Pin Event Streams)**
   - Pin.changes Stream emits on every job processed
   - Dashboard widgets subscribe to aggregated metrics
   - Live counters, progress bars, activity feeds

5. **Production Statistics (Graph Metrics)**
   - Track jobs/hour per agent (Pin event frequency)
   - Monitor costs (token usage per Pin activation)
   - Identify bottlenecks (slowest agent in chain = highest Pin latency)

**Implementation Mapping:**

| Factorio Concept | OpenAgents Equivalent | Unit Primitive |
|------------------|----------------------|----------------|
| Assembling Machine | AI Agent (Code Gen, Test, Review) | Component with pins |
| Conveyor Belt | Data Flow / Job Stream | Pin merge connections |
| Inserter | Job Router / Filter | Pin with conditional logic |
| Power Grid | Relay Network | Graph lifecycle |
| Pollution | Token Usage / Costs | Pin event metadata |
| Production Stats | Dashboard Metrics | Aggregated Pin events |
| Blueprint | Saved Workflow | GraphSpec JSON |

**References:** See `/Users/christopherdavid/code/oapreclean/docs/game/` for detailed Factorio-inspired design:
- `factorio-parallels.md` - Conceptual mappings (resources → data, machines → agents)
- `agent-factory-mechanics.md` - Interaction patterns (drag-drop, click-to-connect)
- `visual-language.md` - Design system (color-coded status, animations)
- `factorio-ui-roadmap.md` - 5-phase implementation plan

### 8.2 What Effuse Visual Language Enables

#### 1. Visual Component Editor

**Description:** Drag-drop graph editor for composing components without code.

**User Flow:**

1. Open visual editor (new route: `/editor`)
2. Drag components from palette onto canvas
3. Connect pins by dragging arrows between nodes
4. Edit component properties in sidebar
5. Save spec to JSON
6. Load spec in main app

**Technical Implementation:**

- Use graph-visualizer.ts as foundation
- Add drag-drop (HTML5 Drag & Drop API)
- Add pin connection UI (click source pin → click target pin → create merge)
- Real-time preview (render graph as you edit)
- Export to WidgetSpec JSON

**Timeline:** 2-3 weeks after Phase 5

**Factorio Connection:** This is essentially Factorio's blueprint system but for AI workflows. Users share "Code Review Pipeline" specs just like Factorio players share "Main Bus" blueprints.

**Unit Enabler:** GraphSpec serialization means the visual editor outputs JSON that can be loaded by `fromSpec()`. No code generation needed.

---

#### 2. Spatial Agent Factory Canvas

**Description:** Factorio-style map view where agents are machines arranged on a grid with visible data flow.

**User Flow:**

1. Open canvas view (grid background with dots)
2. Drag "Code Gen" agent from palette onto canvas
3. Drag "Test Runner" agent next to it
4. Click Code Gen → click Test Runner to connect them
5. See connection line appear with animated "job" dots flowing
6. Click "Run" → watch jobs flow through the pipeline in real-time
7. Check production stats: "47 jobs/hr, $12.34 earned today"

**Technical Implementation:**

```typescript
// Agent Node Component (Unit Component wrapper)
interface AgentNodeSpec extends ComponentSpec {
  id: string
  type: "code_gen" | "test" | "review"
  position: { x: number; y: number }
  config: {
    model: string
    instructions: string
  }
  pins: {
    input: PinSpec    // Job input
    output: PinSpec   // Job output
    status: PinSpec   // Status updates (BUSY, IDLE, ERROR)
  }
}

// Canvas is an EffuseGraph
const AgentFactoryCanvas: EffuseGraphSpec = {
  id: "agent-factory-canvas",
  children: {
    codeGen: {
      id: "agent-node",
      metadata: { type: "code_gen", position: { x: 100, y: 100 } }
    },
    testRunner: {
      id: "agent-node",
      metadata: { type: "test", position: { x: 300, y: 100 } }
    }
  },
  merges: {
    genToTest: {
      input: { "codeGen/output": {} },
      output: { "testRunner/input": {} }
    }
  }
}
```

**Visual Features:**
- Grid background (20px dots, 100px lines) - provides spatial reference
- Agent nodes = Components with status badges (green=BUSY, blue=IDLE, red=ERROR)
- Connection lines = Pin merges with animated dots (represents jobs flowing)
- Real-time updates = Pin.changes Stream triggers status badge updates
- Pan/zoom canvas to navigate large workflows

**References:** `factorio-ui-roadmap.md` Phase 1 (Spatial Canvas & Connections)

**Timeline:** Weeks 1-6 (Phase 1 in game roadmap)

---

#### 3. Production Metrics Dashboard

**Description:** Real-time monitoring of agent performance with Factorio-style statistics.

**User Flow:**

1. View dashboard showing big numbers: "247 jobs today", "1,234 sats earned"
2. See counters tick up as agents complete jobs (count-up animation)
3. Check agent performance table: "Code Gen: 127 jobs, 45s avg, 98.4% success"
4. Receive alert: "⚠️ Bottleneck detected: Test Runner is 70% of total time"
5. Click "Add 2 more test agents" to parallelize

**Technical Implementation:**

```typescript
// Metrics Widget subscribes to Pin events
const ProductionMetrics: Component<MetricsState, MetricsEvent, SocketServiceTag> = {
  id: "production-metrics",

  subscriptions: (ctx) => [
    // Subscribe to all agent status pins
    pipe(
      graphService.getAllAgentPins(),
      Stream.flatMap(pins =>
        Stream.merge(...pins.map(pin => pin.changes))
      ),
      Stream.map(event => {
        // Aggregate job completions, costs, timing
        return Effect.gen(function* () {
          yield* ctx.state.update(s => ({
            ...s,
            jobsCompleted: s.jobsCompleted + 1,
            earnings: s.earnings + event.cost,
            avgTime: calculateAverage(s.times, event.duration)
          }))
        })
      })
    )
  ],

  render: (ctx) => Effect.gen(function* () {
    const state = yield* ctx.state.get
    return html`
      <div class="metrics-grid">
        <div class="metric-card">
          <span class="label">Jobs Completed</span>
          <span class="value">${state.jobsCompleted}</span>
          <span class="trend">▲ +23%</span>
        </div>
        <div class="metric-card">
          <span class="label">Earnings</span>
          <span class="value">${state.earnings} sats</span>
          <span class="trend">▲ +12%</span>
        </div>
      </div>
    `
  })
}
```

**Metrics Tracked:**
- Jobs per hour (Pin event frequency)
- Earnings (sum of job costs from Pin metadata)
- Average job time (Pin start → Pin end delta)
- Success rate (successful Pin completions / total)
- Bottlenecks (agent with highest Pin latency)

**References:** `factorio-ui-roadmap.md` Phase 2 (Production Stats & Monitoring)

**Timeline:** Weeks 7-12 (Phase 2 in game roadmap)

---

#### 4. User-Configurable Dashboards

**Description:** Users save/load custom layouts without developer involvement.

**User Flow:**

1. User rearranges components on dashboard
2. User clicks "Save Layout"
3. Spec saved to localStorage (or backend)
4. On next visit, layout restored from saved spec

**Technical Implementation:**

- Add "Edit Mode" toggle to dashboard
- In edit mode: drag components, resize, reorder
- Capture changes as GraphSpec modifications
- Save spec on "Save Layout"
- Load spec on mount (check localStorage first)

**Timeline:** 1-2 weeks after visual editor

**Factorio Connection:** Factorio players obsess over optimizing factory layouts. This feature lets OpenAgents users do the same with agent workflows.

---

#### 5. Blueprint Marketplace

**Description:** Community-driven library of workflow templates (Factorio's blueprint sharing).

**Description:** Share workflow templates as JSON specs (like Factorio's blueprint library).

**User Flow:**

1. User builds successful "Code Review Pipeline" workflow
2. Clicks "Save Blueprint" → adds name, description, tags
3. Publishes to marketplace (requires Gold tier trust score)
4. Other users discover it: "★★★★★ 4.8 (1,523 installs)"
5. One-click install → workflow appears on their canvas

**Technical Implementation:**

```typescript
// Blueprint is just a GraphSpec with metadata
interface BlueprintSpec extends EffuseGraphSpec {
  metadata: {
    author: string
    version: string  // Semver: 1.0.0
    description: string
    tags: string[]   // ["code", "typescript", "review"]
    rating: number
    installs: number
    published: Date
  }
}

// Save workflow as blueprint
const saveBlueprint = (graph: EffuseGraph): BlueprintSpec => ({
  ...graphToSpec(graph),
  metadata: {
    author: currentUser.username,
    version: "1.0.0",
    description: prompt("Describe your workflow..."),
    tags: promptTags(),
    rating: 0,
    installs: 0,
    published: new Date()
  }
})

// Install blueprint (just load GraphSpec)
const installBlueprint = (blueprint: BlueprintSpec): Effect.Effect<void> =>
  Effect.gen(function* () {
    const graph = yield* graphFromSpec(blueprint)
    yield* mountGraph(graph, canvasContainer)
  })
```

**Marketplace Features:**
- Search by tags, author, rating
- Filter by tier required (Bronze/Silver/Gold)
- Preview canvas layout before installing
- Rate and review after using
- Version updates with changelog

**Unit Advantage:** Specs are JSON, so marketplace is just a JSON database. No code generation or compilation needed.

**References:** `factorio-ui-roadmap.md` Phase 4 (Blueprint System & Marketplace)

**Timeline:** Weeks 19-26 (Phase 4 in game roadmap)

---

#### 6. Tech Tree / Progression System

**Description:** Gamified capability unlocking based on trust score (inspired by Factorio's technology tree).

**User Flow:**

1. User completes first 10 jobs → unlocks "Bronze Tier"
2. Dashboard shows: "Trust Score: 247 / 500" with progress bar
3. Completes 100 jobs → "Silver Tier" unlocked with confetti animation
4. New capabilities appear: "Premium Agents", "Blueprint Publishing"
5. Tech tree visualizes progression: Bronze → Silver → Gold → Platinum

**Technical Implementation:**

```typescript
// Tech tree is a GraphSpec (nodes = capabilities, connections = prerequisites)
const TechTreeSpec: EffuseGraphSpec = {
  id: "tech-tree",
  children: {
    signup: {
      id: "tech-node",
      initialState: { unlocked: true, label: "Sign Up" }
    },
    firstJob: {
      id: "tech-node",
      initialState: { unlocked: false, label: "Complete First Job", requires: 1 }
    },
    bronzeTier: {
      id: "tech-node",
      initialState: { unlocked: false, label: "Bronze Tier", requires: 10 }
    },
    silverTier: {
      id: "tech-node",
      initialState: { unlocked: false, label: "Silver Tier", requires: 100 }
    }
  },
  merges: {
    signupToFirst: {
      input: { "signup/unlocked": {} },
      output: { "firstJob/canUnlock": {} }
    },
    firstToBronze: {
      input: { "firstJob/unlocked": {} },
      output: { "bronzeTier/canUnlock": {} }
    }
  }
}

// Unlock logic uses Pin events
const checkUnlock = (node: TechNode, jobCount: number): boolean => {
  if (jobCount >= node.requires && allPrerequisitesUnlocked(node)) {
    node.pins.unlocked.set(true)  // Trigger unlock
    showCelebration(node.label)
    return true
  }
  return false
}
```

**Progression Metrics:**
- Jobs completed (Pin event count)
- Success rate (successful Pin completions / total)
- Earnings (sum of Pin metadata costs)
- Community contributions (blueprints published, installs)

**Unlockable Capabilities:**
- Bronze (0-500): Basic agents, 10 jobs/day
- Silver (500-2000): Standard agents, 100 jobs/day, workflow templates
- Gold (2000+): Premium agents, unlimited jobs, blueprint publishing
- Platinum (10000+): Custom agents, API access, priority support

**References:** `factorio-ui-roadmap.md` Phase 3 (Tech Tree & Progression)

**Timeline:** Weeks 13-18 (Phase 3 in game roadmap)

---

#### 7. AI-Generated UIs

**Description:** LLM outputs WidgetSpec JSON, renders instantly without code generation.

**User Flow:**

1. User describes desired UI: "Create a dashboard with APM on left, TB controls on right"
2. LLM generates EffuseGraphSpec JSON
3. User pastes JSON into spec editor
4. UI renders instantly
5. User tweaks via visual editor

**Technical Implementation:**

- Provide LLM with EffuseGraphSpec schema
- Provide LLM with ComponentRegistry (available components)
- LLM generates valid JSON
- Validate spec before rendering
- Error messages guide LLM to fix invalid specs

**Timeline:** 1-2 weeks (LLM prompt engineering)

**Factorio Connection:** LLMs can output valid GraphSpec JSON that renders immediately, no code generation needed.

---

#### 8. Time Travel Debugging

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
- Capture all component states via componentToSpec
- Capture all pin states via pin.snapshot()
- Store in IndexedDB
- Restore via componentFromSpec + pin.restore()

**Timeline:** 2-3 weeks

**Unit Advantage:** Pin snapshot/restore captures full graph state (all component states + all pin states) in one operation.

---

#### 9. Multi-User Collaboration

**Description:** Sync WidgetSpec changes via CRDT, multiple users edit same dashboard.

**User Flow:**

1. User A and User B open same dashboard
2. User A moves component, User B sees change instantly
3. User B adds new component, User A sees it
4. No conflicts, automatic merge via CRDT

**Technical Implementation:**

- Convert EffuseGraphSpec to CRDT (Yjs or Automerge)
- WebSocket sync between clients
- Operational transformation for concurrent edits
- Conflict resolution (last-write-wins or custom)

**Timeline:** 6-8 weeks (requires CRDT integration)

---

### 8.3 Factorio-Inspired Advanced Features

These advanced features directly leverage Unit primitives to create Factorio-level complexity:

#### A/B Testing Workflows

**Description:** Run two workflow variants in parallel and compare metrics (inspired by Factorio's experimentation).

```typescript
// A/B test is two parallel Graphs with a router
const ABTestSpec: EffuseGraphSpec = {
  id: "ab-test-code-review",
  children: {
    router: { id: "job-router", initialState: { split: 0.5 } },
    variantA: { id: "code-review-pipeline-sonnet" },
    variantB: { id: "code-review-pipeline-haiku" },
    aggregator: { id: "metrics-aggregator" }
  },
  merges: {
    input_to_router: { input: { "input/jobs": {} }, output: { "router/jobs": {} } },
    router_to_a: { input: { "router/groupA": {} }, output: { "variantA/input": {} } },
    router_to_b: { input: { "router/groupB": {} }, output: { "variantB/input": {} } },
    a_to_agg: { input: { "variantA/metrics": {} }, output: { "aggregator/variantA": {} } },
    b_to_agg: { input: { "variantB/metrics": {} }, output: { "aggregator/variantB": {} } }
  }
}
```

**Metrics Compared:**
- Average job time (Pin duration)
- Cost per job (Pin metadata)
- Success rate (Pin completion status)
- Output quality (user ratings)

**Result:** Dashboard shows statistical significance and recommends winner.

#### Conditional Routing

**Description:** Route jobs based on criteria (like Factorio's circuit network conditions).

```typescript
// Router component with conditional Pin merges
const ConditionalRouter: ComponentSpec = {
  id: "conditional-router",
  pins: {
    input: { type: "job" },
    simple: { type: "job" },    // Routes to fast/cheap agent
    complex: { type: "job" }    // Routes to slow/expensive agent
  }
}

// Router logic examines job metadata
const routeJob = (job: Job): "simple" | "complex" => {
  if (job.lines < 100) return "simple"
  if (job.complexity === "low") return "simple"
  return "complex"
}

// Pin connection includes filter
merges: {
  input_to_simple: {
    input: { "router/input": {} },
    output: { "haikuAgent/input": {} },
    filter: (job) => routeJob(job) === "simple"  // Conditional Pin merge
  }
}
```

#### Bottleneck Detection

**Description:** Automatically identify slow agents in workflows (Factorio's production analysis).

```typescript
// Analyze Pin event timings across graph
const detectBottleneck = (graph: EffuseGraph): Effect.Effect<Bottleneck | null> =>
  Effect.gen(function* () {
    const agents = graph.children
    const timings: Record<string, number> = {}

    // Calculate average Pin duration for each agent
    for (const [agentId, agent] of Object.entries(agents)) {
      const pin = agent.pins.output
      const events = yield* pin.changes.pipe(Stream.take(100), Stream.runCollect)
      timings[agentId] = average(events.map(e => e.duration))
    }

    // Find slowest
    const slowest = maxBy(Object.entries(timings), ([_, time]) => time)
    if (slowest[1] > average(Object.values(timings)) * 2) {
      return {
        agentId: slowest[0],
        avgTime: slowest[1],
        suggestion: "Add 2 more agents to parallelize workload"
      }
    }

    return null
  })
```

**References:** `factorio-ui-roadmap.md` Phase 5 (Advanced Optimization Tools)

---

### 8.4 Roadmap After Initial Implementation

**Aligned with Factorio-Inspired Roadmap:**

**Phase 1: Spatial Canvas (Weeks 1-6)**
- [ ] Agent nodes (visual components with status badges)
- [ ] Connection lines (visual Pin merges with animated flow)
- [ ] Canvas view (pan/zoom, drag-drop, snap-to-grid)
- [ ] Real-time status updates (Pin.changes → UI updates)

**Phase 2: Production Stats (Weeks 7-12)**
- [ ] Metrics dashboard (jobs/hr, earnings, costs)
- [ ] Agent performance table (per-agent metrics)
- [ ] Activity feed (live job log)
- [ ] Bottleneck detection (identify slow agents)

**Phase 3: Tech Tree (Weeks 13-18)**
- [ ] Trust score system (gamified progression)
- [ ] Capability unlocking (Bronze/Silver/Gold tiers)
- [ ] Achievement system (badges for milestones)
- [ ] Tech tree visualization (GraphSpec!)

**Phase 4: Blueprint Marketplace (Weeks 19-26)**
- [ ] Save workflows as blueprints (GraphSpec JSON)
- [ ] Import blueprints (graphFromSpec)
- [ ] Marketplace UI (search, filter, install)
- [ ] Blueprint versioning (semantic versioning)

**Phase 5: Advanced Tools (Weeks 27-36)**
- [ ] Workflow profiler (Pin timing analysis)
- [ ] Cost optimizer (suggest cheaper models)
- [ ] A/B testing (parallel graph comparison)
- [ ] Advanced debugging (step-through execution)

**See:** `/Users/christopherdavid/code/oapreclean/docs/game/factorio-ui-roadmap.md` for full 5-phase roadmap

---

### 8.5 Success Metrics (Future)

**Engagement (Factorio-Inspired):**

- 80% of users try canvas view within first week
- 50% of workflows created using blueprint templates (not from scratch)
- Daily active users increase 3x (monitoring vs chatting)
- Average session time: 5min → 20min (from chat to factory management)
- Users share screenshots of their "agent factories" on social media

**Adoption:**

- 50% of components use EffuseGraph within 6 months
- 100+ public blueprints in marketplace within 3 months
- Top blueprint has 500+ installs
- 20+ community tutorials/guides created

**Business:**

- Conversion to paid increases 2x (value is obvious from visual workflows)
- Retention: 90% at 30 days (sticky factory management habit)
- Revenue per user increases 5x (more agent usage, optimization obsession)

**Performance:**

- Graph mount time <50ms (2x faster than current manual patterns)
- Pin merge latency <1ms (real-time reactivity)
- Spec serialization <10ms (instant save/load)
- Canvas render: 60fps with 50+ agents

**Developer Experience:**

- Component creation time reduced by 50% (graph vs manual)
- Onboarding time reduced by 30% (visual editor + tutorial vs code)
- Bug reproduction time reduced by 70% (time travel debugging + profiler)

**Qualitative Signals:**
- "Cracktorio for AI" memes emerge
- Users obsess over optimizing jobs/$ ratios (like Factorio players with items/minute)
- Community creates blueprint libraries ("Awesome OpenAgents Workflows")
- User-generated optimization guides and tutorials

---

## Conclusion

**Effuse Visual Language** represents a major evolution of the Effuse framework, harvesting the best ideas from Unit's visual programming primitives and adapting them to be Effect-native. This document provides a complete roadmap for implementation, from core primitives (EffusePin) to visual composition (EffuseGraph) to future capabilities (visual editor, AI-generated UIs).

**Key Takeaways:**

1. **We're not replacing Effuse** - We're enhancing it with visual primitives
2. **Effect-native throughout** - All reactivity via Streams, all services via Context.Tag
3. **Backward compatible** - Existing simple components unchanged
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

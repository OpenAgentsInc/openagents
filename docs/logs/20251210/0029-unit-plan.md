# 100% Unit to GPUI Migration Plan

## Executive Summary

**Goal:** Complete migration of the Unit visual programming framework (~185K lines TypeScript) to Rust with GPUI.

**Current State:**
- `crates/unit/` - 3,400 lines, 37 tests (core runtime ~50% complete)
- `crates/hud/` - 1,470 lines, 8 tests (basic visualization)
- `crates/storybook/` - Component explorer

**Scope:** Full framework port including:
- Core runtime with working data propagation
- Visual editor with drag-to-connect, shortcuts, panels
- All 188 system units
- Complete spec serialization (JSON save/load)

**Estimated Effort:** ~10,000 new lines across 8 phases

---

## Phase 1: Complete Core Runtime (~1,500 lines)

### 1.1 Type Erasure with CloneableAny

**File:** `crates/unit/src/cloneable_any.rs` (NEW, ~80 lines)

```rust
pub trait CloneableAny: Any + Send + Sync {
    fn clone_box(&self) -> Box<dyn CloneableAny>;
    fn as_any(&self) -> &dyn Any;
}
```

### 1.2 Event System

**File:** `crates/unit/src/event.rs` (NEW, ~200 lines)

```rust
pub enum RuntimeEvent {
    PinData { unit_id: String, pin_name: String, data: Box<dyn CloneableAny> },
    PinDrop { unit_id: String, pin_name: String },
    UnitError { unit_id: String, error: String },
}

pub struct EventBus { sender, receiver }
```

### 1.3 Event Scheduler with Buffering

**File:** `crates/unit/src/scheduler.rs` (NEW, ~250 lines)

- Queue events when playing
- Buffer events when paused
- Flush buffer on resume

### 1.4 Fix Data Propagation

**File:** `crates/unit/src/merge.rs` (MODIFY, +150 lines)

```rust
impl Merge {
    fn propagate_to_outputs(&mut self) {
        let cloned = self.current_data.as_ref().map(|d| d.clone_box());
        for output in &mut self.outputs.values_mut() {
            output.push_any(cloned.clone());
        }
    }
}
```

### 1.5 Connection Registry

**File:** `crates/unit/src/graph.rs` (MODIFY, +400 lines)

- `connect(source_unit, source_pin, target_unit, target_pin)`
- Type validation on connection
- Event routing through merges

### 1.6 Error Types

**File:** `crates/unit/src/error.rs` (NEW, ~150 lines)

```rust
pub enum UnitError { Pin(PinError), Connection(ConnectionError), Computation { message } }
pub enum PinError { NotFound, TypeMismatch, AlreadyExists }
pub enum ConnectionError { SourceNotFound, TargetNotFound, CycleDetected }
```

---

## Phase 2: Value Type System (~300 lines)

**File:** `crates/unit/src/value.rs` (NEW)

```rust
#[derive(Clone, Serialize, Deserialize)]
pub enum Value {
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

impl Value {
    pub fn as_number(&self) -> Option<f64>;
    pub fn to_boolean(&self) -> bool;  // JS-like coercion
    pub fn type_name(&self) -> &'static str;
}
```

---

## Phase 3: Visual Editor Interactivity (~2,500 lines)

### 3.1 Drag-to-Connect

**Files:**
- `crates/hud/src/graph_view.rs` (MODIFY, +150 lines)
- `crates/hud/src/pin_compatibility.rs` (NEW, ~100 lines)

```rust
enum DragState {
    None,
    Panning { start },
    DraggingUnit { unit_id, offset },
    DraggingConnection { from_unit, from_pin, cursor_pos },
    RubberBand { start, current },
}
```

### 3.2 Multi-Select

**File:** `crates/hud/src/selection.rs` (NEW, ~150 lines)

- `SelectionManager` with `select()`, `toggle()`, `select_in_bounds()`
- Shift+click for additive selection
- Rubber band selection

### 3.3 Keyboard Shortcuts

**File:** `crates/hud/src/actions.rs` (NEW, ~100 lines)

```rust
actions!(graph_editor, [
    Delete, SelectAll, Copy, Paste, Cut, Undo, Redo,
    ZoomIn, ZoomOut, ZoomToFit, DuplicateSelection,
]);
```

### 3.4 Undo/Redo History

**File:** `crates/hud/src/history.rs` (NEW, ~200 lines)

```rust
pub trait Command { fn execute(); fn undo(); }
pub struct CommandHistory { undo_stack, redo_stack }
```

### 3.5 Context Menus

**File:** `crates/hud/src/context_menu.rs` (NEW, ~200 lines)

Using GPUI's `anchored()` element for positioning.

### 3.6 Inspector Panel

**File:** `crates/hud/src/inspector.rs` (NEW, ~300 lines)

- Show selected node properties
- Edit pin values
- Display errors

---

## Phase 4: Spec Serialization (~500 lines)

### 4.1 Extended Spec Types

**File:** `crates/unit/src/spec/types.rs` (NEW, ~200 lines)

```rust
pub struct GraphSpec {
    pub id: String,
    pub inputs: HashMap<String, PinSpec>,
    pub outputs: HashMap<String, PinSpec>,
    pub units: HashMap<String, UnitSpec>,
    pub merges: HashMap<String, GraphMergeSpec>,
}

pub struct BundleSpec {
    pub spec: GraphSpec,
    pub specs: HashMap<String, GraphSpec>,  // Dependencies
}
```

### 4.2 Serialization

**File:** `crates/unit/src/spec/stringify.rs` (NEW, ~150 lines)

```rust
pub fn graph_to_spec(graph: &Graph) -> GraphSpec;
pub fn unit_to_spec(unit: &dyn Unit) -> UnitSpec;
```

### 4.3 Deserialization

**File:** `crates/unit/src/spec/from_spec.rs` (NEW, ~150 lines)

```rust
pub fn graph_from_spec(spec: &GraphSpec, registry: &UnitRegistry) -> Result<Graph>;
pub fn graph_from_bundle(bundle: &BundleSpec, registry: &UnitRegistry) -> Result<Graph>;
```

---

## Phase 5: System Units (~5,000 lines)

### 5.1 Unit Macros

**File:** `crates/unit/src/system/macros.rs` (NEW, ~100 lines)

```rust
macro_rules! arithmetic_unit {
    ($name:ident, $op:tt) => { /* ... */ };
}
arithmetic_unit!(Add, +);
arithmetic_unit!(Subtract, -);
```

### 5.2 Unit Categories

| Category | Files | Strategy |
|----------|-------|----------|
| `system/arithmetic/` | 5 | Macro |
| `system/logic/` | 3 | Macro |
| `system/math/` | 20+ | Macro |
| `system/control/` | 10 | Manual (If, Loop, Wait, Memory) |
| `system/array/` | 20+ | Manual (Filter, Map, Sort) |
| `system/string/` | 10+ | Mixed |
| `system/object/` | 10+ | Manual (DeepGet, DeepSet) |
| `system/time/` | 3 | Manual (Debounce, Delay, Throttle) |
| `system/graph/` | 10+ | Manual (AddUnit, RemoveUnit) |
| `system/meta/` | 6 | Manual (Bundle, New, Spec) |

### 5.3 Unit Registry

**File:** `crates/unit/src/registry.rs` (NEW, ~150 lines)

```rust
pub struct UnitRegistry {
    factories: HashMap<String, Box<dyn Fn() -> Box<dyn Unit>>>,
}

impl UnitRegistry {
    pub fn register<U: Unit + Default>(&mut self, id: &str);
    pub fn create(&self, id: &str) -> Option<Box<dyn Unit>>;
}
```

---

## Phase 6: Advanced Editor Features (~800 lines)

### 6.1 Connection Routing

**File:** `crates/hud/src/routing.rs` (NEW, ~200 lines)

- Waypoint-based routing
- Obstacle avoidance

### 6.2 Subgraph Collapse/Expand

**File:** `crates/hud/src/subgraph.rs` (NEW, ~200 lines)

### 6.3 Alignment Tools

**File:** `crates/hud/src/alignment.rs` (NEW, ~150 lines)

```rust
pub fn align_left(nodes);
pub fn distribute_horizontally(nodes);
```

### 6.4 Export

**File:** `crates/hud/src/export.rs` (NEW, ~150 lines)

- `to_svg(graph) -> String`
- `to_png(graph) -> Vec<u8>`

---

## File Summary

### New Files (~25 files)

| File | Lines | Phase |
|------|-------|-------|
| `unit/src/cloneable_any.rs` | 80 | 1 |
| `unit/src/event.rs` | 200 | 1 |
| `unit/src/scheduler.rs` | 250 | 1 |
| `unit/src/error.rs` | 150 | 1 |
| `unit/src/value.rs` | 300 | 2 |
| `unit/src/registry.rs` | 150 | 5 |
| `unit/src/spec/types.rs` | 200 | 4 |
| `unit/src/spec/stringify.rs` | 150 | 4 |
| `unit/src/spec/from_spec.rs` | 150 | 4 |
| `unit/src/system/**` | 4000 | 5 |
| `hud/src/actions.rs` | 100 | 3 |
| `hud/src/selection.rs` | 150 | 3 |
| `hud/src/history.rs` | 200 | 3 |
| `hud/src/context_menu.rs` | 200 | 3 |
| `hud/src/inspector.rs` | 300 | 3 |
| `hud/src/pin_compatibility.rs` | 100 | 3 |
| `hud/src/routing.rs` | 200 | 6 |
| `hud/src/subgraph.rs` | 200 | 6 |
| `hud/src/alignment.rs` | 150 | 6 |
| `hud/src/export.rs` | 150 | 6 |

### Modified Files

| File | Lines Added | Phase |
|------|-------------|-------|
| `unit/src/merge.rs` | +150 | 1 |
| `unit/src/graph.rs` | +400 | 1 |
| `unit/src/pin.rs` | +100 | 1 |
| `unit/src/any_pin.rs` | +50 | 1 |
| `hud/src/graph_view.rs` | +500 | 3 |
| `hud/src/pin_view.rs` | +100 | 3 |
| `hud/src/unit_view.rs` | +100 | 3 |

---

## Implementation Order

```
Week 1-2: Phase 1 (Core Runtime)
  ├── cloneable_any.rs
  ├── event.rs
  ├── scheduler.rs
  ├── error.rs
  └── Fix merge.rs, graph.rs propagation

Week 3: Phase 2 (Value Type)
  └── value.rs with full coercion

Week 4-5: Phase 3 (Visual Editor)
  ├── Drag-to-connect
  ├── Multi-select
  ├── Keyboard shortcuts
  ├── Context menus
  └── Inspector panel

Week 6: Phase 4 (Spec System)
  ├── spec/types.rs
  ├── spec/stringify.rs
  └── spec/from_spec.rs

Week 7-8: Phase 5 (System Units)
  ├── Macro units (arithmetic, logic, math)
  ├── Control units (If, Loop, Wait)
  ├── Array units (Filter, Map, Sort)
  └── Registry integration

Week 9: Phase 6 (Advanced Features)
  ├── Connection routing
  ├── Subgraph collapse
  ├── Alignment tools
  └── Export

Week 10: Testing & Polish
  ├── Integration tests
  ├── TypeScript spec compatibility
  └── Performance optimization
```

---

## Critical Reference Files

### TypeScript (~/code/unit/)
- `src/Pin.ts` (360 lines) - Pin state machine
- `src/Class/Unit/index.ts` (32K lines) - Core unit
- `src/Class/Graph/index.ts` (149K lines) - Graph manipulation
- `src/system/f/` - 188 system units

### Current Rust
- `crates/unit/src/merge.rs` - Stubbed propagate_to_outputs()
- `crates/unit/src/graph.rs` - Needs connection registry
- `crates/hud/src/graph_view.rs` - Needs drag-to-connect

### GPUI Reference
- `crates/gpui/src/elements/div.rs` - Interactivity patterns
- `crates/gpui/examples/drag_drop.rs` - Drag/drop mechanics
- `crates/commander/src/text_input.rs` - Custom element example

---

## Success Criteria

1. **Core Runtime**: Data flows through connected pins end-to-end
2. **Visual Editor**: Create connections by dragging pins
3. **System Units**: All 188 units with matching TypeScript semantics
4. **Serialization**: Round-trip JSON compatibility with TypeScript Unit
5. **Performance**: 60fps with 100+ nodes

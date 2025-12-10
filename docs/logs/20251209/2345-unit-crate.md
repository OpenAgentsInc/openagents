# Unit Crate Implementation - 2025-12-09

## Summary

Created `crates/unit/` - a pure Rust port of the Unit visual programming framework's core runtime. No GPUI dependency; this is the foundation for the `hud` crate.

## What Was Built

### Crate Structure

```
crates/unit/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports
    ├── pin.rs           # Pin<T> - core data container (~400 lines)
    ├── any_pin.rs       # Type-erased AnyPin trait
    ├── unit.rs          # Unit trait + Lifecycle + UnitExt
    ├── primitive.rs     # Primitive trait (reactive callbacks)
    ├── functional.rs    # Functional trait + SimpleUnit impl
    ├── merge.rs         # Merge (fan-in/fan-out connections)
    ├── graph.rs         # Graph (composite container)
    ├── spec.rs          # Serialization types
    ├── geometry.rs      # Point, Shape, surface distance (~350 lines)
    └── physics.rs       # Force-directed layout simulation (~400 lines)
```

### Core Abstractions

| Type | Description |
|------|-------------|
| `Pin<T>` | State machine: Empty → Valid ↔ Invalid. Operations: push, take, pull, peak |
| `PinOpt` | Configuration: constant, ignored, ref flags |
| `Unit` | MIMO FSM trait with lifecycle (Paused/Playing) and named pins |
| `Primitive` | Reactive callbacks: on_input_data, on_input_drop, on_output_drop |
| `Functional` | Computation: `f(input, done, fail)` callback pattern |
| `Merge` | Fan-in/fan-out connections between pins |
| `Graph` | Composite container with units, merges, pin exposure |

### Geometry & Physics

Ported from Effuse's TypeScript implementation:

| Function | Purpose |
|----------|---------|
| `surface_distance(a, b)` | Calculate distance between shape surfaces |
| `point_in_node(node, direction, padding)` | Find edge point for connection routing |
| `apply_forces(nodes, connections, config)` | Repulsion + attraction + center gravity |
| `integrate(nodes, dt, config)` | Euler integration with velocity damping |
| `run_until_settled(...)` | Run simulation until alpha < threshold |

### Tests

37 tests passing:

```
test pin::tests::test_pin_push_take ... ok
test pin::tests::test_pin_pull_constant ... ok
test pin::tests::test_pin_callbacks ... ok
test any_pin::tests::test_any_pin_push_take ... ok
test functional::tests::test_simple_unit_creation ... ok
test functional::tests::test_simple_unit_computation ... ok
test merge::tests::test_merge_builder ... ok
test merge::tests::test_merge_current ... ok
test graph::tests::test_graph_add_unit ... ok
test graph::tests::test_graph_lifecycle ... ok
test geometry::tests::test_point_distance ... ok
test geometry::tests::test_surface_distance_circles ... ok
test physics::tests::test_apply_forces_repulsion ... ok
test physics::tests::test_run_until_settled ... ok
... (37 total)
```

## Key Decisions

### Type Erasure Pattern

Used `Box<dyn AnyPin>` for heterogeneous pin storage:

```rust
pub trait AnyPin: Send + Sync {
    fn push_any(&mut self, data: Box<dyn Any + Send>) -> Result<(), PinTypeError>;
    fn take_any(&mut self) -> Option<Box<dyn Any + Send>>;
    fn type_id(&self) -> TypeId;
}
```

### Thread Safety

All types are `Send + Sync` for eventual async/parallel graph execution:

```rust
pub struct Pin<T: Clone + Send + Sync + 'static> { ... }
current_data: Option<Box<dyn Any + Send + Sync>>,
```

### Callback Pattern

Adapted TypeScript's EventEmitter to Rust callback vectors:

```rust
on_data: Vec<Box<dyn Fn(&T) + Send + Sync>>,
on_drop: Vec<Box<dyn Fn(&T) + Send + Sync>>,
on_invalid: Vec<Box<dyn Fn() + Send + Sync>>,
```

## Issues Resolved

1. **Borrow checker in physics.rs**: Split mutable borrows with `split_at_mut()` for force application
2. **Center gravity overpowering repulsion**: Test fix - disable center gravity when testing pure repulsion
3. **Lifetime issues with trait object returns**: Added `+ 'static` bounds

---

# HUD Crate Implementation - 2025-12-09

## Summary

Created `crates/hud/` - GPUI visualization layer for Unit dataflow graphs. Provides Entity wrappers for rendering units, pins, and connections with force-directed layout.

## What Was Built

### Crate Structure

```
crates/hud/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports
    ├── connection.rs    # Bezier curve rendering for connections
    ├── pin_view.rs      # PinView GPUI Entity
    ├── unit_view.rs     # UnitView GPUI Entity
    └── graph_view.rs    # GraphView with physics simulation
```

### Core Components

| Component | Description |
|-----------|-------------|
| `Connection` | Cubic bezier curve between two points with state-based coloring |
| `ConnectionLayer` | Collection of connections to paint together |
| `PinView` | GPUI Entity for pins with state-based color indicator |
| `PinSnapshot` | Immutable snapshot of pin state for rendering |
| `UnitView` | GPUI Entity for unit boxes with input/output pins |
| `UnitSnapshot` | Immutable snapshot of unit state for rendering |
| `GraphView` | Full graph canvas with physics, pan/zoom, selection |
| `GraphStyle` | Visual configuration for all graph elements |

### Key Features

**Connection Rendering:**
- Cubic bezier curves via GPUI PathBuilder
- Three states: Inactive (gray), Active (white), Selected (cyan)
- Horizontal-biased control points for clean routing

**PinView:**
- Circular pin indicators
- Color-coded states: Empty (gray), Valid (green), Invalid (red), Constant (cyan)
- Click/drag events for connection creation

**UnitView:**
- Box with header (unit ID) and pin columns
- Background color based on lifecycle (Paused/Playing/Error)
- Selection/hover visual feedback
- Pin connection point calculation

**GraphView:**
- Force-directed physics simulation using unit crate
- Pan (drag) and zoom (scroll wheel)
- Node selection with Cmd+click for multi-select
- Node dragging with real-time physics updates
- Animation loop at ~60fps during active simulation
- Grid background for visual reference

### Tests

8 tests passing:

```
test connection::tests::test_connection_creation ... ok
test connection::tests::test_connection_from_unit_points ... ok
test connection::tests::test_connection_layer ... ok
test pin_view::tests::test_pin_snapshot ... ok
test pin_view::tests::test_pin_direction_from_io ... ok
test unit_view::tests::test_unit_snapshot_size ... ok
test graph_view::tests::test_graph_style_defaults ... ok
test graph_view::tests::test_coordinate_transform ... ok
```

## GPUI API Notes

Key learnings from GPUI integration:

1. **Pixels private field**: Cannot access `Pixels.0` directly, must use `.into()` for f32 conversion
2. **Cubic bezier signature**: `cubic_bezier_to(to, control_a, control_b)` - destination first
3. **Mouse events**: Use explicit type annotations like `event: &gpui::MouseDownEvent`
4. **Spawn syntax**: `cx.spawn(async move |view, cx| { ... })`
5. **Platform modifier**: Use `event.modifiers.platform` for Cmd key (not `.command`)
6. **No on_mouse_enter/leave**: GPUI has `on_hover` instead but different API

## Integration with Unit Crate

The bridge pattern connects unit's pure runtime to GPUI reactivity:

```rust
// Snapshot pattern - take immutable copy for rendering
impl UnitSnapshot {
    pub fn from_unit(unit: &dyn Unit, position: Point<Pixels>) -> Self { ... }
}

// GraphView owns simulation state
pub struct GraphView {
    graph: Option<Graph>,
    sim_nodes: Vec<SimNode>,     // From unit::physics
    sim_config: SimulationConfig,
    // ... GPUI state
}
```

## Next Steps

- Integrate with Commander for live graph visualization
- Add connection drag-creation between pins
- Add right-click context menus for unit/pin actions
- Add inspector panel for selected unit details

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

## Next Steps

Create `hud` crate with GPUI integration:
- `PinView` - Entity wrapping Pin with visual state indicator
- `UnitView` - Entity for unit boxes with pins
- `GraphView` - Full canvas with physics animation

# Unit, HUD, and Storybook Implementation - 2025-12-09

## Summary

Created three Rust crates for the Unit visual programming framework:
- **unit** - Pure runtime (Pin, Unit, Graph, Physics)
- **hud** - GPUI visualization layer
- **storybook** - Visual component explorer

## Unit Crate

### Structure

```
crates/unit/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports
    ├── pin.rs           # Pin<T> - core data container
    ├── any_pin.rs       # Type-erased AnyPin trait
    ├── unit.rs          # Unit trait + Lifecycle
    ├── primitive.rs     # Primitive trait (reactive callbacks)
    ├── functional.rs    # Functional trait + SimpleUnit
    ├── merge.rs         # Merge (fan-in/fan-out)
    ├── graph.rs         # Graph (composite container)
    ├── spec.rs          # Serialization types
    ├── geometry.rs      # Point, Shape, surface distance
    └── physics.rs       # Force-directed layout simulation
```

### Core Abstractions

| Type | Description |
|------|-------------|
| `Pin<T>` | State machine: Empty → Valid ↔ Invalid |
| `Unit` | MIMO FSM trait with lifecycle (Paused/Playing) |
| `Primitive` | Reactive callbacks on pin events |
| `Functional` | Computation with `f(input, done, fail)` |
| `Merge` | Fan-in/fan-out connections |
| `Graph` | Composite container with units and merges |

### Tests: 37 passing

---

## HUD Crate

### Structure

```
crates/hud/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── connection.rs    # Bezier curve rendering
    ├── pin_view.rs      # PinView GPUI Entity
    ├── unit_view.rs     # UnitView GPUI Entity
    └── graph_view.rs    # GraphView with physics
```

### Components

| Component | Description |
|-----------|-------------|
| `Connection` | Cubic bezier curve with state-based coloring |
| `PinView` | Pin indicator with color-coded states |
| `UnitView` | Unit box with header and pin columns |
| `GraphView` | Interactive canvas with physics layout |

### Key Features

- Cubic bezier curves via GPUI PathBuilder
- Pin colors: Empty (gray), Valid (green), Invalid (red), Constant (cyan)
- Unit colors: Playing (dark gray), Paused (darker), Error (red)
- Physics simulation at ~60fps with force-directed layout
- Pan (drag) and zoom (scroll wheel) navigation
- Node selection with Cmd+click for multi-select

### Tests: 8 passing

---

## Storybook Crate

### Structure

```
crates/storybook/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── story.rs
    └── stories/
        ├── mod.rs
        ├── pin_states.rs
        ├── unit_view.rs
        ├── connections.rs
        ├── graph_view.rs
        └── kitchen_sink.rs
```

### Available Stories

| Story | Description |
|-------|-------------|
| `pin_states` | Pin data states: Empty, Valid, Invalid, Constant |
| `unit_view` | Unit boxes with different lifecycle states |
| `connections` | Bezier curve connections between pins |
| `graph_view` | Interactive graph canvas with physics |
| `kitchen_sink` | All components in one view (default) |

### Usage

```bash
cargo run -p storybook              # Kitchen sink view
cargo run -p storybook -- pin_states
cargo run -p storybook -- --list    # List all stories
```

---

## GPUI API Notes

1. **Pixels private field**: Use `.into()` for f32 conversion
2. **Cubic bezier**: `cubic_bezier_to(to, control_a, control_b)`
3. **Canvas signature**: 4 args - `(prepaint_fn, paint_fn)` where paint takes `(bounds, prepaint_result, window, cx)`
4. **Spawn syntax**: `cx.spawn(async move |view, cx| { ... })`
5. **Platform modifier**: `event.modifiers.platform` for Cmd key

---

## Total Test Coverage

- unit: 37 tests
- hud: 8 tests
- **Total: 45 tests passing**

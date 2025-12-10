# Phase 1: Core Runtime Implementation - 2025-12-10

## Summary

Implemented Phase 1 of the Unit to GPUI migration plan: Core Runtime enhancements enabling type-erased data cloning and event-driven architecture.

## New Files Created

### 1. `crates/unit/src/cloneable_any.rs` (~80 lines)

Type erasure with clonable trait objects:

```rust
pub trait CloneableAny: Any + Send + Sync + Debug {
    fn clone_box(&self) -> Box<dyn CloneableAny>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_id_of(&self) -> TypeId;
    fn type_name_of(&self) -> &'static str;
}
```

Key features:
- Blanket impl for `T: Clone + Send + Sync + Debug + 'static`
- `downcast<T>()`, `downcast_ref<T>()`, `downcast_mut<T>()` helpers
- 6 tests passing

### 2. `crates/unit/src/error.rs` (~230 lines)

Comprehensive error types:

```rust
pub enum UnitError { Pin(PinError), Connection(ConnectionError), Computation, Graph(GraphError) }
pub enum PinError { NotFound, TypeMismatch, AlreadyExists, ConstantViolation, InvalidState }
pub enum ConnectionError { SourceNotFound, TargetNotFound, CycleDetected, TypeMismatch, ... }
pub enum GraphError { UnitNotFound, UnitAlreadyExists, MergeNotFound, ... }
```

### 3. `crates/unit/src/event.rs` (~200 lines)

Runtime event system:

```rust
pub enum RuntimeEvent {
    PinData { unit_id, pin_name, data: Arc<dyn CloneableAny> },
    PinDrop { unit_id, pin_name },
    PinInvalid { unit_id, pin_name },
    UnitError { unit_id, error },
    LifecycleChanged { unit_id, playing },
    Connected { source_unit, source_pin, target_unit, target_pin },
    Disconnected { ... },
    UnitAdded { unit_id },
    UnitRemoved { unit_id },
}

pub struct EventBus {
    fn emit(&self, event: RuntimeEvent);
    fn subscribe(&self, handler: EventHandler);
    fn process(&self) -> usize;
}
```

### 4. `crates/unit/src/scheduler.rs` (~250 lines)

Event scheduler with buffering:

```rust
pub enum SchedulerMode { Paused, Playing, Stepping }

pub struct EventScheduler {
    fn play(&self);      // Flush buffer, start processing
    fn pause(&self);     // Buffer events
    fn step_mode(&self); // Process one then pause
    fn emit(&self, event: RuntimeEvent);
    fn tick(&self) -> usize;
}
```

Features:
- Events buffered when paused
- Automatic flush on play()
- Step mode for debugging
- Buffer overflow protection (10K limit)

## Modified Files

### `crates/unit/src/any_pin.rs`

Added new methods to `AnyPin` trait:

```rust
fn push_cloneable(&mut self, data: Arc<dyn CloneableAny>) -> Result<(), PinTypeError>;
fn clone_data(&self) -> Option<Arc<dyn CloneableAny>>;
```

Changed `Pin<T>` bounds to require `Debug`:
```rust
impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> AnyPin for Pin<T>
```

### `crates/unit/src/merge.rs`

**Fixed data propagation** (the critical gap):

```rust
// Before: stubbed, couldn't clone type-erased data
fn propagate_to_outputs(&mut self) {
    // For now, just mark that we have data
}

// After: working fan-out with CloneableAny
fn propagate_to_outputs(&mut self) {
    let data = match &self.current_data {
        Some(d) => d.clone(),
        None => return,
    };
    for output in self.outputs.values_mut() {
        output.push_cloneable(data.clone())?;
    }
}
```

Changed `current_data` type:
```rust
// Before
current_data: Option<Box<dyn Any + Send + Sync>>

// After
current_data: Option<Arc<dyn CloneableAny>>
```

### `crates/unit/src/lib.rs`

Added exports:
```rust
pub use cloneable_any::{CloneableAny, downcast, downcast_ref, downcast_mut};
pub use error::{UnitError, PinError, ConnectionError, GraphError, UnitResult};
pub use event::{RuntimeEvent, EventBus, EventHandler};
pub use scheduler::{EventScheduler, SchedulerMode, SchedulerStats, SchedulerBuilder};
pub use merge::{Merge, MergeBuilder, MergeSpec, MergePlug};
pub use any_pin::{AnyPin, PinTypeError};
```

### `crates/unit/src/functional.rs` and `crates/unit/src/graph.rs`

Added `Debug` bounds to all generic type parameters to support `CloneableAny`.

## Test Results

```
running 58 tests
test cloneable_any::tests::test_clone_box ... ok
test cloneable_any::tests::test_clone_string ... ok
test cloneable_any::tests::test_downcast_ref ... ok
test cloneable_any::tests::test_downcast_wrong_type ... ok
test cloneable_any::tests::test_type_name ... ok
test cloneable_any::tests::test_box_clone_via_clone_box ... ok
test error::tests::test_pin_error_display ... ok
test error::tests::test_connection_error_cycle ... ok
test error::tests::test_unit_error_from_pin ... ok
test error::tests::test_unit_error_source ... ok
test event::tests::test_event_creation ... ok
test event::tests::test_event_bus_emit_process ... ok
test event::tests::test_event_bus_no_reentrant ... ok
test event::tests::test_event_bus_drain ... ok
test event::tests::test_event_clone ... ok
test scheduler::tests::test_scheduler_buffer_when_paused ... ok
test scheduler::tests::test_scheduler_play_flushes_buffer ... ok
test scheduler::tests::test_scheduler_processes_when_playing ... ok
test scheduler::tests::test_scheduler_stepping_mode ... ok
test scheduler::tests::test_scheduler_buffer_overflow ... ok
test scheduler::tests::test_scheduler_builder ... ok
... (37 more passing)

test result: ok. 58 passed; 0 failed
```

## Architecture Notes

### Why Arc<dyn CloneableAny> instead of Box?

- `Arc` allows shared ownership across event system
- Events can be cloned without re-cloning the underlying data
- Fan-out to multiple outputs shares same data reference
- Thread-safe by default

### Debug Bound Propagation

Adding `Debug` to `CloneableAny` required updating type bounds across:
- `any_pin.rs` - `AnyPin` impl for `Pin<T>`
- `merge.rs` - `add_input()`, `add_output()`, `MergeBuilder`
- `functional.rs` - `SimpleUnit::add_input()`, `add_output()`, builder methods
- `graph.rs` - `expose_input()`, `expose_output()`, `GraphBuilder`

This is a reasonable tradeoff since most data types implement `Debug`.

## Next Steps (Phase 1 remaining)

1. **Connection Registry** in `graph.rs`:
   - `connect(source_unit, source_pin, target_unit, target_pin)`
   - Type validation
   - Event routing through merges

2. Integration test for end-to-end data flow

## Lines Changed

| File | Added | Modified |
|------|-------|----------|
| `cloneable_any.rs` | ~80 | - |
| `error.rs` | ~230 | - |
| `event.rs` | ~200 | - |
| `scheduler.rs` | ~250 | - |
| `any_pin.rs` | - | +20 |
| `merge.rs` | - | +50 |
| `lib.rs` | - | +10 |
| `functional.rs` | - | +20 |
| `graph.rs` | - | +10 |
| **Total** | ~760 | +110 |

---

**Status:** Phase 1 core runtime ~80% complete. Data propagation now works.

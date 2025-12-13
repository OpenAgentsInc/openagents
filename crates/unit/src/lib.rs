//! Unit Runtime
//!
//! A Rust port of the Unit visual programming framework.
//! Provides MIMO (Multi-Input Multi-Output) finite state machines
//! for dataflow programming.
//!
//! # Core Concepts
//!
//! - **Pin<T>**: Data container for input/output ports
//! - **Unit**: Base trait for all computation nodes
//! - **Primitive**: Reactive Unit that responds to pin events
//! - **Functional**: Computation node with f(input) -> output
//! - **Merge**: Fan-in/fan-out connection between pins
//! - **Graph**: Composite container holding Units and Merges
//!
//! # Example
//!
//! ```rust,ignore
//! use unit::{Pin, PinOpt};
//!
//! let mut pin: Pin<i32> = Pin::new(PinOpt::default());
//! pin.push(42);
//! assert_eq!(pin.peak(), Some(&42));
//! let value = pin.take();
//! assert_eq!(value, Some(42));
//! assert!(pin.is_empty());
//! ```

mod any_pin;
mod cloneable_any;
mod error;
mod event;
mod functional;
mod geometry;
mod graph;
mod merge;
mod physics;
mod pin;
mod primitive;
mod scheduler;
pub mod spec;
pub mod system;
mod unit;
mod value;

pub use any_pin::{AnyPin, PinTypeError};
pub use cloneable_any::{CloneableAny, downcast, downcast_mut, downcast_ref};
pub use error::{ConnectionError, GraphError, PinError, UnitError, UnitResult};
pub use event::{EventBus, EventHandler, RuntimeEvent};
pub use functional::Functional;
pub use geometry::{Point, Shape, Thing, point_in_node, surface_distance};
pub use graph::{Graph, PinExposure};
pub use merge::{Merge, MergeBuilder, MergePlug, MergeSpec};
pub use physics::{
    SimConnection, SimNode, SimulationConfig, apply_forces, cool, integrate, reheat,
    run_until_settled, should_stop, tick,
};
pub use pin::{Pin, PinEvent, PinOpt, PinSnapshot, PinState};
pub use primitive::{Primitive, PrimitiveState};
pub use scheduler::{EventScheduler, SchedulerBuilder, SchedulerMode, SchedulerStats};
pub use spec::GraphSpec;
pub use system::{register_system_units, system_registry};
pub use unit::{IO, Lifecycle, Unit, UnitEvent};
pub use value::Value;

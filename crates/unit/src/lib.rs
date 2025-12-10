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

mod pin;
mod any_pin;
mod unit;
mod primitive;
mod functional;
mod merge;
mod graph;
mod spec;
mod geometry;
mod physics;

pub use pin::{Pin, PinOpt, PinState, PinEvent, PinSnapshot};
pub use any_pin::AnyPin;
pub use unit::{Unit, Lifecycle, UnitEvent};
pub use primitive::Primitive;
pub use functional::Functional;
pub use merge::Merge;
pub use graph::{Graph, PinExposure};
pub use spec::GraphSpec;
pub use geometry::{Point, Shape, Thing, surface_distance, point_in_node};
pub use physics::{SimNode, SimConnection, SimulationConfig, apply_forces, integrate, should_stop};

//! HUD: GPUI visualization layer for Unit dataflow graphs
//!
//! This crate provides GPUI components for visualizing and interacting with
//! Unit's dataflow graph system. It bridges the Unit runtime with GPUI rendering.
//!
//! # Components
//!
//! - `PinView` - Visualizes a single pin with its state indicator
//! - `UnitView` - Visualizes a unit as a box with input/output pins
//! - `GraphView` - Full graph canvas with physics-based layout
//! - `ConnectionView` - Bezier curve connections between pins
//! - `SelectionManager` - Multi-select state management
//! - `CommandHistory` - Undo/redo with command pattern

mod actions;
mod connection;
mod graph_view;
mod history;
mod pin_view;
mod selection;
mod unit_view;

pub use actions::*;
pub use connection::*;
pub use graph_view::*;
pub use history::*;
pub use pin_view::*;
pub use selection::*;
pub use unit_view::*;

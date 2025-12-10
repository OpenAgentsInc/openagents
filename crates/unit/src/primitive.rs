//! Primitive: Reactive Unit that responds to pin events
//!
//! A Primitive extends Unit with reactive callbacks that fire
//! when data arrives or is consumed on pins.

use crate::unit::Unit;
use std::any::Any;

/// Primitive: Unit with reactive event handlers
///
/// Extends Unit with callbacks for:
/// - Input data arrival (`on_input_data`)
/// - Input data consumption (`on_input_drop`)
/// - Input invalidation (`on_input_invalid`)
/// - Output data consumption (`on_output_drop`)
///
/// The reactive loop:
/// 1. Data arrives on input pin
/// 2. `on_input_data` is called
/// 3. If all inputs ready, `forward` is called
/// 4. Outputs are produced
/// 5. When outputs consumed, `backward` may be called
pub trait Primitive: Unit {
    /// Called when data arrives on an input pin
    ///
    /// This is the main reactive entry point. Implementations should:
    /// 1. Check if all required inputs are ready
    /// 2. If so, perform computation and produce outputs
    fn on_input_data(&mut self, name: &str, data: &dyn Any);

    /// Called when data is consumed (taken) from an input pin
    fn on_input_drop(&mut self, name: &str, data: &dyn Any);

    /// Called when an input pin is invalidated
    fn on_input_invalid(&mut self, name: &str);

    /// Called when data arrives on an output pin (for backpropagation)
    fn on_output_data(&mut self, name: &str, data: &dyn Any) {
        // Default: no-op, override for backpropagation support
        let _ = (name, data);
    }

    /// Called when output data is consumed
    ///
    /// This enables backward propagation - when outputs are consumed,
    /// the unit may want to consume its inputs and prepare for next cycle.
    fn on_output_drop(&mut self, name: &str) {
        let _ = name;
    }

    /// Check if the unit is ready to fire (all inputs have data)
    ///
    /// Default implementation checks all non-ignored inputs have data.
    fn is_ready(&self) -> bool {
        for name in self.input_names() {
            if let Some(pin) = self.input(name) {
                if !pin.is_ignored() && !pin.is_active() {
                    return false;
                }
            }
        }
        true
    }

    /// Forward pass: called when inputs are ready
    ///
    /// Override this to implement the main computation.
    /// Default implementation does nothing.
    fn forward(&mut self) {}

    /// Backward pass: called when outputs are consumed
    ///
    /// Override this to implement cleanup/backpropagation.
    /// Default implementation does nothing.
    fn backward(&mut self) {}
}

/// Helper struct for building Primitive implementations
pub struct PrimitiveState {
    /// Tracks which inputs have data
    pub inputs_ready: std::collections::HashSet<String>,
    /// Tracks which outputs have been consumed
    pub outputs_consumed: std::collections::HashSet<String>,
}

impl Default for PrimitiveState {
    fn default() -> Self {
        Self::new()
    }
}

impl PrimitiveState {
    pub fn new() -> Self {
        Self {
            inputs_ready: std::collections::HashSet::new(),
            outputs_consumed: std::collections::HashSet::new(),
        }
    }

    /// Mark an input as having data
    pub fn mark_input_ready(&mut self, name: &str) {
        self.inputs_ready.insert(name.to_string());
    }

    /// Mark an input as consumed
    pub fn mark_input_consumed(&mut self, name: &str) {
        self.inputs_ready.remove(name);
    }

    /// Mark an output as consumed
    pub fn mark_output_consumed(&mut self, name: &str) {
        self.outputs_consumed.insert(name.to_string());
    }

    /// Clear output consumed tracking
    pub fn clear_outputs_consumed(&mut self) {
        self.outputs_consumed.clear();
    }

    /// Check if a specific input is ready
    pub fn is_input_ready(&self, name: &str) -> bool {
        self.inputs_ready.contains(name)
    }

    /// Check if all specified inputs are ready
    pub fn all_inputs_ready(&self, names: &[&str]) -> bool {
        names.iter().all(|n| self.inputs_ready.contains(*n))
    }

    /// Check if all outputs have been consumed
    pub fn all_outputs_consumed(&self, names: &[&str]) -> bool {
        names.iter().all(|n| self.outputs_consumed.contains(*n))
    }

    /// Reset state
    pub fn reset(&mut self) {
        self.inputs_ready.clear();
        self.outputs_consumed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_state() {
        let mut state = PrimitiveState::new();

        state.mark_input_ready("a");
        state.mark_input_ready("b");

        assert!(state.is_input_ready("a"));
        assert!(state.is_input_ready("b"));
        assert!(!state.is_input_ready("c"));

        assert!(state.all_inputs_ready(&["a", "b"]));
        assert!(!state.all_inputs_ready(&["a", "b", "c"]));

        state.mark_input_consumed("a");
        assert!(!state.is_input_ready("a"));
    }
}

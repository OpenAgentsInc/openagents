//! Primitive: Reactive Unit that responds to pin events
//!
//! A Primitive extends Unit with reactive callbacks that fire
//! when data arrives or is consumed on pins.

use crate::any_pin::{AnyPin, AnyPinExt};
use crate::pin::{Pin, PinOpt};
use crate::unit::{Lifecycle, Unit};
use std::any::Any;
use std::collections::HashMap;

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

/// State container for Primitive/System units
///
/// Holds input/output pins, lifecycle state, and tracking for reactive units.
pub struct PrimitiveState {
    /// Unit identifier
    id: String,
    /// Input pins by name
    inputs: HashMap<String, Box<dyn AnyPin>>,
    /// Output pins by name
    outputs: HashMap<String, Box<dyn AnyPin>>,
    /// Current lifecycle state
    lifecycle: Lifecycle,
    /// Error message if any
    error: Option<String>,
    /// Tracks which inputs have data ready
    inputs_ready: std::collections::HashSet<String>,
    /// Tracks which outputs have been consumed
    outputs_consumed: std::collections::HashSet<String>,
}

impl Default for PrimitiveState {
    fn default() -> Self {
        Self::new("default")
    }
}

impl std::fmt::Debug for PrimitiveState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimitiveState")
            .field("id", &self.id)
            .field("lifecycle", &self.lifecycle)
            .field("inputs", &self.inputs.keys().collect::<Vec<_>>())
            .field("outputs", &self.outputs.keys().collect::<Vec<_>>())
            .field("error", &self.error)
            .finish()
    }
}

impl PrimitiveState {
    /// Create a new PrimitiveState with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            lifecycle: Lifecycle::Paused,
            error: None,
            inputs_ready: std::collections::HashSet::new(),
            outputs_consumed: std::collections::HashSet::new(),
        }
    }

    /// Get the unit ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the lifecycle state
    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }

    /// Set lifecycle to playing
    pub fn play(&mut self) {
        self.lifecycle = Lifecycle::Playing;
    }

    /// Set lifecycle to paused
    pub fn pause(&mut self) {
        self.lifecycle = Lifecycle::Paused;
    }

    /// Add a typed input pin with default options
    pub fn add_input<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        &mut self,
        name: impl Into<String>,
    ) {
        let pin: Pin<T> = Pin::new(PinOpt::default());
        self.inputs.insert(name.into(), Box::new(pin));
    }

    /// Add a typed input pin with options
    pub fn add_input_with_opt<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        &mut self,
        name: impl Into<String>,
        opt: PinOpt,
    ) {
        let pin: Pin<T> = Pin::new(opt);
        self.inputs.insert(name.into(), Box::new(pin));
    }

    /// Add a typed output pin with default options
    pub fn add_output<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        &mut self,
        name: impl Into<String>,
    ) {
        let pin: Pin<T> = Pin::new(PinOpt::default());
        self.outputs.insert(name.into(), Box::new(pin));
    }

    /// Add a typed output pin with options
    pub fn add_output_with_opt<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        &mut self,
        name: impl Into<String>,
        opt: PinOpt,
    ) {
        let pin: Pin<T> = Pin::new(opt);
        self.outputs.insert(name.into(), Box::new(pin));
    }

    /// Push data to an input pin
    pub fn push_input(&mut self, name: &str, data: Box<dyn Any + Send>) -> Result<(), String> {
        if let Some(pin) = self.inputs.get_mut(name) {
            pin.push_any(data).map_err(|e| e.to_string())?;
            self.inputs_ready.insert(name.to_string());
            Ok(())
        } else {
            Err(format!("Input pin '{}' not found", name))
        }
    }

    /// Take data from an output pin
    pub fn take_output(&mut self, name: &str) -> Option<Box<dyn Any + Send>> {
        if let Some(pin) = self.outputs.get_mut(name) {
            let result = pin.take_any();
            if result.is_some() {
                self.outputs_consumed.insert(name.to_string());
            }
            result
        } else {
            None
        }
    }

    /// Get a typed input pin reference
    pub fn input<T: Clone + Send + Sync + 'static>(&self, name: &str) -> Option<&Pin<T>> {
        self.inputs
            .get(name)
            .and_then(|pin| pin.downcast_ref::<T>())
    }

    /// Get a mutable typed input pin reference
    pub fn input_mut<T: Clone + Send + Sync + 'static>(
        &mut self,
        name: &str,
    ) -> Option<&mut Pin<T>> {
        self.inputs
            .get_mut(name)
            .and_then(|pin| pin.downcast_mut::<T>())
    }

    /// Get a typed output pin reference
    pub fn output<T: Clone + Send + Sync + 'static>(&self, name: &str) -> Option<&Pin<T>> {
        self.outputs
            .get(name)
            .and_then(|pin| pin.downcast_ref::<T>())
    }

    /// Get a mutable typed output pin reference
    pub fn output_mut<T: Clone + Send + Sync + 'static>(
        &mut self,
        name: &str,
    ) -> Option<&mut Pin<T>> {
        self.outputs
            .get_mut(name)
            .and_then(|pin| pin.downcast_mut::<T>())
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

    /// Get input pin names
    pub fn input_names(&self) -> Vec<&str> {
        self.inputs.keys().map(|s| s.as_str()).collect()
    }

    /// Get output pin names
    pub fn output_names(&self) -> Vec<&str> {
        self.outputs.keys().map(|s| s.as_str()).collect()
    }

    /// Set error state
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    /// Clear error state
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Get error if any
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_state() {
        let mut state = PrimitiveState::new("test");

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

    #[test]
    fn test_primitive_state_pins() {
        let mut state = PrimitiveState::new("test");

        state.add_input::<f64>("x");
        state.add_output::<f64>("result");

        assert!(state.input::<f64>("x").is_some());
        assert!(state.output::<f64>("result").is_some());
        assert!(state.input::<f64>("nonexistent").is_none());
    }

    #[test]
    fn test_primitive_state_lifecycle() {
        let mut state = PrimitiveState::new("test");

        assert_eq!(state.lifecycle(), Lifecycle::Paused);

        state.play();
        assert_eq!(state.lifecycle(), Lifecycle::Playing);

        state.pause();
        assert_eq!(state.lifecycle(), Lifecycle::Paused);
    }
}

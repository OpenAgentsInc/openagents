//! Functional: Computation node with callback-based execution
//!
//! A Functional extends Primitive with a structured computation model:
//! - `f(input, done, fail)` - async-style with callbacks
//! - `m(input) -> output` - sync-style returning directly

use crate::any_pin::AnyPin;
use crate::pin::{Pin, PinOpt};
use crate::primitive::{Primitive, PrimitiveState};
use crate::unit::{Lifecycle, Unit};
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;

/// Callback type for signaling successful computation
pub type DoneCallback<O> = Box<dyn FnOnce(O) + Send>;

/// Callback type for signaling failed computation
pub type FailCallback = Box<dyn FnOnce(String) + Send>;

/// Functional: Computation node trait
///
/// Provides two computation styles:
/// - `f()`: Callback-based, for async operations
/// - `m()`: Direct return, for sync operations
///
/// Implementations should override one or both methods.
pub trait Functional<I, O>: Primitive
where
    I: Send + 'static,
    O: Send + 'static,
{
    /// Execute computation with callbacks (async style)
    ///
    /// Called when all inputs are ready. Implementation should:
    /// 1. Process the input
    /// 2. Call `done(output)` on success
    /// 3. Call `fail(error)` on failure
    ///
    /// Default implementation calls `m()` if it returns Some.
    fn f(&mut self, input: I, done: DoneCallback<O>, fail: FailCallback) {
        match self.m(&input) {
            Some(output) => done(output),
            None => fail("m() returned None and f() not overridden".to_string()),
        }
    }

    /// Execute computation directly (sync style)
    ///
    /// Returns Some(output) on success, None if computation can't complete.
    /// Override this for simple, synchronous computations.
    fn m(&self, _input: &I) -> Option<O> {
        None
    }

    /// Called when inputs change (cleanup)
    ///
    /// Override to cancel in-flight async operations.
    fn d(&mut self) {}
}

/// A simple concrete Unit implementation for building functional units
pub struct SimpleUnit {
    id: String,
    inputs: HashMap<String, Box<dyn AnyPin>>,
    outputs: HashMap<String, Box<dyn AnyPin>>,
    lifecycle: Lifecycle,
    error: Option<String>,
    state: PrimitiveState,
}

impl SimpleUnit {
    /// Create a new SimpleUnit with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            lifecycle: Lifecycle::Paused,
            error: None,
            state: PrimitiveState::new("simple"),
        }
    }

    /// Add a typed input pin
    pub fn add_input<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        &mut self,
        name: impl Into<String>,
        opt: PinOpt,
    ) {
        let pin: Pin<T> = Pin::new(opt);
        self.inputs.insert(name.into(), Box::new(pin));
    }

    /// Add a typed output pin
    pub fn add_output<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        &mut self,
        name: impl Into<String>,
        opt: PinOpt,
    ) {
        let pin: Pin<T> = Pin::new(opt);
        self.outputs.insert(name.into(), Box::new(pin));
    }

    /// Get the primitive state for tracking
    pub fn primitive_state(&self) -> &PrimitiveState {
        &self.state
    }

    /// Get mutable primitive state
    pub fn primitive_state_mut(&mut self) -> &mut PrimitiveState {
        &mut self.state
    }

    /// Push data to an input pin
    pub fn push_input(&mut self, name: &str, data: Box<dyn Any + Send>) -> Result<(), String> {
        if let Some(pin) = self.inputs.get_mut(name) {
            pin.push_any(data)
                .map_err(|e| e.to_string())
        } else {
            Err(format!("Input '{}' not found", name))
        }
    }

    /// Push data to an output pin
    pub fn push_output(&mut self, name: &str, data: Box<dyn Any + Send>) -> Result<(), String> {
        if let Some(pin) = self.outputs.get_mut(name) {
            pin.push_any(data)
                .map_err(|e| e.to_string())
        } else {
            Err(format!("Output '{}' not found", name))
        }
    }
}

impl Unit for SimpleUnit {
    fn id(&self) -> &str {
        &self.id
    }

    fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }

    fn play(&mut self) {
        self.lifecycle = Lifecycle::Playing;
    }

    fn pause(&mut self) {
        self.lifecycle = Lifecycle::Paused;
    }

    fn reset(&mut self) {
        self.error = None;
        self.state.reset();
        // Reset all pins
        for pin in self.inputs.values_mut() {
            pin.invalidate();
        }
        for pin in self.outputs.values_mut() {
            pin.invalidate();
        }
    }

    fn input(&self, name: &str) -> Option<&dyn AnyPin> {
        self.inputs.get(name).map(|p| p.as_ref())
    }

    fn input_mut(&mut self, name: &str) -> Option<&mut (dyn AnyPin + 'static)> {
        self.inputs.get_mut(name).map(|p| p.as_mut() as &mut (dyn AnyPin + 'static))
    }

    fn output(&self, name: &str) -> Option<&dyn AnyPin> {
        self.outputs.get(name).map(|p| p.as_ref())
    }

    fn output_mut(&mut self, name: &str) -> Option<&mut (dyn AnyPin + 'static)> {
        self.outputs.get_mut(name).map(|p| p.as_mut() as &mut (dyn AnyPin + 'static))
    }

    fn input_names(&self) -> Vec<&str> {
        self.inputs.keys().map(|s| s.as_str()).collect()
    }

    fn output_names(&self) -> Vec<&str> {
        self.outputs.keys().map(|s| s.as_str()).collect()
    }

    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    fn clear_error(&mut self) {
        self.error = None;
    }

    fn snapshot(&self) -> Value {
        // TODO: Implement proper serialization
        serde_json::json!({
            "id": self.id,
            "lifecycle": match self.lifecycle {
                Lifecycle::Paused => "paused",
                Lifecycle::Playing => "playing",
            },
            "error": self.error,
        })
    }

    fn restore(&mut self, state: &Value) {
        if let Some(error) = state.get("error").and_then(|v| v.as_str()) {
            self.error = Some(error.to_string());
        }
        if let Some(lifecycle) = state.get("lifecycle").and_then(|v| v.as_str()) {
            self.lifecycle = match lifecycle {
                "playing" => Lifecycle::Playing,
                _ => Lifecycle::Paused,
            };
        }
    }
}

impl Primitive for SimpleUnit {
    fn on_input_data(&mut self, name: &str, _data: &dyn Any) {
        self.state.mark_input_ready(name);

        // Check if we should fire
        if self.lifecycle == Lifecycle::Playing && self.is_ready() {
            self.forward();
        }
    }

    fn on_input_drop(&mut self, name: &str, _data: &dyn Any) {
        self.state.mark_input_consumed(name);
    }

    fn on_input_invalid(&mut self, name: &str) {
        self.state.mark_input_consumed(name);
    }

    fn on_output_drop(&mut self, name: &str) {
        self.state.mark_output_consumed(name);

        // Check if all outputs consumed
        let output_names: Vec<&str> = self.output_names();
        if self.state.all_outputs_consumed(&output_names) {
            self.backward();
            self.state.clear_outputs_consumed();
        }
    }

    fn is_ready(&self) -> bool {
        let input_names: Vec<&str> = self.input_names();
        for name in input_names {
            if let Some(pin) = self.input(name) {
                if !pin.is_ignored() && !pin.is_active() {
                    return false;
                }
            }
        }
        true
    }
}

/// Builder for creating SimpleUnit instances
pub struct SimpleUnitBuilder {
    unit: SimpleUnit,
}

impl SimpleUnitBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            unit: SimpleUnit::new(id),
        }
    }

    /// Add a typed input pin
    pub fn input<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        mut self,
        name: impl Into<String>,
    ) -> Self {
        self.unit.add_input::<T>(name, PinOpt::default());
        self
    }

    /// Add a typed input pin with options
    pub fn input_with_opt<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        mut self,
        name: impl Into<String>,
        opt: PinOpt,
    ) -> Self {
        self.unit.add_input::<T>(name, opt);
        self
    }

    /// Add a typed output pin
    pub fn output<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        mut self,
        name: impl Into<String>,
    ) -> Self {
        self.unit.add_output::<T>(name, PinOpt::default());
        self
    }

    /// Add a typed output pin with options
    pub fn output_with_opt<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        mut self,
        name: impl Into<String>,
        opt: PinOpt,
    ) -> Self {
        self.unit.add_output::<T>(name, opt);
        self
    }

    /// Build the unit
    pub fn build(self) -> SimpleUnit {
        self.unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_unit_builder() {
        let unit = SimpleUnitBuilder::new("test")
            .input::<i32>("a")
            .input::<i32>("b")
            .output::<i32>("sum")
            .build();

        assert_eq!(unit.id(), "test");
        assert!(unit.has_input("a"));
        assert!(unit.has_input("b"));
        assert!(unit.has_output("sum"));
        assert!(!unit.has_input("c"));
    }

    #[test]
    fn test_simple_unit_lifecycle() {
        let mut unit = SimpleUnit::new("test");
        assert!(unit.is_paused());

        unit.play();
        assert!(unit.is_playing());

        unit.pause();
        assert!(unit.is_paused());
    }

    #[test]
    fn test_simple_unit_error() {
        let mut unit = SimpleUnit::new("test");
        assert!(!unit.has_error());

        unit.set_error("something went wrong".to_string());
        assert!(unit.has_error());
        assert_eq!(unit.error(), Some("something went wrong"));

        unit.clear_error();
        assert!(!unit.has_error());
    }
}

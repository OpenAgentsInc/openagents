//! Merge: Fan-in/fan-out connection between pins
//!
//! A Merge routes data from multiple inputs to multiple outputs.
//! It acts like a virtual wire that connects pins across units.
//!
//! Semantics:
//! - Fan-in: Multiple inputs, data from any one becomes "current"
//! - Fan-out: Current data is distributed to all outputs
//! - Only one input can be "active" at a time

use crate::any_pin::AnyPin;
use crate::cloneable_any::CloneableAny;
use crate::pin::{Pin, PinOpt};
use crate::primitive::Primitive;
use crate::unit::{Lifecycle, Unit};
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// Merge: Routes data between pins
///
/// Acts as a virtual wire connecting multiple pins.
/// Data pushed to any input becomes the "current" value
/// and is distributed to all outputs.
pub struct Merge {
    id: String,
    /// Input pins (fan-in)
    inputs: HashMap<String, Box<dyn AnyPin>>,
    /// Output pins (fan-out)
    outputs: HashMap<String, Box<dyn AnyPin>>,
    /// Which input is currently active
    current_input: Option<String>,
    /// The current data (cloneable for fan-out)
    current_data: Option<Arc<dyn CloneableAny>>,
    /// Lifecycle state
    lifecycle: Lifecycle,
    /// Error state
    error: Option<String>,
}

impl Merge {
    /// Create a new Merge with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            current_input: None,
            current_data: None,
            lifecycle: Lifecycle::Paused,
            error: None,
        }
    }

    /// Add an input to the merge
    ///
    /// Type parameter determines what data type this input accepts.
    pub fn add_input<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        &mut self,
        name: impl Into<String>,
    ) {
        let pin: Pin<T> = Pin::new(PinOpt::default());
        self.inputs.insert(name.into(), Box::new(pin));
    }

    /// Add an output to the merge
    ///
    /// Type parameter determines what data type this output produces.
    pub fn add_output<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        &mut self,
        name: impl Into<String>,
    ) {
        let pin: Pin<T> = Pin::new(PinOpt::default());
        self.outputs.insert(name.into(), Box::new(pin));
    }

    /// Get the name of the currently active input
    pub fn current(&self) -> Option<&str> {
        self.current_input.as_deref()
    }

    /// Check if merge has any current data
    pub fn has_data(&self) -> bool {
        self.current_data.is_some()
    }

    /// Propagate current data to all outputs
    fn propagate_to_outputs(&mut self) {
        let data = match &self.current_data {
            Some(d) => d.clone(),
            None => return,
        };

        // Clone data to each output pin
        for output in self.outputs.values_mut() {
            if let Err(e) = output.push_cloneable(data.clone()) {
                // Type mismatch - this shouldn't happen if merge is set up correctly
                // Log error but continue with other outputs
                eprintln!("Merge propagation error: {}", e);
            }
        }
    }

    /// Get the current data (if any)
    pub fn data(&self) -> Option<&Arc<dyn CloneableAny>> {
        self.current_data.as_ref()
    }

    /// Store data from an input and propagate to outputs
    pub fn set_data(&mut self, input_name: &str, data: Arc<dyn CloneableAny>) {
        // If different input is now active, clear old state
        if let Some(ref current) = self.current_input {
            if current != input_name {
                self.clear_current();
            }
        }

        self.current_input = Some(input_name.to_string());
        self.current_data = Some(data);

        if self.lifecycle == Lifecycle::Playing {
            self.propagate_to_outputs();
        }
    }

    /// Clear current data and propagate drop to outputs
    fn clear_current(&mut self) {
        self.current_input = None;
        self.current_data = None;

        // Invalidate all outputs
        for output in self.outputs.values_mut() {
            output.invalidate();
        }
    }
}

impl Unit for Merge {
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
        self.clear_current();
        self.error = None;

        for pin in self.inputs.values_mut() {
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
        serde_json::json!({
            "id": self.id,
            "current_input": self.current_input,
            "has_data": self.current_data.is_some(),
        })
    }

    fn restore(&mut self, state: &Value) {
        if let Some(current) = state.get("current_input").and_then(|v| v.as_str()) {
            self.current_input = Some(current.to_string());
        }
    }
}

impl Primitive for Merge {
    fn on_input_data(&mut self, name: &str, _data: &dyn Any) {
        if self.lifecycle != Lifecycle::Playing {
            return;
        }

        // If we have a current input that's different, clear it first
        if let Some(ref current) = self.current_input {
            if current != name {
                self.clear_current();
            }
        }

        // Set this input as current
        self.current_input = Some(name.to_string());

        // Try to clone data from the input pin
        if let Some(input_pin) = self.inputs.get(name) {
            if let Some(cloned_data) = input_pin.clone_data() {
                self.current_data = Some(cloned_data);
                self.propagate_to_outputs();
            }
        }
    }

    fn on_input_drop(&mut self, name: &str, _data: &dyn Any) {
        // If this was the current input, clear everything
        if self.current_input.as_deref() == Some(name) {
            self.clear_current();
        }
    }

    fn on_input_invalid(&mut self, name: &str) {
        // If this was the current input, clear everything
        if self.current_input.as_deref() == Some(name) {
            self.clear_current();
        }
    }

    fn on_output_drop(&mut self, _name: &str) {
        // When an output is consumed, we might want to consume the input
        // This implements backward propagation
        // For now, do nothing - let the graph handle this
    }
}

/// Builder for creating Merge instances
pub struct MergeBuilder {
    merge: Merge,
}

impl MergeBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            merge: Merge::new(id),
        }
    }

    /// Add a typed input
    pub fn input<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        mut self,
        name: impl Into<String>,
    ) -> Self {
        self.merge.add_input::<T>(name);
        self
    }

    /// Add a typed output
    pub fn output<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
        mut self,
        name: impl Into<String>,
    ) -> Self {
        self.merge.add_output::<T>(name);
        self
    }

    /// Build the merge
    pub fn build(self) -> Merge {
        self.merge
    }
}

/// Specification for a merge plug (connection point)
#[derive(Debug, Clone)]
pub struct MergePlug {
    /// ID of the unit this plug connects to
    pub unit_id: String,
    /// Type of pin (input or output)
    pub pin_type: crate::unit::IO,
    /// Name of the pin
    pub pin_name: String,
}

/// Full specification of a merge's connections
#[derive(Debug, Clone, Default)]
pub struct MergeSpec {
    /// ID of this merge
    pub id: String,
    /// Input plugs (connections from unit outputs to merge inputs)
    pub input_plugs: Vec<MergePlug>,
    /// Output plugs (connections from merge outputs to unit inputs)
    pub output_plugs: Vec<MergePlug>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_builder() {
        let merge = MergeBuilder::new("test-merge")
            .input::<i32>("in1")
            .input::<i32>("in2")
            .output::<i32>("out1")
            .build();

        assert_eq!(merge.id(), "test-merge");
        assert!(merge.has_input("in1"));
        assert!(merge.has_input("in2"));
        assert!(merge.has_output("out1"));
    }

    #[test]
    fn test_merge_current() {
        let mut merge = MergeBuilder::new("test")
            .input::<i32>("a")
            .input::<i32>("b")
            .output::<i32>("out")
            .build();

        merge.play();

        // Initially no current
        assert!(merge.current().is_none());

        // Simulate data arriving on input "a"
        let data: i32 = 42;
        merge.on_input_data("a", &data as &dyn Any);

        assert_eq!(merge.current(), Some("a"));
    }

    #[test]
    fn test_merge_lifecycle() {
        let mut merge = Merge::new("test");

        assert!(merge.is_paused());

        merge.play();
        assert!(merge.is_playing());

        merge.pause();
        assert!(merge.is_paused());
    }
}

//! Graph: Composite container for Units and Merges
//!
//! A Graph is itself a Unit, containing child Units connected by Merges.
//! It exposes selected internal pins as its own interface.

use crate::any_pin::AnyPin;
use crate::merge::Merge;
use crate::pin::{Pin, PinOpt};
use crate::primitive::Primitive;
use crate::unit::{Lifecycle, Unit};
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;

/// Specification for exposing an internal pin to the graph's interface
#[derive(Debug, Clone)]
pub struct PinExposure {
    /// ID of the internal unit
    pub unit_id: String,
    /// Name of the pin on the internal unit
    pub pin_name: String,
    /// Name exposed on the graph's interface
    pub exposed_name: String,
}

/// Graph: Composite Unit containing child Units and Merges
///
/// A Graph is the fundamental composition mechanism in Unit.
/// It contains:
/// - Child units (computation nodes)
/// - Merges (connections between pins)
/// - Exposed pins (graph's external interface)
///
/// The graph itself is a Unit, so graphs can be nested.
pub struct Graph {
    id: String,
    /// Child units
    units: HashMap<String, Box<dyn Unit>>,
    /// Merges connecting pins
    merges: HashMap<String, Merge>,
    /// Exposed input pins (from internal units to graph interface)
    exposed_inputs: HashMap<String, PinExposure>,
    /// Exposed output pins (from internal units to graph interface)
    exposed_outputs: HashMap<String, PinExposure>,
    /// Graph-level input pins (for the external interface)
    inputs: HashMap<String, Box<dyn AnyPin>>,
    /// Graph-level output pins (for the external interface)
    outputs: HashMap<String, Box<dyn AnyPin>>,
    /// Lifecycle state
    lifecycle: Lifecycle,
    /// Error state
    error: Option<String>,
}

impl Graph {
    /// Create a new empty Graph
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            units: HashMap::new(),
            merges: HashMap::new(),
            exposed_inputs: HashMap::new(),
            exposed_outputs: HashMap::new(),
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            lifecycle: Lifecycle::Paused,
            error: None,
        }
    }

    // Unit management

    /// Add a child unit to the graph
    pub fn add_unit(&mut self, id: impl Into<String>, unit: Box<dyn Unit>) {
        self.units.insert(id.into(), unit);
    }

    /// Remove a child unit from the graph
    pub fn remove_unit(&mut self, id: &str) -> Option<Box<dyn Unit>> {
        self.units.remove(id)
    }

    /// Get a child unit by ID
    pub fn get_unit(&self, id: &str) -> Option<&dyn Unit> {
        self.units.get(id).map(|u| u.as_ref())
    }

    /// Get a mutable child unit by ID
    pub fn get_unit_mut(&mut self, id: &str) -> Option<&mut (dyn Unit + 'static)> {
        self.units.get_mut(id).map(|u| u.as_mut() as &mut (dyn Unit + 'static))
    }

    /// Get all unit IDs
    pub fn unit_ids(&self) -> Vec<&str> {
        self.units.keys().map(|s| s.as_str()).collect()
    }

    /// Check if graph contains a unit
    pub fn has_unit(&self, id: &str) -> bool {
        self.units.contains_key(id)
    }

    // Merge management

    /// Add a merge to the graph
    pub fn add_merge(&mut self, id: impl Into<String>, merge: Merge) {
        self.merges.insert(id.into(), merge);
    }

    /// Create and add a new merge
    pub fn create_merge(&mut self, id: impl Into<String>) -> &mut Merge {
        let id = id.into();
        self.merges.insert(id.clone(), Merge::new(&id));
        self.merges.get_mut(&id).unwrap()
    }

    /// Remove a merge from the graph
    pub fn remove_merge(&mut self, id: &str) -> Option<Merge> {
        self.merges.remove(id)
    }

    /// Get a merge by ID
    pub fn get_merge(&self, id: &str) -> Option<&Merge> {
        self.merges.get(id)
    }

    /// Get a mutable merge by ID
    pub fn get_merge_mut(&mut self, id: &str) -> Option<&mut Merge> {
        self.merges.get_mut(id)
    }

    /// Get all merge IDs
    pub fn merge_ids(&self) -> Vec<&str> {
        self.merges.keys().map(|s| s.as_str()).collect()
    }

    // Pin exposure

    /// Expose an internal input as part of the graph's interface
    pub fn expose_input<T: Clone + Send + Sync + 'static>(
        &mut self,
        exposed_name: impl Into<String>,
        unit_id: impl Into<String>,
        pin_name: impl Into<String>,
    ) {
        let exposed_name = exposed_name.into();
        let exposure = PinExposure {
            unit_id: unit_id.into(),
            pin_name: pin_name.into(),
            exposed_name: exposed_name.clone(),
        };
        self.exposed_inputs.insert(exposed_name.clone(), exposure);

        // Create the interface pin
        let pin: Pin<T> = Pin::new(PinOpt::default());
        self.inputs.insert(exposed_name, Box::new(pin));
    }

    /// Expose an internal output as part of the graph's interface
    pub fn expose_output<T: Clone + Send + Sync + 'static>(
        &mut self,
        exposed_name: impl Into<String>,
        unit_id: impl Into<String>,
        pin_name: impl Into<String>,
    ) {
        let exposed_name = exposed_name.into();
        let exposure = PinExposure {
            unit_id: unit_id.into(),
            pin_name: pin_name.into(),
            exposed_name: exposed_name.clone(),
        };
        self.exposed_outputs.insert(exposed_name.clone(), exposure);

        // Create the interface pin
        let pin: Pin<T> = Pin::new(PinOpt::default());
        self.outputs.insert(exposed_name, Box::new(pin));
    }

    /// Get exposure info for an exposed input
    pub fn get_input_exposure(&self, name: &str) -> Option<&PinExposure> {
        self.exposed_inputs.get(name)
    }

    /// Get exposure info for an exposed output
    pub fn get_output_exposure(&self, name: &str) -> Option<&PinExposure> {
        self.exposed_outputs.get(name)
    }

    // Connection helpers

    /// Connect a unit's output to a merge input
    pub fn connect_to_merge(
        &mut self,
        unit_id: &str,
        output_name: &str,
        merge_id: &str,
        _merge_input: &str,
    ) -> Result<(), String> {
        // Verify unit exists and has the output
        if !self.has_unit(unit_id) {
            return Err(format!("Unit '{}' not found", unit_id));
        }
        if let Some(unit) = self.get_unit(unit_id) {
            if !unit.has_output(output_name) {
                return Err(format!(
                    "Unit '{}' has no output '{}'",
                    unit_id, output_name
                ));
            }
        }

        // Verify merge exists
        if !self.merges.contains_key(merge_id) {
            return Err(format!("Merge '{}' not found", merge_id));
        }

        // Note: In a full implementation, we'd set up event forwarding here
        // For now, just validate the connection is possible

        Ok(())
    }

    /// Connect a merge output to a unit's input
    pub fn connect_from_merge(
        &mut self,
        merge_id: &str,
        _merge_output: &str,
        unit_id: &str,
        input_name: &str,
    ) -> Result<(), String> {
        // Verify merge exists
        if !self.merges.contains_key(merge_id) {
            return Err(format!("Merge '{}' not found", merge_id));
        }

        // Verify unit exists and has the input
        if !self.has_unit(unit_id) {
            return Err(format!("Unit '{}' not found", unit_id));
        }
        if let Some(unit) = self.get_unit(unit_id) {
            if !unit.has_input(input_name) {
                return Err(format!("Unit '{}' has no input '{}'", unit_id, input_name));
            }
        }

        Ok(())
    }

    // Lifecycle propagation

    /// Play all child units
    fn play_children(&mut self) {
        for unit in self.units.values_mut() {
            unit.play();
        }
        for merge in self.merges.values_mut() {
            merge.play();
        }
    }

    /// Pause all child units
    fn pause_children(&mut self) {
        for unit in self.units.values_mut() {
            unit.pause();
        }
        for merge in self.merges.values_mut() {
            merge.pause();
        }
    }

    /// Reset all child units
    fn reset_children(&mut self) {
        for unit in self.units.values_mut() {
            unit.reset();
        }
        for merge in self.merges.values_mut() {
            merge.reset();
        }
    }
}

impl Unit for Graph {
    fn id(&self) -> &str {
        &self.id
    }

    fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }

    fn play(&mut self) {
        self.lifecycle = Lifecycle::Playing;
        self.play_children();
    }

    fn pause(&mut self) {
        self.lifecycle = Lifecycle::Paused;
        self.pause_children();
    }

    fn reset(&mut self) {
        self.error = None;
        self.reset_children();

        // Reset interface pins
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
        let units: HashMap<_, _> = self
            .units
            .iter()
            .map(|(id, unit)| (id.clone(), unit.snapshot()))
            .collect();

        let merges: HashMap<_, _> = self
            .merges
            .iter()
            .map(|(id, merge)| (id.clone(), merge.snapshot()))
            .collect();

        serde_json::json!({
            "id": self.id,
            "units": units,
            "merges": merges,
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

        // Restore child units
        if let Some(units) = state.get("units").and_then(|v| v.as_object()) {
            for (id, unit_state) in units {
                if let Some(unit) = self.units.get_mut(id) {
                    unit.restore(unit_state);
                }
            }
        }

        // Restore merges
        if let Some(merges) = state.get("merges").and_then(|v| v.as_object()) {
            for (id, merge_state) in merges {
                if let Some(merge) = self.merges.get_mut(id) {
                    merge.restore(merge_state);
                }
            }
        }
    }
}

impl Primitive for Graph {
    fn on_input_data(&mut self, name: &str, data: &dyn Any) {
        // Forward data to the exposed internal unit's pin
        if let Some(exposure) = self.exposed_inputs.get(name).cloned() {
            if let Some(unit) = self.units.get_mut(&exposure.unit_id) {
                // Note: Would need to forward the data properly here
                // This is simplified - real impl would need type handling
                let _ = (unit, data);
            }
        }
    }

    fn on_input_drop(&mut self, name: &str, data: &dyn Any) {
        // Forward drop to the exposed internal unit's pin
        if let Some(exposure) = self.exposed_inputs.get(name).cloned() {
            if let Some(unit) = self.units.get_mut(&exposure.unit_id) {
                let _ = (unit, data);
            }
        }
    }

    fn on_input_invalid(&mut self, name: &str) {
        // Forward invalidation to the exposed internal unit's pin
        if let Some(exposure) = self.exposed_inputs.get(name).cloned() {
            if let Some(unit) = self.units.get_mut(&exposure.unit_id) {
                if let Some(pin) = unit.input_mut(&exposure.pin_name) {
                    pin.invalidate();
                }
            }
        }
    }
}

/// Builder for creating Graph instances
pub struct GraphBuilder {
    graph: Graph,
}

impl GraphBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            graph: Graph::new(id),
        }
    }

    /// Add a child unit
    pub fn unit(mut self, id: impl Into<String>, unit: Box<dyn Unit>) -> Self {
        self.graph.add_unit(id, unit);
        self
    }

    /// Add a merge
    pub fn merge(mut self, id: impl Into<String>, merge: Merge) -> Self {
        self.graph.add_merge(id, merge);
        self
    }

    /// Expose an input
    pub fn expose_input<T: Clone + Send + Sync + 'static>(
        mut self,
        exposed_name: impl Into<String>,
        unit_id: impl Into<String>,
        pin_name: impl Into<String>,
    ) -> Self {
        self.graph.expose_input::<T>(exposed_name, unit_id, pin_name);
        self
    }

    /// Expose an output
    pub fn expose_output<T: Clone + Send + Sync + 'static>(
        mut self,
        exposed_name: impl Into<String>,
        unit_id: impl Into<String>,
        pin_name: impl Into<String>,
    ) -> Self {
        self.graph
            .expose_output::<T>(exposed_name, unit_id, pin_name);
        self
    }

    /// Build the graph
    pub fn build(self) -> Graph {
        self.graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functional::SimpleUnit;

    #[test]
    fn test_graph_add_unit() {
        let mut graph = Graph::new("test-graph");

        let unit = SimpleUnit::new("adder");
        graph.add_unit("adder", Box::new(unit));

        assert!(graph.has_unit("adder"));
        assert!(!graph.has_unit("nonexistent"));
    }

    #[test]
    fn test_graph_lifecycle() {
        let mut graph = Graph::new("test");

        let unit = SimpleUnit::new("child");
        graph.add_unit("child", Box::new(unit));

        assert!(graph.is_paused());
        assert!(graph.get_unit("child").unwrap().is_paused());

        graph.play();
        assert!(graph.is_playing());
        assert!(graph.get_unit("child").unwrap().is_playing());

        graph.pause();
        assert!(graph.is_paused());
    }

    #[test]
    fn test_graph_expose_pins() {
        let mut graph = Graph::new("test");

        graph.expose_input::<i32>("x", "adder", "a");
        graph.expose_output::<i32>("result", "adder", "sum");

        assert!(graph.has_input("x"));
        assert!(graph.has_output("result"));

        let exposure = graph.get_input_exposure("x").unwrap();
        assert_eq!(exposure.unit_id, "adder");
        assert_eq!(exposure.pin_name, "a");
    }

    #[test]
    fn test_graph_builder() {
        let unit = SimpleUnit::new("child");

        let graph = GraphBuilder::new("test")
            .unit("child", Box::new(unit))
            .expose_input::<i32>("in", "child", "input")
            .expose_output::<i32>("out", "child", "output")
            .build();

        assert!(graph.has_unit("child"));
        assert!(graph.has_input("in"));
        assert!(graph.has_output("out"));
    }
}

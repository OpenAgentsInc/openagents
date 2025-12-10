//! From Spec: Convert GraphSpec to Graph instances
//!
//! This module provides functions to deserialize a GraphSpec into a running Graph.

use std::collections::HashMap;
use std::fmt;

use crate::graph::Graph;
use crate::merge::Merge;
use crate::unit::Unit;

use super::types::{BundleSpec, GraphSpec, MergeSpec, UnitSpec};

/// Error during spec deserialization
#[derive(Debug, Clone)]
pub enum FromSpecError {
    /// Unknown unit type
    UnknownUnitType { type_id: String },
    /// Unit creation failed
    UnitCreationFailed { type_id: String, reason: String },
    /// Invalid merge specification
    InvalidMerge { merge_id: String, reason: String },
    /// Missing referenced unit
    MissingUnit { unit_id: String },
    /// Pin not found
    PinNotFound { unit_id: String, pin_name: String },
    /// Type mismatch when setting data
    TypeMismatch { expected: String, found: String },
}

impl fmt::Display for FromSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUnitType { type_id } => {
                write!(f, "Unknown unit type: {}", type_id)
            }
            Self::UnitCreationFailed { type_id, reason } => {
                write!(f, "Failed to create unit '{}': {}", type_id, reason)
            }
            Self::InvalidMerge { merge_id, reason } => {
                write!(f, "Invalid merge '{}': {}", merge_id, reason)
            }
            Self::MissingUnit { unit_id } => {
                write!(f, "Missing unit: {}", unit_id)
            }
            Self::PinNotFound { unit_id, pin_name } => {
                write!(f, "Pin '{}' not found on unit '{}'", pin_name, unit_id)
            }
            Self::TypeMismatch { expected, found } => {
                write!(f, "Type mismatch: expected {}, found {}", expected, found)
            }
        }
    }
}

impl std::error::Error for FromSpecError {}

/// Factory function type for creating units
pub type UnitFactory = Box<dyn Fn() -> Box<dyn Unit> + Send + Sync>;

/// Registry of unit types and their factories
pub struct UnitRegistry {
    factories: HashMap<String, UnitFactory>,
}

impl UnitRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a unit type with a factory function
    pub fn register(&mut self, type_id: impl Into<String>, factory: UnitFactory) {
        self.factories.insert(type_id.into(), factory);
    }

    /// Register a unit type using Default trait
    pub fn register_default<U>(&mut self, type_id: impl Into<String>)
    where
        U: Unit + Default + 'static,
    {
        let factory: UnitFactory = Box::new(|| Box::new(U::default()) as Box<dyn Unit>);
        self.factories.insert(type_id.into(), factory);
    }

    /// Create a unit by type ID
    pub fn create(&self, type_id: &str) -> Result<Box<dyn Unit>, FromSpecError> {
        self.factories
            .get(type_id)
            .map(|f| f())
            .ok_or_else(|| FromSpecError::UnknownUnitType {
                type_id: type_id.to_string(),
            })
    }

    /// Check if a type is registered
    pub fn has_type(&self, type_id: &str) -> bool {
        self.factories.contains_key(type_id)
    }

    /// Get all registered type IDs
    pub fn type_ids(&self) -> Vec<&str> {
        self.factories.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for UnitRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for UnitRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnitRegistry")
            .field("types", &self.factories.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Create a unit from a UnitSpec
pub fn unit_from_spec(
    spec: &UnitSpec,
    registry: &UnitRegistry,
) -> Result<Box<dyn Unit>, FromSpecError> {
    let mut unit = registry.create(&spec.id)?;

    // Apply input configurations
    // Note: This would need runtime introspection to set values
    // For now, we just create the unit without data

    Ok(unit)
}

/// Create a merge from a MergeSpec
pub fn merge_from_spec(merge_id: &str, _spec: &MergeSpec) -> Merge {
    Merge::new(merge_id)
    // Connection setup would happen after all units are created
}

/// Create a Graph from a GraphSpec
pub fn graph_from_spec(spec: &GraphSpec, registry: &UnitRegistry) -> Result<Graph, FromSpecError> {
    let mut graph = Graph::new(&spec.id);

    // First pass: create all units
    for (unit_id, unit_spec) in &spec.units {
        let unit = unit_from_spec(unit_spec, registry)?;
        graph.add_unit(unit_id.clone(), unit);
    }

    // Second pass: create all merges
    for (merge_id, merge_spec) in &spec.merges {
        let merge = merge_from_spec(merge_id, merge_spec);
        graph.add_merge(merge_id.clone(), merge);
    }

    // Third pass: setup connections
    // Note: This requires additional API on Graph for connecting units

    Ok(graph)
}

/// Create a Graph from a BundleSpec
///
/// First processes all dependencies, then creates the main graph.
pub fn graph_from_bundle(
    bundle: &BundleSpec,
    registry: &UnitRegistry,
) -> Result<(Graph, HashMap<String, Graph>), FromSpecError> {
    let mut dependencies = HashMap::new();

    // Create all dependency graphs first
    for (id, dep_spec) in &bundle.specs {
        let dep_graph = graph_from_spec(dep_spec, registry)?;
        dependencies.insert(id.clone(), dep_graph);
    }

    // Create the main graph
    let main_graph = graph_from_spec(&bundle.spec, registry)?;

    Ok((main_graph, dependencies))
}

/// Parse and create a Graph from JSON
pub fn graph_from_json(json: &str, registry: &UnitRegistry) -> Result<Graph, FromSpecError> {
    let spec = GraphSpec::from_json(json)
        .map_err(|e| FromSpecError::UnitCreationFailed {
            type_id: "json".to_string(),
            reason: e.to_string(),
        })?;
    graph_from_spec(&spec, registry)
}

/// Parse and create a Graph from bundle JSON
pub fn graph_from_bundle_json(
    json: &str,
    registry: &UnitRegistry,
) -> Result<(Graph, HashMap<String, Graph>), FromSpecError> {
    let bundle = BundleSpec::from_json(json)
        .map_err(|e| FromSpecError::UnitCreationFailed {
            type_id: "json".to_string(),
            reason: e.to_string(),
        })?;
    graph_from_bundle(&bundle, registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_registry_empty() {
        let registry = UnitRegistry::new();
        assert!(registry.type_ids().is_empty());
    }

    #[test]
    fn test_unit_registry_unknown_type() {
        let registry = UnitRegistry::new();
        let result = registry.create("unknown/Type");

        assert!(matches!(
            result,
            Err(FromSpecError::UnknownUnitType { type_id }) if type_id == "unknown/Type"
        ));
    }

    #[test]
    fn test_graph_from_empty_spec() {
        let registry = UnitRegistry::new();
        let spec = GraphSpec::new("empty");

        let graph = graph_from_spec(&spec, &registry).unwrap();
        assert_eq!(graph.id(), "empty");
        assert!(graph.unit_ids().is_empty());
    }

    #[test]
    fn test_graph_from_spec_unknown_unit() {
        let registry = UnitRegistry::new();
        let mut spec = GraphSpec::new("test");
        spec.add_unit("u1", super::super::types::UnitSpec::new("unknown/Type"));

        let result = graph_from_spec(&spec, &registry);
        assert!(matches!(
            result,
            Err(FromSpecError::UnknownUnitType { type_id }) if type_id == "unknown/Type"
        ));
    }

    #[test]
    fn test_from_spec_error_display() {
        let err = FromSpecError::UnknownUnitType {
            type_id: "test/Type".to_string(),
        };
        assert!(format!("{}", err).contains("test/Type"));

        let err = FromSpecError::PinNotFound {
            unit_id: "u1".to_string(),
            pin_name: "in".to_string(),
        };
        assert!(format!("{}", err).contains("u1"));
        assert!(format!("{}", err).contains("in"));
    }

    #[test]
    fn test_bundle_from_spec() {
        let registry = UnitRegistry::new();
        let mut bundle = super::super::types::BundleSpec::new(GraphSpec::new("main"));
        bundle.add_dependency("dep1", GraphSpec::new("dep1"));

        let (main, deps) = graph_from_bundle(&bundle, &registry).unwrap();

        assert_eq!(main.id(), "main");
        assert_eq!(deps.len(), 1);
        assert!(deps.contains_key("dep1"));
    }
}

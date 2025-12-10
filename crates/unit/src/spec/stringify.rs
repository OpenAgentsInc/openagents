//! Stringify: Convert Graph instances to GraphSpec for serialization
//!
//! This module provides functions to serialize a running Graph into a GraphSpec
//! that can be saved to JSON.

use std::collections::HashMap;

use crate::graph::Graph;
use crate::merge::Merge;
use crate::pin::Pin;
use crate::unit::Unit;

use super::types::{
    BundleSpec, GraphSpec, MergeSpec, PinSpec, UnitSpec,
};

/// Convert a Pin to a PinSpec
pub fn pin_to_spec<T: Clone + Send + Sync + 'static>(pin: &Pin<T>) -> PinSpec
where
    T: serde::Serialize + std::fmt::Debug,
{
    let data = pin.peak().map(|v| {
        serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
    });

    PinSpec {
        data,
        constant: pin.is_constant(),
        ignored: pin.is_ignored(),
        ref_pin: false,
        type_hint: Some(std::any::type_name::<T>().to_string()),
    }
}

/// Convert a unit to a UnitSpec
///
/// Note: This function requires the unit type ID to be provided externally
/// since the Unit trait doesn't include type information.
pub fn unit_to_spec(_unit: &dyn Unit, type_id: &str) -> UnitSpec {
    UnitSpec {
        id: type_id.to_string(),
        input: HashMap::new(),  // Would need introspection to fill
        output: HashMap::new(), // Would need introspection to fill
        metadata: None,
    }
}

/// Convert a Merge to a MergeSpec
pub fn merge_to_spec(_merge: &Merge) -> MergeSpec {
    // Note: The Merge structure doesn't currently expose connection details
    // This would need to be enhanced to track connection metadata
    MergeSpec {
        input_plugs: Vec::new(),
        output_plugs: Vec::new(),
    }
}

/// Convert a Graph to a GraphSpec
///
/// Includes all units, merges, and exposed pins.
pub fn graph_to_spec(graph: &Graph) -> GraphSpec {
    let mut spec = GraphSpec::new(graph.id());

    // Add all units
    for unit_id in graph.unit_ids() {
        if let Some(unit) = graph.get_unit(&unit_id) {
            // For now, use a placeholder type ID
            // A real implementation would use a type registry
            let type_id = format!("unit/{}", unit_id);
            let unit_spec = unit_to_spec(unit, &type_id);
            spec.add_unit(unit_id, unit_spec);
        }
    }

    // Add all merges
    for merge_id in graph.merge_ids() {
        if let Some(merge) = graph.get_merge(&merge_id) {
            let merge_spec = merge_to_spec(merge);
            spec.add_merge(merge_id, merge_spec);
        }
    }

    // Note: Exposed pins would need additional tracking in Graph

    spec
}

/// Convert a Graph and its dependencies to a BundleSpec
pub fn graph_to_bundle(graph: &Graph, dependencies: &HashMap<String, Graph>) -> BundleSpec {
    let main_spec = graph_to_spec(graph);
    let mut bundle = BundleSpec::new(main_spec);

    for (id, dep_graph) in dependencies {
        let dep_spec = graph_to_spec(dep_graph);
        bundle.add_dependency(id, dep_spec);
    }

    bundle
}

/// Serialize a graph to a JSON string
pub fn graph_to_json(graph: &Graph) -> Result<String, serde_json::Error> {
    let spec = graph_to_spec(graph);
    spec.to_json()
}

/// Serialize a graph and dependencies to a JSON string
pub fn bundle_to_json(
    graph: &Graph,
    dependencies: &HashMap<String, Graph>,
) -> Result<String, serde_json::Error> {
    let bundle = graph_to_bundle(graph, dependencies);
    bundle.to_json()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    #[test]
    fn test_graph_to_spec_empty() {
        let graph = Graph::new("empty");
        let spec = graph_to_spec(&graph);

        assert_eq!(spec.id, "empty");
        assert!(spec.units.is_empty());
        assert!(spec.merges.is_empty());
    }

    #[test]
    fn test_graph_to_json() {
        let graph = Graph::new("test-graph");
        let json = graph_to_json(&graph).unwrap();

        assert!(json.contains("test-graph"));
    }

    #[test]
    fn test_graph_to_bundle() {
        let main = Graph::new("main");
        let dep = Graph::new("dependency");

        let mut deps = HashMap::new();
        deps.insert("custom/Dep".to_string(), dep);

        let bundle = graph_to_bundle(&main, &deps);

        assert_eq!(bundle.spec.id, "main");
        assert!(bundle.specs.contains_key("custom/Dep"));
    }

    #[test]
    fn test_bundle_to_json() {
        let main = Graph::new("main");
        let deps = HashMap::new();

        let json = bundle_to_json(&main, &deps).unwrap();

        assert!(json.contains("main"));
        assert!(json.contains("spec"));
    }
}

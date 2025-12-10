//! Spec Types: Serialization types for Graph specifications
//!
//! These types define how Units and Graphs are serialized to JSON.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pin data specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PinSpec {
    /// Current data value (JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Whether pin is constant
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub constant: bool,
    /// Whether pin is ignored
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignored: bool,
    /// Whether pin is a reference
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[serde(rename = "ref")]
    pub ref_pin: bool,
    /// Type hint for the pin
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub type_hint: Option<String>,
}

impl PinSpec {
    /// Create a new empty pin spec
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with data value
    pub fn with_data(data: serde_json::Value) -> Self {
        Self {
            data: Some(data),
            ..Default::default()
        }
    }

    /// Create a constant pin
    pub fn constant(data: serde_json::Value) -> Self {
        Self {
            data: Some(data),
            constant: true,
            ..Default::default()
        }
    }
}

/// Unit specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitSpec {
    /// Unit type ID (references a class/spec)
    pub id: String,
    /// Input pin configurations
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub input: HashMap<String, PinSpec>,
    /// Output pin configurations
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub output: HashMap<String, PinSpec>,
    /// Unit metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl UnitSpec {
    /// Create a new unit spec
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            input: HashMap::new(),
            output: HashMap::new(),
            metadata: None,
        }
    }

    /// Add an input configuration
    pub fn with_input(mut self, name: impl Into<String>, spec: PinSpec) -> Self {
        self.input.insert(name.into(), spec);
        self
    }

    /// Add an output configuration
    pub fn with_output(mut self, name: impl Into<String>, spec: PinSpec) -> Self {
        self.output.insert(name.into(), spec);
        self
    }
}

/// Merge plug specification (connection point)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePlugSpec {
    /// Unit ID
    pub unit_id: String,
    /// Pin type ("input" or "output")
    pub pin_type: String,
    /// Pin name
    pub pin_name: String,
    /// Sub-pin ID (for multi-connection pins)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_pin_id: Option<String>,
}

impl MergePlugSpec {
    /// Create an input plug (from unit output to merge)
    pub fn input(unit_id: impl Into<String>, pin_name: impl Into<String>) -> Self {
        Self {
            unit_id: unit_id.into(),
            pin_type: "output".to_string(), // Output from unit goes to merge input
            pin_name: pin_name.into(),
            sub_pin_id: None,
        }
    }

    /// Create an output plug (from merge to unit input)
    pub fn output(unit_id: impl Into<String>, pin_name: impl Into<String>) -> Self {
        Self {
            unit_id: unit_id.into(),
            pin_type: "input".to_string(), // Goes to unit input
            pin_name: pin_name.into(),
            sub_pin_id: None,
        }
    }
}

/// Merge specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MergeSpec {
    /// Input plugs (connections from outputs to this merge)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_plugs: Vec<MergePlugSpec>,
    /// Output plugs (connections from this merge to inputs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_plugs: Vec<MergePlugSpec>,
}

impl MergeSpec {
    /// Create a new empty merge spec
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an input plug
    pub fn add_input(&mut self, plug: MergePlugSpec) {
        self.input_plugs.push(plug);
    }

    /// Add an output plug
    pub fn add_output(&mut self, plug: MergePlugSpec) {
        self.output_plugs.push(plug);
    }
}

/// Pin exposure specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureSpec {
    /// Unit ID containing the pin
    pub unit_id: String,
    /// Pin name on the unit
    pub pin_name: String,
}

impl ExposureSpec {
    /// Create a new exposure spec
    pub fn new(unit_id: impl Into<String>, pin_name: impl Into<String>) -> Self {
        Self {
            unit_id: unit_id.into(),
            pin_name: pin_name.into(),
        }
    }
}

/// Graph position for layout
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PositionSpec {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
}

impl PositionSpec {
    /// Create a new position
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Graph specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSpec {
    /// Graph ID/name
    pub id: String,
    /// Child units
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub units: HashMap<String, UnitSpec>,
    /// Merges (connections)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub merges: HashMap<String, MergeSpec>,
    /// Exposed inputs
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub exposed_inputs: HashMap<String, ExposureSpec>,
    /// Exposed outputs
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub exposed_outputs: HashMap<String, ExposureSpec>,
    /// Unit positions for layout
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub positions: HashMap<String, PositionSpec>,
    /// Graph metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl GraphSpec {
    /// Create a new empty graph spec
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            units: HashMap::new(),
            merges: HashMap::new(),
            exposed_inputs: HashMap::new(),
            exposed_outputs: HashMap::new(),
            positions: HashMap::new(),
            metadata: None,
        }
    }

    /// Add a unit to the graph
    pub fn add_unit(&mut self, id: impl Into<String>, spec: UnitSpec) {
        self.units.insert(id.into(), spec);
    }

    /// Add a merge to the graph
    pub fn add_merge(&mut self, id: impl Into<String>, spec: MergeSpec) {
        self.merges.insert(id.into(), spec);
    }

    /// Expose an input
    pub fn expose_input(&mut self, name: impl Into<String>, exposure: ExposureSpec) {
        self.exposed_inputs.insert(name.into(), exposure);
    }

    /// Expose an output
    pub fn expose_output(&mut self, name: impl Into<String>, exposure: ExposureSpec) {
        self.exposed_outputs.insert(name.into(), exposure);
    }

    /// Set a unit position
    pub fn set_position(&mut self, unit_id: impl Into<String>, x: f64, y: f64) {
        self.positions.insert(unit_id.into(), PositionSpec::new(x, y));
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Bundle specification - a graph with its dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleSpec {
    /// The main graph
    pub spec: GraphSpec,
    /// Dependent graph specs (custom unit definitions)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub specs: HashMap<String, GraphSpec>,
}

impl BundleSpec {
    /// Create a new bundle from a graph
    pub fn new(spec: GraphSpec) -> Self {
        Self {
            spec,
            specs: HashMap::new(),
        }
    }

    /// Add a dependency spec
    pub fn add_dependency(&mut self, id: impl Into<String>, spec: GraphSpec) {
        self.specs.insert(id.into(), spec);
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_spec_serialize() {
        let mut spec = GraphSpec::new("test-graph");

        spec.units.insert(
            "adder".to_string(),
            UnitSpec {
                id: "core/Add".to_string(),
                input: HashMap::new(),
                output: HashMap::new(),
                metadata: None,
            },
        );

        let json = spec.to_json().unwrap();
        assert!(json.contains("test-graph"));
        assert!(json.contains("adder"));
    }

    #[test]
    fn test_graph_spec_deserialize() {
        let json = r#"
        {
            "id": "my-graph",
            "units": {
                "u1": {
                    "id": "core/Identity",
                    "input": {},
                    "output": {}
                }
            },
            "merges": {},
            "exposed_inputs": {},
            "exposed_outputs": {}
        }
        "#;

        let spec = GraphSpec::from_json(json).unwrap();
        assert_eq!(spec.id, "my-graph");
        assert!(spec.units.contains_key("u1"));
    }

    #[test]
    fn test_pin_spec_builders() {
        let empty = PinSpec::new();
        assert!(empty.data.is_none());
        assert!(!empty.constant);

        let with_data = PinSpec::with_data(serde_json::json!(42));
        assert_eq!(with_data.data.unwrap(), serde_json::json!(42));
        assert!(!with_data.constant);

        let constant = PinSpec::constant(serde_json::json!("hello"));
        assert!(constant.constant);
        assert_eq!(constant.data.unwrap(), serde_json::json!("hello"));
    }

    #[test]
    fn test_unit_spec_builder() {
        let spec = UnitSpec::new("core/Add")
            .with_input("a", PinSpec::constant(serde_json::json!(1)))
            .with_input("b", PinSpec::constant(serde_json::json!(2)));

        assert_eq!(spec.id, "core/Add");
        assert!(spec.input.contains_key("a"));
        assert!(spec.input.contains_key("b"));
    }

    #[test]
    fn test_bundle_spec() {
        let main = GraphSpec::new("main");
        let dependency = GraphSpec::new("custom/MyUnit");

        let mut bundle = BundleSpec::new(main);
        bundle.add_dependency("custom/MyUnit", dependency);

        assert_eq!(bundle.spec.id, "main");
        assert!(bundle.specs.contains_key("custom/MyUnit"));
    }

    #[test]
    fn test_merge_spec() {
        let mut merge = MergeSpec::new();
        merge.add_input(MergePlugSpec::input("unit1", "out"));
        merge.add_output(MergePlugSpec::output("unit2", "in"));

        assert_eq!(merge.input_plugs.len(), 1);
        assert_eq!(merge.output_plugs.len(), 1);
        assert_eq!(merge.input_plugs[0].unit_id, "unit1");
        assert_eq!(merge.output_plugs[0].unit_id, "unit2");
    }

    #[test]
    fn test_graph_positions() {
        let mut spec = GraphSpec::new("positioned-graph");
        spec.set_position("node1", 100.0, 200.0);
        spec.set_position("node2", 300.0, 400.0);

        assert_eq!(spec.positions.get("node1").unwrap().x, 100.0);
        assert_eq!(spec.positions.get("node1").unwrap().y, 200.0);
        assert_eq!(spec.positions.get("node2").unwrap().x, 300.0);
        assert_eq!(spec.positions.get("node2").unwrap().y, 400.0);
    }
}

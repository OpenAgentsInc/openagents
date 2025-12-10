//! Spec: Serialization types for Graph specifications
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

/// Pin exposure specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureSpec {
    /// Unit ID containing the pin
    pub unit_id: String,
    /// Pin name on the unit
    pub pin_name: String,
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
            metadata: None,
        }
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
}

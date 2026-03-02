use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::assembly::{CadAssemblyJoint, CadPartDef, CadPartInstance};
use crate::{CadError, CadResult};

/// Versioned CAD document schema version for Wave 1.
pub const CAD_DOCUMENT_SCHEMA_VERSION: u32 = 1;

/// Canonical CAD units for serialized documents.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadUnits {
    #[serde(rename = "mm")]
    Millimeter,
}

/// Core CAD document schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadDocument {
    pub schema_version: u32,
    pub document_id: String,
    pub revision: u64,
    pub units: CadUnits,
    pub metadata: BTreeMap<String, String>,
    pub feature_ids: Vec<String>,
    #[serde(rename = "partDefs", skip_serializing_if = "Option::is_none")]
    pub part_defs: Option<BTreeMap<String, CadPartDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instances: Option<Vec<CadPartInstance>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joints: Option<Vec<CadAssemblyJoint>>,
    #[serde(rename = "groundInstanceId", skip_serializing_if = "Option::is_none")]
    pub ground_instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis_cache: Option<BTreeMap<String, String>>,
}

impl CadDocument {
    /// Create an empty CAD document with deterministic defaults.
    pub fn new_empty(document_id: impl Into<String>) -> Self {
        Self {
            schema_version: CAD_DOCUMENT_SCHEMA_VERSION,
            document_id: document_id.into(),
            revision: 0,
            units: CadUnits::Millimeter,
            metadata: BTreeMap::new(),
            feature_ids: Vec::new(),
            part_defs: None,
            instances: None,
            joints: None,
            ground_instance_id: None,
            analysis_cache: None,
        }
    }

    /// Serialize this CAD document to compact JSON.
    pub fn to_json(&self) -> CadResult<String> {
        serde_json::to_string(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize CadDocument json: {error}"),
        })
    }

    /// Serialize this CAD document to pretty JSON.
    pub fn to_pretty_json(&self) -> CadResult<String> {
        serde_json::to_string_pretty(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize CadDocument pretty json: {error}"),
        })
    }

    /// Parse a CAD document from JSON.
    pub fn from_json(payload: &str) -> CadResult<Self> {
        serde_json::from_str(payload).map_err(|error| CadError::Serialization {
            reason: format!("failed to parse CadDocument json: {error}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CAD_DOCUMENT_SCHEMA_VERSION, CadDocument, CadUnits};
    use crate::assembly::{CadAssemblyJoint, CadJointKind, CadPartDef, CadPartInstance};
    use crate::kernel_math::Vec3;
    use std::collections::BTreeMap;

    fn golden(path: &str) -> String {
        let root = env!("CARGO_MANIFEST_DIR");
        let full_path = format!("{root}/tests/goldens/{path}");
        let contents = std::fs::read_to_string(&full_path);
        assert!(
            contents.is_ok(),
            "golden fixture should exist and be readable: {full_path}"
        );
        contents.unwrap_or_default()
    }

    #[test]
    fn empty_document_matches_golden_fixture() {
        let document = CadDocument::new_empty("doc-empty");
        let actual = document.to_pretty_json();
        assert!(
            actual.is_ok(),
            "empty document serialization should succeed"
        );

        if let Ok(actual_json) = actual {
            let expected = golden("cad_document_empty.json");
            assert_eq!(actual_json.trim_end(), expected.trim_end());
        }
    }

    #[test]
    fn minimal_document_matches_golden_fixture() {
        let mut document = CadDocument::new_empty("doc-minimal");
        document.revision = 1;
        document
            .metadata
            .insert("material".to_string(), "6061-T6".to_string());
        document
            .metadata
            .insert("title".to_string(), "Mac Studio Rack".to_string());
        document.feature_ids.push("feature.base".to_string());

        let actual = document.to_pretty_json();
        assert!(
            actual.is_ok(),
            "minimal document serialization should succeed"
        );

        if let Ok(actual_json) = actual {
            let expected = golden("cad_document_minimal.json");
            assert_eq!(actual_json.trim_end(), expected.trim_end());
        }
    }

    #[test]
    fn round_trip_is_deterministic() {
        let mut document = CadDocument::new_empty("doc-roundtrip");
        document.revision = 7;
        document.metadata = BTreeMap::from([
            ("material".to_string(), "6061-T6".to_string()),
            ("objective".to_string(), "stiffness".to_string()),
        ]);
        document.feature_ids = vec![
            "feature.base".to_string(),
            "feature.mount_holes".to_string(),
        ];
        document.analysis_cache = Some(BTreeMap::from([(
            "weight_kg".to_string(),
            "2.71".to_string(),
        )]));

        let serialized = document.to_json();
        assert!(
            serialized.is_ok(),
            "round trip serialization should succeed"
        );

        if let Ok(payload) = serialized {
            let parsed = CadDocument::from_json(&payload);
            assert!(parsed.is_ok(), "round trip parse should succeed");
            if let Ok(parsed_document) = parsed {
                assert_eq!(parsed_document, document);
                assert_eq!(parsed_document.schema_version, CAD_DOCUMENT_SCHEMA_VERSION);
                assert_eq!(parsed_document.units, CadUnits::Millimeter);
            }
        }
    }

    #[test]
    fn assembly_fields_round_trip_is_deterministic() {
        let mut document = CadDocument::new_empty("doc-assembly");
        document.part_defs = Some(BTreeMap::from([
            (
                "base".to_string(),
                CadPartDef {
                    id: "base".to_string(),
                    name: Some("Base Plate".to_string()),
                    root: 1,
                    default_material: Some("aluminum".to_string()),
                },
            ),
            (
                "arm".to_string(),
                CadPartDef {
                    id: "arm".to_string(),
                    name: Some("Arm".to_string()),
                    root: 2,
                    default_material: None,
                },
            ),
        ]));
        document.instances = Some(vec![
            CadPartInstance {
                id: "base_inst".to_string(),
                part_def_id: "base".to_string(),
                name: Some("Base".to_string()),
                transform: None,
                material: None,
            },
            CadPartInstance {
                id: "arm_inst".to_string(),
                part_def_id: "arm".to_string(),
                name: Some("Arm".to_string()),
                transform: Some(crate::assembly::CadTransform3D {
                    translation: Vec3::new(0.0, 0.0, 10.0),
                    rotation: Vec3::new(0.0, 0.0, 0.0),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                }),
                material: Some("steel".to_string()),
            },
        ]);
        document.joints = Some(vec![CadAssemblyJoint {
            id: "joint.revolute.001".to_string(),
            name: Some("Base-Arm".to_string()),
            parent_instance_id: Some("base_inst".to_string()),
            child_instance_id: "arm_inst".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 10.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Revolute {
                axis: Vec3::new(0.0, 0.0, 1.0),
                limits: Some((-90.0, 90.0)),
            },
            state: 0.0,
        }]);
        document.ground_instance_id = Some("base_inst".to_string());

        let payload = document
            .to_json()
            .expect("assembly document serialization should succeed");
        let parsed = CadDocument::from_json(&payload).expect("assembly parse should succeed");
        assert_eq!(parsed, document);
        assert!(payload.contains("\"partDefs\""));
        assert!(payload.contains("\"partDefId\""));
        assert!(payload.contains("\"parentInstanceId\""));
        assert!(payload.contains("\"childInstanceId\""));
        assert!(payload.contains("\"groundInstanceId\""));
        assert!(payload.contains("\"defaultMaterial\""));
    }
}

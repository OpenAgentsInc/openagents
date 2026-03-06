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

/// Drawing viewport mode persisted in the document schema.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadDrawingViewMode {
    #[serde(rename = "3d")]
    #[default]
    ThreeD,
    #[serde(rename = "2d")]
    TwoD,
}

/// Drawing projection direction persisted in the document schema.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadDrawingViewDirection {
    #[serde(rename = "front")]
    #[default]
    Front,
    #[serde(rename = "back")]
    Back,
    #[serde(rename = "top")]
    Top,
    #[serde(rename = "bottom")]
    Bottom,
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
    #[serde(rename = "isometric")]
    Isometric,
}

/// 2D pan offset in drawing coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadDrawingPan {
    pub x: f64,
    pub y: f64,
}

impl Default for CadDrawingPan {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// Persisted definition for a drawing detail view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadDrawingDetailView {
    pub id: String,
    #[serde(rename = "centerX")]
    pub center_x: f64,
    #[serde(rename = "centerY")]
    pub center_y: f64,
    pub scale: f64,
    pub width: f64,
    pub height: f64,
    pub label: String,
}

/// Drawing persistence payload stored in the CAD document schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadDrawingState {
    #[serde(rename = "viewMode")]
    pub view_mode: CadDrawingViewMode,
    #[serde(rename = "viewDirection")]
    pub view_direction: CadDrawingViewDirection,
    #[serde(rename = "showHiddenLines")]
    pub show_hidden_lines: bool,
    #[serde(rename = "showDimensions")]
    pub show_dimensions: bool,
    pub zoom: f64,
    pub pan: CadDrawingPan,
    #[serde(rename = "detailViews")]
    pub detail_views: Vec<CadDrawingDetailView>,
    #[serde(rename = "nextDetailId")]
    pub next_detail_id: u64,
}

impl Default for CadDrawingState {
    fn default() -> Self {
        Self {
            view_mode: CadDrawingViewMode::ThreeD,
            view_direction: CadDrawingViewDirection::Front,
            show_hidden_lines: true,
            show_dimensions: true,
            zoom: 1.0,
            pan: CadDrawingPan::default(),
            detail_views: Vec::new(),
            next_detail_id: 1,
        }
    }
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
    pub drawing: Option<CadDrawingState>,
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
            drawing: None,
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
    use super::{
        CAD_DOCUMENT_SCHEMA_VERSION, CadDocument, CadDrawingDetailView, CadDrawingPan,
        CadDrawingState, CadDrawingViewDirection, CadDrawingViewMode, CadUnits,
    };
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

    #[test]
    fn drawing_state_defaults_match_vcad_contract() {
        let drawing = CadDrawingState::default();
        assert_eq!(drawing.view_mode, CadDrawingViewMode::ThreeD);
        assert_eq!(drawing.view_direction, CadDrawingViewDirection::Front);
        assert!(drawing.show_hidden_lines);
        assert!(drawing.show_dimensions);
        assert_eq!(drawing.zoom, 1.0);
        assert_eq!(drawing.pan, CadDrawingPan { x: 0.0, y: 0.0 });
        assert!(drawing.detail_views.is_empty());
        assert_eq!(drawing.next_detail_id, 1);
    }

    #[test]
    fn drawing_fields_round_trip_is_deterministic() {
        let mut document = CadDocument::new_empty("doc-drawing");
        document.drawing = Some(CadDrawingState {
            view_mode: CadDrawingViewMode::TwoD,
            view_direction: CadDrawingViewDirection::Isometric,
            show_hidden_lines: false,
            show_dimensions: false,
            zoom: 2.25,
            pan: CadDrawingPan { x: 14.0, y: -8.0 },
            detail_views: vec![CadDrawingDetailView {
                id: "detail-1".to_string(),
                center_x: 25.0,
                center_y: 10.0,
                scale: 2.0,
                width: 40.0,
                height: 30.0,
                label: "A".to_string(),
            }],
            next_detail_id: 2,
        });

        let payload = document
            .to_json()
            .expect("drawing document serialization should succeed");
        let parsed = CadDocument::from_json(&payload).expect("drawing parse should succeed");
        assert_eq!(parsed, document);
        assert!(payload.contains("\"drawing\""));
        assert!(payload.contains("\"viewMode\""));
        assert!(payload.contains("\"viewDirection\""));
        assert!(payload.contains("\"showHiddenLines\""));
        assert!(payload.contains("\"showDimensions\""));
        assert!(payload.contains("\"detailViews\""));
        assert!(payload.contains("\"nextDetailId\""));
        assert!(payload.contains("\"centerX\""));
        assert!(payload.contains("\"centerY\""));
    }
}

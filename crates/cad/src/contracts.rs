use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Severity levels for CAD warnings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadWarningSeverity {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "critical")]
    Critical,
}

/// Stable warning codes for CAD validity and analysis workflows.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum CadWarningCode {
    #[serde(rename = "CAD-WARN-NON-MANIFOLD")]
    NonManifoldBody,
    #[serde(rename = "CAD-WARN-SELF-INTERSECTION")]
    SelfIntersection,
    #[serde(rename = "CAD-WARN-ZERO-THICKNESS")]
    ZeroThicknessFace,
    #[serde(rename = "CAD-WARN-SLIVER-FACE")]
    SliverFace,
    #[serde(rename = "CAD-WARN-FILLET-FAILED")]
    FilletFailed,
    #[serde(rename = "CAD-WARN-CHAMFER-FAILED")]
    ChamferFailed,
    #[serde(rename = "CAD-WARN-SHELL-FAILED")]
    ShellFailed,
    #[serde(rename = "CAD-WARN-SEMANTIC-REF-EXPIRED")]
    SemanticRefExpired,
    Unknown(String),
}

impl CadWarningCode {
    pub fn stable_code(&self) -> &str {
        match self {
            Self::NonManifoldBody => "CAD-WARN-NON-MANIFOLD",
            Self::SelfIntersection => "CAD-WARN-SELF-INTERSECTION",
            Self::ZeroThicknessFace => "CAD-WARN-ZERO-THICKNESS",
            Self::SliverFace => "CAD-WARN-SLIVER-FACE",
            Self::FilletFailed => "CAD-WARN-FILLET-FAILED",
            Self::ChamferFailed => "CAD-WARN-CHAMFER-FAILED",
            Self::ShellFailed => "CAD-WARN-SHELL-FAILED",
            Self::SemanticRefExpired => "CAD-WARN-SEMANTIC-REF-EXPIRED",
            Self::Unknown(code) => code.as_str(),
        }
    }
}

/// Structured CAD warning receipt consumed by pane/UI/activity feed layers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadWarning {
    pub code: CadWarningCode,
    pub severity: CadWarningSeverity,
    pub message: String,
    pub remediation_hint: String,
    pub semantic_refs: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

/// Selection kind for CAD entities.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadSelectionKind {
    Body,
    Face,
    Edge,
}

/// Stable selection item for selection set persistence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadSelection {
    pub selection_id: String,
    pub entity_id: String,
    pub semantic_ref: Option<String>,
    pub kind: CadSelectionKind,
}

/// Current CAD selection state for pane and command handlers.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadSelectionState {
    pub primary: Option<CadSelection>,
    pub selected: Vec<CadSelection>,
    pub selection_revision: u64,
}

/// Analysis snapshot for current CAD document/variant state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CadAnalysis {
    pub document_revision: u64,
    pub variant_id: String,
    pub material_id: Option<String>,
    pub volume_mm3: Option<f64>,
    pub mass_kg: Option<f64>,
    pub center_of_gravity_mm: Option<[f64; 3]>,
    pub estimated_cost_usd: Option<f64>,
    pub max_deflection_mm: Option<f64>,
    pub estimator_metadata: BTreeMap<String, String>,
    pub objective_scores: BTreeMap<String, f64>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        CadAnalysis, CadSelection, CadSelectionKind, CadSelectionState, CadWarning, CadWarningCode,
        CadWarningSeverity,
    };

    #[test]
    fn cad_warning_schema_is_stable() {
        let warning = CadWarning {
            code: CadWarningCode::SliverFace,
            severity: CadWarningSeverity::Warning,
            message: "tiny sliver face detected".to_string(),
            remediation_hint: "Increase adjacent edge length or remove narrow fillet.".to_string(),
            semantic_refs: vec!["vent_face_set".to_string()],
            metadata: BTreeMap::from([("face_count".to_string(), "1".to_string())]),
        };
        let encoded = serde_json::to_string(&warning);
        assert!(encoded.is_ok(), "warning serialization should succeed");
        let payload = encoded.unwrap_or_default();
        assert!(payload.contains("CAD-WARN-SLIVER-FACE"));
        assert!(payload.contains("vent_face_set"));
    }

    #[test]
    fn cad_selection_schema_is_stable() {
        let selection = CadSelectionState {
            primary: Some(CadSelection {
                selection_id: "sel.001".to_string(),
                entity_id: "face.12".to_string(),
                semantic_ref: Some("rack_outer_face".to_string()),
                kind: CadSelectionKind::Face,
            }),
            selected: vec![CadSelection {
                selection_id: "sel.001".to_string(),
                entity_id: "face.12".to_string(),
                semantic_ref: Some("rack_outer_face".to_string()),
                kind: CadSelectionKind::Face,
            }],
            selection_revision: 7,
        };

        let encoded = serde_json::to_string(&selection);
        assert!(encoded.is_ok(), "selection serialization should succeed");
        let payload = encoded.unwrap_or_default();
        assert!(payload.contains("rack_outer_face"));
        assert!(payload.contains("selection_revision"));
    }

    #[test]
    fn cad_analysis_schema_is_stable() {
        let analysis = CadAnalysis {
            document_revision: 3,
            variant_id: "variant.lightweight".to_string(),
            material_id: Some("al-6061-t6".to_string()),
            volume_mm3: Some(1_500_000.0),
            mass_kg: Some(2.7),
            center_of_gravity_mm: Some([40.0, 18.0, 72.0]),
            estimated_cost_usd: Some(128.0),
            max_deflection_mm: Some(0.6),
            estimator_metadata: BTreeMap::from([(
                "cost.model_id".to_string(),
                "cad.cost.wave1.v1".to_string(),
            )]),
            objective_scores: BTreeMap::from([
                ("weight".to_string(), 0.92),
                ("cost".to_string(), 0.87),
            ]),
        };
        let encoded = serde_json::to_string(&analysis);
        assert!(encoded.is_ok(), "analysis serialization should succeed");
        let payload = encoded.unwrap_or_default();
        assert!(payload.contains("variant.lightweight"));
        assert!(payload.contains("objective_scores"));
    }
}

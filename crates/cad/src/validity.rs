use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::contracts::{CadWarning, CadWarningCode, CadWarningSeverity};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelValidityEntity {
    pub entity_id: String,
    pub feature_id: String,
    pub semantic_ref: Option<String>,
    pub is_manifold: bool,
    pub self_intersection_count: u32,
    pub min_thickness_mm: f64,
    pub min_face_area_mm2: f64,
    pub sliver_face_count: u32,
    pub fillet_failure_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelValiditySnapshot {
    pub document_revision: u64,
    pub variant_id: String,
    pub tolerance_mm: f64,
    pub entities: Vec<ModelValidityEntity>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadWarningReceipt {
    pub document_revision: u64,
    pub variant_id: String,
    pub warnings: Vec<CadWarning>,
    pub severity_counts: BTreeMap<String, u32>,
}

pub fn run_model_validity_checks(snapshot: &ModelValiditySnapshot) -> CadWarningReceipt {
    let mut warnings = Vec::<CadWarning>::new();
    for entity in &snapshot.entities {
        if !entity.is_manifold {
            warnings.push(build_warning(
                CadWarningCode::NonManifoldBody,
                CadWarningSeverity::Critical,
                "non-manifold body detected",
                "Repair shell continuity or remove invalid boolean inputs.",
                entity,
                BTreeMap::new(),
            ));
        }

        if entity.self_intersection_count > 0 {
            warnings.push(build_warning(
                CadWarningCode::SelfIntersection,
                CadWarningSeverity::Critical,
                "self-intersection detected",
                "Reduce overlap depth or adjust source profile constraints.",
                entity,
                BTreeMap::from([(
                    "self_intersection_count".to_string(),
                    entity.self_intersection_count.to_string(),
                )]),
            ));
        }

        if entity.min_thickness_mm <= snapshot.tolerance_mm {
            warnings.push(build_warning(
                CadWarningCode::ZeroThicknessFace,
                CadWarningSeverity::Critical,
                "zero-thickness or near-zero-thickness face detected",
                "Increase wall thickness beyond modeling tolerance.",
                entity,
                BTreeMap::from([
                    (
                        "min_thickness_mm".to_string(),
                        format!("{:.6}", entity.min_thickness_mm),
                    ),
                    (
                        "tolerance_mm".to_string(),
                        format!("{:.6}", snapshot.tolerance_mm),
                    ),
                ]),
            ));
        }

        if entity.sliver_face_count > 0 || entity.min_face_area_mm2 <= snapshot.tolerance_mm.powi(2) {
            warnings.push(build_warning(
                CadWarningCode::SliverFace,
                CadWarningSeverity::Warning,
                "sliver faces detected",
                "Increase adjacent edge length or reduce aggressive fillet/chamfer values.",
                entity,
                BTreeMap::from([
                    (
                        "sliver_face_count".to_string(),
                        entity.sliver_face_count.to_string(),
                    ),
                    (
                        "min_face_area_mm2".to_string(),
                        format!("{:.6}", entity.min_face_area_mm2),
                    ),
                ]),
            ));
        }

        if let Some(reason) = &entity.fillet_failure_reason {
            warnings.push(build_warning(
                CadWarningCode::FilletFailed,
                CadWarningSeverity::Warning,
                "fillet/chamfer operation failed",
                "Reduce fillet radius or split operation into smaller edge sets.",
                entity,
                BTreeMap::from([("reason".to_string(), reason.clone())]),
            ));
        }
    }

    warnings.sort_by(|lhs, rhs| {
        lhs.code
            .stable_code()
            .cmp(rhs.code.stable_code())
            .then_with(|| lhs.semantic_refs.cmp(&rhs.semantic_refs))
            .then_with(|| lhs.message.cmp(&rhs.message))
    });

    let mut severity_counts = BTreeMap::<String, u32>::new();
    for warning in &warnings {
        let key = match warning.severity {
            CadWarningSeverity::Info => "info",
            CadWarningSeverity::Warning => "warning",
            CadWarningSeverity::Critical => "critical",
        };
        *severity_counts.entry(key.to_string()).or_insert(0) += 1;
    }

    CadWarningReceipt {
        document_revision: snapshot.document_revision,
        variant_id: snapshot.variant_id.clone(),
        warnings,
        severity_counts,
    }
}

fn build_warning(
    code: CadWarningCode,
    severity: CadWarningSeverity,
    message: &str,
    remediation_hint: &str,
    entity: &ModelValidityEntity,
    mut metadata: BTreeMap<String, String>,
) -> CadWarning {
    metadata.insert("entity_id".to_string(), entity.entity_id.clone());
    metadata.insert("feature_id".to_string(), entity.feature_id.clone());
    metadata.insert(
        "deep_link".to_string(),
        format!("cad://feature/{}/entity/{}", entity.feature_id, entity.entity_id),
    );

    let semantic_refs = entity
        .semantic_ref
        .as_ref()
        .map(|value| vec![value.clone()])
        .unwrap_or_default();

    CadWarning {
        code,
        severity,
        message: message.to_string(),
        remediation_hint: remediation_hint.to_string(),
        semantic_refs,
        metadata,
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelValiditySnapshot, run_model_validity_checks};

    fn load_fixture(name: &str) -> ModelValiditySnapshot {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{root}/tests/goldens/{name}");
        let payload = std::fs::read_to_string(path).expect("fixture should be readable");
        serde_json::from_str(&payload).expect("fixture should parse")
    }

    #[test]
    fn warning_suite_returns_deterministic_order_for_fixture() {
        let snapshot = load_fixture("model_validity_fixture_all_warning_classes.json");
        let first = run_model_validity_checks(&snapshot);
        let second = run_model_validity_checks(&snapshot);
        assert_eq!(first, second);
        let codes = first
            .warnings
            .iter()
            .map(|warning| warning.code.stable_code().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            vec![
                "CAD-WARN-FILLET-FAILED".to_string(),
                "CAD-WARN-NON-MANIFOLD".to_string(),
                "CAD-WARN-SELF-INTERSECTION".to_string(),
                "CAD-WARN-SLIVER-FACE".to_string(),
                "CAD-WARN-ZERO-THICKNESS".to_string(),
            ]
        );
    }

    #[test]
    fn warning_fixture_covers_all_required_classes_with_deep_links() {
        let snapshot = load_fixture("model_validity_fixture_all_warning_classes.json");
        let receipt = run_model_validity_checks(&snapshot);
        assert_eq!(receipt.warnings.len(), 5);
        assert_eq!(receipt.severity_counts.get("critical"), Some(&3));
        assert_eq!(receipt.severity_counts.get("warning"), Some(&2));
        for warning in &receipt.warnings {
            assert!(
                warning.metadata.contains_key("deep_link"),
                "warning {} should include deep link metadata",
                warning.code.stable_code()
            );
            assert!(
                !warning.remediation_hint.trim().is_empty(),
                "warning {} should include remediation hint",
                warning.code.stable_code()
            );
        }
    }

    #[test]
    fn insertion_order_of_entities_does_not_change_warning_order() {
        let mut snapshot = load_fixture("model_validity_fixture_all_warning_classes.json");
        let mut reversed = snapshot.clone();
        reversed.entities.reverse();
        let forward = run_model_validity_checks(&snapshot);
        let backward = run_model_validity_checks(&reversed);
        assert_eq!(forward.warnings, backward.warnings);
        snapshot.entities.rotate_left(1);
        let rotated = run_model_validity_checks(&snapshot);
        assert_eq!(forward.warnings, rotated.warnings);
    }
}

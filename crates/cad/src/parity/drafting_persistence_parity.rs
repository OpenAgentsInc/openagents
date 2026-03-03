use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::document::{
    CadDocument, CadDrawingDetailView, CadDrawingPan, CadDrawingState, CadDrawingViewDirection,
    CadDrawingViewMode,
};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_PERSISTENCE_ISSUE_ID: &str = "VCAD-PARITY-075";
pub const DRAFTING_PERSISTENCE_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_persistence_vcad_reference.json";
const DRAFTING_PERSISTENCE_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_persistence_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingPersistenceParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub document_field_names: Vec<String>,
    pub drawing_field_names: Vec<String>,
    pub drawing_pan_field_names: Vec<String>,
    pub drawing_detail_field_names: Vec<String>,
    pub default_view_mode: String,
    pub default_view_direction: String,
    pub default_show_hidden_lines: bool,
    pub default_show_dimensions: bool,
    pub default_zoom: f64,
    pub default_pan_x: f64,
    pub default_pan_y: f64,
    pub default_detail_count: usize,
    pub default_next_detail_id: u64,
    pub sample_detail_count: usize,
    pub schema_field_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingPersistenceReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    scalar_tolerance: f64,
    document_field_names: Vec<String>,
    drawing_field_names: Vec<String>,
    drawing_pan_field_names: Vec<String>,
    drawing_detail_field_names: Vec<String>,
    default_view_mode: String,
    default_view_direction: String,
    default_show_hidden_lines: bool,
    default_show_dimensions: bool,
    default_zoom: f64,
    default_pan_x: f64,
    default_pan_y: f64,
    default_detail_count: usize,
    default_next_detail_id: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct DraftingPersistenceSnapshot {
    document_field_names: Vec<String>,
    drawing_field_names: Vec<String>,
    drawing_pan_field_names: Vec<String>,
    drawing_detail_field_names: Vec<String>,
    default_view_mode: String,
    default_view_direction: String,
    default_show_hidden_lines: bool,
    default_show_dimensions: bool,
    default_zoom: f64,
    default_pan_x: f64,
    default_pan_y: f64,
    default_detail_count: usize,
    default_next_detail_id: u64,
    sample_detail_count: usize,
}

pub fn build_drafting_persistence_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingPersistenceParityManifest> {
    let corpus: DraftingPersistenceReferenceCorpus =
        serde_json::from_str(DRAFTING_PERSISTENCE_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse drafting persistence reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(DRAFTING_PERSISTENCE_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_drafting_persistence_snapshot()?;
    let replay_snapshot = collect_drafting_persistence_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let schema_field_match = snapshot.document_field_names == sorted(corpus.document_field_names)
        && snapshot.drawing_field_names == sorted(corpus.drawing_field_names)
        && snapshot.drawing_pan_field_names == sorted(corpus.drawing_pan_field_names)
        && snapshot.drawing_detail_field_names == sorted(corpus.drawing_detail_field_names)
        && snapshot.default_view_mode == corpus.default_view_mode
        && snapshot.default_view_direction == corpus.default_view_direction
        && snapshot.default_show_hidden_lines == corpus.default_show_hidden_lines
        && snapshot.default_show_dimensions == corpus.default_show_dimensions
        && approx_eq(
            snapshot.default_zoom,
            corpus.default_zoom,
            corpus.scalar_tolerance,
        )
        && approx_eq(
            snapshot.default_pan_x,
            corpus.default_pan_x,
            corpus.scalar_tolerance,
        )
        && approx_eq(
            snapshot.default_pan_y,
            corpus.default_pan_y,
            corpus.scalar_tolerance,
        )
        && snapshot.default_detail_count == corpus.default_detail_count
        && snapshot.default_next_detail_id == corpus.default_next_detail_id;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        schema_field_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingPersistenceParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_PERSISTENCE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_PERSISTENCE_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        document_field_names: snapshot.document_field_names,
        drawing_field_names: snapshot.drawing_field_names,
        drawing_pan_field_names: snapshot.drawing_pan_field_names,
        drawing_detail_field_names: snapshot.drawing_detail_field_names,
        default_view_mode: snapshot.default_view_mode,
        default_view_direction: snapshot.default_view_direction,
        default_show_hidden_lines: snapshot.default_show_hidden_lines,
        default_show_dimensions: snapshot.default_show_dimensions,
        default_zoom: snapshot.default_zoom,
        default_pan_x: snapshot.default_pan_x,
        default_pan_y: snapshot.default_pan_y,
        default_detail_count: snapshot.default_detail_count,
        default_next_detail_id: snapshot.default_next_detail_id,
        sample_detail_count: snapshot.sample_detail_count,
        schema_field_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "cad document schema includes optional drawing payload".to_string(),
            "drawing payload persists vcad-compatible camelCase keys and enum values".to_string(),
            "drawing defaults persist as 3d/front with hidden-lines+dimensions enabled".to_string(),
            "drawing persistence fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_drafting_persistence_snapshot() -> CadResult<DraftingPersistenceSnapshot> {
    let mut document = CadDocument::new_empty("doc.drafting.persistence.parity");
    document.drawing = Some(sample_drawing_state());

    let document_value =
        serde_json::to_value(&document).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize drawing sample document to json value: {error}"),
        })?;
    let drawing_value =
        serde_json::to_value(
            document
                .drawing
                .as_ref()
                .ok_or_else(|| CadError::ParseFailed {
                    reason: "drawing sample state missing".to_string(),
                })?,
        )
        .map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize drawing sample state to json value: {error}"),
        })?;
    let drawing_pan_value =
        serde_json::to_value(CadDrawingPan { x: 0.0, y: 0.0 }).map_err(|error| {
            CadError::Serialization {
                reason: format!("failed to serialize drawing pan sample to json value: {error}"),
            }
        })?;
    let drawing_detail_value = serde_json::to_value(CadDrawingDetailView {
        id: "detail-1".to_string(),
        center_x: 0.0,
        center_y: 0.0,
        scale: 2.0,
        width: 40.0,
        height: 40.0,
        label: "A".to_string(),
    })
    .map_err(|error| CadError::Serialization {
        reason: format!("failed to serialize drawing detail sample to json value: {error}"),
    })?;

    let drawing_default = CadDrawingState::default();

    Ok(DraftingPersistenceSnapshot {
        document_field_names: filtered_document_fields(&document_value)?,
        drawing_field_names: object_field_names(&drawing_value)?,
        drawing_pan_field_names: object_field_names(&drawing_pan_value)?,
        drawing_detail_field_names: object_field_names(&drawing_detail_value)?,
        default_view_mode: enum_value_as_string(drawing_default.view_mode)?,
        default_view_direction: enum_value_as_string(drawing_default.view_direction)?,
        default_show_hidden_lines: drawing_default.show_hidden_lines,
        default_show_dimensions: drawing_default.show_dimensions,
        default_zoom: drawing_default.zoom,
        default_pan_x: drawing_default.pan.x,
        default_pan_y: drawing_default.pan.y,
        default_detail_count: drawing_default.detail_views.len(),
        default_next_detail_id: drawing_default.next_detail_id,
        sample_detail_count: document
            .drawing
            .as_ref()
            .map(|drawing| drawing.detail_views.len())
            .unwrap_or(0),
    })
}

fn sample_drawing_state() -> CadDrawingState {
    CadDrawingState {
        view_mode: CadDrawingViewMode::TwoD,
        view_direction: CadDrawingViewDirection::Top,
        show_hidden_lines: false,
        show_dimensions: true,
        zoom: 1.75,
        pan: CadDrawingPan { x: 12.0, y: -6.0 },
        detail_views: vec![CadDrawingDetailView {
            id: "detail-1".to_string(),
            center_x: 24.0,
            center_y: 12.0,
            scale: 2.0,
            width: 48.0,
            height: 32.0,
            label: "A".to_string(),
        }],
        next_detail_id: 2,
    }
}

fn filtered_document_fields(document_value: &serde_json::Value) -> CadResult<Vec<String>> {
    let object = document_value
        .as_object()
        .ok_or_else(|| CadError::ParseFailed {
            reason: "drawing sample document should serialize as object".to_string(),
        })?;
    let mut fields = Vec::new();
    if object.contains_key("drawing") {
        fields.push("drawing".to_string());
    }
    Ok(sorted(fields))
}

fn object_field_names(value: &serde_json::Value) -> CadResult<Vec<String>> {
    let object = value.as_object().ok_or_else(|| CadError::ParseFailed {
        reason: "drawing sample payload should serialize as object".to_string(),
    })?;
    Ok(sorted(object.keys().cloned().collect()))
}

fn enum_value_as_string<T: Serialize>(value: T) -> CadResult<String> {
    serde_json::to_value(value)
        .map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize enum tag: {error}"),
        })?
        .as_str()
        .map(|tag| tag.to_string())
        .ok_or_else(|| CadError::ParseFailed {
            reason: "serialized enum tag should be string".to_string(),
        })
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn approx_eq(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn parity_signature(
    snapshot: &DraftingPersistenceSnapshot,
    reference_commit_match: bool,
    schema_field_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            snapshot,
            reference_commit_match,
            schema_field_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting persistence parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{DraftingPersistenceSnapshot, parity_signature};

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = DraftingPersistenceSnapshot {
            document_field_names: vec!["drawing".to_string()],
            drawing_field_names: vec!["viewMode".to_string()],
            drawing_pan_field_names: vec!["x".to_string(), "y".to_string()],
            drawing_detail_field_names: vec!["id".to_string()],
            default_view_mode: "3d".to_string(),
            default_view_direction: "front".to_string(),
            default_show_hidden_lines: true,
            default_show_dimensions: true,
            default_zoom: 1.0,
            default_pan_x: 0.0,
            default_pan_y: 0.0,
            default_detail_count: 0,
            default_next_detail_id: 1,
            sample_detail_count: 1,
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}

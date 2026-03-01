use super::{CadSketchModel, CadSketchSolveReport};
use crate::{CadError, CadResult};

/// Serialize a sketch model to compact deterministic JSON.
pub fn sketch_model_to_json(model: &CadSketchModel) -> CadResult<String> {
    serde_json::to_string(model).map_err(|error| CadError::Serialization {
        reason: format!("failed to serialize CadSketchModel json: {error}"),
    })
}

/// Deserialize a sketch model from JSON payload.
pub fn sketch_model_from_json(payload: &str) -> CadResult<CadSketchModel> {
    serde_json::from_str(payload).map_err(|error| CadError::Serialization {
        reason: format!("failed to parse CadSketchModel json: {error}"),
    })
}

/// Serialize a sketch solve report to compact deterministic JSON.
pub fn sketch_solve_report_to_json(report: &CadSketchSolveReport) -> CadResult<String> {
    serde_json::to_string(report).map_err(|error| CadError::Serialization {
        reason: format!("failed to serialize CadSketchSolveReport json: {error}"),
    })
}

mod common;
mod constraints;
mod model;
mod serde;
mod solver;

#[cfg(test)]
mod tests;

pub use constraints::{CadDimensionConstraintKind, CadSketchConstraint};
pub use model::{CadSketchEntity, CadSketchModel, CadSketchPlane, CadSketchPlanePreset};
pub use serde::{sketch_model_from_json, sketch_model_to_json, sketch_solve_report_to_json};
pub use solver::{
    CadSketchLmConfig, CadSketchLmPipelineSummary, CadSketchSolveDiagnostic, CadSketchSolveReport,
    CadSketchSolveSeverity,
};

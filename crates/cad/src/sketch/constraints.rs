use serde::{Deserialize, Serialize};

use super::common::{validate_stable_id, validate_tolerance_opt};
use crate::{CadError, CadResult};

/// Supported sketch dimension constraint kind.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadDimensionConstraintKind {
    Length,
    Radius,
}

/// Deterministic Wave 2 sketch constraint schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadSketchConstraint {
    Coincident {
        id: String,
        first_anchor_id: String,
        second_anchor_id: String,
        tolerance_mm: Option<f64>,
    },
    Horizontal {
        id: String,
        line_entity_id: String,
    },
    Vertical {
        id: String,
        line_entity_id: String,
    },
    Tangent {
        id: String,
        line_entity_id: String,
        arc_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    Dimension {
        id: String,
        entity_id: String,
        dimension_kind: CadDimensionConstraintKind,
        target_mm: f64,
        tolerance_mm: Option<f64>,
    },
}

impl CadSketchConstraint {
    pub fn id(&self) -> &str {
        match self {
            Self::Coincident { id, .. } => id,
            Self::Horizontal { id, .. } => id,
            Self::Vertical { id, .. } => id,
            Self::Tangent { id, .. } => id,
            Self::Dimension { id, .. } => id,
        }
    }

    pub fn validate(&self) -> CadResult<()> {
        validate_stable_id(self.id(), "constraint id")?;
        match self {
            Self::Coincident {
                first_anchor_id,
                second_anchor_id,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(first_anchor_id, "constraint first_anchor_id")?;
                validate_stable_id(second_anchor_id, "constraint second_anchor_id")?;
                if first_anchor_id == second_anchor_id {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "coincident constraint {} must reference two distinct anchors",
                            self.id()
                        ),
                    });
                }
                validate_tolerance_opt(*tolerance_mm, "coincident tolerance_mm")?;
            }
            Self::Horizontal { line_entity_id, .. } => {
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
            }
            Self::Vertical { line_entity_id, .. } => {
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
            }
            Self::Tangent {
                line_entity_id,
                arc_entity_id,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
                validate_stable_id(arc_entity_id, "constraint arc_entity_id")?;
                validate_tolerance_opt(*tolerance_mm, "tangent tolerance_mm")?;
            }
            Self::Dimension {
                entity_id,
                target_mm,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(entity_id, "constraint entity_id")?;
                if !target_mm.is_finite() || *target_mm <= 0.0 {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "dimension constraint {} target_mm must be finite and > 0",
                            self.id()
                        ),
                    });
                }
                validate_tolerance_opt(*tolerance_mm, "dimension tolerance_mm")?;
            }
        }
        Ok(())
    }
}

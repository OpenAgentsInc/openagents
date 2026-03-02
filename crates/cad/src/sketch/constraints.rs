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

/// Deterministic sketch constraint schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadSketchConstraint {
    Coincident {
        id: String,
        first_anchor_id: String,
        second_anchor_id: String,
        tolerance_mm: Option<f64>,
    },
    PointOnLine {
        id: String,
        point_anchor_id: String,
        line_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    Parallel {
        id: String,
        first_line_entity_id: String,
        second_line_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    Perpendicular {
        id: String,
        first_line_entity_id: String,
        second_line_entity_id: String,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at_anchor_id: Option<String>,
        tolerance_mm: Option<f64>,
    },
    EqualLength {
        id: String,
        first_line_entity_id: String,
        second_line_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    EqualRadius {
        id: String,
        first_curve_entity_id: String,
        second_curve_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    Concentric {
        id: String,
        first_curve_entity_id: String,
        second_curve_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    Fixed {
        id: String,
        point_anchor_id: String,
        target_mm: [f64; 2],
        tolerance_mm: Option<f64>,
    },
    PointOnCircle {
        id: String,
        point_anchor_id: String,
        circle_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    LineThroughCenter {
        id: String,
        line_entity_id: String,
        circle_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    Midpoint {
        id: String,
        midpoint_anchor_id: String,
        line_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    Symmetric {
        id: String,
        first_anchor_id: String,
        second_anchor_id: String,
        axis_line_entity_id: String,
        tolerance_mm: Option<f64>,
    },
    Distance {
        id: String,
        first_anchor_id: String,
        second_anchor_id: String,
        target_mm: f64,
        tolerance_mm: Option<f64>,
    },
    PointLineDistance {
        id: String,
        point_anchor_id: String,
        line_entity_id: String,
        target_mm: f64,
        tolerance_mm: Option<f64>,
    },
    Angle {
        id: String,
        first_line_entity_id: String,
        second_line_entity_id: String,
        target_deg: f64,
        tolerance_deg: Option<f64>,
    },
    Radius {
        id: String,
        curve_entity_id: String,
        target_mm: f64,
        tolerance_mm: Option<f64>,
    },
    Length {
        id: String,
        line_entity_id: String,
        target_mm: f64,
        tolerance_mm: Option<f64>,
    },
    HorizontalDistance {
        id: String,
        point_anchor_id: String,
        target_mm: f64,
        tolerance_mm: Option<f64>,
    },
    VerticalDistance {
        id: String,
        point_anchor_id: String,
        target_mm: f64,
        tolerance_mm: Option<f64>,
    },
    Diameter {
        id: String,
        circle_entity_id: String,
        target_mm: f64,
        tolerance_mm: Option<f64>,
    },
    /// Legacy compatibility shape retained while parity migrates to explicit
    /// dimensional variants.
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
            Self::Coincident { id, .. }
            | Self::PointOnLine { id, .. }
            | Self::Parallel { id, .. }
            | Self::Perpendicular { id, .. }
            | Self::Horizontal { id, .. }
            | Self::Vertical { id, .. }
            | Self::Tangent { id, .. }
            | Self::EqualLength { id, .. }
            | Self::EqualRadius { id, .. }
            | Self::Concentric { id, .. }
            | Self::Fixed { id, .. }
            | Self::PointOnCircle { id, .. }
            | Self::LineThroughCenter { id, .. }
            | Self::Midpoint { id, .. }
            | Self::Symmetric { id, .. }
            | Self::Distance { id, .. }
            | Self::PointLineDistance { id, .. }
            | Self::Angle { id, .. }
            | Self::Radius { id, .. }
            | Self::Length { id, .. }
            | Self::HorizontalDistance { id, .. }
            | Self::VerticalDistance { id, .. }
            | Self::Diameter { id, .. }
            | Self::Dimension { id, .. } => id,
        }
    }

    pub fn kind_key(&self) -> &'static str {
        match self {
            Self::Coincident { .. } => "coincident",
            Self::PointOnLine { .. } => "point_on_line",
            Self::Parallel { .. } => "parallel",
            Self::Perpendicular { .. } => "perpendicular",
            Self::Horizontal { .. } => "horizontal",
            Self::Vertical { .. } => "vertical",
            Self::Tangent { .. } => "tangent",
            Self::EqualLength { .. } => "equal_length",
            Self::EqualRadius { .. } => "equal_radius",
            Self::Concentric { .. } => "concentric",
            Self::Fixed { .. } => "fixed",
            Self::PointOnCircle { .. } => "point_on_circle",
            Self::LineThroughCenter { .. } => "line_through_center",
            Self::Midpoint { .. } => "midpoint",
            Self::Symmetric { .. } => "symmetric",
            Self::Distance { .. } => "distance",
            Self::PointLineDistance { .. } => "point_line_distance",
            Self::Angle { .. } => "angle",
            Self::Radius { .. } => "radius",
            Self::Length { .. } => "length",
            Self::HorizontalDistance { .. } => "horizontal_distance",
            Self::VerticalDistance { .. } => "vertical_distance",
            Self::Diameter { .. } => "diameter",
            Self::Dimension { .. } => "dimension",
        }
    }

    pub fn residual_component_count(&self) -> usize {
        match self {
            Self::Coincident { .. } => 2,
            Self::Concentric { .. } => 2,
            Self::Fixed { .. } => 2,
            Self::Midpoint { .. } => 2,
            Self::Symmetric { .. } => 2,
            Self::PointOnLine { .. }
            | Self::Parallel { .. }
            | Self::Perpendicular { .. }
            | Self::Horizontal { .. }
            | Self::Vertical { .. }
            | Self::Tangent { .. }
            | Self::EqualLength { .. }
            | Self::EqualRadius { .. }
            | Self::PointOnCircle { .. }
            | Self::LineThroughCenter { .. }
            | Self::Distance { .. }
            | Self::PointLineDistance { .. }
            | Self::Angle { .. }
            | Self::Radius { .. }
            | Self::Length { .. }
            | Self::HorizontalDistance { .. }
            | Self::VerticalDistance { .. }
            | Self::Diameter { .. }
            | Self::Dimension { .. } => 1,
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
                validate_anchor_pair(
                    self,
                    first_anchor_id,
                    second_anchor_id,
                    "coincident constraint",
                )?;
                validate_tolerance_opt(*tolerance_mm, "coincident tolerance_mm")?;
            }
            Self::PointOnLine {
                point_anchor_id,
                line_entity_id,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(point_anchor_id, "constraint point_anchor_id")?;
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
                validate_tolerance_opt(*tolerance_mm, "point_on_line tolerance_mm")?;
            }
            Self::Parallel {
                first_line_entity_id,
                second_line_entity_id,
                tolerance_mm,
                ..
            }
            | Self::Perpendicular {
                first_line_entity_id,
                second_line_entity_id,
                tolerance_mm,
                ..
            }
            | Self::EqualLength {
                first_line_entity_id,
                second_line_entity_id,
                tolerance_mm,
                ..
            } => {
                validate_line_pair(
                    self,
                    first_line_entity_id,
                    second_line_entity_id,
                    "line pair",
                )?;
                validate_tolerance_opt(*tolerance_mm, "line-pair tolerance_mm")?;
            }
            Self::Horizontal { line_entity_id, .. } | Self::Vertical { line_entity_id, .. } => {
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
            }
            Self::Tangent {
                line_entity_id,
                arc_entity_id,
                at_anchor_id,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
                validate_stable_id(arc_entity_id, "constraint arc_entity_id")?;
                if let Some(anchor_id) = at_anchor_id {
                    validate_stable_id(anchor_id, "constraint at_anchor_id")?;
                }
                validate_tolerance_opt(*tolerance_mm, "tangent tolerance_mm")?;
            }
            Self::EqualRadius {
                first_curve_entity_id,
                second_curve_entity_id,
                tolerance_mm,
                ..
            }
            | Self::Concentric {
                first_curve_entity_id,
                second_curve_entity_id,
                tolerance_mm,
                ..
            } => {
                validate_curve_pair(self, first_curve_entity_id, second_curve_entity_id)?;
                validate_tolerance_opt(*tolerance_mm, "curve-pair tolerance_mm")?;
            }
            Self::Fixed {
                point_anchor_id,
                target_mm,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(point_anchor_id, "constraint point_anchor_id")?;
                validate_vec2_finite(*target_mm, "fixed target_mm")?;
                validate_tolerance_opt(*tolerance_mm, "fixed tolerance_mm")?;
            }
            Self::PointOnCircle {
                point_anchor_id,
                circle_entity_id,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(point_anchor_id, "constraint point_anchor_id")?;
                validate_stable_id(circle_entity_id, "constraint circle_entity_id")?;
                validate_tolerance_opt(*tolerance_mm, "point_on_circle tolerance_mm")?;
            }
            Self::LineThroughCenter {
                line_entity_id,
                circle_entity_id,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
                validate_stable_id(circle_entity_id, "constraint circle_entity_id")?;
                validate_tolerance_opt(*tolerance_mm, "line_through_center tolerance_mm")?;
            }
            Self::Midpoint {
                midpoint_anchor_id,
                line_entity_id,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(midpoint_anchor_id, "constraint midpoint_anchor_id")?;
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
                validate_tolerance_opt(*tolerance_mm, "midpoint tolerance_mm")?;
            }
            Self::Symmetric {
                first_anchor_id,
                second_anchor_id,
                axis_line_entity_id,
                tolerance_mm,
                ..
            } => {
                validate_anchor_pair(
                    self,
                    first_anchor_id,
                    second_anchor_id,
                    "symmetric constraint",
                )?;
                validate_stable_id(axis_line_entity_id, "constraint axis_line_entity_id")?;
                validate_tolerance_opt(*tolerance_mm, "symmetric tolerance_mm")?;
            }
            Self::Distance {
                first_anchor_id,
                second_anchor_id,
                target_mm,
                tolerance_mm,
                ..
            } => {
                validate_anchor_pair(self, first_anchor_id, second_anchor_id, "distance")?;
                validate_positive_target(*target_mm, "distance target_mm", self)?;
                validate_tolerance_opt(*tolerance_mm, "distance tolerance_mm")?;
            }
            Self::PointLineDistance {
                point_anchor_id,
                line_entity_id,
                target_mm,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(point_anchor_id, "constraint point_anchor_id")?;
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
                validate_positive_target(*target_mm, "point_line_distance target_mm", self)?;
                validate_tolerance_opt(*tolerance_mm, "point_line_distance tolerance_mm")?;
            }
            Self::Angle {
                first_line_entity_id,
                second_line_entity_id,
                target_deg,
                tolerance_deg,
                ..
            } => {
                validate_line_pair(
                    self,
                    first_line_entity_id,
                    second_line_entity_id,
                    "angle line pair",
                )?;
                if !target_deg.is_finite() || *target_deg <= 0.0 || *target_deg > 360.0 {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "angle constraint {} target_deg must be finite and in (0, 360]",
                            self.id()
                        ),
                    });
                }
                validate_tolerance_opt(*tolerance_deg, "angle tolerance_deg")?;
            }
            Self::Radius {
                curve_entity_id,
                target_mm,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(curve_entity_id, "constraint curve_entity_id")?;
                validate_positive_target(*target_mm, "radius target_mm", self)?;
                validate_tolerance_opt(*tolerance_mm, "radius tolerance_mm")?;
            }
            Self::Length {
                line_entity_id,
                target_mm,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(line_entity_id, "constraint line_entity_id")?;
                validate_positive_target(*target_mm, "length target_mm", self)?;
                validate_tolerance_opt(*tolerance_mm, "length tolerance_mm")?;
            }
            Self::HorizontalDistance {
                point_anchor_id,
                target_mm,
                tolerance_mm,
                ..
            }
            | Self::VerticalDistance {
                point_anchor_id,
                target_mm,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(point_anchor_id, "constraint point_anchor_id")?;
                if !target_mm.is_finite() {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "{} constraint {} target_mm must be finite",
                            self.kind_key(),
                            self.id()
                        ),
                    });
                }
                validate_tolerance_opt(*tolerance_mm, "axis distance tolerance_mm")?;
            }
            Self::Diameter {
                circle_entity_id,
                target_mm,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(circle_entity_id, "constraint circle_entity_id")?;
                validate_positive_target(*target_mm, "diameter target_mm", self)?;
                validate_tolerance_opt(*tolerance_mm, "diameter tolerance_mm")?;
            }
            Self::Dimension {
                entity_id,
                target_mm,
                tolerance_mm,
                ..
            } => {
                validate_stable_id(entity_id, "constraint entity_id")?;
                validate_positive_target(*target_mm, "dimension target_mm", self)?;
                validate_tolerance_opt(*tolerance_mm, "dimension tolerance_mm")?;
            }
        }
        Ok(())
    }
}

fn validate_vec2_finite(value: [f64; 2], label: &str) -> CadResult<()> {
    if value.iter().all(|component| component.is_finite()) {
        return Ok(());
    }
    Err(CadError::ParseFailed {
        reason: format!("{label} must contain finite values"),
    })
}

fn validate_positive_target(
    value: f64,
    label: &str,
    constraint: &CadSketchConstraint,
) -> CadResult<()> {
    if value.is_finite() && value > 0.0 {
        return Ok(());
    }
    Err(CadError::ParseFailed {
        reason: format!(
            "{} constraint {} {label} must be finite and > 0",
            constraint.kind_key(),
            constraint.id()
        ),
    })
}

fn validate_anchor_pair(
    constraint: &CadSketchConstraint,
    first_anchor_id: &str,
    second_anchor_id: &str,
    label: &str,
) -> CadResult<()> {
    validate_stable_id(first_anchor_id, "constraint first_anchor_id")?;
    validate_stable_id(second_anchor_id, "constraint second_anchor_id")?;
    if first_anchor_id == second_anchor_id {
        return Err(CadError::ParseFailed {
            reason: format!(
                "{label} constraint {} must reference two distinct anchors",
                constraint.id()
            ),
        });
    }
    Ok(())
}

fn validate_line_pair(
    constraint: &CadSketchConstraint,
    first_line_entity_id: &str,
    second_line_entity_id: &str,
    label: &str,
) -> CadResult<()> {
    validate_stable_id(first_line_entity_id, "constraint first_line_entity_id")?;
    validate_stable_id(second_line_entity_id, "constraint second_line_entity_id")?;
    if first_line_entity_id == second_line_entity_id {
        return Err(CadError::ParseFailed {
            reason: format!(
                "{label} constraint {} must reference two distinct line entities",
                constraint.id()
            ),
        });
    }
    Ok(())
}

fn validate_curve_pair(
    constraint: &CadSketchConstraint,
    first_curve_entity_id: &str,
    second_curve_entity_id: &str,
) -> CadResult<()> {
    validate_stable_id(first_curve_entity_id, "constraint first_curve_entity_id")?;
    validate_stable_id(second_curve_entity_id, "constraint second_curve_entity_id")?;
    if first_curve_entity_id == second_curve_entity_id {
        return Err(CadError::ParseFailed {
            reason: format!(
                "curve-pair constraint {} must reference two distinct curve entities",
                constraint.id()
            ),
        });
    }
    Ok(())
}

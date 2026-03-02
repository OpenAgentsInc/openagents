use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::{
    arc_point, distance_mm, tangent_residual_mm, validate_vec2_finite, vector_length_mm,
};
use super::constraints::{CadDimensionConstraintKind, CadSketchConstraint};
use super::model::{CadSketchEntity, CadSketchModel};
use crate::{CadError, CadResult};

/// Solver diagnostic severity for deterministic sketch solving.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadSketchSolveSeverity {
    Info,
    Warning,
    Error,
}

/// Deterministic solver diagnostic payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadSketchSolveDiagnostic {
    pub code: String,
    pub severity: CadSketchSolveSeverity,
    pub constraint_id: String,
    pub message: String,
    pub remediation_hint: String,
}

/// Deterministic constraint solver report with per-constraint statuses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadSketchSolveReport {
    pub passed: bool,
    pub iteration_count: u32,
    pub solved_constraints: usize,
    pub unsolved_constraints: usize,
    pub constraint_status: BTreeMap<String, String>,
    pub residuals_mm: BTreeMap<String, f64>,
    pub diagnostics: Vec<CadSketchSolveDiagnostic>,
}

impl CadSketchModel {
    pub fn validate(&self) -> CadResult<()> {
        for (plane_id, plane) in &self.planes {
            plane.validate()?;
            if plane.id != *plane_id {
                return Err(CadError::ParseFailed {
                    reason: format!(
                        "sketch plane map key mismatch: key={plane_id} plane.id={}",
                        plane.id
                    ),
                });
            }
        }

        for (entity_id, entity) in &self.entities {
            entity.validate()?;
            if entity.id() != entity_id {
                return Err(CadError::ParseFailed {
                    reason: format!(
                        "sketch entity map key mismatch: key={entity_id} entity.id={}",
                        entity.id()
                    ),
                });
            }
            if !self.planes.contains_key(entity.plane_id()) {
                return Err(CadError::ParseFailed {
                    reason: format!(
                        "sketch entity {} references unknown plane {}",
                        entity.id(),
                        entity.plane_id()
                    ),
                });
            }
        }

        let anchor_bindings = self.collect_anchor_bindings()?;
        for (constraint_id, constraint) in &self.constraints {
            constraint.validate()?;
            if constraint.id() != constraint_id {
                return Err(CadError::ParseFailed {
                    reason: format!(
                        "sketch constraint map key mismatch: key={constraint_id} constraint.id={}",
                        constraint.id()
                    ),
                });
            }
            match constraint {
                CadSketchConstraint::Coincident {
                    first_anchor_id,
                    second_anchor_id,
                    ..
                }
                | CadSketchConstraint::Distance {
                    first_anchor_id,
                    second_anchor_id,
                    ..
                }
                | CadSketchConstraint::Symmetric {
                    first_anchor_id,
                    second_anchor_id,
                    ..
                } => {
                    ensure_anchor_exists(
                        anchor_bindings.get(first_anchor_id),
                        constraint,
                        first_anchor_id,
                    )?;
                    ensure_anchor_exists(
                        anchor_bindings.get(second_anchor_id),
                        constraint,
                        second_anchor_id,
                    )?;
                }
                CadSketchConstraint::PointOnLine {
                    point_anchor_id,
                    line_entity_id,
                    ..
                }
                | CadSketchConstraint::PointLineDistance {
                    point_anchor_id,
                    line_entity_id,
                    ..
                }
                | CadSketchConstraint::Midpoint {
                    midpoint_anchor_id: point_anchor_id,
                    line_entity_id,
                    ..
                } => {
                    ensure_anchor_exists(
                        anchor_bindings.get(point_anchor_id),
                        constraint,
                        point_anchor_id,
                    )?;
                    ensure_line_entity(
                        self.entities.get(line_entity_id),
                        constraint,
                        line_entity_id,
                    )?;
                }
                CadSketchConstraint::Parallel {
                    first_line_entity_id,
                    second_line_entity_id,
                    ..
                }
                | CadSketchConstraint::Perpendicular {
                    first_line_entity_id,
                    second_line_entity_id,
                    ..
                }
                | CadSketchConstraint::EqualLength {
                    first_line_entity_id,
                    second_line_entity_id,
                    ..
                }
                | CadSketchConstraint::Angle {
                    first_line_entity_id,
                    second_line_entity_id,
                    ..
                } => {
                    ensure_line_entity(
                        self.entities.get(first_line_entity_id),
                        constraint,
                        first_line_entity_id,
                    )?;
                    ensure_line_entity(
                        self.entities.get(second_line_entity_id),
                        constraint,
                        second_line_entity_id,
                    )?;
                }
                CadSketchConstraint::Tangent {
                    line_entity_id,
                    arc_entity_id,
                    at_anchor_id,
                    ..
                } => {
                    ensure_line_entity(
                        self.entities.get(line_entity_id),
                        constraint,
                        line_entity_id,
                    )?;
                    ensure_curve_entity(
                        self.entities.get(arc_entity_id),
                        constraint,
                        arc_entity_id,
                    )?;
                    if let Some(anchor_id) = at_anchor_id {
                        ensure_anchor_exists(
                            anchor_bindings.get(anchor_id),
                            constraint,
                            anchor_id,
                        )?;
                    }
                }
                CadSketchConstraint::Horizontal { line_entity_id, .. }
                | CadSketchConstraint::Vertical { line_entity_id, .. }
                | CadSketchConstraint::Length { line_entity_id, .. } => {
                    ensure_line_entity(
                        self.entities.get(line_entity_id),
                        constraint,
                        line_entity_id,
                    )?;
                }
                CadSketchConstraint::EqualRadius {
                    first_curve_entity_id,
                    second_curve_entity_id,
                    ..
                }
                | CadSketchConstraint::Concentric {
                    first_curve_entity_id,
                    second_curve_entity_id,
                    ..
                } => {
                    ensure_curve_entity(
                        self.entities.get(first_curve_entity_id),
                        constraint,
                        first_curve_entity_id,
                    )?;
                    ensure_curve_entity(
                        self.entities.get(second_curve_entity_id),
                        constraint,
                        second_curve_entity_id,
                    )?;
                }
                CadSketchConstraint::Fixed {
                    point_anchor_id, ..
                }
                | CadSketchConstraint::HorizontalDistance {
                    point_anchor_id, ..
                }
                | CadSketchConstraint::VerticalDistance {
                    point_anchor_id, ..
                } => {
                    ensure_anchor_exists(
                        anchor_bindings.get(point_anchor_id),
                        constraint,
                        point_anchor_id,
                    )?;
                }
                CadSketchConstraint::PointOnCircle {
                    point_anchor_id,
                    circle_entity_id,
                    ..
                } => {
                    ensure_anchor_exists(
                        anchor_bindings.get(point_anchor_id),
                        constraint,
                        point_anchor_id,
                    )?;
                    ensure_curve_entity(
                        self.entities.get(circle_entity_id),
                        constraint,
                        circle_entity_id,
                    )?;
                }
                CadSketchConstraint::LineThroughCenter {
                    line_entity_id,
                    circle_entity_id,
                    ..
                } => {
                    ensure_line_entity(
                        self.entities.get(line_entity_id),
                        constraint,
                        line_entity_id,
                    )?;
                    ensure_curve_entity(
                        self.entities.get(circle_entity_id),
                        constraint,
                        circle_entity_id,
                    )?;
                }
                CadSketchConstraint::Dimension {
                    entity_id,
                    dimension_kind,
                    ..
                } => match (dimension_kind, self.entities.get(entity_id)) {
                    (CadDimensionConstraintKind::Length, Some(CadSketchEntity::Line { .. })) => {}
                    (CadDimensionConstraintKind::Radius, Some(CadSketchEntity::Arc { .. }))
                    | (CadDimensionConstraintKind::Radius, Some(CadSketchEntity::Circle { .. })) => {
                    }
                    (_, Some(_)) => {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "constraint {} has dimension kind/entity mismatch for {}",
                                constraint.id(),
                                entity_id
                            ),
                        });
                    }
                    (_, None) => {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "constraint {} references unknown entity {}",
                                constraint.id(),
                                entity_id
                            ),
                        });
                    }
                },
                CadSketchConstraint::Radius {
                    curve_entity_id, ..
                } => {
                    ensure_curve_entity(
                        self.entities.get(curve_entity_id),
                        constraint,
                        curve_entity_id,
                    )?;
                }
                CadSketchConstraint::Diameter {
                    circle_entity_id, ..
                } => {
                    ensure_curve_entity(
                        self.entities.get(circle_entity_id),
                        constraint,
                        circle_entity_id,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Deterministic MVP solver pass over common sketch constraint scenarios.
    pub fn solve_constraints_deterministic(&mut self) -> CadResult<CadSketchSolveReport> {
        self.validate()?;

        let mut diagnostics = Vec::<CadSketchSolveDiagnostic>::new();
        let mut constraint_status = BTreeMap::<String, String>::new();
        let mut residuals_mm = BTreeMap::<String, f64>::new();
        let mut solved_constraints = 0usize;
        let mut unsolved_constraints = 0usize;

        let constraints = self.constraints.clone();
        for (constraint_id, constraint) in constraints {
            let outcome = match constraint {
                CadSketchConstraint::Coincident {
                    first_anchor_id,
                    second_anchor_id,
                    tolerance_mm,
                    ..
                } => self.solve_coincident(
                    &constraint_id,
                    &first_anchor_id,
                    &second_anchor_id,
                    tolerance_mm.unwrap_or(0.001),
                ),
                CadSketchConstraint::Horizontal { line_entity_id, .. } => {
                    self.solve_horizontal(&constraint_id, &line_entity_id)
                }
                CadSketchConstraint::Vertical { line_entity_id, .. } => {
                    self.solve_vertical(&constraint_id, &line_entity_id)
                }
                CadSketchConstraint::Tangent {
                    line_entity_id,
                    arc_entity_id,
                    at_anchor_id: _,
                    tolerance_mm,
                    ..
                } => self.solve_tangent(
                    &constraint_id,
                    &line_entity_id,
                    &arc_entity_id,
                    tolerance_mm.unwrap_or(0.001),
                ),
                CadSketchConstraint::Dimension {
                    entity_id,
                    dimension_kind,
                    target_mm,
                    tolerance_mm,
                    ..
                } => self.solve_dimension(
                    &constraint_id,
                    &entity_id,
                    dimension_kind,
                    target_mm,
                    tolerance_mm.unwrap_or(0.001),
                ),
                CadSketchConstraint::Length {
                    line_entity_id,
                    target_mm,
                    tolerance_mm,
                    ..
                } => self.solve_dimension(
                    &constraint_id,
                    &line_entity_id,
                    CadDimensionConstraintKind::Length,
                    target_mm,
                    tolerance_mm.unwrap_or(0.001),
                ),
                CadSketchConstraint::Radius {
                    curve_entity_id,
                    target_mm,
                    tolerance_mm,
                    ..
                } => self.solve_dimension(
                    &constraint_id,
                    &curve_entity_id,
                    CadDimensionConstraintKind::Radius,
                    target_mm,
                    tolerance_mm.unwrap_or(0.001),
                ),
                other => Ok(self.unsupported_constraint_outcome(&constraint_id, other.kind_key())),
            }?;

            residuals_mm.insert(constraint_id.clone(), outcome.residual_mm);
            if outcome.solved {
                solved_constraints = solved_constraints.saturating_add(1);
                constraint_status.insert(constraint_id.clone(), "solved".to_string());
            } else {
                unsolved_constraints = unsolved_constraints.saturating_add(1);
                constraint_status.insert(constraint_id.clone(), "unsolved".to_string());
            }
            if let Some(diagnostic) = outcome.diagnostic {
                diagnostics.push(diagnostic);
            }
        }

        let passed = unsolved_constraints == 0
            && diagnostics
                .iter()
                .all(|entry| entry.severity != CadSketchSolveSeverity::Error);
        Ok(CadSketchSolveReport {
            passed,
            iteration_count: 1,
            solved_constraints,
            unsolved_constraints,
            constraint_status,
            residuals_mm,
            diagnostics,
        })
    }

    fn solve_coincident(
        &mut self,
        constraint_id: &str,
        first_anchor_id: &str,
        second_anchor_id: &str,
        tolerance_mm: f64,
    ) -> CadResult<ConstraintSolveOutcome> {
        let bindings = self.collect_anchor_bindings()?;
        let first = bindings.get(first_anchor_id).cloned().ok_or_else(|| {
            CadError::ParseFailed {
                reason: format!(
                    "constraint {constraint_id} references missing first anchor {first_anchor_id}"
                ),
            }
        })?;
        let second = bindings.get(second_anchor_id).cloned().ok_or_else(|| {
            CadError::ParseFailed {
                reason: format!(
                    "constraint {constraint_id} references missing second anchor {second_anchor_id}"
                ),
            }
        })?;

        let mut residual = distance_mm(first.position_mm, second.position_mm);
        if residual > tolerance_mm {
            self.apply_anchor_position(&second, first.position_mm)?;
            let updated = self.collect_anchor_bindings()?;
            let first_after = updated
                .get(first_anchor_id)
                .ok_or_else(|| CadError::EvalFailed {
                    reason: format!(
                        "constraint {constraint_id} lost first anchor binding after apply: {first_anchor_id}"
                    ),
                })?;
            let second_after = updated
                .get(second_anchor_id)
                .ok_or_else(|| CadError::EvalFailed {
                    reason: format!(
                        "constraint {constraint_id} lost second anchor binding after apply: {second_anchor_id}"
                    ),
                })?;
            residual = distance_mm(first_after.position_mm, second_after.position_mm);
        }

        let solved = residual <= tolerance_mm;
        let diagnostic = (!solved).then(|| CadSketchSolveDiagnostic {
            code: "SKETCH_CONSTRAINT_COINCIDENT_UNSATISFIED".to_string(),
            severity: CadSketchSolveSeverity::Error,
            constraint_id: constraint_id.to_string(),
            message: format!(
                "coincident anchors remain {residual:.6}mm apart (tol={tolerance_mm:.6})"
            ),
            remediation_hint: "adjust anchor references or add dimensions to remove ambiguity"
                .to_string(),
        });

        Ok(ConstraintSolveOutcome {
            solved,
            residual_mm: residual,
            diagnostic,
        })
    }

    fn solve_horizontal(
        &mut self,
        constraint_id: &str,
        line_entity_id: &str,
    ) -> CadResult<ConstraintSolveOutcome> {
        let (start_before, mut end_before) = self.line_endpoints(line_entity_id)?;
        end_before[1] = start_before[1];
        self.set_line_endpoints(line_entity_id, start_before, end_before)?;
        let (start_after, end_after) = self.line_endpoints(line_entity_id)?;
        let residual = (start_after[1] - end_after[1]).abs();
        let solved = residual <= 0.000_001;
        let diagnostic = (!solved).then(|| CadSketchSolveDiagnostic {
            code: "SKETCH_CONSTRAINT_HORIZONTAL_UNSATISFIED".to_string(),
            severity: CadSketchSolveSeverity::Error,
            constraint_id: constraint_id.to_string(),
            message: format!("line {line_entity_id} is not horizontal after solve"),
            remediation_hint: "ensure line has valid finite endpoints".to_string(),
        });
        Ok(ConstraintSolveOutcome {
            solved,
            residual_mm: residual,
            diagnostic,
        })
    }

    fn solve_vertical(
        &mut self,
        constraint_id: &str,
        line_entity_id: &str,
    ) -> CadResult<ConstraintSolveOutcome> {
        let (start_before, mut end_before) = self.line_endpoints(line_entity_id)?;
        end_before[0] = start_before[0];
        self.set_line_endpoints(line_entity_id, start_before, end_before)?;
        let (start_after, end_after) = self.line_endpoints(line_entity_id)?;
        let residual = (start_after[0] - end_after[0]).abs();
        let solved = residual <= 0.000_001;
        let diagnostic = (!solved).then(|| CadSketchSolveDiagnostic {
            code: "SKETCH_CONSTRAINT_VERTICAL_UNSATISFIED".to_string(),
            severity: CadSketchSolveSeverity::Error,
            constraint_id: constraint_id.to_string(),
            message: format!("line {line_entity_id} is not vertical after solve"),
            remediation_hint: "ensure line has valid finite endpoints".to_string(),
        });
        Ok(ConstraintSolveOutcome {
            solved,
            residual_mm: residual,
            diagnostic,
        })
    }

    fn solve_tangent(
        &mut self,
        constraint_id: &str,
        line_entity_id: &str,
        arc_entity_id: &str,
        tolerance_mm: f64,
    ) -> CadResult<ConstraintSolveOutcome> {
        let (line_start, line_end) = self.line_endpoints(line_entity_id)?;
        let (arc_center, arc_radius) = self.curve_center_radius(arc_entity_id)?;
        let mut residual =
            tangent_residual_mm(line_start, line_end, arc_center, arc_radius, tolerance_mm)?;

        if residual > tolerance_mm {
            let dx = (line_end[0] - line_start[0]).abs();
            let dy = (line_end[1] - line_start[1]).abs();

            // Deterministic MVP tangent correction for common horizontal/vertical lines.
            if dy <= 0.000_001 {
                let sign = if line_start[1] >= arc_center[1] {
                    1.0
                } else {
                    -1.0
                };
                let target_y = arc_center[1] + sign * arc_radius;
                self.set_line_endpoints(
                    line_entity_id,
                    [line_start[0], target_y],
                    [line_end[0], target_y],
                )?;
            } else if dx <= 0.000_001 {
                let sign = if line_start[0] >= arc_center[0] {
                    1.0
                } else {
                    -1.0
                };
                let target_x = arc_center[0] + sign * arc_radius;
                self.set_line_endpoints(
                    line_entity_id,
                    [target_x, line_start[1]],
                    [target_x, line_end[1]],
                )?;
            }
            let (updated_start, updated_end) = self.line_endpoints(line_entity_id)?;
            residual = tangent_residual_mm(
                updated_start,
                updated_end,
                arc_center,
                arc_radius,
                tolerance_mm,
            )?;
        }

        let solved = residual <= tolerance_mm;
        let diagnostic = (!solved).then(|| CadSketchSolveDiagnostic {
            code: "SKETCH_CONSTRAINT_TANGENT_UNSATISFIED".to_string(),
            severity: CadSketchSolveSeverity::Error,
            constraint_id: constraint_id.to_string(),
            message: format!(
                "line {line_entity_id} and arc {arc_entity_id} remain non-tangent (residual={residual:.6}mm, tol={tolerance_mm:.6})"
            ),
            remediation_hint: "use horizontal/vertical helper lines or add supporting dimensions"
                .to_string(),
        });

        Ok(ConstraintSolveOutcome {
            solved,
            residual_mm: residual,
            diagnostic,
        })
    }

    fn solve_dimension(
        &mut self,
        constraint_id: &str,
        entity_id: &str,
        dimension_kind: CadDimensionConstraintKind,
        target_mm: f64,
        tolerance_mm: f64,
    ) -> CadResult<ConstraintSolveOutcome> {
        match dimension_kind {
            CadDimensionConstraintKind::Length => {
                let (start, mut end) = self.line_endpoints(entity_id)?;
                let vector = [end[0] - start[0], end[1] - start[1]];
                let length = vector_length_mm(vector);
                if length <= 0.000_001 {
                    end = [start[0] + target_mm, start[1]];
                } else {
                    let scale = target_mm / length;
                    end = [start[0] + vector[0] * scale, start[1] + vector[1] * scale];
                }
                self.set_line_endpoints(entity_id, start, end)?;

                let (start_after, end_after) = self.line_endpoints(entity_id)?;
                let residual = (vector_length_mm([
                    end_after[0] - start_after[0],
                    end_after[1] - start_after[1],
                ]) - target_mm)
                    .abs();
                let solved = residual <= tolerance_mm;
                let diagnostic = (!solved).then(|| CadSketchSolveDiagnostic {
                    code: "SKETCH_CONSTRAINT_DIMENSION_LENGTH_UNSATISFIED".to_string(),
                    severity: CadSketchSolveSeverity::Error,
                    constraint_id: constraint_id.to_string(),
                    message: format!(
                        "line {entity_id} length residual={residual:.6}mm exceeds tol={tolerance_mm:.6}"
                    ),
                    remediation_hint: "verify line endpoints are finite and not locked by conflicting constraints".to_string(),
                });
                Ok(ConstraintSolveOutcome {
                    solved,
                    residual_mm: residual,
                    diagnostic,
                })
            }
            CadDimensionConstraintKind::Radius => {
                self.set_curve_radius(entity_id, target_mm)?;
                let (_center, radius_after) = self.curve_center_radius(entity_id)?;
                let residual = (radius_after - target_mm).abs();
                let solved = residual <= tolerance_mm;
                let diagnostic = (!solved).then(|| CadSketchSolveDiagnostic {
                    code: "SKETCH_CONSTRAINT_DIMENSION_RADIUS_UNSATISFIED".to_string(),
                    severity: CadSketchSolveSeverity::Error,
                    constraint_id: constraint_id.to_string(),
                    message: format!(
                        "arc {entity_id} radius residual={residual:.6}mm exceeds tol={tolerance_mm:.6}"
                    ),
                    remediation_hint: "verify arc center/radius parameters are finite".to_string(),
                });
                Ok(ConstraintSolveOutcome {
                    solved,
                    residual_mm: residual,
                    diagnostic,
                })
            }
        }
    }

    fn unsupported_constraint_outcome(
        &self,
        constraint_id: &str,
        constraint_kind: &str,
    ) -> ConstraintSolveOutcome {
        ConstraintSolveOutcome {
            solved: false,
            residual_mm: 1.0,
            diagnostic: Some(CadSketchSolveDiagnostic {
                code: "SKETCH_CONSTRAINT_KIND_NOT_IMPLEMENTED".to_string(),
                severity: CadSketchSolveSeverity::Warning,
                constraint_id: constraint_id.to_string(),
                message: format!(
                    "constraint kind {constraint_kind} is validated but not yet solved in deterministic pass"
                ),
                remediation_hint: "iterative LM solver parity is delivered in VCAD-PARITY-044/045"
                    .to_string(),
            }),
        }
    }

    pub(super) fn collect_anchor_bindings(&self) -> CadResult<BTreeMap<String, AnchorBinding>> {
        let mut bindings = BTreeMap::<String, AnchorBinding>::new();
        for entity in self.entities.values() {
            match entity {
                CadSketchEntity::Line {
                    id,
                    plane_id,
                    start_mm,
                    end_mm,
                    anchor_ids,
                    ..
                } => {
                    validate_vec2_finite(*start_mm, "line start_mm")?;
                    validate_vec2_finite(*end_mm, "line end_mm")?;
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[0],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::LineStart,
                            position_mm: *start_mm,
                        },
                    )?;
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[1],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::LineEnd,
                            position_mm: *end_mm,
                        },
                    )?;
                }
                CadSketchEntity::Arc {
                    id,
                    plane_id,
                    center_mm,
                    radius_mm,
                    start_deg,
                    end_deg,
                    anchor_ids,
                    ..
                } => {
                    validate_vec2_finite(*center_mm, "arc center_mm")?;
                    if !radius_mm.is_finite() || *radius_mm <= 0.0 {
                        return Err(CadError::ParseFailed {
                            reason: format!("arc {id} radius_mm must be finite and > 0"),
                        });
                    }
                    let start_mm = arc_point(*center_mm, *radius_mm, *start_deg)?;
                    let end_mm = arc_point(*center_mm, *radius_mm, *end_deg)?;
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[0],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::ArcCenter,
                            position_mm: *center_mm,
                        },
                    )?;
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[1],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::ArcStart,
                            position_mm: start_mm,
                        },
                    )?;
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[2],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::ArcEnd,
                            position_mm: end_mm,
                        },
                    )?;
                }
                CadSketchEntity::Rectangle {
                    id,
                    plane_id,
                    min_mm,
                    max_mm,
                    anchor_ids,
                    ..
                } => {
                    validate_vec2_finite(*min_mm, "rectangle min_mm")?;
                    validate_vec2_finite(*max_mm, "rectangle max_mm")?;
                    if max_mm[0] <= min_mm[0] || max_mm[1] <= min_mm[1] {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "rectangle {id} requires max_mm strictly greater than min_mm"
                            ),
                        });
                    }
                    let corners = [
                        [min_mm[0], min_mm[1]],
                        [max_mm[0], min_mm[1]],
                        [max_mm[0], max_mm[1]],
                        [min_mm[0], max_mm[1]],
                    ];
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[0],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::RectangleMinMin,
                            position_mm: corners[0],
                        },
                    )?;
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[1],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::RectangleMaxMin,
                            position_mm: corners[1],
                        },
                    )?;
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[2],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::RectangleMaxMax,
                            position_mm: corners[2],
                        },
                    )?;
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[3],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::RectangleMinMax,
                            position_mm: corners[3],
                        },
                    )?;
                }
                CadSketchEntity::Circle {
                    id,
                    plane_id,
                    center_mm,
                    radius_mm,
                    anchor_ids,
                    ..
                } => {
                    validate_vec2_finite(*center_mm, "circle center_mm")?;
                    if !radius_mm.is_finite() || *radius_mm <= 0.0 {
                        return Err(CadError::ParseFailed {
                            reason: format!("circle {id} radius_mm must be finite and > 0"),
                        });
                    }
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[0],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::CircleCenter,
                            position_mm: *center_mm,
                        },
                    )?;
                    insert_binding(
                        &mut bindings,
                        &anchor_ids[1],
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::CirclePerimeter,
                            position_mm: [center_mm[0] + *radius_mm, center_mm[1]],
                        },
                    )?;
                }
                CadSketchEntity::Spline {
                    id,
                    plane_id,
                    control_points_mm,
                    anchor_ids,
                    ..
                } => {
                    if control_points_mm.len() != anchor_ids.len() {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "spline {id} control_points_mm count must match anchor_ids count"
                            ),
                        });
                    }
                    for (index, (point, anchor_id)) in
                        control_points_mm.iter().zip(anchor_ids.iter()).enumerate()
                    {
                        validate_vec2_finite(*point, "spline control_points_mm")?;
                        insert_binding(
                            &mut bindings,
                            anchor_id,
                            AnchorBinding {
                                entity_id: id.clone(),
                                plane_id: plane_id.clone(),
                                role: AnchorRole::SplineControlPoint(index),
                                position_mm: *point,
                            },
                        )?;
                    }
                }
                CadSketchEntity::Point {
                    id,
                    plane_id,
                    position_mm,
                    anchor_id,
                    ..
                } => {
                    validate_vec2_finite(*position_mm, "point position_mm")?;
                    insert_binding(
                        &mut bindings,
                        anchor_id,
                        AnchorBinding {
                            entity_id: id.clone(),
                            plane_id: plane_id.clone(),
                            role: AnchorRole::Point,
                            position_mm: *position_mm,
                        },
                    )?;
                }
            }
        }
        Ok(bindings)
    }

    fn apply_anchor_position(
        &mut self,
        binding: &AnchorBinding,
        position_mm: [f64; 2],
    ) -> CadResult<()> {
        validate_vec2_finite(position_mm, "anchor position_mm")?;
        match self.entities.get_mut(&binding.entity_id) {
            Some(CadSketchEntity::Line {
                start_mm, end_mm, ..
            }) => match binding.role {
                AnchorRole::LineStart => {
                    *start_mm = position_mm;
                }
                AnchorRole::LineEnd => {
                    *end_mm = position_mm;
                }
                _ => {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "anchor role mismatch while updating line {}",
                            binding.entity_id
                        ),
                    });
                }
            },
            Some(CadSketchEntity::Arc {
                center_mm,
                radius_mm,
                start_deg,
                end_deg,
                ..
            }) => match binding.role {
                AnchorRole::ArcCenter => {
                    *center_mm = position_mm;
                }
                AnchorRole::ArcStart => {
                    let vector = [position_mm[0] - center_mm[0], position_mm[1] - center_mm[1]];
                    let radius = vector_length_mm(vector);
                    if radius <= 0.000_001 {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "cannot place arc start anchor on center for {}",
                                binding.entity_id
                            ),
                        });
                    }
                    *radius_mm = radius;
                    *start_deg = vector[1].atan2(vector[0]).to_degrees();
                }
                AnchorRole::ArcEnd => {
                    let vector = [position_mm[0] - center_mm[0], position_mm[1] - center_mm[1]];
                    let radius = vector_length_mm(vector);
                    if radius <= 0.000_001 {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "cannot place arc end anchor on center for {}",
                                binding.entity_id
                            ),
                        });
                    }
                    *radius_mm = radius;
                    *end_deg = vector[1].atan2(vector[0]).to_degrees();
                }
                _ => {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "anchor role mismatch while updating arc {}",
                            binding.entity_id
                        ),
                    });
                }
            },
            Some(CadSketchEntity::Point {
                position_mm: point, ..
            }) => match binding.role {
                AnchorRole::Point => {
                    *point = position_mm;
                }
                _ => {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "anchor role mismatch while updating point {}",
                            binding.entity_id
                        ),
                    });
                }
            },
            Some(CadSketchEntity::Rectangle { min_mm, max_mm, .. }) => {
                match binding.role {
                    AnchorRole::RectangleMinMin => {
                        *min_mm = position_mm;
                    }
                    AnchorRole::RectangleMaxMin => {
                        max_mm[0] = position_mm[0];
                        min_mm[1] = position_mm[1];
                    }
                    AnchorRole::RectangleMaxMax => {
                        *max_mm = position_mm;
                    }
                    AnchorRole::RectangleMinMax => {
                        min_mm[0] = position_mm[0];
                        max_mm[1] = position_mm[1];
                    }
                    _ => {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "anchor role mismatch while updating rectangle {}",
                                binding.entity_id
                            ),
                        });
                    }
                }
                if max_mm[0] <= min_mm[0] || max_mm[1] <= min_mm[1] {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "rectangle {} update produced invalid min/max bounds",
                            binding.entity_id
                        ),
                    });
                }
            }
            Some(CadSketchEntity::Circle {
                center_mm,
                radius_mm,
                ..
            }) => match binding.role {
                AnchorRole::CircleCenter => {
                    *center_mm = position_mm;
                }
                AnchorRole::CirclePerimeter => {
                    let vector = [position_mm[0] - center_mm[0], position_mm[1] - center_mm[1]];
                    let radius = vector_length_mm(vector);
                    if radius <= 0.000_001 {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "cannot place circle perimeter anchor on center for {}",
                                binding.entity_id
                            ),
                        });
                    }
                    *radius_mm = radius;
                }
                _ => {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "anchor role mismatch while updating circle {}",
                            binding.entity_id
                        ),
                    });
                }
            },
            Some(CadSketchEntity::Spline {
                control_points_mm, ..
            }) => match binding.role {
                AnchorRole::SplineControlPoint(index) => {
                    if index >= control_points_mm.len() {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "spline {} missing control point index {}",
                                binding.entity_id, index
                            ),
                        });
                    }
                    control_points_mm[index] = position_mm;
                }
                _ => {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "anchor role mismatch while updating spline {}",
                            binding.entity_id
                        ),
                    });
                }
            },
            None => {
                return Err(CadError::ParseFailed {
                    reason: format!("missing entity {} for anchor update", binding.entity_id),
                });
            }
        }
        Ok(())
    }

    fn line_endpoints(&self, entity_id: &str) -> CadResult<([f64; 2], [f64; 2])> {
        match self.entities.get(entity_id) {
            Some(CadSketchEntity::Line {
                start_mm, end_mm, ..
            }) => Ok((*start_mm, *end_mm)),
            Some(_) => Err(CadError::ParseFailed {
                reason: format!("entity {entity_id} is not a line"),
            }),
            None => Err(CadError::ParseFailed {
                reason: format!("missing line entity {entity_id}"),
            }),
        }
    }

    fn set_line_endpoints(
        &mut self,
        entity_id: &str,
        start_mm: [f64; 2],
        end_mm: [f64; 2],
    ) -> CadResult<()> {
        match self.entities.get_mut(entity_id) {
            Some(CadSketchEntity::Line {
                start_mm: start,
                end_mm: end,
                ..
            }) => {
                *start = start_mm;
                *end = end_mm;
                Ok(())
            }
            Some(_) => Err(CadError::ParseFailed {
                reason: format!("entity {entity_id} is not a line"),
            }),
            None => Err(CadError::ParseFailed {
                reason: format!("missing line entity {entity_id}"),
            }),
        }
    }

    fn curve_center_radius(&self, entity_id: &str) -> CadResult<([f64; 2], f64)> {
        match self.entities.get(entity_id) {
            Some(CadSketchEntity::Arc {
                center_mm,
                radius_mm,
                ..
            }) => Ok((*center_mm, *radius_mm)),
            Some(CadSketchEntity::Circle {
                center_mm,
                radius_mm,
                ..
            }) => Ok((*center_mm, *radius_mm)),
            Some(_) => Err(CadError::ParseFailed {
                reason: format!("entity {entity_id} is not a curve"),
            }),
            None => Err(CadError::ParseFailed {
                reason: format!("missing curve entity {entity_id}"),
            }),
        }
    }

    fn set_curve_radius(&mut self, entity_id: &str, radius_mm: f64) -> CadResult<()> {
        if !radius_mm.is_finite() || radius_mm <= 0.0 {
            return Err(CadError::ParseFailed {
                reason: format!("curve {entity_id} radius must be finite and > 0"),
            });
        }
        match self.entities.get_mut(entity_id) {
            Some(CadSketchEntity::Arc {
                radius_mm: radius, ..
            }) => {
                *radius = radius_mm;
                Ok(())
            }
            Some(CadSketchEntity::Circle {
                radius_mm: radius, ..
            }) => {
                *radius = radius_mm;
                Ok(())
            }
            Some(_) => Err(CadError::ParseFailed {
                reason: format!("entity {entity_id} is not a curve"),
            }),
            None => Err(CadError::ParseFailed {
                reason: format!("missing curve entity {entity_id}"),
            }),
        }
    }
}

fn ensure_anchor_exists(
    binding: Option<&AnchorBinding>,
    constraint: &CadSketchConstraint,
    anchor_id: &str,
) -> CadResult<()> {
    if binding.is_some() {
        return Ok(());
    }
    Err(CadError::ParseFailed {
        reason: format!(
            "constraint {} references unknown anchor {}",
            constraint.id(),
            anchor_id
        ),
    })
}

fn ensure_line_entity(
    entity: Option<&CadSketchEntity>,
    constraint: &CadSketchConstraint,
    line_entity_id: &str,
) -> CadResult<()> {
    match entity {
        Some(CadSketchEntity::Line { .. }) => Ok(()),
        Some(_) => Err(CadError::ParseFailed {
            reason: format!(
                "constraint {} requires line entity {}, found non-line entity",
                constraint.id(),
                line_entity_id
            ),
        }),
        None => Err(CadError::ParseFailed {
            reason: format!(
                "constraint {} references unknown line entity {}",
                constraint.id(),
                line_entity_id
            ),
        }),
    }
}

fn ensure_curve_entity(
    entity: Option<&CadSketchEntity>,
    constraint: &CadSketchConstraint,
    curve_entity_id: &str,
) -> CadResult<()> {
    match entity {
        Some(CadSketchEntity::Arc { .. }) | Some(CadSketchEntity::Circle { .. }) => Ok(()),
        Some(_) => Err(CadError::ParseFailed {
            reason: format!(
                "constraint {} requires curve entity {}, found non-curve entity",
                constraint.id(),
                curve_entity_id
            ),
        }),
        None => Err(CadError::ParseFailed {
            reason: format!(
                "constraint {} references unknown curve entity {}",
                constraint.id(),
                curve_entity_id
            ),
        }),
    }
}

#[derive(Clone, Debug)]
pub(super) struct AnchorBinding {
    pub(super) entity_id: String,
    pub(super) plane_id: String,
    role: AnchorRole,
    pub(super) position_mm: [f64; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnchorRole {
    LineStart,
    LineEnd,
    ArcCenter,
    ArcStart,
    ArcEnd,
    RectangleMinMin,
    RectangleMaxMin,
    RectangleMaxMax,
    RectangleMinMax,
    CircleCenter,
    CirclePerimeter,
    SplineControlPoint(usize),
    Point,
}

#[derive(Clone, Debug)]
struct ConstraintSolveOutcome {
    solved: bool,
    residual_mm: f64,
    diagnostic: Option<CadSketchSolveDiagnostic>,
}

fn insert_binding(
    bindings: &mut BTreeMap<String, AnchorBinding>,
    anchor_id: &str,
    binding: AnchorBinding,
) -> CadResult<()> {
    if let Some(existing) = bindings.insert(anchor_id.to_string(), binding.clone()) {
        return Err(CadError::ParseFailed {
            reason: format!(
                "duplicate anchor id {anchor_id} between entities {} and {} on plane {}",
                existing.entity_id, binding.entity_id, existing.plane_id
            ),
        });
    }
    Ok(())
}

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
                } => {
                    if !anchor_bindings.contains_key(first_anchor_id) {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "constraint {} references unknown anchor {}",
                                constraint.id(),
                                first_anchor_id
                            ),
                        });
                    }
                    if !anchor_bindings.contains_key(second_anchor_id) {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "constraint {} references unknown anchor {}",
                                constraint.id(),
                                second_anchor_id
                            ),
                        });
                    }
                }
                CadSketchConstraint::Horizontal { line_entity_id, .. }
                | CadSketchConstraint::Vertical { line_entity_id, .. } => {
                    match self.entities.get(line_entity_id) {
                        Some(CadSketchEntity::Line { .. }) => {}
                        Some(_) => {
                            return Err(CadError::ParseFailed {
                                reason: format!(
                                    "constraint {} requires line entity {}, found non-line entity",
                                    constraint.id(),
                                    line_entity_id
                                ),
                            });
                        }
                        None => {
                            return Err(CadError::ParseFailed {
                                reason: format!(
                                    "constraint {} references unknown line entity {}",
                                    constraint.id(),
                                    line_entity_id
                                ),
                            });
                        }
                    }
                }
                CadSketchConstraint::Tangent {
                    line_entity_id,
                    arc_entity_id,
                    ..
                } => {
                    match self.entities.get(line_entity_id) {
                        Some(CadSketchEntity::Line { .. }) => {}
                        Some(_) => {
                            return Err(CadError::ParseFailed {
                                reason: format!(
                                    "constraint {} requires line entity {}, found non-line entity",
                                    constraint.id(),
                                    line_entity_id
                                ),
                            });
                        }
                        None => {
                            return Err(CadError::ParseFailed {
                                reason: format!(
                                    "constraint {} references unknown line entity {}",
                                    constraint.id(),
                                    line_entity_id
                                ),
                            });
                        }
                    }
                    match self.entities.get(arc_entity_id) {
                        Some(CadSketchEntity::Arc { .. }) => {}
                        Some(_) => {
                            return Err(CadError::ParseFailed {
                                reason: format!(
                                    "constraint {} requires arc entity {}, found non-arc entity",
                                    constraint.id(),
                                    arc_entity_id
                                ),
                            });
                        }
                        None => {
                            return Err(CadError::ParseFailed {
                                reason: format!(
                                    "constraint {} references unknown arc entity {}",
                                    constraint.id(),
                                    arc_entity_id
                                ),
                            });
                        }
                    }
                }
                CadSketchConstraint::Dimension {
                    entity_id,
                    dimension_kind,
                    ..
                } => match (dimension_kind, self.entities.get(entity_id)) {
                    (CadDimensionConstraintKind::Length, Some(CadSketchEntity::Line { .. })) => {}
                    (CadDimensionConstraintKind::Radius, Some(CadSketchEntity::Arc { .. })) => {}
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
        let first =
            bindings
                .get(first_anchor_id)
                .cloned()
                .ok_or_else(|| CadError::ParseFailed {
                    reason: format!(
                    "constraint {constraint_id} references missing first anchor {first_anchor_id}"
                ),
                })?;
        let second =
            bindings
                .get(second_anchor_id)
                .cloned()
                .ok_or_else(|| CadError::ParseFailed {
                    reason: format!(
                    "constraint {constraint_id} references missing second anchor {second_anchor_id}"
                ),
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
        let (arc_center, arc_radius) = self.arc_center_radius(arc_entity_id)?;
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
                self.set_arc_radius(entity_id, target_mm)?;
                let (_center, radius_after) = self.arc_center_radius(entity_id)?;
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

    fn arc_center_radius(&self, entity_id: &str) -> CadResult<([f64; 2], f64)> {
        match self.entities.get(entity_id) {
            Some(CadSketchEntity::Arc {
                center_mm,
                radius_mm,
                ..
            }) => Ok((*center_mm, *radius_mm)),
            Some(_) => Err(CadError::ParseFailed {
                reason: format!("entity {entity_id} is not an arc"),
            }),
            None => Err(CadError::ParseFailed {
                reason: format!("missing arc entity {entity_id}"),
            }),
        }
    }

    fn set_arc_radius(&mut self, entity_id: &str, radius_mm: f64) -> CadResult<()> {
        if !radius_mm.is_finite() || radius_mm <= 0.0 {
            return Err(CadError::ParseFailed {
                reason: format!("arc {entity_id} radius must be finite and > 0"),
            });
        }
        match self.entities.get_mut(entity_id) {
            Some(CadSketchEntity::Arc {
                radius_mm: radius, ..
            }) => {
                *radius = radius_mm;
                Ok(())
            }
            Some(_) => Err(CadError::ParseFailed {
                reason: format!("entity {entity_id} is not an arc"),
            }),
            None => Err(CadError::ParseFailed {
                reason: format!("missing arc entity {entity_id}"),
            }),
        }
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

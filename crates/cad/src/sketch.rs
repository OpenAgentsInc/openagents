use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{CadError, CadResult};

/// Stable sketch plane definition in model-space millimeters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadSketchPlane {
    pub id: String,
    pub name: String,
    pub origin_mm: [f64; 3],
    pub normal: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
}

impl CadSketchPlane {
    pub fn validate(&self) -> CadResult<()> {
        validate_stable_id(&self.id, "sketch plane id")?;
        if self.name.trim().is_empty() {
            return Err(CadError::ParseFailed {
                reason: "sketch plane name must not be empty".to_string(),
            });
        }
        validate_vec3_finite(self.origin_mm, "sketch plane origin_mm")?;
        validate_vec3_finite(self.normal, "sketch plane normal")?;
        validate_vec3_finite(self.x_axis, "sketch plane x_axis")?;
        validate_vec3_finite(self.y_axis, "sketch plane y_axis")?;
        Ok(())
    }
}

/// Stable sketch entity model for Wave 2 kickoff.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadSketchEntity {
    Line {
        id: String,
        plane_id: String,
        start_mm: [f64; 2],
        end_mm: [f64; 2],
        anchor_ids: [String; 2],
        construction: bool,
    },
    Arc {
        id: String,
        plane_id: String,
        center_mm: [f64; 2],
        radius_mm: f64,
        start_deg: f64,
        end_deg: f64,
        anchor_ids: [String; 3],
        construction: bool,
    },
    Point {
        id: String,
        plane_id: String,
        position_mm: [f64; 2],
        anchor_id: String,
        construction: bool,
    },
}

impl CadSketchEntity {
    pub fn id(&self) -> &str {
        match self {
            Self::Line { id, .. } => id,
            Self::Arc { id, .. } => id,
            Self::Point { id, .. } => id,
        }
    }

    pub fn plane_id(&self) -> &str {
        match self {
            Self::Line { plane_id, .. } => plane_id,
            Self::Arc { plane_id, .. } => plane_id,
            Self::Point { plane_id, .. } => plane_id,
        }
    }

    pub fn anchor_ids(&self) -> Vec<&str> {
        match self {
            Self::Line { anchor_ids, .. } => anchor_ids.iter().map(String::as_str).collect(),
            Self::Arc { anchor_ids, .. } => anchor_ids.iter().map(String::as_str).collect(),
            Self::Point { anchor_id, .. } => vec![anchor_id.as_str()],
        }
    }

    pub fn validate(&self) -> CadResult<()> {
        validate_stable_id(self.id(), "sketch entity id")?;
        validate_stable_id(self.plane_id(), "sketch entity plane_id")?;
        for anchor in self.anchor_ids() {
            validate_stable_id(anchor, "sketch anchor id")?;
        }

        let anchors = self.anchor_ids();
        let mut unique = BTreeSet::<&str>::new();
        for anchor in &anchors {
            unique.insert(anchor);
        }
        if unique.len() != anchors.len() {
            return Err(CadError::ParseFailed {
                reason: format!("sketch entity {} has duplicate anchor ids", self.id()),
            });
        }

        match self {
            Self::Line {
                start_mm, end_mm, ..
            } => {
                validate_vec2_finite(*start_mm, "line start_mm")?;
                validate_vec2_finite(*end_mm, "line end_mm")?;
                if start_mm == end_mm {
                    return Err(CadError::ParseFailed {
                        reason: format!("line {} start/end points must differ", self.id()),
                    });
                }
            }
            Self::Arc {
                center_mm,
                radius_mm,
                start_deg,
                end_deg,
                ..
            } => {
                validate_vec2_finite(*center_mm, "arc center_mm")?;
                if !radius_mm.is_finite() || *radius_mm <= 0.0 {
                    return Err(CadError::ParseFailed {
                        reason: format!("arc {} radius_mm must be finite and > 0", self.id()),
                    });
                }
                if !start_deg.is_finite() || !end_deg.is_finite() {
                    return Err(CadError::ParseFailed {
                        reason: format!("arc {} angle bounds must be finite", self.id()),
                    });
                }
            }
            Self::Point { position_mm, .. } => {
                validate_vec2_finite(*position_mm, "point position_mm")?;
            }
        }
        Ok(())
    }
}

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

/// Deterministic sketch model persisted in `.apcad`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CadSketchModel {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub planes: BTreeMap<String, CadSketchPlane>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub entities: BTreeMap<String, CadSketchEntity>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub constraints: BTreeMap<String, CadSketchConstraint>,
}

impl CadSketchModel {
    pub fn is_empty(&self) -> bool {
        self.planes.is_empty() && self.entities.is_empty() && self.constraints.is_empty()
    }

    pub fn insert_plane(&mut self, plane: CadSketchPlane) -> CadResult<()> {
        plane.validate()?;
        self.planes.insert(plane.id.clone(), plane);
        self.validate()
    }

    pub fn insert_entity(&mut self, entity: CadSketchEntity) -> CadResult<()> {
        entity.validate()?;
        self.entities.insert(entity.id().to_string(), entity);
        self.validate()
    }

    pub fn insert_constraint(&mut self, constraint: CadSketchConstraint) -> CadResult<()> {
        constraint.validate()?;
        self.constraints
            .insert(constraint.id().to_string(), constraint);
        self.validate()
    }

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

    fn collect_anchor_bindings(&self) -> CadResult<BTreeMap<String, AnchorBinding>> {
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
struct AnchorBinding {
    entity_id: String,
    plane_id: String,
    role: AnchorRole,
    position_mm: [f64; 2],
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

fn arc_point(center: [f64; 2], radius_mm: f64, angle_deg: f64) -> CadResult<[f64; 2]> {
    if !angle_deg.is_finite() {
        return Err(CadError::ParseFailed {
            reason: "arc angle must be finite".to_string(),
        });
    }
    let radians = angle_deg.to_radians();
    Ok([
        center[0] + radius_mm * radians.cos(),
        center[1] + radius_mm * radians.sin(),
    ])
}

fn tangent_residual_mm(
    line_start: [f64; 2],
    line_end: [f64; 2],
    arc_center: [f64; 2],
    arc_radius: f64,
    tolerance_mm: f64,
) -> CadResult<f64> {
    let dx = line_end[0] - line_start[0];
    let dy = line_end[1] - line_start[1];
    let denom = (dx * dx + dy * dy).sqrt();
    if denom <= tolerance_mm {
        return Err(CadError::ParseFailed {
            reason: "tangent constraint line is degenerate".to_string(),
        });
    }
    let numerator = (dy * arc_center[0] - dx * arc_center[1] + line_end[0] * line_start[1]
        - line_end[1] * line_start[0])
        .abs();
    let distance_to_line = numerator / denom;
    Ok((distance_to_line - arc_radius).abs())
}

fn distance_mm(a: [f64; 2], b: [f64; 2]) -> f64 {
    vector_length_mm([a[0] - b[0], a[1] - b[1]])
}

fn vector_length_mm(vector: [f64; 2]) -> f64 {
    (vector[0] * vector[0] + vector[1] * vector[1]).sqrt()
}

fn validate_tolerance_opt(value: Option<f64>, label: &str) -> CadResult<()> {
    if let Some(value) = value {
        if !value.is_finite() || value <= 0.0 {
            return Err(CadError::ParseFailed {
                reason: format!("{label} must be finite and > 0"),
            });
        }
    }
    Ok(())
}

fn validate_stable_id(value: &str, label: &str) -> CadResult<()> {
    if value.trim().is_empty() {
        return Err(CadError::ParseFailed {
            reason: format!("{label} must not be empty"),
        });
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    {
        return Err(CadError::ParseFailed {
            reason: format!(
                "{label} contains invalid characters: {value} (allowed: [A-Za-z0-9._-])"
            ),
        });
    }
    Ok(())
}

fn validate_vec2_finite(value: [f64; 2], label: &str) -> CadResult<()> {
    if value.iter().all(|component| component.is_finite()) {
        return Ok(());
    }
    Err(CadError::ParseFailed {
        reason: format!("{label} must contain finite values"),
    })
}

fn validate_vec3_finite(value: [f64; 3], label: &str) -> CadResult<()> {
    if value.iter().all(|component| component.is_finite()) {
        return Ok(());
    }
    Err(CadError::ParseFailed {
        reason: format!("{label} must contain finite values"),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CadDimensionConstraintKind, CadSketchConstraint, CadSketchEntity, CadSketchModel,
        CadSketchPlane, CadSketchSolveSeverity,
    };

    fn primary_plane() -> CadSketchPlane {
        CadSketchPlane {
            id: "plane.front".to_string(),
            name: "Front".to_string(),
            origin_mm: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
        }
    }

    #[test]
    fn sketch_model_serialization_is_deterministic_across_insertion_order() {
        let line = CadSketchEntity::Line {
            id: "entity.line.001".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [120.0, 0.0],
            anchor_ids: ["anchor.l.start".to_string(), "anchor.l.end".to_string()],
            construction: false,
        };
        let arc = CadSketchEntity::Arc {
            id: "entity.arc.001".to_string(),
            plane_id: "plane.front".to_string(),
            center_mm: [60.0, 40.0],
            radius_mm: 20.0,
            start_deg: 0.0,
            end_deg: 180.0,
            anchor_ids: [
                "anchor.a.center".to_string(),
                "anchor.a.start".to_string(),
                "anchor.a.end".to_string(),
            ],
            construction: false,
        };

        let mut left = CadSketchModel::default();
        left.insert_plane(primary_plane())
            .expect("left plane insert should succeed");
        left.insert_entity(line.clone())
            .expect("left line insert should succeed");
        left.insert_entity(arc.clone())
            .expect("left arc insert should succeed");

        let mut right = CadSketchModel::default();
        right
            .insert_plane(primary_plane())
            .expect("right plane insert should succeed");
        right
            .insert_entity(arc)
            .expect("right arc insert should succeed");
        right
            .insert_entity(line)
            .expect("right line insert should succeed");

        let left_json =
            serde_json::to_string(&left).expect("left model should serialize deterministically");
        let right_json =
            serde_json::to_string(&right).expect("right model should serialize deterministically");
        assert_eq!(left_json, right_json);
    }

    #[test]
    fn sketch_model_rejects_entities_that_reference_unknown_planes() {
        let mut model = CadSketchModel::default();
        let entity = CadSketchEntity::Point {
            id: "entity.point.001".to_string(),
            plane_id: "plane.missing".to_string(),
            position_mm: [5.0, 8.0],
            anchor_id: "anchor.p.001".to_string(),
            construction: true,
        };
        let result = model.insert_entity(entity);
        assert!(result.is_err(), "unknown plane reference must fail");
    }

    #[test]
    fn sketch_entity_validation_rejects_duplicate_anchor_ids() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(primary_plane())
            .expect("plane insert should succeed");
        let result = model.insert_entity(CadSketchEntity::Line {
            id: "entity.line.dup".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [50.0, 0.0],
            anchor_ids: ["anchor.dup".to_string(), "anchor.dup".to_string()],
            construction: false,
        });
        assert!(result.is_err(), "duplicate anchors must fail validation");
    }

    #[test]
    fn constraint_solver_solves_common_mvp_constraints_deterministically() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(primary_plane())
            .expect("plane should insert");

        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.line.edit".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [0.0, 0.0],
                end_mm: [20.0, 4.0],
                anchor_ids: [
                    "anchor.edit.start".to_string(),
                    "anchor.edit.end".to_string(),
                ],
                construction: false,
            })
            .expect("editable line should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.line.vertical".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [30.0, 0.0],
                end_mm: [35.0, 20.0],
                anchor_ids: [
                    "anchor.vert.start".to_string(),
                    "anchor.vert.end".to_string(),
                ],
                construction: false,
            })
            .expect("vertical line should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.line.tangent".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [0.0, 10.0],
                end_mm: [20.0, 10.0],
                anchor_ids: ["anchor.tan.start".to_string(), "anchor.tan.end".to_string()],
                construction: false,
            })
            .expect("tangent line should insert");
        model
            .insert_entity(CadSketchEntity::Point {
                id: "entity.point.coincident".to_string(),
                plane_id: "plane.front".to_string(),
                position_mm: [9.0, 7.0],
                anchor_id: "anchor.point.coincident".to_string(),
                construction: false,
            })
            .expect("coincident point should insert");
        model
            .insert_entity(CadSketchEntity::Arc {
                id: "entity.arc.dimension".to_string(),
                plane_id: "plane.front".to_string(),
                center_mm: [50.0, 20.0],
                radius_mm: 8.0,
                start_deg: 0.0,
                end_deg: 180.0,
                anchor_ids: [
                    "anchor.dim.center".to_string(),
                    "anchor.dim.start".to_string(),
                    "anchor.dim.end".to_string(),
                ],
                construction: false,
            })
            .expect("dimension arc should insert");
        model
            .insert_entity(CadSketchEntity::Arc {
                id: "entity.arc.tangent".to_string(),
                plane_id: "plane.front".to_string(),
                center_mm: [10.0, 0.0],
                radius_mm: 10.0,
                start_deg: 0.0,
                end_deg: 180.0,
                anchor_ids: [
                    "anchor.tan.center".to_string(),
                    "anchor.tan.arc_start".to_string(),
                    "anchor.tan.arc_end".to_string(),
                ],
                construction: false,
            })
            .expect("tangent arc should insert");

        model
            .insert_constraint(CadSketchConstraint::Horizontal {
                id: "constraint.horizontal.001".to_string(),
                line_entity_id: "entity.line.edit".to_string(),
            })
            .expect("horizontal constraint should insert");
        model
            .insert_constraint(CadSketchConstraint::Vertical {
                id: "constraint.vertical.001".to_string(),
                line_entity_id: "entity.line.vertical".to_string(),
            })
            .expect("vertical constraint should insert");
        model
            .insert_constraint(CadSketchConstraint::Coincident {
                id: "constraint.zz.coincident.001".to_string(),
                first_anchor_id: "anchor.edit.end".to_string(),
                second_anchor_id: "anchor.point.coincident".to_string(),
                tolerance_mm: Some(0.001),
            })
            .expect("coincident constraint should insert");
        model
            .insert_constraint(CadSketchConstraint::Dimension {
                id: "constraint.dimension.length.001".to_string(),
                entity_id: "entity.line.edit".to_string(),
                dimension_kind: CadDimensionConstraintKind::Length,
                target_mm: 30.0,
                tolerance_mm: Some(0.001),
            })
            .expect("length dimension should insert");
        model
            .insert_constraint(CadSketchConstraint::Dimension {
                id: "constraint.dimension.radius.001".to_string(),
                entity_id: "entity.arc.dimension".to_string(),
                dimension_kind: CadDimensionConstraintKind::Radius,
                target_mm: 12.0,
                tolerance_mm: Some(0.001),
            })
            .expect("radius dimension should insert");
        model
            .insert_constraint(CadSketchConstraint::Tangent {
                id: "constraint.tangent.001".to_string(),
                line_entity_id: "entity.line.tangent".to_string(),
                arc_entity_id: "entity.arc.tangent".to_string(),
                tolerance_mm: Some(0.001),
            })
            .expect("tangent constraint should insert");

        let report_first = model
            .solve_constraints_deterministic()
            .expect("solver should run");
        assert!(report_first.passed, "common scenario should solve");
        assert_eq!(report_first.unsolved_constraints, 0);
        assert_eq!(report_first.solved_constraints, 6);
        assert!(report_first.diagnostics.is_empty());

        let report_json_first =
            serde_json::to_string(&report_first).expect("solver report should serialize");
        let report_second = model
            .solve_constraints_deterministic()
            .expect("solver should stay deterministic across repeated runs");
        let report_json_second =
            serde_json::to_string(&report_second).expect("solver report should serialize");
        assert_eq!(
            report_json_first, report_json_second,
            "solver report must remain deterministic for same inputs"
        );

        match model
            .entities
            .get("entity.line.edit")
            .expect("line.edit should exist")
        {
            CadSketchEntity::Line {
                start_mm, end_mm, ..
            } => {
                assert!((start_mm[1] - end_mm[1]).abs() <= 0.001);
                let length =
                    ((end_mm[0] - start_mm[0]).powi(2) + (end_mm[1] - start_mm[1]).powi(2)).sqrt();
                assert!((length - 30.0).abs() <= 0.001);
            }
            _ => panic!("line.edit should remain a line"),
        }
        match model
            .entities
            .get("entity.line.vertical")
            .expect("line.vertical should exist")
        {
            CadSketchEntity::Line {
                start_mm, end_mm, ..
            } => {
                assert!((start_mm[0] - end_mm[0]).abs() <= 0.001);
            }
            _ => panic!("line.vertical should remain a line"),
        }
        match model
            .entities
            .get("entity.arc.dimension")
            .expect("arc.dimension should exist")
        {
            CadSketchEntity::Arc { radius_mm, .. } => {
                assert!((*radius_mm - 12.0).abs() <= 0.001);
            }
            _ => panic!("arc.dimension should remain an arc"),
        }
        let anchors = model
            .collect_anchor_bindings()
            .expect("anchors should remain resolvable");
        let line_end = anchors
            .get("anchor.edit.end")
            .expect("line end anchor should exist");
        let point = anchors
            .get("anchor.point.coincident")
            .expect("point anchor should exist");
        let delta_x = (line_end.position_mm[0] - point.position_mm[0]).abs();
        let delta_y = (line_end.position_mm[1] - point.position_mm[1]).abs();
        assert!(delta_x <= 0.001 && delta_y <= 0.001);
    }

    #[test]
    fn tangent_constraint_reports_diagnostic_when_unsolved() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(primary_plane())
            .expect("plane should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.line.diag".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [0.0, 0.0],
                end_mm: [10.0, 7.0],
                anchor_ids: [
                    "anchor.diag.start".to_string(),
                    "anchor.diag.end".to_string(),
                ],
                construction: false,
            })
            .expect("line should insert");
        model
            .insert_entity(CadSketchEntity::Arc {
                id: "entity.arc.unsolved".to_string(),
                plane_id: "plane.front".to_string(),
                center_mm: [20.0, 20.0],
                radius_mm: 5.0,
                start_deg: 0.0,
                end_deg: 180.0,
                anchor_ids: [
                    "anchor.unsolved.center".to_string(),
                    "anchor.unsolved.start".to_string(),
                    "anchor.unsolved.end".to_string(),
                ],
                construction: false,
            })
            .expect("arc should insert");
        model
            .insert_constraint(CadSketchConstraint::Tangent {
                id: "constraint.tangent.unsolved".to_string(),
                line_entity_id: "entity.line.diag".to_string(),
                arc_entity_id: "entity.arc.unsolved".to_string(),
                tolerance_mm: Some(0.001),
            })
            .expect("constraint should insert");

        let report = model
            .solve_constraints_deterministic()
            .expect("solver should run");
        assert!(!report.passed, "unsatisfied tangent should fail");
        assert_eq!(
            report.constraint_status.get("constraint.tangent.unsolved"),
            Some(&"unsolved".to_string())
        );
        assert!(report.diagnostics.iter().any(|entry| entry.code
            == "SKETCH_CONSTRAINT_TANGENT_UNSATISFIED"
            && entry.severity == CadSketchSolveSeverity::Error));
    }

    #[test]
    fn constraint_validation_rejects_unknown_entity_references() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(primary_plane())
            .expect("plane should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.line.known".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [0.0, 0.0],
                end_mm: [10.0, 0.0],
                anchor_ids: [
                    "anchor.known.start".to_string(),
                    "anchor.known.end".to_string(),
                ],
                construction: false,
            })
            .expect("known line should insert");

        let result = model.insert_constraint(CadSketchConstraint::Horizontal {
            id: "constraint.horizontal.bad".to_string(),
            line_entity_id: "entity.line.missing".to_string(),
        });
        assert!(
            result.is_err(),
            "constraint must fail when referencing unknown entity"
        );
    }
}

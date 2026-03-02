use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

/// Deterministic LM configuration used by the sketch solver.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadSketchLmConfig {
    pub max_iterations: u32,
    pub residual_tolerance_mm: f64,
    pub initial_lambda: f64,
    pub lambda_increase: f64,
    pub lambda_decrease: f64,
    pub min_lambda: f64,
    pub max_lambda: f64,
}

/// Deterministic LM Jacobian/residual summary aligned to vcad pipeline semantics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadSketchLmPipelineSummary {
    pub epsilon: f64,
    pub residual_component_count: usize,
    pub parameter_count: usize,
    pub residual_l2_norm: f64,
    pub residual_max_abs: f64,
    pub residual_hash: String,
    pub jacobian_nonzero_count: usize,
    pub jacobian_max_abs: f64,
    pub jacobian_rank: usize,
    pub jacobian_rank_deficient: bool,
    pub jacobian_has_non_finite: bool,
    pub jacobian_hash: String,
    pub constraint_component_counts: BTreeMap<String, usize>,
}

impl Default for CadSketchLmConfig {
    fn default() -> Self {
        Self {
            max_iterations: LM_MAX_ITERATIONS,
            residual_tolerance_mm: LM_RESIDUAL_TOLERANCE_MM,
            initial_lambda: LM_INITIAL_LAMBDA,
            lambda_increase: LM_LAMBDA_INCREASE,
            lambda_decrease: LM_LAMBDA_DECREASE,
            min_lambda: LM_MIN_LAMBDA,
            max_lambda: LM_MAX_LAMBDA,
        }
    }
}

const LM_MAX_ITERATIONS: u32 = 100;
const LM_RESIDUAL_TOLERANCE_MM: f64 = 1e-6;
const LM_INITIAL_LAMBDA: f64 = 1e-3;
const LM_LAMBDA_INCREASE: f64 = 10.0;
const LM_LAMBDA_DECREASE: f64 = 0.1;
const LM_MIN_LAMBDA: f64 = 1e-12;
const LM_MAX_LAMBDA: f64 = 1e12;
const LM_IMPROVEMENT_EPSILON: f64 = 1e-12;
const LM_JACOBIAN_EPSILON: f64 = 1e-8;
const LM_JACOBIAN_NONZERO_TOLERANCE: f64 = 1e-15;
const LM_JACOBIAN_RANK_TOLERANCE: f64 = 1e-9;

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

    /// Build deterministic Jacobian/residual summary for the current sketch model.
    pub fn lm_pipeline_summary(&self) -> CadResult<CadSketchLmPipelineSummary> {
        self.validate()?;

        let anchor_bindings = self.collect_anchor_bindings()?;
        let parameter_anchor_order: Vec<String> = anchor_bindings.keys().cloned().collect();
        let parameter_count = parameter_anchor_order.len().saturating_mul(2);

        let (residual_components, constraint_component_counts) =
            self.collect_lm_residual_vector()?;
        let residual_component_count = residual_components.len();
        let residual_l2_norm = residual_components
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        let residual_max_abs = residual_components
            .iter()
            .map(|value| value.abs())
            .fold(0.0, f64::max);
        let residual_hash = short_sha256_f64_slice(&residual_components);

        let mut jacobian = vec![vec![0.0; parameter_count]; residual_component_count];
        if residual_component_count > 0 && parameter_count > 0 {
            for column in 0..parameter_count {
                let anchor_id = &parameter_anchor_order[column / 2];
                let axis = column % 2;

                let mut plus_model = self.clone();
                plus_model.apply_anchor_parameter_delta(anchor_id, axis, LM_JACOBIAN_EPSILON)?;
                let (residual_plus, _) = plus_model.collect_lm_residual_vector()?;

                let mut minus_model = self.clone();
                minus_model.apply_anchor_parameter_delta(anchor_id, axis, -LM_JACOBIAN_EPSILON)?;
                let (residual_minus, _) = minus_model.collect_lm_residual_vector()?;

                if residual_plus.len() != residual_component_count
                    || residual_minus.len() != residual_component_count
                {
                    return Err(CadError::EvalFailed {
                        reason: format!(
                            "LM residual vector length changed during Jacobian evaluation for anchor {anchor_id}"
                        ),
                    });
                }

                for row in 0..residual_component_count {
                    jacobian[row][column] =
                        (residual_plus[row] - residual_minus[row]) / (2.0 * LM_JACOBIAN_EPSILON);
                }
            }
        }

        let mut jacobian_nonzero_count = 0usize;
        let mut jacobian_max_abs = 0.0f64;
        let mut jacobian_has_non_finite = false;
        for row in &jacobian {
            for value in row {
                if !value.is_finite() {
                    jacobian_has_non_finite = true;
                }
                let abs = value.abs();
                if abs > LM_JACOBIAN_NONZERO_TOLERANCE {
                    jacobian_nonzero_count = jacobian_nonzero_count.saturating_add(1);
                }
                jacobian_max_abs = jacobian_max_abs.max(abs);
            }
        }
        let jacobian_rank = estimate_matrix_rank(&jacobian, LM_JACOBIAN_RANK_TOLERANCE);
        let jacobian_rank_deficient = jacobian_rank < residual_component_count.min(parameter_count);
        let jacobian_hash = short_sha256_f64_matrix(&jacobian);

        Ok(CadSketchLmPipelineSummary {
            epsilon: LM_JACOBIAN_EPSILON,
            residual_component_count,
            parameter_count,
            residual_l2_norm,
            residual_max_abs,
            residual_hash,
            jacobian_nonzero_count,
            jacobian_max_abs,
            jacobian_rank,
            jacobian_rank_deficient,
            jacobian_has_non_finite,
            jacobian_hash,
            constraint_component_counts,
        })
    }

    /// Deterministic iterative LM-style solver pass over sketch constraints.
    pub fn solve_constraints_deterministic(&mut self) -> CadResult<CadSketchSolveReport> {
        self.validate()?;
        let config = CadSketchLmConfig::default();

        let mut lambda = config.initial_lambda;
        let mut best_report = None::<CadSketchSolveReport>;
        let mut best_norm = f64::INFINITY;
        let mut completed_iterations = 0u32;

        for iteration in 1..=config.max_iterations {
            let snapshot = self.clone();
            let iteration_outcome =
                self.solve_constraints_iteration(lambda, config.residual_tolerance_mm)?;
            completed_iterations = iteration;

            let improved = iteration_outcome.lm_residual_norm + LM_IMPROVEMENT_EPSILON < best_norm;
            if improved || best_report.is_none() {
                best_norm = iteration_outcome.lm_residual_norm;
                best_report = Some(iteration_outcome.report);
                lambda = (lambda * config.lambda_decrease).max(config.min_lambda);
            } else {
                *self = snapshot;
                lambda = (lambda * config.lambda_increase).min(config.max_lambda);
                if lambda >= config.max_lambda {
                    break;
                }
            }

            if best_norm <= config.residual_tolerance_mm && iteration >= 2 {
                break;
            }
        }

        let mut report = best_report.ok_or_else(|| CadError::EvalFailed {
            reason: "iterative sketch solver produced no report".to_string(),
        })?;
        report.iteration_count = completed_iterations.max(1);
        Ok(report)
    }

    fn solve_constraints_iteration(
        &mut self,
        lambda: f64,
        residual_tolerance_mm: f64,
    ) -> CadResult<LmIterationOutcome> {
        let step_scale = (1.0 / (1.0 + lambda)).clamp(0.0, 1.0);
        let mut diagnostics = Vec::<CadSketchSolveDiagnostic>::new();
        let mut constraint_status = BTreeMap::<String, String>::new();
        let mut residuals_mm = BTreeMap::<String, f64>::new();
        let mut solved_constraints = 0usize;
        let mut unsolved_constraints = 0usize;
        let mut lm_residual_sq = 0.0f64;

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
                    step_scale,
                ),
                CadSketchConstraint::Horizontal { line_entity_id, .. } => {
                    self.solve_horizontal(&constraint_id, &line_entity_id, step_scale)
                }
                CadSketchConstraint::Vertical { line_entity_id, .. } => {
                    self.solve_vertical(&constraint_id, &line_entity_id, step_scale)
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
                    step_scale,
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
                    step_scale,
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
                    step_scale,
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
                    step_scale,
                ),
                other => Ok(self.unsupported_constraint_outcome(&constraint_id, other.kind_key())),
            }?;

            residuals_mm.insert(constraint_id.clone(), outcome.residual_mm);
            if outcome.contributes_to_lm {
                lm_residual_sq += outcome.residual_mm * outcome.residual_mm;
            }
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

        match self.lm_pipeline_summary() {
            Ok(summary) => {
                if summary.jacobian_has_non_finite || !summary.residual_l2_norm.is_finite() {
                    diagnostics.push(CadSketchSolveDiagnostic {
                        code: "SKETCH_LM_PIPELINE_NON_FINITE".to_string(),
                        severity: CadSketchSolveSeverity::Error,
                        constraint_id: "lm.pipeline".to_string(),
                        message: "LM Jacobian/residual pipeline produced non-finite values"
                            .to_string(),
                        remediation_hint:
                            "check sketch geometry for invalid/degenerate parameters and re-solve"
                                .to_string(),
                    });
                }
                if unsolved_constraints > 0
                    && summary.jacobian_rank_deficient
                    && summary.residual_l2_norm > residual_tolerance_mm
                {
                    diagnostics.push(CadSketchSolveDiagnostic {
                        code: "SKETCH_LM_JACOBIAN_RANK_DEFICIENT".to_string(),
                        severity: CadSketchSolveSeverity::Warning,
                        constraint_id: "lm.pipeline".to_string(),
                        message: format!(
                            "LM Jacobian rank {} is below expected full rank for rows={} cols={}",
                            summary.jacobian_rank,
                            summary.residual_component_count,
                            summary.parameter_count
                        ),
                        remediation_hint:
                            "remove conflicting/redundant constraints or add driving dimensions"
                                .to_string(),
                    });
                }
            }
            Err(error) => {
                diagnostics.push(CadSketchSolveDiagnostic {
                    code: "SKETCH_LM_PIPELINE_BUILD_FAILED".to_string(),
                    severity: CadSketchSolveSeverity::Warning,
                    constraint_id: "lm.pipeline".to_string(),
                    message: format!("LM Jacobian/residual pipeline snapshot failed: {}", error),
                    remediation_hint:
                        "verify constraint references and geometric validity before solving"
                            .to_string(),
                })
            }
        }

        let passed = unsolved_constraints == 0
            && diagnostics
                .iter()
                .all(|entry| entry.severity != CadSketchSolveSeverity::Error);
        Ok(LmIterationOutcome {
            report: CadSketchSolveReport {
                passed,
                iteration_count: 1,
                solved_constraints,
                unsolved_constraints,
                constraint_status,
                residuals_mm,
                diagnostics,
            },
            lm_residual_norm: lm_residual_sq.sqrt(),
        })
    }

    fn solve_coincident(
        &mut self,
        constraint_id: &str,
        first_anchor_id: &str,
        second_anchor_id: &str,
        tolerance_mm: f64,
        step_scale: f64,
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
            self.apply_anchor_position(
                &second,
                interpolate_point(second.position_mm, first.position_mm, step_scale),
            )?;
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
            contributes_to_lm: true,
            diagnostic,
        })
    }

    fn solve_horizontal(
        &mut self,
        constraint_id: &str,
        line_entity_id: &str,
        step_scale: f64,
    ) -> CadResult<ConstraintSolveOutcome> {
        let (start_before, mut end_before) = self.line_endpoints(line_entity_id)?;
        end_before[1] = interpolate_scalar(end_before[1], start_before[1], step_scale);
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
            contributes_to_lm: true,
            diagnostic,
        })
    }

    fn solve_vertical(
        &mut self,
        constraint_id: &str,
        line_entity_id: &str,
        step_scale: f64,
    ) -> CadResult<ConstraintSolveOutcome> {
        let (start_before, mut end_before) = self.line_endpoints(line_entity_id)?;
        end_before[0] = interpolate_scalar(end_before[0], start_before[0], step_scale);
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
            contributes_to_lm: true,
            diagnostic,
        })
    }

    fn solve_tangent(
        &mut self,
        constraint_id: &str,
        line_entity_id: &str,
        arc_entity_id: &str,
        tolerance_mm: f64,
        step_scale: f64,
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
                    [
                        line_start[0],
                        interpolate_scalar(line_start[1], target_y, step_scale),
                    ],
                    [
                        line_end[0],
                        interpolate_scalar(line_end[1], target_y, step_scale),
                    ],
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
                    [
                        interpolate_scalar(line_start[0], target_x, step_scale),
                        line_start[1],
                    ],
                    [
                        interpolate_scalar(line_end[0], target_x, step_scale),
                        line_end[1],
                    ],
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
            contributes_to_lm: true,
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
        step_scale: f64,
    ) -> CadResult<ConstraintSolveOutcome> {
        match dimension_kind {
            CadDimensionConstraintKind::Length => {
                let (start, mut end) = self.line_endpoints(entity_id)?;
                let vector = [end[0] - start[0], end[1] - start[1]];
                let length = vector_length_mm(vector);
                if length <= 0.000_001 {
                    end = interpolate_point(end, [start[0] + target_mm, start[1]], step_scale);
                } else {
                    let scale = target_mm / length;
                    end = interpolate_point(
                        end,
                        [start[0] + vector[0] * scale, start[1] + vector[1] * scale],
                        step_scale,
                    );
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
                    contributes_to_lm: true,
                    diagnostic,
                })
            }
            CadDimensionConstraintKind::Radius => {
                let (_center_before, radius_before) = self.curve_center_radius(entity_id)?;
                let next_radius = interpolate_scalar(radius_before, target_mm, step_scale);
                self.set_curve_radius(entity_id, next_radius)?;
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
                    contributes_to_lm: true,
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
            contributes_to_lm: false,
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

    fn apply_anchor_parameter_delta(
        &mut self,
        anchor_id: &str,
        axis: usize,
        delta: f64,
    ) -> CadResult<()> {
        if axis > 1 {
            return Err(CadError::ParseFailed {
                reason: format!("invalid LM parameter axis {axis} for anchor {anchor_id}"),
            });
        }
        let bindings = self.collect_anchor_bindings()?;
        let binding = bindings
            .get(anchor_id)
            .ok_or_else(|| CadError::ParseFailed {
                reason: format!("missing anchor {anchor_id} while perturbing LM parameter"),
            })?;
        let mut position_mm = binding.position_mm;
        position_mm[axis] += delta;
        self.apply_anchor_position(binding, position_mm)
    }

    fn collect_lm_residual_vector(&self) -> CadResult<(Vec<f64>, BTreeMap<String, usize>)> {
        let anchor_bindings = self.collect_anchor_bindings()?;
        let mut residual_components = Vec::<f64>::new();
        let mut constraint_component_counts = BTreeMap::<String, usize>::new();
        for (constraint_id, constraint) in &self.constraints {
            let components = self.constraint_residual_components(constraint, &anchor_bindings)?;
            constraint_component_counts.insert(constraint_id.clone(), components.len());
            residual_components.extend(components);
        }
        Ok((residual_components, constraint_component_counts))
    }

    fn constraint_residual_components(
        &self,
        constraint: &CadSketchConstraint,
        anchor_bindings: &BTreeMap<String, AnchorBinding>,
    ) -> CadResult<Vec<f64>> {
        let constraint_id = constraint.id();
        match constraint {
            CadSketchConstraint::Coincident {
                first_anchor_id,
                second_anchor_id,
                ..
            } => {
                let first = anchor_position_for_constraint(
                    anchor_bindings,
                    first_anchor_id,
                    constraint_id,
                )?;
                let second = anchor_position_for_constraint(
                    anchor_bindings,
                    second_anchor_id,
                    constraint_id,
                )?;
                Ok(vec![first[0] - second[0], first[1] - second[1]])
            }
            CadSketchConstraint::PointOnLine {
                point_anchor_id,
                line_entity_id,
                ..
            } => {
                let point = anchor_position_for_constraint(
                    anchor_bindings,
                    point_anchor_id,
                    constraint_id,
                )?;
                let (start, end) = self.line_endpoints(line_entity_id)?;
                Ok(vec![signed_distance_to_line(point, start, end)])
            }
            CadSketchConstraint::Parallel {
                first_line_entity_id,
                second_line_entity_id,
                ..
            } => {
                let (first_start, first_end) = self.line_endpoints(first_line_entity_id)?;
                let (second_start, second_end) = self.line_endpoints(second_line_entity_id)?;
                let first_vector = [first_end[0] - first_start[0], first_end[1] - first_start[1]];
                let second_vector = [
                    second_end[0] - second_start[0],
                    second_end[1] - second_start[1],
                ];
                let first_len = vector_length_mm(first_vector);
                let second_len = vector_length_mm(second_vector);
                if first_len < 1e-15 || second_len < 1e-15 {
                    return Ok(vec![0.0]);
                }
                let cross = (first_vector[0] * second_vector[1]
                    - first_vector[1] * second_vector[0])
                    / (first_len * second_len);
                Ok(vec![cross])
            }
            CadSketchConstraint::Perpendicular {
                first_line_entity_id,
                second_line_entity_id,
                ..
            } => {
                let (first_start, first_end) = self.line_endpoints(first_line_entity_id)?;
                let (second_start, second_end) = self.line_endpoints(second_line_entity_id)?;
                let first_vector = [first_end[0] - first_start[0], first_end[1] - first_start[1]];
                let second_vector = [
                    second_end[0] - second_start[0],
                    second_end[1] - second_start[1],
                ];
                let first_len = vector_length_mm(first_vector);
                let second_len = vector_length_mm(second_vector);
                if first_len < 1e-15 || second_len < 1e-15 {
                    return Ok(vec![0.0]);
                }
                let dot = (first_vector[0] * second_vector[0] + first_vector[1] * second_vector[1])
                    / (first_len * second_len);
                Ok(vec![dot])
            }
            CadSketchConstraint::Horizontal { line_entity_id, .. } => {
                let (start, end) = self.line_endpoints(line_entity_id)?;
                Ok(vec![end[1] - start[1]])
            }
            CadSketchConstraint::Vertical { line_entity_id, .. } => {
                let (start, end) = self.line_endpoints(line_entity_id)?;
                Ok(vec![end[0] - start[0]])
            }
            CadSketchConstraint::Tangent {
                line_entity_id,
                arc_entity_id,
                at_anchor_id,
                ..
            } => {
                let (line_start, line_end) = self.line_endpoints(line_entity_id)?;
                if let Some(anchor_id) = at_anchor_id {
                    let tangent_point =
                        anchor_position_for_constraint(anchor_bindings, anchor_id, constraint_id)?;
                    let (center, _radius) = self.curve_center_radius(arc_entity_id)?;
                    let line_vector = [line_end[0] - line_start[0], line_end[1] - line_start[1]];
                    let radius_vector =
                        [tangent_point[0] - center[0], tangent_point[1] - center[1]];
                    let line_len = vector_length_mm(line_vector);
                    let radius_len = vector_length_mm(radius_vector);
                    if line_len < 1e-15 || radius_len < 1e-15 {
                        return Ok(vec![0.0]);
                    }
                    let dot = (line_vector[0] * radius_vector[0]
                        + line_vector[1] * radius_vector[1])
                        / (line_len * radius_len);
                    return Ok(vec![dot]);
                }
                let (center, radius) = self.curve_center_radius(arc_entity_id)?;
                Ok(vec![tangent_residual_mm(
                    line_start, line_end, center, radius, 0.0,
                )?])
            }
            CadSketchConstraint::EqualLength {
                first_line_entity_id,
                second_line_entity_id,
                ..
            } => {
                let (first_start, first_end) = self.line_endpoints(first_line_entity_id)?;
                let (second_start, second_end) = self.line_endpoints(second_line_entity_id)?;
                let first_len = vector_length_mm([
                    first_end[0] - first_start[0],
                    first_end[1] - first_start[1],
                ]);
                let second_len = vector_length_mm([
                    second_end[0] - second_start[0],
                    second_end[1] - second_start[1],
                ]);
                Ok(vec![first_len - second_len])
            }
            CadSketchConstraint::EqualRadius {
                first_curve_entity_id,
                second_curve_entity_id,
                ..
            } => {
                let (_, first_radius) = self.curve_center_radius(first_curve_entity_id)?;
                let (_, second_radius) = self.curve_center_radius(second_curve_entity_id)?;
                Ok(vec![first_radius - second_radius])
            }
            CadSketchConstraint::Concentric {
                first_curve_entity_id,
                second_curve_entity_id,
                ..
            } => {
                let (first_center, _) = self.curve_center_radius(first_curve_entity_id)?;
                let (second_center, _) = self.curve_center_radius(second_curve_entity_id)?;
                Ok(vec![
                    first_center[0] - second_center[0],
                    first_center[1] - second_center[1],
                ])
            }
            CadSketchConstraint::Fixed {
                point_anchor_id,
                target_mm,
                ..
            } => {
                let point = anchor_position_for_constraint(
                    anchor_bindings,
                    point_anchor_id,
                    constraint_id,
                )?;
                Ok(vec![point[0] - target_mm[0], point[1] - target_mm[1]])
            }
            CadSketchConstraint::PointOnCircle {
                point_anchor_id,
                circle_entity_id,
                ..
            } => {
                let point = anchor_position_for_constraint(
                    anchor_bindings,
                    point_anchor_id,
                    constraint_id,
                )?;
                let (center, radius) = self.curve_center_radius(circle_entity_id)?;
                Ok(vec![distance_mm(point, center) - radius])
            }
            CadSketchConstraint::LineThroughCenter {
                line_entity_id,
                circle_entity_id,
                ..
            } => {
                let (line_start, line_end) = self.line_endpoints(line_entity_id)?;
                let (center, _) = self.curve_center_radius(circle_entity_id)?;
                Ok(vec![signed_distance_to_line(center, line_start, line_end)])
            }
            CadSketchConstraint::Midpoint {
                midpoint_anchor_id,
                line_entity_id,
                ..
            } => {
                let point = anchor_position_for_constraint(
                    anchor_bindings,
                    midpoint_anchor_id,
                    constraint_id,
                )?;
                let (line_start, line_end) = self.line_endpoints(line_entity_id)?;
                let midpoint = [
                    (line_start[0] + line_end[0]) / 2.0,
                    (line_start[1] + line_end[1]) / 2.0,
                ];
                Ok(vec![point[0] - midpoint[0], point[1] - midpoint[1]])
            }
            CadSketchConstraint::Symmetric {
                first_anchor_id,
                second_anchor_id,
                axis_line_entity_id,
                ..
            } => {
                let first = anchor_position_for_constraint(
                    anchor_bindings,
                    first_anchor_id,
                    constraint_id,
                )?;
                let second = anchor_position_for_constraint(
                    anchor_bindings,
                    second_anchor_id,
                    constraint_id,
                )?;
                let (axis_start, axis_end) = self.line_endpoints(axis_line_entity_id)?;
                let midpoint = [(first[0] + second[0]) / 2.0, (first[1] + second[1]) / 2.0];
                let dist_to_axis = signed_distance_to_line(midpoint, axis_start, axis_end);
                let axis_vector = [axis_end[0] - axis_start[0], axis_end[1] - axis_start[1]];
                let axis_len = vector_length_mm(axis_vector);
                if axis_len < 1e-15 {
                    return Ok(vec![0.0, 0.0]);
                }
                let ab_vector = [second[0] - first[0], second[1] - first[1]];
                let ab_len = vector_length_mm(ab_vector);
                if ab_len < 1e-15 {
                    return Ok(vec![dist_to_axis, 0.0]);
                }
                let perpendicular = (ab_vector[0] * axis_vector[0] + ab_vector[1] * axis_vector[1])
                    / (ab_len * axis_len);
                Ok(vec![dist_to_axis, perpendicular])
            }
            CadSketchConstraint::Distance {
                first_anchor_id,
                second_anchor_id,
                target_mm,
                ..
            } => {
                let first = anchor_position_for_constraint(
                    anchor_bindings,
                    first_anchor_id,
                    constraint_id,
                )?;
                let second = anchor_position_for_constraint(
                    anchor_bindings,
                    second_anchor_id,
                    constraint_id,
                )?;
                Ok(vec![distance_mm(first, second) - *target_mm])
            }
            CadSketchConstraint::PointLineDistance {
                point_anchor_id,
                line_entity_id,
                target_mm,
                ..
            } => {
                let point = anchor_position_for_constraint(
                    anchor_bindings,
                    point_anchor_id,
                    constraint_id,
                )?;
                let (line_start, line_end) = self.line_endpoints(line_entity_id)?;
                Ok(vec![
                    signed_distance_to_line(point, line_start, line_end).abs() - *target_mm,
                ])
            }
            CadSketchConstraint::Angle {
                first_line_entity_id,
                second_line_entity_id,
                target_deg,
                ..
            } => {
                let (first_start, first_end) = self.line_endpoints(first_line_entity_id)?;
                let (second_start, second_end) = self.line_endpoints(second_line_entity_id)?;
                let first_vector = [first_end[0] - first_start[0], first_end[1] - first_start[1]];
                let second_vector = [
                    second_end[0] - second_start[0],
                    second_end[1] - second_start[1],
                ];
                let first_len = vector_length_mm(first_vector);
                let second_len = vector_length_mm(second_vector);
                if first_len < 1e-15 || second_len < 1e-15 {
                    return Ok(vec![0.0]);
                }
                let cos_angle = (first_vector[0] * second_vector[0]
                    + first_vector[1] * second_vector[1])
                    / (first_len * second_len);
                let sin_angle = (first_vector[0] * second_vector[1]
                    - first_vector[1] * second_vector[0])
                    / (first_len * second_len);
                let actual_angle = sin_angle.atan2(cos_angle);
                let target_angle = target_deg.to_radians();
                Ok(vec![normalize_angle_delta_radians(
                    actual_angle - target_angle,
                )])
            }
            CadSketchConstraint::Radius {
                curve_entity_id,
                target_mm,
                ..
            } => {
                let (_, radius) = self.curve_center_radius(curve_entity_id)?;
                Ok(vec![radius - *target_mm])
            }
            CadSketchConstraint::Length {
                line_entity_id,
                target_mm,
                ..
            } => {
                let (start, end) = self.line_endpoints(line_entity_id)?;
                let length = vector_length_mm([end[0] - start[0], end[1] - start[1]]);
                Ok(vec![length - *target_mm])
            }
            CadSketchConstraint::HorizontalDistance {
                point_anchor_id,
                target_mm,
                ..
            } => {
                let point = anchor_position_for_constraint(
                    anchor_bindings,
                    point_anchor_id,
                    constraint_id,
                )?;
                Ok(vec![point[0] - *target_mm])
            }
            CadSketchConstraint::VerticalDistance {
                point_anchor_id,
                target_mm,
                ..
            } => {
                let point = anchor_position_for_constraint(
                    anchor_bindings,
                    point_anchor_id,
                    constraint_id,
                )?;
                Ok(vec![point[1] - *target_mm])
            }
            CadSketchConstraint::Diameter {
                circle_entity_id,
                target_mm,
                ..
            } => {
                let (_, radius) = self.curve_center_radius(circle_entity_id)?;
                Ok(vec![2.0 * radius - *target_mm])
            }
            CadSketchConstraint::Dimension {
                entity_id,
                dimension_kind,
                target_mm,
                ..
            } => match dimension_kind {
                CadDimensionConstraintKind::Length => {
                    let (start, end) = self.line_endpoints(entity_id)?;
                    let length = vector_length_mm([end[0] - start[0], end[1] - start[1]]);
                    Ok(vec![length - *target_mm])
                }
                CadDimensionConstraintKind::Radius => {
                    let (_, radius) = self.curve_center_radius(entity_id)?;
                    Ok(vec![radius - *target_mm])
                }
            },
        }
    }
}

fn anchor_position_for_constraint(
    bindings: &BTreeMap<String, AnchorBinding>,
    anchor_id: &str,
    constraint_id: &str,
) -> CadResult<[f64; 2]> {
    bindings
        .get(anchor_id)
        .map(|binding| binding.position_mm)
        .ok_or_else(|| CadError::ParseFailed {
            reason: format!("constraint {constraint_id} references unknown anchor {anchor_id}"),
        })
}

fn signed_distance_to_line(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = vector_length_mm([dx, dy]);
    if length < 1e-15 {
        return 0.0;
    }
    ((point[0] - start[0]) * dy - (point[1] - start[1]) * dx) / length
}

fn normalize_angle_delta_radians(mut delta: f64) -> f64 {
    while delta > std::f64::consts::PI {
        delta -= 2.0 * std::f64::consts::PI;
    }
    while delta < -std::f64::consts::PI {
        delta += 2.0 * std::f64::consts::PI;
    }
    delta
}

fn estimate_matrix_rank(matrix: &[Vec<f64>], tolerance: f64) -> usize {
    if matrix.is_empty() || matrix[0].is_empty() {
        return 0;
    }

    let mut work = matrix.to_vec();
    let row_count = work.len();
    let column_count = work[0].len();
    let mut rank = 0usize;
    let mut pivot_column = 0usize;

    while rank < row_count && pivot_column < column_count {
        let mut pivot_row = rank;
        let mut pivot_abs = work[pivot_row][pivot_column].abs();
        for row in (rank + 1)..row_count {
            let candidate_abs = work[row][pivot_column].abs();
            if candidate_abs > pivot_abs {
                pivot_abs = candidate_abs;
                pivot_row = row;
            }
        }

        if pivot_abs <= tolerance {
            pivot_column += 1;
            continue;
        }

        work.swap(rank, pivot_row);
        let pivot = work[rank][pivot_column];
        for row in (rank + 1)..row_count {
            let factor = work[row][pivot_column] / pivot;
            if factor.abs() <= tolerance {
                continue;
            }
            for column in pivot_column..column_count {
                work[row][column] -= factor * work[rank][column];
            }
        }
        rank += 1;
        pivot_column += 1;
    }

    rank
}

fn short_sha256_f64_slice(values: &[f64]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(values.len().to_le_bytes());
    for value in values {
        hasher.update(value.to_bits().to_le_bytes());
    }
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn short_sha256_f64_matrix(rows: &[Vec<f64>]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(rows.len().to_le_bytes());
    hasher.update(
        rows.first()
            .map(|row| row.len())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    for row in rows {
        hasher.update(row.len().to_le_bytes());
        for value in row {
            hasher.update(value.to_bits().to_le_bytes());
        }
    }
    format!("{:x}", hasher.finalize())[..16].to_string()
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

fn interpolate_scalar(current: f64, target: f64, step_scale: f64) -> f64 {
    let scale = step_scale.clamp(0.0, 1.0);
    current + (target - current) * scale
}

fn interpolate_point(current: [f64; 2], target: [f64; 2], step_scale: f64) -> [f64; 2] {
    [
        interpolate_scalar(current[0], target[0], step_scale),
        interpolate_scalar(current[1], target[1], step_scale),
    ]
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
    contributes_to_lm: bool,
    diagnostic: Option<CadSketchSolveDiagnostic>,
}

#[derive(Clone, Debug)]
struct LmIterationOutcome {
    report: CadSketchSolveReport,
    lm_residual_norm: f64,
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

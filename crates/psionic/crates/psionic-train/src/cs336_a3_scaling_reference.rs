use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable lane identifier for the bounded CS336 A3 scaling reference lane.
pub const CS336_A3_REFERENCE_LANE_ID: &str = "psion_cs336_a3_scaling_reference_v1";
/// Claim boundary for the bounded CS336 A3 scaling reference lane.
pub const CS336_A3_REFERENCE_CLAIM_BOUNDARY: &str = "bounded deterministic IsoFLOP analysis \
     math over supplied sweep cells only; no training runs, no dispatch authority, and no claim \
     of fitted-law validity beyond the committed synthetic recovery test";

/// FLOPs-per-parameter-token constant in the C = 6 N D accounting.
pub const CS336_A3_FLOPS_PER_PARAMETER_TOKEN: f64 = 6.0;

/// One completed sweep cell.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A3SweepCell {
    /// Model parameter count N.
    pub model_parameters: f64,
    /// Training tokens D.
    pub training_tokens: f64,
    /// Compute budget C, normally 6 N D.
    pub compute_flops: f64,
    /// Final validation loss.
    pub final_loss: f64,
}

/// One planned sweep cell awaiting execution.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A3PlannedRun {
    /// Compute budget C for this cell.
    pub compute_flops: f64,
    /// Model parameter count N for this cell.
    pub model_parameters: f64,
    /// Training tokens D = C / (6 N) for this cell.
    pub training_tokens: f64,
}

/// One per-budget parabola minimum.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A3BudgetOptimum {
    /// Compute budget C.
    pub compute_flops: f64,
    /// Optimal model parameter count at this budget.
    pub optimal_parameters: f64,
    /// Fitted minimum loss at the optimum.
    pub fitted_loss: f64,
    /// Cells contributing to this budget's fit.
    pub cell_count: usize,
}

/// One fitted IsoFLOP scaling report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A3IsoflopFit {
    /// Lane identifier.
    pub lane_id: String,
    /// Per-budget optima, ascending by budget.
    pub budget_optima: Vec<Cs336A3BudgetOptimum>,
    /// Fitted exponent `a` in `N_opt = k * C^a`.
    pub parameter_exponent: f64,
    /// Fitted coefficient `k` in `N_opt = k * C^a`.
    pub parameter_coefficient: f64,
    /// Implied token exponent `b = 1 - a` in `D_opt ∝ C^b`.
    pub token_exponent: f64,
}

impl Cs336A3IsoflopFit {
    /// Predicts the compute-optimal `(N, D)` for one budget.
    #[must_use]
    pub fn predict_optimal(&self, compute_flops: f64) -> Cs336A3PlannedRun {
        let optimal_parameters =
            self.parameter_coefficient * compute_flops.powf(self.parameter_exponent);
        Cs336A3PlannedRun {
            compute_flops,
            model_parameters: optimal_parameters,
            training_tokens: compute_flops
                / (CS336_A3_FLOPS_PER_PARAMETER_TOKEN * optimal_parameters),
        }
    }

    /// Returns a stable digest over the fit encoding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cs336_a3_isoflop_fit|");
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        hex::encode(hasher.finalize())
    }
}

/// Failure for one bounded A3 scaling computation.
#[derive(Debug, Error, PartialEq)]
pub enum Cs336A3ScalingError {
    /// A budget has too few cells for a parabola fit.
    #[error("budget {budget:e} has {found} cells; at least 3 required")]
    InsufficientCells {
        /// Offending budget.
        budget: f64,
        /// Cells found.
        found: usize,
    },
    /// A budget's parabola has no interior minimum.
    #[error("budget {budget:e} parabola has no interior minimum")]
    NoInteriorMinimum {
        /// Offending budget.
        budget: f64,
    },
    /// Fewer than two budgets are available for the power-law fit.
    #[error("{found} budgets found; at least 2 required for the power-law fit")]
    InsufficientBudgets {
        /// Budgets found.
        found: usize,
    },
    /// One cell carries a non-positive quantity.
    #[error("cell has non-positive parameters, tokens, compute, or loss inputs")]
    NonPositiveCell,
    /// One planner bound is invalid.
    #[error("invalid planner bound `{parameter}`")]
    InvalidPlannerBound {
        /// Offending parameter name.
        parameter: &'static str,
    },
}

/// Plans one IsoFLOP sweep grid: geometric N spacing per budget with
/// `D = C / (6 N)`.
pub fn cs336_a3_plan_isoflop_sweep(
    budgets: &[f64],
    cells_per_budget: usize,
    parameters_min: f64,
    parameters_max: f64,
) -> Result<Vec<Cs336A3PlannedRun>, Cs336A3ScalingError> {
    if budgets.is_empty() || budgets.iter().any(|budget| *budget <= 0.0) {
        return Err(Cs336A3ScalingError::InvalidPlannerBound {
            parameter: "budgets",
        });
    }
    if cells_per_budget < 3 {
        return Err(Cs336A3ScalingError::InvalidPlannerBound {
            parameter: "cells_per_budget",
        });
    }
    if parameters_min <= 0.0 || parameters_max <= parameters_min {
        return Err(Cs336A3ScalingError::InvalidPlannerBound {
            parameter: "parameters_range",
        });
    }
    let log_min = parameters_min.ln();
    let log_max = parameters_max.ln();
    let mut runs = Vec::with_capacity(budgets.len() * cells_per_budget);
    for budget in budgets {
        for index in 0..cells_per_budget {
            let fraction = index as f64 / (cells_per_budget - 1) as f64;
            let model_parameters = (log_min + fraction * (log_max - log_min)).exp();
            runs.push(Cs336A3PlannedRun {
                compute_flops: *budget,
                model_parameters,
                training_tokens: budget / (CS336_A3_FLOPS_PER_PARAMETER_TOKEN * model_parameters),
            });
        }
    }
    Ok(runs)
}

/// Least-squares quadratic fit `y = c2 x^2 + c1 x + c0` over `(x, y)` pairs.
fn quadratic_fit(points: &[(f64, f64)]) -> Option<(f64, f64, f64)> {
    let n = points.len() as f64;
    let (mut sx, mut sx2, mut sx3, mut sx4) = (0.0, 0.0, 0.0, 0.0);
    let (mut sy, mut sxy, mut sx2y) = (0.0, 0.0, 0.0);
    for (x, y) in points {
        let x2 = x * x;
        sx += x;
        sx2 += x2;
        sx3 += x2 * x;
        sx4 += x2 * x2;
        sy += y;
        sxy += x * y;
        sx2y += x2 * y;
    }
    // Normal equations: [[n, sx, sx2], [sx, sx2, sx3], [sx2, sx3, sx4]]
    // * [c0, c1, c2] = [sy, sxy, sx2y]. Solve by Gaussian elimination.
    let mut matrix = [[n, sx, sx2, sy], [sx, sx2, sx3, sxy], [sx2, sx3, sx4, sx2y]];
    for pivot in 0..3 {
        let mut best = pivot;
        for row in (pivot + 1)..3 {
            if matrix[row][pivot].abs() > matrix[best][pivot].abs() {
                best = row;
            }
        }
        matrix.swap(pivot, best);
        if matrix[pivot][pivot].abs() < 1e-12 {
            return None;
        }
        for row in (pivot + 1)..3 {
            let factor = matrix[row][pivot] / matrix[pivot][pivot];
            for column in pivot..4 {
                matrix[row][column] -= factor * matrix[pivot][column];
            }
        }
    }
    let c2 = matrix[2][3] / matrix[2][2];
    let c1 = (matrix[1][3] - matrix[1][2] * c2) / matrix[1][1];
    let c0 = (matrix[0][3] - matrix[0][2] * c2 - matrix[0][1] * c1) / matrix[0][0];
    Some((c0, c1, c2))
}

/// Fits IsoFLOP scaling laws from completed sweep cells.
pub fn cs336_a3_fit_isoflop(
    cells: &[Cs336A3SweepCell],
) -> Result<Cs336A3IsoflopFit, Cs336A3ScalingError> {
    for cell in cells {
        if cell.model_parameters <= 0.0
            || cell.training_tokens <= 0.0
            || cell.compute_flops <= 0.0
            || !cell.final_loss.is_finite()
        {
            return Err(Cs336A3ScalingError::NonPositiveCell);
        }
    }
    // Group by budget through a stable decimal key to tolerate f64 identity.
    let mut by_budget: BTreeMap<String, Vec<&Cs336A3SweepCell>> = BTreeMap::new();
    for cell in cells {
        by_budget
            .entry(format!("{:.6e}", cell.compute_flops))
            .or_default()
            .push(cell);
    }
    if by_budget.len() < 2 {
        return Err(Cs336A3ScalingError::InsufficientBudgets {
            found: by_budget.len(),
        });
    }
    let mut budget_optima: Vec<Cs336A3BudgetOptimum> = Vec::with_capacity(by_budget.len());
    for group in by_budget.values() {
        let budget = group[0].compute_flops;
        if group.len() < 3 {
            return Err(Cs336A3ScalingError::InsufficientCells {
                budget,
                found: group.len(),
            });
        }
        let points: Vec<(f64, f64)> = group
            .iter()
            .map(|cell| (cell.model_parameters.ln(), cell.final_loss))
            .collect();
        let Some((c0, c1, c2)) = quadratic_fit(&points) else {
            return Err(Cs336A3ScalingError::NoInteriorMinimum { budget });
        };
        if c2 <= 0.0 {
            return Err(Cs336A3ScalingError::NoInteriorMinimum { budget });
        }
        let log_optimum = -c1 / (2.0 * c2);
        let fitted_loss = c0 + c1 * log_optimum + c2 * log_optimum * log_optimum;
        budget_optima.push(Cs336A3BudgetOptimum {
            compute_flops: budget,
            optimal_parameters: log_optimum.exp(),
            fitted_loss,
            cell_count: group.len(),
        });
    }
    budget_optima.sort_by(|a, b| {
        a.compute_flops
            .partial_cmp(&b.compute_flops)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // Log-log linear regression of optimal N against budget C.
    let n = budget_optima.len() as f64;
    let (mut sx, mut sy, mut sxy, mut sx2) = (0.0, 0.0, 0.0, 0.0);
    for optimum in &budget_optima {
        let x = optimum.compute_flops.ln();
        let y = optimum.optimal_parameters.ln();
        sx += x;
        sy += y;
        sxy += x * y;
        sx2 += x * x;
    }
    let denominator = n * sx2 - sx * sx;
    if denominator.abs() < 1e-12 {
        return Err(Cs336A3ScalingError::InsufficientBudgets {
            found: budget_optima.len(),
        });
    }
    let parameter_exponent = (n * sxy - sx * sy) / denominator;
    let intercept = (sy - parameter_exponent * sx) / n;
    Ok(Cs336A3IsoflopFit {
        lane_id: CS336_A3_REFERENCE_LANE_ID.to_string(),
        budget_optima,
        parameter_exponent,
        parameter_coefficient: intercept.exp(),
        token_exponent: 1.0 - parameter_exponent,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// Chinchilla-form synthetic loss with alpha = beta = 0.5, for which the
    /// analytic compute-optimal exponent is a = beta / (alpha + beta) = 0.5.
    fn synthetic_loss(model_parameters: f64, training_tokens: f64) -> f64 {
        1.69 + 406.4 / model_parameters.powf(0.5) + 410.7 / training_tokens.powf(0.5)
    }

    #[test]
    fn planner_builds_geometric_isoflop_grids() {
        let runs = cs336_a3_plan_isoflop_sweep(&[6e8, 6e10], 5, 1e3, 1e5).expect("plans");
        assert_eq!(runs.len(), 10);
        for run in &runs {
            let reconstructed =
                CS336_A3_FLOPS_PER_PARAMETER_TOKEN * run.model_parameters * run.training_tokens;
            assert!((reconstructed - run.compute_flops).abs() / run.compute_flops < 1e-9);
        }
        // Geometric spacing is monotonically increasing in N per budget.
        for budget_runs in runs.chunks(5) {
            for pair in budget_runs.windows(2) {
                assert!(pair[1].model_parameters > pair[0].model_parameters);
            }
        }
        assert!((runs[0].model_parameters - 1e3).abs() < 1e-6);
        assert!((runs[4].model_parameters - 1e5).abs() < 1e-1);
    }

    #[test]
    fn planner_refuses_invalid_bounds() {
        assert!(matches!(
            cs336_a3_plan_isoflop_sweep(&[], 5, 1e3, 1e5),
            Err(Cs336A3ScalingError::InvalidPlannerBound {
                parameter: "budgets"
            })
        ));
        assert!(matches!(
            cs336_a3_plan_isoflop_sweep(&[1e9], 2, 1e3, 1e5),
            Err(Cs336A3ScalingError::InvalidPlannerBound {
                parameter: "cells_per_budget"
            })
        ));
        assert!(matches!(
            cs336_a3_plan_isoflop_sweep(&[1e9], 5, 1e5, 1e3),
            Err(Cs336A3ScalingError::InvalidPlannerBound {
                parameter: "parameters_range"
            })
        ));
    }

    #[test]
    fn fit_recovers_the_synthetic_optimal_exponent() {
        // Full pipeline: plan the sweep, score cells from the synthetic law,
        // fit, and recover the analytic exponent a = 0.5.
        let budgets = [1e12, 1e14, 1e16, 1e18];
        let mut cells = Vec::new();
        for budget in budgets {
            // Center the N grid around the analytic optimum so every budget
            // has an interior minimum: N* = (A alpha / (B beta))^(...) — for
            // alpha = beta and A ~ B the optimum sits near sqrt(C / 6).
            let center = (budget / CS336_A3_FLOPS_PER_PARAMETER_TOKEN).sqrt();
            let runs = cs336_a3_plan_isoflop_sweep(&[budget], 9, center / 100.0, center * 100.0)
                .expect("plans");
            for run in runs {
                cells.push(Cs336A3SweepCell {
                    model_parameters: run.model_parameters,
                    training_tokens: run.training_tokens,
                    compute_flops: run.compute_flops,
                    final_loss: synthetic_loss(run.model_parameters, run.training_tokens),
                });
            }
        }
        let fit = cs336_a3_fit_isoflop(&cells).expect("fits");
        assert!(
            (fit.parameter_exponent - 0.5).abs() < 0.05,
            "exponent {} should be near 0.5",
            fit.parameter_exponent
        );
        assert!((fit.token_exponent - 0.5).abs() < 0.05);
        // The prediction matches the analytic optimum within tolerance.
        let predicted = fit.predict_optimal(1e15);
        let analytic =
            (1e15_f64 / CS336_A3_FLOPS_PER_PARAMETER_TOKEN).sqrt() * (410.7_f64 / 406.4).powf(0.5);
        let ratio = predicted.model_parameters / analytic;
        assert!(
            (0.5..2.0).contains(&ratio),
            "predicted {} vs analytic {analytic}",
            predicted.model_parameters
        );
    }

    #[test]
    fn fit_refuses_thin_or_degenerate_inputs() {
        let cell = Cs336A3SweepCell {
            model_parameters: 1e4,
            training_tokens: 1e6,
            compute_flops: 6e10,
            final_loss: 2.0,
        };
        assert!(matches!(
            cs336_a3_fit_isoflop(&[cell, cell, cell]),
            Err(Cs336A3ScalingError::InsufficientBudgets { found: 1 })
        ));
        let mut thin = vec![cell];
        thin.push(Cs336A3SweepCell {
            compute_flops: 6e12,
            ..cell
        });
        assert!(matches!(
            cs336_a3_fit_isoflop(&thin),
            Err(Cs336A3ScalingError::InsufficientCells { .. })
        ));
        let bad = Cs336A3SweepCell {
            model_parameters: -1.0,
            ..cell
        };
        assert!(matches!(
            cs336_a3_fit_isoflop(&[bad]),
            Err(Cs336A3ScalingError::NonPositiveCell)
        ));
    }

    #[test]
    fn rising_loss_curves_refuse_with_no_interior_minimum() {
        // Strictly increasing loss in N at both budgets: no interior minimum.
        let mut cells = Vec::new();
        for (budget, base) in [(6e10, 2.0), (6e12, 1.8)] {
            for (index, n) in [1e3, 1e4, 1e5].iter().enumerate() {
                cells.push(Cs336A3SweepCell {
                    model_parameters: *n,
                    training_tokens: budget / (CS336_A3_FLOPS_PER_PARAMETER_TOKEN * n),
                    compute_flops: budget,
                    final_loss: base + index as f64,
                });
            }
        }
        assert!(matches!(
            cs336_a3_fit_isoflop(&cells),
            Err(Cs336A3ScalingError::NoInteriorMinimum { .. })
        ));
    }

    #[test]
    fn fit_digest_is_stable() {
        let budgets = [1e12, 1e14];
        let mut cells = Vec::new();
        for budget in budgets {
            let center = (budget / CS336_A3_FLOPS_PER_PARAMETER_TOKEN).sqrt();
            for run in cs336_a3_plan_isoflop_sweep(&[budget], 5, center / 10.0, center * 10.0)
                .expect("plans")
            {
                cells.push(Cs336A3SweepCell {
                    model_parameters: run.model_parameters,
                    training_tokens: run.training_tokens,
                    compute_flops: run.compute_flops,
                    final_loss: synthetic_loss(run.model_parameters, run.training_tokens),
                });
            }
        }
        let a = cs336_a3_fit_isoflop(&cells).expect("fits");
        let b = cs336_a3_fit_isoflop(&cells).expect("fits");
        assert_eq!(a.stable_digest(), b.stable_digest());
    }
}

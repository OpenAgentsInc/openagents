//! TestGen Scoring
//!
//! Scoring functions for test generation runs.
//! Uses 0-1000 scale to match HillClimber scoring.

use crate::types::TestGenAnalysis;

// ============================================================================
// Scoring Formula
// ============================================================================

/// Score a test generation run (0-1000 scale).
///
/// Formula:
/// - Comprehensiveness (1-10) -> 0-400 points (40 points per point)
/// - Category balance (0-1) -> 0-200 points
/// - Anti-cheat coverage (0-1) -> 0-200 points
/// - Token efficiency (0-1) -> 0-200 points
///
/// Total: 0-1000 points
pub fn score_testgen_run(analysis: &TestGenAnalysis, comprehensiveness_score: Option<f64>) -> i32 {
    // Comprehensiveness score (from LLM self-assessment)
    let comp_score = comprehensiveness_score.unwrap_or(5.0); // 5 = baseline
    let comprehensiveness_points = comp_score * 40.0;

    // Category balance
    let balance_points = analysis.category_balance * 200.0;

    // Anti-cheat coverage
    let anti_cheat_points = analysis.anti_cheat_coverage * 200.0;

    // Token efficiency
    let efficiency_points = analysis.token_efficiency * 200.0;

    let total = comprehensiveness_points + balance_points + anti_cheat_points + efficiency_points;

    // Clamp to 0-1000
    total.round().clamp(0.0, 1000.0) as i32
}

/// Update analysis with computed overall score.
pub fn compute_overall_score(
    mut analysis: TestGenAnalysis,
    comprehensiveness_score: Option<f64>,
) -> TestGenAnalysis {
    analysis.overall_score = score_testgen_run(&analysis, comprehensiveness_score);
    analysis
}

/// Breakdown of scoring components for debugging/display
#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub comprehensiveness_points: f64,
    pub balance_points: f64,
    pub anti_cheat_points: f64,
    pub efficiency_points: f64,
    pub total: i32,
}

/// Get detailed breakdown of scoring components
pub fn score_breakdown(
    analysis: &TestGenAnalysis,
    comprehensiveness_score: Option<f64>,
) -> ScoreBreakdown {
    let comp_score = comprehensiveness_score.unwrap_or(5.0);
    let comprehensiveness_points = comp_score * 40.0;
    let balance_points = analysis.category_balance * 200.0;
    let anti_cheat_points = analysis.anti_cheat_coverage * 200.0;
    let efficiency_points = analysis.token_efficiency * 200.0;

    let total =
        (comprehensiveness_points + balance_points + anti_cheat_points + efficiency_points)
            .round()
            .clamp(0.0, 1000.0) as i32;

    ScoreBreakdown {
        comprehensiveness_points,
        balance_points,
        anti_cheat_points,
        efficiency_points,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_scoring_formula() {
        let analysis = TestGenAnalysis {
            category_distribution: HashMap::new(),
            category_balance: 0.8,
            anti_cheat_coverage: 0.9,
            parameter_discovery: 0.7,
            reflection_effectiveness: 0.6,
            token_efficiency: 0.5,
            overall_score: 0,
        };

        let score = score_testgen_run(&analysis, Some(8.0));
        // 8*40 + 0.8*200 + 0.9*200 + 0.5*200 = 320 + 160 + 180 + 100 = 760
        assert_eq!(score, 760);
    }

    #[test]
    fn test_scoring_max() {
        let analysis = TestGenAnalysis {
            category_distribution: HashMap::new(),
            category_balance: 1.0,
            anti_cheat_coverage: 1.0,
            parameter_discovery: 1.0,
            reflection_effectiveness: 1.0,
            token_efficiency: 1.0,
            overall_score: 0,
        };

        let score = score_testgen_run(&analysis, Some(10.0));
        // 10*40 + 1.0*200 + 1.0*200 + 1.0*200 = 400 + 200 + 200 + 200 = 1000
        assert_eq!(score, 1000);
    }

    #[test]
    fn test_scoring_min() {
        let analysis = TestGenAnalysis {
            category_distribution: HashMap::new(),
            category_balance: 0.0,
            anti_cheat_coverage: 0.0,
            parameter_discovery: 0.0,
            reflection_effectiveness: 0.0,
            token_efficiency: 0.0,
            overall_score: 0,
        };

        let score = score_testgen_run(&analysis, Some(0.0));
        assert_eq!(score, 0);
    }

    #[test]
    fn test_score_breakdown() {
        let analysis = TestGenAnalysis {
            category_distribution: HashMap::new(),
            category_balance: 0.8,
            anti_cheat_coverage: 0.9,
            parameter_discovery: 0.7,
            reflection_effectiveness: 0.6,
            token_efficiency: 0.5,
            overall_score: 0,
        };

        let breakdown = score_breakdown(&analysis, Some(8.0));
        assert!((breakdown.comprehensiveness_points - 320.0).abs() < 0.01);
        assert!((breakdown.balance_points - 160.0).abs() < 0.01);
        assert!((breakdown.anti_cheat_points - 180.0).abs() < 0.01);
        assert!((breakdown.efficiency_points - 100.0).abs() < 0.01);
        assert_eq!(breakdown.total, 760);
    }

    #[test]
    fn test_compute_overall_score() {
        let analysis = TestGenAnalysis {
            category_distribution: HashMap::new(),
            category_balance: 1.0,
            anti_cheat_coverage: 1.0,
            parameter_discovery: 1.0,
            reflection_effectiveness: 1.0,
            token_efficiency: 1.0,
            overall_score: 0,
        };

        let updated = compute_overall_score(analysis, Some(10.0));
        assert_eq!(updated.overall_score, 1000);
    }
}

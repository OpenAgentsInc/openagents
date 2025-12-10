//! HillClimber Scoring
//!
//! Score calculation for comparing task run results.
//! Higher scores are better.
//!
//! Scoring philosophy:
//! - Passing is worth much more than failing (1000 point bonus)
//! - Fewer turns are better (100 - turns points, capped at 0)
//! - This means: pass in 5 turns (1095) > pass in 50 turns (1050) > fail in 5 turns (95)

// ============================================================================
// Score Constants
// ============================================================================

/// Bonus points for passing a task.
pub const PASS_BONUS: i32 = 1000;

/// Base points for turn efficiency (100 - turns).
pub const TURN_BASE: i32 = 100;

/// Minimum score (floor).
pub const MIN_SCORE: i32 = 0;

/// Minimum score to consider a config "good enough" for export.
/// This is effectively "must pass" since PASS_BONUS = 1000.
pub const EXPORT_THRESHOLD: i32 = PASS_BONUS;

/// Minimum consecutive passes required before exporting a config.
pub const MIN_CONSECUTIVE_PASSES: u32 = 3;

// ============================================================================
// Score Calculation
// ============================================================================

/// Calculate a score for a task run result.
///
/// # Arguments
///
/// * `passed` - Whether the task passed verification
/// * `turns` - Number of turns (FM calls) used
///
/// # Returns
///
/// Numeric score (higher = better)
///
/// # Examples
///
/// ```
/// use hillclimber::scoring::score_result;
///
/// assert_eq!(score_result(true, 5), 1095);   // pass + efficiency
/// assert_eq!(score_result(true, 50), 1050); // pass + low efficiency
/// assert_eq!(score_result(false, 5), 95);   // no pass bonus
/// assert_eq!(score_result(false, 50), 50);  // no pass bonus
/// ```
pub fn score_result(passed: bool, turns: u32) -> i32 {
    let pass_bonus = if passed { PASS_BONUS } else { 0 };
    let turns_score = (TURN_BASE - turns as i32).max(0);
    pass_bonus + turns_score
}

// ============================================================================
// Score Comparison
// ============================================================================

/// Compare two scores to determine if the new score is better.
///
/// # Arguments
///
/// * `new_score` - The new score to compare
/// * `old_score` - The existing best score (or None if none)
///
/// # Returns
///
/// `true` if `new_score` is better than `old_score`
pub fn is_better_score(new_score: i32, old_score: Option<i32>) -> bool {
    match old_score {
        None => true,
        Some(old) => new_score > old,
    }
}

/// Compare two run results to determine if the new result is better.
///
/// Comparison priority:
/// 1. Passing is always better than failing
/// 2. If both pass or both fail, fewer turns is better
///
/// # Arguments
///
/// * `new_passed` - Whether the new run passed
/// * `new_turns` - Turns used in new run
/// * `old_passed` - Whether the old run passed
/// * `old_turns` - Turns used in old run
///
/// # Returns
///
/// `true` if the new result is better
pub fn is_better_result(new_passed: bool, new_turns: u32, old_passed: bool, old_turns: u32) -> bool {
    // Passing is always better than failing
    if new_passed && !old_passed {
        return true;
    }
    if !new_passed && old_passed {
        return false;
    }

    // If both have same pass status, fewer turns is better
    new_turns < old_turns
}

// ============================================================================
// Score Formatting
// ============================================================================

/// Format a score for display.
///
/// # Arguments
///
/// * `score` - The score to format
///
/// # Returns
///
/// Formatted string like "1095 (PASS)" or "95 (FAIL)"
pub fn format_score(score: i32) -> String {
    let passed = score >= PASS_BONUS;
    let status = if passed { "PASS" } else { "FAIL" };
    format!("{} ({})", score, status)
}

/// Format a run result summary for display.
///
/// # Arguments
///
/// * `passed` - Whether the task passed
/// * `turns` - Number of turns used
/// * `score` - The calculated score
///
/// # Returns
///
/// Formatted string like "PASSED in 5 turns (score: 1095)"
pub fn format_run_summary(passed: bool, turns: u32, score: i32) -> String {
    let status = if passed { "PASSED" } else { "FAILED" };
    format!("{} in {} turns (score: {})", status, turns, score)
}

// ============================================================================
// Stability Check
// ============================================================================

/// Check if a config is stable enough for export.
///
/// # Arguments
///
/// * `pass_count` - Number of passes with this config
/// * `total_runs` - Total runs with this config
/// * `score` - Best score achieved
///
/// # Returns
///
/// `true` if the config is stable enough for export
pub fn is_stable_for_export(pass_count: u32, total_runs: u32, score: i32) -> bool {
    // Must have passed at least MIN_CONSECUTIVE_PASSES times
    if pass_count < MIN_CONSECUTIVE_PASSES {
        return false;
    }

    // Must have a passing score
    if score < EXPORT_THRESHOLD {
        return false;
    }

    // Must have a decent pass rate (>= 50%)
    let pass_rate = if total_runs > 0 {
        pass_count as f64 / total_runs as f64
    } else {
        0.0
    };
    if pass_rate < 0.5 {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_result() {
        assert_eq!(score_result(true, 5), 1095);
        assert_eq!(score_result(true, 50), 1050);
        assert_eq!(score_result(true, 100), 1000); // capped at 0 turns bonus
        assert_eq!(score_result(true, 150), 1000); // capped at 0 turns bonus
        assert_eq!(score_result(false, 5), 95);
        assert_eq!(score_result(false, 50), 50);
        assert_eq!(score_result(false, 100), 0);
        assert_eq!(score_result(false, 150), 0);
    }

    #[test]
    fn test_is_better_score() {
        assert!(is_better_score(1000, None));
        assert!(is_better_score(1095, Some(1090)));
        assert!(!is_better_score(1090, Some(1095)));
        assert!(!is_better_score(1090, Some(1090)));
    }

    #[test]
    fn test_is_better_result() {
        // Passing beats failing
        assert!(is_better_result(true, 100, false, 5));
        assert!(!is_better_result(false, 5, true, 100));

        // Fewer turns is better when both pass
        assert!(is_better_result(true, 5, true, 10));
        assert!(!is_better_result(true, 10, true, 5));

        // Fewer turns is better when both fail
        assert!(is_better_result(false, 5, false, 10));
        assert!(!is_better_result(false, 10, false, 5));
    }

    #[test]
    fn test_format_score() {
        assert_eq!(format_score(1095), "1095 (PASS)");
        assert_eq!(format_score(95), "95 (FAIL)");
        assert_eq!(format_score(1000), "1000 (PASS)");
        assert_eq!(format_score(999), "999 (FAIL)");
    }

    #[test]
    fn test_is_stable_for_export() {
        // Not enough passes
        assert!(!is_stable_for_export(2, 4, 1095));

        // Score too low (failing)
        assert!(!is_stable_for_export(3, 4, 95));

        // Pass rate too low
        assert!(!is_stable_for_export(3, 10, 1095));

        // All criteria met
        assert!(is_stable_for_export(3, 4, 1095));
        assert!(is_stable_for_export(5, 10, 1050));
    }
}

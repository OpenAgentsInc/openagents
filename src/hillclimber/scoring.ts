/**
 * HillClimber Scoring
 *
 * Score calculation for comparing task run results.
 * Higher scores are better.
 *
 * Scoring philosophy:
 * - Passing is worth much more than failing (1000 point bonus)
 * - Fewer turns are better (100 - turns points, capped at 0)
 * - This means: pass in 5 turns (1095) > pass in 50 turns (1050) > fail in 5 turns (95)
 */

// ============================================================================
// Score Constants
// ============================================================================

/** Bonus points for passing a task */
export const PASS_BONUS = 1000;

/** Base points for turn efficiency (100 - turns) */
export const TURN_BASE = 100;

/** Minimum score (floor) */
export const MIN_SCORE = 0;

// ============================================================================
// Score Calculation
// ============================================================================

/**
 * Calculate a score for a task run result.
 *
 * @param passed Whether the task passed verification
 * @param turns Number of turns (FM calls) used
 * @returns Numeric score (higher = better)
 *
 * @example
 * scoreResult(true, 5)   // 1095 (pass + efficiency)
 * scoreResult(true, 50)  // 1050 (pass + low efficiency)
 * scoreResult(false, 5)  // 95 (no pass bonus)
 * scoreResult(false, 50) // 50 (no pass bonus)
 */
export const scoreResult = (passed: boolean, turns: number): number => {
  const passBonus = passed ? PASS_BONUS : 0;
  const turnsScore = Math.max(0, TURN_BASE - turns);
  return passBonus + turnsScore;
};

// ============================================================================
// Score Comparison
// ============================================================================

/**
 * Compare two scores to determine if the new score is better.
 *
 * @param newScore The new score to compare
 * @param oldScore The existing best score (or null if none)
 * @returns true if newScore is better than oldScore
 */
export const isBetterScore = (
  newScore: number,
  oldScore: number | null,
): boolean => {
  if (oldScore === null) return true;
  return newScore > oldScore;
};

/**
 * Compare two run results to determine if the new result is better.
 *
 * Comparison priority:
 * 1. Passing is always better than failing
 * 2. If both pass or both fail, fewer turns is better
 *
 * @param newPassed Whether the new run passed
 * @param newTurns Turns used in new run
 * @param oldPassed Whether the old run passed
 * @param oldTurns Turns used in old run
 * @returns true if the new result is better
 */
export const isBetterResult = (
  newPassed: boolean,
  newTurns: number,
  oldPassed: boolean,
  oldTurns: number,
): boolean => {
  // Passing is always better than failing
  if (newPassed && !oldPassed) return true;
  if (!newPassed && oldPassed) return false;

  // If both have same pass status, fewer turns is better
  return newTurns < oldTurns;
};

// ============================================================================
// Score Formatting
// ============================================================================

/**
 * Format a score for display.
 *
 * @param score The score to format
 * @returns Formatted string like "1095 (PASS)" or "95 (FAIL)"
 */
export const formatScore = (score: number): string => {
  const passed = score >= PASS_BONUS;
  const status = passed ? "PASS" : "FAIL";
  return `${score} (${status})`;
};

/**
 * Format a run result summary for display.
 *
 * @param passed Whether the task passed
 * @param turns Number of turns used
 * @param score The calculated score
 * @returns Formatted string like "PASSED in 5 turns (score: 1095)"
 */
export const formatRunSummary = (
  passed: boolean,
  turns: number,
  score: number,
): string => {
  const status = passed ? "PASSED" : "FAILED";
  return `${status} in ${turns} turns (score: ${score})`;
};

// ============================================================================
// Score Thresholds
// ============================================================================

/**
 * Minimum score to consider a config "good enough" for export.
 * This is effectively "must pass" since PASS_BONUS = 1000.
 */
export const EXPORT_THRESHOLD = PASS_BONUS;

/**
 * Minimum consecutive passes required before exporting a config.
 */
export const MIN_CONSECUTIVE_PASSES = 3;

/**
 * Check if a config is stable enough for export.
 *
 * @param passCount Number of passes with this config
 * @param totalRuns Total runs with this config
 * @param score Best score achieved
 * @returns true if the config is stable enough for export
 */
export const isStableForExport = (
  passCount: number,
  totalRuns: number,
  score: number,
): boolean => {
  // Must have passed at least MIN_CONSECUTIVE_PASSES times
  if (passCount < MIN_CONSECUTIVE_PASSES) return false;

  // Must have a passing score
  if (score < EXPORT_THRESHOLD) return false;

  // Must have a decent pass rate (>= 50%)
  const passRate = totalRuns > 0 ? passCount / totalRuns : 0;
  if (passRate < 0.5) return false;

  return true;
};

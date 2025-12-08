/**
 * TestGen Scoring
 *
 * Scoring functions for test generation runs.
 * Uses 0-1000 scale to match HillClimber scoring.
 */

import type { TestGenAnalysis } from "./testgen-analyzer.js";

// ============================================================================
// Scoring Formula
// ============================================================================

/**
 * Score a test generation run (0-1000 scale).
 *
 * Formula:
 * - Comprehensiveness (1-10) → 0-400 points (40 points per point)
 * - Category balance (0-1) → 0-200 points
 * - Anti-cheat coverage (0-1) → 0-200 points
 * - Token efficiency (0-1) → 0-200 points
 *
 * Total: 0-1000 points
 */
export const scoreTestGenRun = (
  analysis: TestGenAnalysis,
  comprehensivenessScore: number | null,
): number => {
  // Comprehensiveness score (from LLM self-assessment)
  const comprehensivenessPoints = (comprehensivenessScore ?? 5) * 40; // 5 = baseline

  // Category balance
  const balancePoints = analysis.categoryBalance * 200;

  // Anti-cheat coverage
  const antiCheatPoints = analysis.antiCheatCoverage * 200;

  // Token efficiency
  const efficiencyPoints = analysis.tokenEfficiency * 200;

  const total = Math.round(
    comprehensivenessPoints +
      balancePoints +
      antiCheatPoints +
      efficiencyPoints,
  );

  return Math.max(0, Math.min(1000, total)); // Clamp to 0-1000
};

/**
 * Update analysis with computed overall score.
 */
export const computeOverallScore = (
  analysis: TestGenAnalysis,
  comprehensivenessScore: number | null,
): TestGenAnalysis => {
  return {
    ...analysis,
    overallScore: scoreTestGenRun(analysis, comprehensivenessScore),
  };
};


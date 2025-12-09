/**
 * TestGen Analyzer
 *
 * Programmatic analysis of test generation runs to extract quality metrics.
 * Used by the evolution system to evaluate config performance.
 */

import type { GeneratedTest, TestCategory } from "./test-generator.js";
import type { EnvironmentInfo } from "./environment-info.js";

// ============================================================================
// Types
// ============================================================================

/**
 * Complete analysis of a test generation run.
 */
export interface TestGenAnalysis {
  /** Distribution of tests across categories */
  categoryDistribution: Record<string, number>;
  /** How balanced the distribution is (0-1, 1 = perfectly balanced) */
  categoryBalance: number;
  /** Coverage of anti-cheat tests (0-1, 1 = all prohibited tools covered) */
  antiCheatCoverage: number;
  /** Coverage of parameter discovery (0-1, 1 = all parameters discovered) */
  parameterDiscovery: number;
  /** How effective reflections were (0-1, 1 = reflections added many new tests) */
  reflectionEffectiveness: number;
  /** Token efficiency: comprehensiveness per 1k tokens */
  tokenEfficiency: number;
  /** Overall composite score (0-1000) - computed by scoring.ts */
  overallScore: number;
}

/**
 * Trajectory data structure (from database)
 */
export interface TestGenTrajectory {
  sessionId: string;
  taskId: string;
  taskDescription: string;
  totalTests: number;
  totalRounds: number;
  categoryRounds: Record<string, number>;
  comprehensivenessScore: number | null;
  totalTokensUsed: number;
  durationMs: number;
  tests: GeneratedTest[];
  reflections: Array<{
    category?: string;
    reflectionText: string;
    action: "refining" | "assessing" | "complete";
  }>;
  environment: EnvironmentInfo;
  uncertainties: string[];
}

// ============================================================================
// Main Analysis Function
// ============================================================================

/**
 * Analyze a complete test generation trajectory.
 */
export const analyzeTestGenRun = (
  trajectory: TestGenTrajectory,
): TestGenAnalysis => {
  const categoryDist = analyzeCategoryDistribution(trajectory.tests);
  const balance = calculateCategoryBalance(categoryDist);
  const antiCheat = analyzeAntiCheatCoverage(
    trajectory.tests,
    trajectory.environment,
    trajectory.taskDescription,
  );
  const paramDiscovery = analyzeParameterDiscovery(
    trajectory.tests,
    trajectory.environment,
  );
  const reflectionEff = analyzeReflectionEffectiveness(
    trajectory.reflections,
    trajectory.tests,
    trajectory.categoryRounds,
  );
  const tokenEff = analyzeTokenEfficiency(
    trajectory.totalTokensUsed,
    trajectory.comprehensivenessScore ?? 5,
  );

  return {
    categoryDistribution: categoryDist,
    categoryBalance: balance,
    antiCheatCoverage: antiCheat,
    parameterDiscovery: paramDiscovery,
    reflectionEffectiveness: reflectionEff,
    tokenEfficiency: tokenEff,
    overallScore: 0, // Will be computed by scoring.ts
  };
};

// ============================================================================
// Category Distribution Analysis
// ============================================================================

/**
 * Count tests by category.
 */
export const analyzeCategoryDistribution = (
  tests: GeneratedTest[],
): Record<string, number> => {
  const distribution: Record<string, number> = {
    existence: 0,
    format: 0,
    happy_path: 0,
    boundary: 0,
    edge_case: 0,
    invalid_input: 0,
    integration: 0,
    anti_cheat: 0, // Extended category from iterative generator
  };

  for (const test of tests) {
    const cat = test.category as string;
    distribution[cat] = (distribution[cat] || 0) + 1;
  }

  return distribution;
};

/**
 * Calculate how balanced the category distribution is.
 * Returns 0-1, where 1 = perfectly balanced (equal counts per category).
 */
export const calculateCategoryBalance = (
  distribution: Record<string, number>,
): number => {
  const categories = Object.keys(distribution);
  const counts = categories.map((cat) => distribution[cat] || 0);
  const total = counts.reduce((sum, count) => sum + count, 0);

  if (total === 0) return 0;

  // Ideal: equal distribution
  const ideal = total / categories.length;
  const variance = counts.reduce((sum, count) => {
    const diff = count - ideal;
    return sum + diff * diff;
  }, 0) / categories.length;

  // Normalize: 0 variance = 1.0, max variance = 0.0
  const maxVariance = ideal * ideal * (categories.length - 1);
  return maxVariance > 0 ? Math.max(0, 1 - variance / maxVariance) : 1;
};

// ============================================================================
// Anti-Cheat Coverage Analysis
// ============================================================================

/**
 * Analyze how well anti-cheat tests cover prohibited tools.
 * Returns 0-1, where 1 = all prohibited tools have anti-cheat tests.
 */
export const analyzeAntiCheatCoverage = (
  tests: GeneratedTest[],
  environment: EnvironmentInfo | unknown,
  taskDescription: string,
): number => {
  // Extract prohibited tools from environment
  const env = environment as EnvironmentInfo;
  if (!env || !env.tools) {
    // No environment info = can't check anti-cheat coverage
    return 0.0;
  }
  const prohibitedTools = env.tools.prohibited || [];
  if (prohibitedTools.length === 0) {
    // No prohibited tools = no anti-cheat needed = perfect coverage
    return 1.0;
  }

  // Find anti-cheat tests (check reasoning/input for anti-cheat keywords since category doesn't exist)
  const antiCheatTests = tests.filter((t) =>
    t.reasoning.toLowerCase().includes("anti-cheat") ||
    t.reasoning.toLowerCase().includes("prohibited") ||
    t.input.toLowerCase().includes("prohibited")
  );
  if (antiCheatTests.length === 0) {
    // Prohibited tools exist but no anti-cheat tests = 0 coverage
    return 0.0;
  }

  // Check if anti-cheat tests mention prohibited tools
  const toolNames = prohibitedTools.map((t) => t.name.toLowerCase());
  const coveredTools = new Set<string>();

  for (const test of antiCheatTests) {
    const testText = (
      test.input + " " + (test.expectedOutput || "") + " " + test.reasoning
    ).toLowerCase();

    for (const toolName of toolNames) {
      if (testText.includes(toolName)) {
        coveredTools.add(toolName);
      }
    }
  }

  return toolNames.length > 0 ? coveredTools.size / toolNames.length : 1.0;
};

// ============================================================================
// Parameter Discovery Analysis
// ============================================================================

/**
 * Analyze how well tests discovered parameters from environment files.
 * Returns 0-1, where 1 = all parameters from file previews are used in tests.
 */
export const analyzeParameterDiscovery = (
  tests: GeneratedTest[],
  environment: EnvironmentInfo | unknown,
): number => {
  // Extract parameters from file previews
  const env = environment as EnvironmentInfo;
  if (!env || !env.files) {
    // No environment info = can't check parameter discovery
    return 0.0;
  }
  const filePreviews = env.files.taskFiles || [];
  const discoveredParams = new Set<string>();

  // Simple heuristic: look for function parameters, variable names, etc. in previews
  for (const file of filePreviews) {
    const preview = file.preview || "";
    // Look for common parameter patterns (function definitions, variable assignments)
    const paramMatches = preview.matchAll(
      /(?:def|function|let|const|var)\s+(\w+)|\(([^)]+)\)/g,
    );
    for (const match of paramMatches) {
      const param = match[1] || match[2];
      if (param) {
        discoveredParams.add(param.trim().split(",")[0].trim());
      }
    }
  }

  if (discoveredParams.size === 0) {
    // No parameters to discover = perfect coverage
    return 1.0;
  }

  // Check if tests use these parameters
  const testText = tests
    .map((t) => t.input + " " + (t.expectedOutput || "") + " " + t.reasoning)
    .join(" ")
    .toLowerCase();

  let usedParams = 0;
  for (const param of discoveredParams) {
    if (testText.includes(param.toLowerCase())) {
      usedParams++;
    }
  }

  return discoveredParams.size > 0 ? usedParams / discoveredParams.size : 1.0;
};

// ============================================================================
// Reflection Effectiveness Analysis
// ============================================================================

/**
 * Analyze how effective reflections were at improving test quality.
 * Returns 0-1, where 1 = reflections led to many new tests.
 */
export const analyzeReflectionEffectiveness = (
  reflections: Array<{
    category?: string;
    reflectionText: string;
    action: "refining" | "assessing" | "complete";
  }>,
  tests: GeneratedTest[],
  categoryRounds: Record<string, number>,
): number => {
  if (reflections.length === 0) {
    return 0.0;
  }

  // Count refining reflections (these should lead to new tests)
  const refiningReflections = reflections.filter(
    (r) => r.action === "refining",
  ).length;

  // Estimate tests added per reflection
  // If we have multiple rounds per category, reflections likely added tests
  const totalRounds = Object.values(categoryRounds).reduce(
    (sum, rounds) => sum + rounds,
    0,
  );
  const avgRoundsPerCategory =
    Object.keys(categoryRounds).length > 0
      ? totalRounds / Object.keys(categoryRounds).length
      : 1;

  // More rounds = more reflections = more tests added
  // Normalize: 1 round = 0.0, 3+ rounds = 1.0
  const roundScore = Math.min(1.0, (avgRoundsPerCategory - 1) / 2);

  // Combine: reflection count + round score
  const reflectionScore = Math.min(1.0, refiningReflections / 5); // 5+ reflections = 1.0

  return (roundScore + reflectionScore) / 2;
};

// ============================================================================
// Token Efficiency Analysis
// ============================================================================

/**
 * Analyze token efficiency: quality per token spent.
 * Returns comprehensiveness score per 1k tokens.
 */
export const analyzeTokenEfficiency = (
  totalTokens: number,
  comprehensivenessScore: number,
): number => {
  if (totalTokens === 0 || comprehensivenessScore === 0) return 0;

  // Normalize: comprehensiveness per 1k tokens
  // Higher is better, but cap at 1.0 (10 comprehensiveness / 10k tokens = 1.0)
  // Formula: (comprehensiveness / tokens) * 1000 / 10
  // Example: (8.0 / 55221) * 1000 / 10 = 0.0145
  const efficiency = (comprehensivenessScore / totalTokens) * 1000;
  const normalized = efficiency / 10; // Scale to 0-1 range
  return Math.min(1.0, Math.max(0.0, normalized));
};

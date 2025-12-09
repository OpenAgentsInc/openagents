/**
 * TestGen Evolution Types
 *
 * Type definitions for the TestGen HillClimber evolution system.
 * These types map to the SQLite schema in .openagents/migrations/005_testgen_evolution.sql
 */

import type { TestCategory } from "./test-generator.js";

// ============================================================================
// Core Domain Types
// ============================================================================

/**
 * Test generation configuration - the "knobs" being tuned
 */
export interface TestGenConfig {
  id: number;
  version: string;
  temperature: number;
  maxTokens: number;
  minTestsPerCategory: number;
  maxTestsPerCategory: number;
  maxRoundsPerCategory: number;
  environmentWeight: number;
  antiCheatWeight: number;
  precisionWeight: number;
  categoryOrder: TestCategory[];
  categoryPrompts?: Record<TestCategory, string>;
  antiCheatPrompt?: string;
  reflectionPrompt?: string;
  primaryModel: "local" | "claude";
  reflectionModel: "local" | "claude";
  minComprehensivenessScore: number;
  targetComprehensivenessScore: number;
  configHash: string;
  isCurrent: boolean;
  createdAt: string;
}

/**
 * Input for creating a new config (without auto-generated fields)
 */
export interface TestGenConfigInput {
  version?: string;
  temperature?: number;
  maxTokens?: number;
  minTestsPerCategory?: number;
  maxTestsPerCategory?: number;
  maxRoundsPerCategory?: number;
  environmentWeight?: number;
  antiCheatWeight?: number;
  precisionWeight?: number;
  categoryOrder?: TestCategory[];
  categoryPrompts?: Record<TestCategory, string>;
  antiCheatPrompt?: string;
  reflectionPrompt?: string;
  primaryModel?: "local" | "claude";
  reflectionModel?: "local" | "claude";
  minComprehensivenessScore?: number;
  targetComprehensivenessScore?: number;
}

/**
 * Run record - every test generation session
 */
export interface TestGenRun {
  id: number;
  runId: string;
  sessionId: string;
  configId: number;
  taskId: string;
  totalTests: number;
  comprehensivenessScore: number | null;
  durationMs: number;
  totalTokens: number;
  categoryBalance: number | null;
  antiCheatCoverage: number | null;
  parameterDiscovery: number | null;
  reflectionEffectiveness: number | null;
  tokenEfficiency: number | null;
  metaModel: string | null;
  proposedChange: string | null;
  changeAccepted: boolean;
  score: number;
  isBest: boolean;
  createdAt: string;
}

/**
 * Input for creating a new run (without auto-generated fields)
 */
export interface TestGenRunInput {
  runId: string;
  sessionId: string;
  configId: number;
  taskId: string;
  totalTests: number;
  comprehensivenessScore?: number | null;
  durationMs: number;
  totalTokens: number;
  categoryBalance?: number | null;
  antiCheatCoverage?: number | null;
  parameterDiscovery?: number | null;
  reflectionEffectiveness?: number | null;
  tokenEfficiency?: number | null;
  metaModel?: string | null;
  proposedChange?: string | null;
  changeAccepted?: boolean;
  score: number;
  isBest?: boolean;
}

/**
 * Best config per task type
 */
export interface TestGenBestConfig {
  taskType: string; // "_global_" | "conversion" | "implementation" | etc.
  configId: number;
  runId: number;
  score: number;
  passCount: number;
  totalRuns: number;
  isOverride: boolean;
  updatedAt: string;
}

/**
 * Evolution history entry
 */
export interface TestGenEvolution {
  id: number;
  fromConfigId: number | null;
  toConfigId: number | null;
  changes: Record<string, unknown>;
  reasoning: string;
  expectedImprovement: string | null;
  actualImprovement: number | null;
  qualityDelta: number | null;
  createdAt: string;
}

/**
 * Config change proposal from meta-reasoner
 */
export interface TestGenConfigChange {
  type: "keep" | "update_params" | "update_prompts" | "update_weights";
  changes?: Partial<TestGenConfigInput>;
  reasoning: string;
  model?: string;
}

/**
 * Aggregate statistics
 */
export interface TestGenStats {
  totalRuns: number;
  totalConfigs: number;
  averageScore: number;
  bestScore: number;
  averageComprehensiveness: number;
  averageTokenEfficiency: number;
  configEvolutionCount: number;
}

/**
 * Task-specific statistics
 */
export interface TestGenTaskStats {
  taskId: string;
  totalRuns: number;
  averageScore: number;
  bestScore: number;
  bestConfigId: number | null;
  averageComprehensiveness: number;
  averageTokenEfficiency: number;
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Generate a unique run ID
 */
export const generateTestGenRunId = (): string => {
  const now = new Date();
  const dateStr = now.toISOString().slice(0, 10).replace(/-/g, "");
  const timeStr = now.toTimeString().slice(0, 8).replace(/:/g, "");
  const random = Math.random().toString(36).slice(2, 8);
  return `tg-${dateStr}-${timeStr}-${random}`;
};

/**
 * Convert database row to TestGenConfig
 */
export const rowToTestGenConfig = (row: any): TestGenConfig => ({
  id: row.id,
  version: row.version,
  temperature: row.temperature,
  maxTokens: row.max_tokens,
  minTestsPerCategory: row.min_tests_per_category,
  maxTestsPerCategory: row.max_tests_per_category,
  maxRoundsPerCategory: row.max_rounds_per_category,
  environmentWeight: row.environment_weight,
  antiCheatWeight: row.anti_cheat_weight,
  precisionWeight: row.precision_weight,
  categoryOrder: JSON.parse(row.category_order || '[]'),
  categoryPrompts: row.category_prompts ? JSON.parse(row.category_prompts) : undefined,
  antiCheatPrompt: row.anti_cheat_prompt ?? undefined,
  reflectionPrompt: row.reflection_prompt ?? undefined,
  primaryModel: row.primary_model as "local" | "claude",
  reflectionModel: row.reflection_model as "local" | "claude",
  minComprehensivenessScore: row.min_comprehensiveness_score,
  targetComprehensivenessScore: row.target_comprehensiveness_score,
  configHash: row.config_hash,
  isCurrent: Boolean(row.is_current),
  createdAt: row.created_at,
});

/**
 * Convert database row to TestGenRun
 */
export const rowToTestGenRun = (row: any): TestGenRun => ({
  id: row.id,
  runId: row.run_id,
  sessionId: row.session_id,
  configId: row.config_id,
  taskId: row.task_id,
  totalTests: row.total_tests,
  comprehensivenessScore: row.comprehensiveness_score ?? null,
  durationMs: row.duration_ms,
  totalTokens: row.total_tokens,
  categoryBalance: row.category_balance ?? null,
  antiCheatCoverage: row.anti_cheat_coverage ?? null,
  parameterDiscovery: row.parameter_discovery ?? null,
  reflectionEffectiveness: row.reflection_effectiveness ?? null,
  tokenEfficiency: row.token_efficiency ?? null,
  metaModel: row.meta_model ?? null,
  proposedChange: row.proposed_change ?? null,
  changeAccepted: Boolean(row.change_accepted),
  score: row.score,
  isBest: Boolean(row.is_best),
  createdAt: row.created_at,
});

/**
 * Convert database row to TestGenBestConfig
 */
export const rowToTestGenBestConfig = (row: any): TestGenBestConfig => ({
  taskType: row.task_type,
  configId: row.config_id,
  runId: row.run_id,
  score: row.score,
  passCount: row.pass_count,
  totalRuns: row.total_runs,
  isOverride: Boolean(row.is_override),
  updatedAt: row.updated_at,
});

/**
 * Convert database row to TestGenEvolution
 */
export const rowToTestGenEvolution = (row: any): TestGenEvolution => ({
  id: row.id,
  fromConfigId: row.from_config_id ?? null,
  toConfigId: row.to_config_id ?? null,
  changes: JSON.parse(row.changes || '{}'),
  reasoning: row.reasoning,
  expectedImprovement: row.expected_improvement ?? null,
  actualImprovement: row.actual_improvement ?? null,
  qualityDelta: row.quality_delta ?? null,
  createdAt: row.created_at,
});

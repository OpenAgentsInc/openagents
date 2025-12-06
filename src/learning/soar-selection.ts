/**
 * SOAR Greedy-Diverse Selection
 *
 * Implements SOAR's data selection strategy for training.
 * Key insight: Greedy-diverse outperforms both greedy and uniform selection.
 *
 * Selection strategy:
 * - Top 25 attempts: High-quality examples for learning correct patterns
 * - Bottom 25 attempts: Diverse examples for learning what NOT to do
 * - Result: 36.46% vs 34.30% (greedy) vs 32.38% (uniform) on ARC-AGI
 *
 * This creates a curriculum that balances exploitation (best examples)
 * with exploration (diverse failure modes).
 */

import * as S from "effect/Schema";
import { Effect, Context, Layer } from "effect";
import type { SyntheticTaskSolution } from "./soar-hindsight.js";

// --- Selection Configuration ---

export interface SelectionConfig {
  /** Number of top attempts to select (default: 25) */
  topK: number;

  /** Number of bottom attempts to select (default: 25) */
  bottomK: number;

  /** Minimum quality score for inclusion */
  minQualityScore: number;

  /** Maximum quality score for bottom selection (to avoid near-correct) */
  maxBottomQualityScore: number;

  /** Enable diversity bonus in scoring */
  enableDiversityBonus: boolean;

  /** Weight for diversity in final score (0-1) */
  diversityWeight: number;
}

export const DEFAULT_SELECTION_CONFIG: SelectionConfig = {
  topK: 25,
  bottomK: 25,
  minQualityScore: 0.01, // At least 1% accuracy
  maxBottomQualityScore: 0.5, // Bottom selection from < 50%
  enableDiversityBonus: true,
  diversityWeight: 0.3,
};

// --- Selection Result ---

export const SelectionResult = S.Struct({
  /** Selected top examples */
  topExamples: S.Array(
    S.Struct({
      synthetic: S.Unknown, // SyntheticTaskSolution
      selectionScore: S.Number,
      rank: S.Number,
    }),
  ),

  /** Selected bottom examples */
  bottomExamples: S.Array(
    S.Struct({
      synthetic: S.Unknown, // SyntheticTaskSolution
      selectionScore: S.Number,
      rank: S.Number,
    }),
  ),

  /** Total candidates considered */
  totalCandidates: S.Number,

  /** Selection timestamp */
  selectedAt: S.String,
});
export type SelectionResult = S.Schema.Type<typeof SelectionResult>;

// --- Diversity Calculation ---

/**
 * Calculate code signature for diversity comparison.
 * Uses structural features rather than exact text matching.
 */
const calculateCodeSignature = (code: string): Set<string> => {
  const features = new Set<string>();

  // Extract language constructs
  const constructs = code.match(
    /\b(if|else|for|while|return|function|const|let|var|class|try|catch|throw|async|await|import|export)\b/g,
  );
  for (const c of constructs ?? []) {
    features.add(`construct:${c}`);
  }

  // Extract operators
  const operators = code.match(/[+\-*/%=<>!&|^~?:]+/g);
  for (const op of operators ?? []) {
    features.add(`op:${op}`);
  }

  // Extract function calls (normalized)
  const calls = code.match(/\b([a-zA-Z_]\w*)\s*\(/g);
  for (const call of calls ?? []) {
    const name = call.replace(/\s*\($/, "");
    features.add(`call:${name}`);
  }

  // Extract structure patterns
  const hasLoop = /\b(for|while)\b/.test(code);
  const hasConditional = /\b(if|switch|case)\b/.test(code);
  const hasRecursion = /function\s+(\w+)[^}]*\1\s*\(/.test(code);
  const hasArray = /\[[\s\S]*\]/.test(code);
  const hasObject = /\{[\s\S]*:[\s\S]*\}/.test(code);

  if (hasLoop) features.add("pattern:loop");
  if (hasConditional) features.add("pattern:conditional");
  if (hasRecursion) features.add("pattern:recursion");
  if (hasArray) features.add("pattern:array");
  if (hasObject) features.add("pattern:object");

  return features;
};

/**
 * Calculate Jaccard similarity between two code signatures.
 */
const calculateJaccardSimilarity = (a: Set<string>, b: Set<string>): number => {
  if (a.size === 0 && b.size === 0) return 1;
  if (a.size === 0 || b.size === 0) return 0;

  const aArray = Array.from(a);
  const bArray = Array.from(b);
  const intersection = new Set(aArray.filter((x) => b.has(x)));
  const union = new Set([...aArray, ...bArray]);

  return intersection.size / union.size;
};

/**
 * Calculate diversity score for a candidate relative to already selected.
 * Higher score = more diverse from existing selection.
 */
const calculateDiversityScore = (
  candidate: SyntheticTaskSolution,
  selected: SyntheticTaskSolution[],
): number => {
  if (selected.length === 0) return 1;

  const candidateSig = calculateCodeSignature(candidate.solution);

  let totalSimilarity = 0;
  for (const existing of selected) {
    const existingSig = calculateCodeSignature(existing.solution);
    totalSimilarity += calculateJaccardSimilarity(candidateSig, existingSig);
  }

  const avgSimilarity = totalSimilarity / selected.length;
  return 1 - avgSimilarity; // Diversity = 1 - similarity
};

// --- Selection Functions ---

/**
 * Calculate combined score for greedy-diverse selection.
 */
const calculateSelectionScore = (
  candidate: SyntheticTaskSolution,
  selected: SyntheticTaskSolution[],
  config: SelectionConfig,
): number => {
  const qualityScore = candidate.qualityScore;

  if (!config.enableDiversityBonus) {
    return qualityScore;
  }

  const diversityScore = calculateDiversityScore(candidate, selected);

  // Weighted combination
  return (1 - config.diversityWeight) * qualityScore + config.diversityWeight * diversityScore;
};

/**
 * Select top K examples using greedy-diverse selection.
 */
export const selectTop = (
  candidates: SyntheticTaskSolution[],
  config: SelectionConfig = DEFAULT_SELECTION_CONFIG,
): { synthetic: SyntheticTaskSolution; selectionScore: number; rank: number }[] => {
  // Filter to valid candidates
  const validCandidates = candidates.filter((c) => c.qualityScore >= config.minQualityScore);

  const selected: { synthetic: SyntheticTaskSolution; selectionScore: number; rank: number }[] = [];
  const remaining = [...validCandidates];

  // Greedy selection with diversity
  while (selected.length < config.topK && remaining.length > 0) {
    let bestIdx = 0;
    let bestScore = -1;

    for (let i = 0; i < remaining.length; i++) {
      const score = calculateSelectionScore(
        remaining[i]!,
        selected.map((s) => s.synthetic),
        config,
      );
      if (score > bestScore) {
        bestScore = score;
        bestIdx = i;
      }
    }

    selected.push({
      synthetic: remaining[bestIdx]!,
      selectionScore: bestScore,
      rank: selected.length + 1,
    });
    remaining.splice(bestIdx, 1);
  }

  return selected;
};

/**
 * Select bottom K examples for diverse failure modes.
 */
export const selectBottom = (
  candidates: SyntheticTaskSolution[],
  config: SelectionConfig = DEFAULT_SELECTION_CONFIG,
): { synthetic: SyntheticTaskSolution; selectionScore: number; rank: number }[] => {
  // Filter to low-quality candidates (failure modes)
  const lowQualityCandidates = candidates.filter(
    (c) => c.qualityScore >= config.minQualityScore && c.qualityScore <= config.maxBottomQualityScore,
  );

  // Sort by quality ascending (lowest first)
  const sorted = [...lowQualityCandidates].sort((a, b) => a.qualityScore - b.qualityScore);

  const selected: { synthetic: SyntheticTaskSolution; selectionScore: number; rank: number }[] = [];
  const remaining = [...sorted];

  // Greedy selection prioritizing diversity among low-quality examples
  while (selected.length < config.bottomK && remaining.length > 0) {
    let bestIdx = 0;
    let bestDiversity = -1;

    // For bottom selection, prioritize diversity over quality
    for (let i = 0; i < remaining.length; i++) {
      const diversity = calculateDiversityScore(
        remaining[i]!,
        selected.map((s) => s.synthetic),
      );
      if (diversity > bestDiversity) {
        bestDiversity = diversity;
        bestIdx = i;
      }
    }

    const candidate = remaining[bestIdx]!;
    selected.push({
      synthetic: candidate,
      selectionScore: bestDiversity,
      rank: selected.length + 1,
    });
    remaining.splice(bestIdx, 1);
  }

  return selected;
};

/**
 * Perform full greedy-diverse selection.
 */
export const selectGreedyDiverse = (
  candidates: SyntheticTaskSolution[],
  config: SelectionConfig = DEFAULT_SELECTION_CONFIG,
): SelectionResult => {
  const topExamples = selectTop(candidates, config);
  const bottomExamples = selectBottom(candidates, config);

  return {
    topExamples: topExamples as unknown as SelectionResult["topExamples"],
    bottomExamples: bottomExamples as unknown as SelectionResult["bottomExamples"],
    totalCandidates: candidates.length,
    selectedAt: new Date().toISOString(),
  };
};

// --- Task-Grouped Selection ---

/**
 * Group candidates by original task for balanced selection.
 */
export const groupByTask = (
  candidates: SyntheticTaskSolution[],
): Map<string, SyntheticTaskSolution[]> => {
  const groups = new Map<string, SyntheticTaskSolution[]>();

  for (const candidate of candidates) {
    const taskId = candidate.task.originalTaskId;
    const existing = groups.get(taskId) ?? [];
    groups.set(taskId, [...existing, candidate]);
  }

  return groups;
};

/**
 * Select with task balance (ensure representation from multiple tasks).
 */
export const selectWithTaskBalance = (
  candidates: SyntheticTaskSolution[],
  config: SelectionConfig = DEFAULT_SELECTION_CONFIG,
): SelectionResult => {
  const byTask = groupByTask(candidates);

  // Calculate per-task quota
  const numTasks = byTask.size;
  if (numTasks === 0) {
    return {
      topExamples: [],
      bottomExamples: [],
      totalCandidates: 0,
      selectedAt: new Date().toISOString(),
    };
  }

  const topPerTask = Math.max(1, Math.ceil(config.topK / numTasks));
  const bottomPerTask = Math.max(1, Math.ceil(config.bottomK / numTasks));

  const allTop: { synthetic: SyntheticTaskSolution; selectionScore: number; rank: number }[] = [];
  const allBottom: { synthetic: SyntheticTaskSolution; selectionScore: number; rank: number }[] = [];

  for (const [_taskId, taskCandidates] of Array.from(byTask)) {
    const taskConfig = { ...config, topK: topPerTask, bottomK: bottomPerTask };
    const taskTop = selectTop(taskCandidates, taskConfig);
    const taskBottom = selectBottom(taskCandidates, taskConfig);

    allTop.push(...taskTop);
    allBottom.push(...taskBottom);
  }

  // Re-rank across all tasks
  allTop.sort((a, b) => b.selectionScore - a.selectionScore);
  allBottom.sort((a, b) => b.selectionScore - a.selectionScore);

  // Trim to final quotas
  const finalTop = allTop.slice(0, config.topK).map((s, i) => ({ ...s, rank: i + 1 }));
  const finalBottom = allBottom.slice(0, config.bottomK).map((s, i) => ({ ...s, rank: i + 1 }));

  return {
    topExamples: finalTop as unknown as SelectionResult["topExamples"],
    bottomExamples: finalBottom as unknown as SelectionResult["bottomExamples"],
    totalCandidates: candidates.length,
    selectedAt: new Date().toISOString(),
  };
};

// --- Service Interface ---

export interface ISelectionService {
  /** Select using greedy-diverse strategy */
  readonly selectGreedyDiverse: (
    candidates: SyntheticTaskSolution[],
  ) => Effect.Effect<SelectionResult, never>;

  /** Select with task balance */
  readonly selectWithTaskBalance: (
    candidates: SyntheticTaskSolution[],
  ) => Effect.Effect<SelectionResult, never>;

  /** Select top K only */
  readonly selectTop: (
    candidates: SyntheticTaskSolution[],
    k?: number,
  ) => Effect.Effect<{ synthetic: SyntheticTaskSolution; selectionScore: number; rank: number }[], never>;

  /** Select bottom K only */
  readonly selectBottom: (
    candidates: SyntheticTaskSolution[],
    k?: number,
  ) => Effect.Effect<{ synthetic: SyntheticTaskSolution; selectionScore: number; rank: number }[], never>;

  /** Calculate diversity score */
  readonly calculateDiversity: (
    candidate: SyntheticTaskSolution,
    reference: SyntheticTaskSolution[],
  ) => Effect.Effect<number, never>;

  /** Get current config */
  readonly getConfig: () => Effect.Effect<SelectionConfig, never>;

  /** Update config */
  readonly updateConfig: (updates: Partial<SelectionConfig>) => Effect.Effect<SelectionConfig, never>;

  /** Get selection statistics */
  readonly getStats: () => Effect.Effect<SelectionStats, never>;
}

export interface SelectionStats {
  totalSelections: number;
  totalCandidatesProcessed: number;
  averageTopScore: number;
  averageBottomDiversity: number;
}

// --- Service Tag ---

export class SelectionService extends Context.Tag("SelectionService")<
  SelectionService,
  ISelectionService
>() {}

// --- Service Implementation ---

const makeSelectionService = (
  initialConfig: SelectionConfig = DEFAULT_SELECTION_CONFIG,
): ISelectionService => {
  let config = { ...initialConfig };
  let stats: SelectionStats = {
    totalSelections: 0,
    totalCandidatesProcessed: 0,
    averageTopScore: 0,
    averageBottomDiversity: 0,
  };

  const updateStats = (result: SelectionResult): void => {
    stats.totalSelections++;
    stats.totalCandidatesProcessed += result.totalCandidates;

    // Update average top score
    if (result.topExamples.length > 0) {
      const topScoreSum = result.topExamples.reduce((sum, e) => sum + (e as { selectionScore: number }).selectionScore, 0);
      const newAvg = topScoreSum / result.topExamples.length;
      const prevSum = stats.averageTopScore * (stats.totalSelections - 1);
      stats.averageTopScore = (prevSum + newAvg) / stats.totalSelections;
    }

    // Update average bottom diversity
    if (result.bottomExamples.length > 0) {
      const bottomScoreSum = result.bottomExamples.reduce(
        (sum, e) => sum + (e as { selectionScore: number }).selectionScore,
        0,
      );
      const newAvg = bottomScoreSum / result.bottomExamples.length;
      const prevSum = stats.averageBottomDiversity * (stats.totalSelections - 1);
      stats.averageBottomDiversity = (prevSum + newAvg) / stats.totalSelections;
    }
  };

  return {
    selectGreedyDiverse: (candidates) =>
      Effect.sync(() => {
        const result = selectGreedyDiverse(candidates, config);
        updateStats(result);
        return result;
      }),

    selectWithTaskBalance: (candidates) =>
      Effect.sync(() => {
        const result = selectWithTaskBalance(candidates, config);
        updateStats(result);
        return result;
      }),

    selectTop: (candidates, k) =>
      Effect.sync(() => selectTop(candidates, k !== undefined ? { ...config, topK: k } : config)),

    selectBottom: (candidates, k) =>
      Effect.sync(() =>
        selectBottom(candidates, k !== undefined ? { ...config, bottomK: k } : config),
      ),

    calculateDiversity: (candidate, reference) =>
      Effect.sync(() => calculateDiversityScore(candidate, reference)),

    getConfig: () => Effect.sync(() => ({ ...config })),

    updateConfig: (updates) =>
      Effect.sync(() => {
        config = { ...config, ...updates };
        return { ...config };
      }),

    getStats: () => Effect.sync(() => ({ ...stats })),
  };
};

// --- Layer ---

export const SelectionServiceLive: Layer.Layer<SelectionService, never, never> = Layer.succeed(
  SelectionService,
  makeSelectionService(),
);

/**
 * Create a SelectionService layer with custom config.
 */
export const makeSelectionServiceLayer = (
  config: Partial<SelectionConfig> = {},
): Layer.Layer<SelectionService, never, never> =>
  Layer.succeed(
    SelectionService,
    makeSelectionService({ ...DEFAULT_SELECTION_CONFIG, ...config }),
  );

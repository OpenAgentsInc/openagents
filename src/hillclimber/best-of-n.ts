/**
 * Best-of-N Runner for HillClimber
 *
 * Implements test-time compute by running N parallel attempts
 * with different random seeds and selecting the best result.
 *
 * Based on research showing repeated sampling + verification significantly
 * improves pass rates (e.g., "Large Language Monkeys": 15.9% → 56%).
 */

import type { TerminalBenchTask } from "../bench/terminal-bench.js";
import type { HillClimberConfig } from "./types.js";
import { runTaskWithMAP } from "./map-orchestrator.js";

// ============================================================================
// Types
// ============================================================================

export interface CandidateResult {
  /** Unique candidate ID */
  id: string;
  /** Whether this candidate passed verification */
  passed: boolean;
  /** Progress score (0-1) */
  progress: number;
  /** Number of turns used */
  turns: number;
  /** Duration in milliseconds */
  durationMs: number;
  /** Error message if failed */
  error?: string;
  /** Random seed used for this attempt */
  seed: number;
}

export interface BestOfNOptions {
  /** Number of parallel candidates to run */
  n: number;
  /** Maximum turns per candidate */
  maxTurns: number;
  /** Timeout per candidate in seconds */
  timeout: number;
  /** Base workspace path */
  workspaceBase: string;
  /** Optional callback for progress updates */
  onProgress?: (completed: number, total: number, best: CandidateResult | null) => void;
  /** Optional callback for output */
  onOutput?: (candidateId: string, text: string) => void;
}

export interface BestOfNResult {
  /** All candidate results */
  candidates: CandidateResult[];
  /** The best candidate (highest score or first to pass) */
  best: CandidateResult | null;
  /** Whether any candidate passed */
  anyPassed: boolean;
  /** Total time for all candidates */
  totalDurationMs: number;
  /** Summary statistics */
  stats: {
    totalCandidates: number;
    passedCount: number;
    avgProgress: number;
    avgTurns: number;
    bestProgress: number;
  };
}

// ============================================================================
// Implementation
// ============================================================================

/**
 * Generate a pseudo-random seed for a candidate.
 * Uses task ID and index to ensure reproducibility.
 */
function generateSeed(taskId: string, index: number): number {
  // Simple hash based on task ID and index
  let hash = 0;
  const str = `${taskId}-${index}`;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash; // Convert to 32-bit integer
  }
  return Math.abs(hash);
}

/**
 * Run a single candidate attempt.
 */
async function runCandidate(
  task: TerminalBenchTask,
  config: HillClimberConfig,
  candidateId: string,
  seed: number,
  workspace: string,
  timeout: number,
  maxTurns: number,
  onOutput?: (text: string) => void,
): Promise<CandidateResult> {
  const startTime = Date.now();

  try {
    // Create candidate-specific workspace
    const fs = await import("node:fs");
    const path = await import("node:path");
    const candidateWorkspace = path.join(workspace, candidateId);
    fs.mkdirSync(candidateWorkspace, { recursive: true });

    // Run the MAP orchestrator
    const result = await runTaskWithMAP(
      task,
      config,
      candidateWorkspace,
      timeout,
      maxTurns,
      onOutput,
    );

    const candidateResult: CandidateResult = {
      id: candidateId,
      passed: result.passed,
      progress: result.progress,
      turns: result.turns,
      durationMs: result.durationMs,
      seed,
    };

    if (result.error !== undefined) {
      candidateResult.error = result.error;
    }

    return candidateResult;
  } catch (e) {
    const errMsg = e instanceof Error ? e.message : String(e);
    return {
      id: candidateId,
      passed: false,
      progress: 0,
      turns: 0,
      durationMs: Date.now() - startTime,
      seed,
      error: errMsg,
    } satisfies CandidateResult;
  }
}

/**
 * Run Best-of-N candidates in parallel.
 *
 * @param task The task to run
 * @param config HillClimber configuration
 * @param options Best-of-N options
 * @returns Results from all candidates with best selection
 */
export async function runBestOfN(
  task: TerminalBenchTask,
  config: HillClimberConfig,
  options: BestOfNOptions,
): Promise<BestOfNResult> {
  const startTime = Date.now();
  const { n, maxTurns, timeout, workspaceBase, onProgress, onOutput } = options;

  // Generate candidate IDs and seeds
  const candidates = Array.from({ length: n }, (_, i) => ({
    id: `candidate-${i + 1}`,
    seed: generateSeed(task.id, i),
  }));

  // Track best result as we go (early termination on pass)
  let best: CandidateResult | null = null;
  let completed = 0;
  const results: CandidateResult[] = [];

  // Run candidates in parallel batches
  // Use smaller batches to allow early termination
  const batchSize = Math.min(n, 3); // Run 3 at a time

  for (let batchStart = 0; batchStart < n; batchStart += batchSize) {
    // Check if we already have a passing result
    if (best?.passed) {
      // Early termination - we have a winner
      break;
    }

    const batchEnd = Math.min(batchStart + batchSize, n);
    const batchCandidates = candidates.slice(batchStart, batchEnd);

    // Run batch in parallel
    const batchPromises = batchCandidates.map((candidate) =>
      runCandidate(
        task,
        config,
        candidate.id,
        candidate.seed,
        workspaceBase,
        timeout,
        maxTurns,
        onOutput ? (text) => onOutput(candidate.id, text) : undefined,
      )
    );

    const batchResults = await Promise.all(batchPromises);

    // Process batch results
    for (const result of batchResults) {
      results.push(result);
      completed++;

      // Update best if this is better
      if (
        !best ||
        result.passed ||
        (!best.passed && result.progress > best.progress)
      ) {
        best = result;
      }

      onProgress?.(completed, n, best);
    }
  }

  // Calculate statistics
  const passedCount = results.filter((r) => r.passed).length;
  const avgProgress = results.reduce((sum, r) => sum + r.progress, 0) / results.length;
  const avgTurns = results.reduce((sum, r) => sum + r.turns, 0) / results.length;
  const bestProgress = Math.max(...results.map((r) => r.progress));

  return {
    candidates: results,
    best,
    anyPassed: passedCount > 0,
    totalDurationMs: Date.now() - startTime,
    stats: {
      totalCandidates: results.length,
      passedCount,
      avgProgress,
      avgTurns,
      bestProgress,
    },
  };
}

/**
 * Run Best-of-N with adaptive N.
 * Starts with fewer candidates and increases if no pass found.
 *
 * @param task The task to run
 * @param config HillClimber configuration
 * @param options Best-of-N options (n is initial N)
 * @param maxN Maximum N to try
 * @returns Results from best adaptive run
 */
export async function runAdaptiveBestOfN(
  task: TerminalBenchTask,
  config: HillClimberConfig,
  options: BestOfNOptions,
  maxN: number = 10,
): Promise<BestOfNResult> {
  let currentN = options.n;
  let allResults: CandidateResult[] = [];
  let best: CandidateResult | null = null;
  const startTime = Date.now();

  while (currentN <= maxN) {
    const result = await runBestOfN(task, config, {
      ...options,
      n: currentN,
    });

    allResults = [...allResults, ...result.candidates];

    // Update best
    for (const candidate of result.candidates) {
      if (
        !best ||
        candidate.passed ||
        (!best.passed && candidate.progress > best.progress)
      ) {
        best = candidate;
      }
    }

    // If we found a passing result, stop
    if (result.anyPassed) {
      break;
    }

    // Double N for next round
    currentN = Math.min(currentN * 2, maxN);
  }

  // Calculate final statistics
  const passedCount = allResults.filter((r) => r.passed).length;
  const avgProgress = allResults.reduce((sum, r) => sum + r.progress, 0) / allResults.length;
  const avgTurns = allResults.reduce((sum, r) => sum + r.turns, 0) / allResults.length;
  const bestProgress = Math.max(...allResults.map((r) => r.progress));

  return {
    candidates: allResults,
    best,
    anyPassed: passedCount > 0,
    totalDurationMs: Date.now() - startTime,
    stats: {
      totalCandidates: allResults.length,
      passedCount,
      avgProgress,
      avgTurns,
      bestProgress,
    },
  };
}

/**
 * Format Best-of-N result as a summary string.
 */
export function formatBestOfNSummary(result: BestOfNResult): string {
  const lines: string[] = [];

  lines.push(`Best-of-N Results:`);
  lines.push(`  Candidates: ${result.stats.totalCandidates}`);
  lines.push(`  Passed: ${result.stats.passedCount} (${(result.stats.passedCount / result.stats.totalCandidates * 100).toFixed(1)}%)`);
  lines.push(`  Avg Progress: ${(result.stats.avgProgress * 100).toFixed(1)}%`);
  lines.push(`  Best Progress: ${(result.stats.bestProgress * 100).toFixed(1)}%`);
  lines.push(`  Avg Turns: ${result.stats.avgTurns.toFixed(1)}`);
  lines.push(`  Total Time: ${(result.totalDurationMs / 1000).toFixed(1)}s`);

  if (result.best) {
    lines.push(`\nBest Candidate: ${result.best.id}`);
    lines.push(`  Passed: ${result.best.passed}`);
    lines.push(`  Progress: ${(result.best.progress * 100).toFixed(1)}%`);
    lines.push(`  Turns: ${result.best.turns}`);
  }

  return lines.join("\n");
}

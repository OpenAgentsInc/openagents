/**
 * Candidate Selector for HillClimber
 *
 * Implements selection strategies for choosing the best candidate
 * from multiple attempts. Based on "Shortest Majority Vote" from
 * Zeng et al. 2025 research.
 *
 * Selection Strategies:
 * 1. First Pass - Return first passing candidate
 * 2. Shortest Correct - Among passing, prefer shorter solutions
 * 3. Majority Vote - Among passing, prefer most common solution
 * 4. Highest Progress - If none pass, return highest partial score
 */

import type { CandidateResult } from "./best-of-n.js";

// ============================================================================
// Types
// ============================================================================

export type SelectionStrategy =
  | "first_pass"
  | "shortest_correct"
  | "majority_vote"
  | "highest_progress";

export interface SelectionOptions {
  /** Selection strategy to use */
  strategy: SelectionStrategy;
  /** For majority vote: minimum agreement threshold (0-1) */
  minAgreement?: number;
}

export interface SelectionResult {
  /** Selected candidate */
  selected: CandidateResult | null;
  /** Selection strategy used */
  strategy: SelectionStrategy;
  /** Reason for selection */
  reason: string;
  /** Additional selection metadata */
  metadata: {
    /** Number of passing candidates */
    passingCount: number;
    /** Number of candidates considered */
    totalConsidered: number;
    /** Agreement score (for majority vote) */
    agreementScore?: number;
    /** Solution length (for shortest correct) */
    solutionLength?: number;
  };
}

// ============================================================================
// Selection Strategies
// ============================================================================

/**
 * First Pass strategy - return first candidate that passed.
 */
function selectFirstPass(candidates: CandidateResult[]): SelectionResult {
  const passing = candidates.filter((c) => c.passed);

  if (passing.length === 0) {
    // Fallback to highest progress
    return selectHighestProgress(candidates);
  }

  // Sort by order (first in list wins)
  const selected = passing[0];

  return {
    selected,
    strategy: "first_pass",
    reason: `First passing candidate: ${selected.id}`,
    metadata: {
      passingCount: passing.length,
      totalConsidered: candidates.length,
    },
  };
}

/**
 * Shortest Correct strategy - among passing, prefer shorter solutions.
 * "Shorter" means fewer turns (less compute used).
 */
function selectShortestCorrect(candidates: CandidateResult[]): SelectionResult {
  const passing = candidates.filter((c) => c.passed);

  if (passing.length === 0) {
    // Fallback to highest progress
    return selectHighestProgress(candidates);
  }

  // Sort by turns (fewer is better), then by duration (faster is better)
  const sorted = [...passing].sort((a, b) => {
    if (a.turns !== b.turns) return a.turns - b.turns;
    return a.durationMs - b.durationMs;
  });

  const selected = sorted[0];

  return {
    selected,
    strategy: "shortest_correct",
    reason: `Shortest correct solution: ${selected.id} (${selected.turns} turns)`,
    metadata: {
      passingCount: passing.length,
      totalConsidered: candidates.length,
      solutionLength: selected.turns,
    },
  };
}

/**
 * Majority Vote strategy - among passing, prefer most common solution.
 * Since we don't have solution content to compare, we use progress scores
 * as a proxy for solution similarity.
 */
function selectMajorityVote(
  candidates: CandidateResult[],
  minAgreement: number = 0.5,
): SelectionResult {
  const passing = candidates.filter((c) => c.passed);

  if (passing.length === 0) {
    // Fallback to highest progress
    return selectHighestProgress(candidates);
  }

  // Group by progress score (rounded to 1 decimal)
  const progressGroups = new Map<string, CandidateResult[]>();
  for (const candidate of passing) {
    const key = candidate.progress.toFixed(1);
    if (!progressGroups.has(key)) {
      progressGroups.set(key, []);
    }
    progressGroups.get(key)!.push(candidate);
  }

  // Find largest group
  let largestGroup: CandidateResult[] = [];
  for (const group of progressGroups.values()) {
    if (group.length > largestGroup.length) {
      largestGroup = group;
    }
  }

  const agreementScore = largestGroup.length / passing.length;

  if (agreementScore < minAgreement) {
    // Not enough agreement, fall back to shortest correct
    return selectShortestCorrect(candidates);
  }

  // From the majority group, select shortest
  const sorted = [...largestGroup].sort((a, b) => a.turns - b.turns);
  const selected = sorted[0];

  return {
    selected,
    strategy: "majority_vote",
    reason: `Majority vote: ${largestGroup.length}/${passing.length} agree on ${selected.progress.toFixed(1)} progress`,
    metadata: {
      passingCount: passing.length,
      totalConsidered: candidates.length,
      agreementScore,
      solutionLength: selected.turns,
    },
  };
}

/**
 * Highest Progress strategy - for when no candidates pass.
 * Select the candidate with highest partial progress.
 */
function selectHighestProgress(candidates: CandidateResult[]): SelectionResult {
  if (candidates.length === 0) {
    return {
      selected: null,
      strategy: "highest_progress",
      reason: "No candidates to select from",
      metadata: {
        passingCount: 0,
        totalConsidered: 0,
      },
    };
  }

  // Sort by progress (highest first), then by turns (fewer is better)
  const sorted = [...candidates].sort((a, b) => {
    if (a.progress !== b.progress) return b.progress - a.progress;
    return a.turns - b.turns;
  });

  const selected = sorted[0];

  return {
    selected,
    strategy: "highest_progress",
    reason: `Highest progress: ${selected.id} (${(selected.progress * 100).toFixed(1)}%)`,
    metadata: {
      passingCount: candidates.filter((c) => c.passed).length,
      totalConsidered: candidates.length,
    },
  };
}

// ============================================================================
// Main Selector
// ============================================================================

/**
 * Select the best candidate using the specified strategy.
 *
 * @param candidates List of candidate results
 * @param options Selection options
 * @returns Selection result with chosen candidate and metadata
 */
export function selectCandidate(
  candidates: CandidateResult[],
  options: SelectionOptions = { strategy: "shortest_correct" },
): SelectionResult {
  const { strategy, minAgreement } = options;

  switch (strategy) {
    case "first_pass":
      return selectFirstPass(candidates);

    case "shortest_correct":
      return selectShortestCorrect(candidates);

    case "majority_vote":
      return selectMajorityVote(candidates, minAgreement ?? 0.5);

    case "highest_progress":
      return selectHighestProgress(candidates);

    default:
      // Default to shortest correct
      return selectShortestCorrect(candidates);
  }
}

/**
 * Auto-select the best strategy based on candidate distribution.
 *
 * - If many candidates pass with similar progress → majority vote
 * - If few candidates pass → shortest correct
 * - If none pass → highest progress
 */
export function autoSelectCandidate(
  candidates: CandidateResult[],
): SelectionResult {
  const passing = candidates.filter((c) => c.passed);

  if (passing.length === 0) {
    return selectHighestProgress(candidates);
  }

  if (passing.length >= 3) {
    // Enough passing to try majority vote
    return selectMajorityVote(candidates, 0.4);
  }

  // Few passing, use shortest correct
  return selectShortestCorrect(candidates);
}

/**
 * Format selection result as a summary string.
 */
export function formatSelectionSummary(result: SelectionResult): string {
  const lines: string[] = [];

  lines.push(`Selection Result:`);
  lines.push(`  Strategy: ${result.strategy}`);
  lines.push(`  Reason: ${result.reason}`);
  lines.push(`  Passing: ${result.metadata.passingCount}/${result.metadata.totalConsidered}`);

  if (result.selected) {
    lines.push(`\nSelected Candidate:`);
    lines.push(`  ID: ${result.selected.id}`);
    lines.push(`  Passed: ${result.selected.passed}`);
    lines.push(`  Progress: ${(result.selected.progress * 100).toFixed(1)}%`);
    lines.push(`  Turns: ${result.selected.turns}`);
  } else {
    lines.push(`\nNo candidate selected.`);
  }

  return lines.join("\n");
}

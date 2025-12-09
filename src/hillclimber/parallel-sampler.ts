/**
 * Parallel Sampling for Test-Time Compute (TTC)
 *
 * Samples multiple candidate solutions per turn and picks the best based on test progress.
 *
 * Key insight from TTC research:
 * - Sample N candidates instead of 1
 * - Verify all in parallel
 * - Pick best based on progress
 * - Expected improvement: 10-20% per turn
 */

import { mkdtempSync, rmSync, copyFileSync, existsSync, mkdirSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import type { TerminalBenchTask } from "../bench/terminal-bench.js";

export interface SamplingOptions {
  /** Number of candidates to sample */
  numSamples: number;
  /** Base workspace (will create N temp workspaces) */
  baseWorkspace: string;
  /** Task being solved */
  task: TerminalBenchTask;
  /** Temperature variation range (e.g., [0.3, 0.7]) */
  temperatureRange?: [number, number];
  /** Variation prompts for diversity */
  variationPrompts?: string[];
}

export interface CandidateResult {
  /** Candidate index */
  index: number;
  /** Temperature used */
  temperature: number;
  /** Variation prompt (if any) */
  variationHint?: string;
  /** Workspace path */
  workspace: string;
  /** Verification result */
  passed: boolean;
  progress: number;
  testsPassing: number;
  testsTotal: number;
  failedTests?: string[];
  /** Candidate solution (e.g., regex pattern) */
  solution?: string;
}

export interface SamplingResult {
  /** Best candidate */
  best: CandidateResult;
  /** All candidates (sorted by progress) */
  all: CandidateResult[];
  /** Average progress across all candidates */
  averageProgress: number;
  /** Improvement over previous best */
  improvement: number;
}

/**
 * Generate variation prompts for diversity.
 *
 * These are added to the FM prompt to encourage different approaches.
 */
export function generateVariationPrompts(numSamples: number): string[] {
  const basePrompts = [
    // Neutral baseline
    "",

    // Different strategies
    "Try a more conservative/defensive approach",
    "Try a more aggressive/comprehensive approach",

    // Different focuses
    "Focus on correctness over brevity",
    "Focus on edge case handling",
    "Focus on the most common cases first",

    // Different techniques
    "Consider using lookahead/lookbehind assertions",
    "Consider using character classes more carefully",
    "Consider using word boundaries",

    // Meta-hints
    "Think step-by-step about each constraint",
    "Start simple and add complexity",
  ];

  // Return numSamples prompts, cycling through base prompts if needed
  return Array.from({ length: numSamples }, (_, i) => basePrompts[i % basePrompts.length]);
}

/**
 * Generate temperature range for sampling.
 *
 * Spreads temperatures evenly across the range to encourage diversity.
 */
export function generateTemperatures(
  numSamples: number,
  range: [number, number] = [0.3, 0.7]
): number[] {
  const [min, max] = range;

  if (numSamples === 1) return [min];

  const step = (max - min) / (numSamples - 1);
  return Array.from({ length: numSamples }, (_, i) => min + i * step);
}

/**
 * Create temporary workspaces for parallel sampling.
 *
 * Each candidate gets its own isolated workspace.
 */
export function createSampleWorkspaces(
  baseWorkspace: string,
  numSamples: number
): string[] {
  const workspaces: string[] = [];

  for (let i = 0; i < numSamples; i++) {
    const sampleWorkspace = mkdtempSync(join(tmpdir(), `sample-${i}-`));

    // Copy base workspace contents to sample workspace
    if (existsSync(baseWorkspace)) {
      const { readdirSync, statSync, cpSync } = require("fs");
      const entries = readdirSync(baseWorkspace);
      for (const entry of entries) {
        const srcPath = join(baseWorkspace, entry);
        const destPath = join(sampleWorkspace, entry);
        if (statSync(srcPath).isDirectory()) {
          cpSync(srcPath, destPath, { recursive: true });
        } else {
          cpSync(srcPath, destPath);
        }
      }
    }

    workspaces.push(sampleWorkspace);
  }

  return workspaces;
}

/**
 * Cleanup sample workspaces.
 */
export function cleanupSampleWorkspaces(workspaces: string[]): void {
  for (const workspace of workspaces) {
    try {
      if (existsSync(workspace)) {
        rmSync(workspace, { recursive: true, force: true });
      }
    } catch (e) {
      console.warn(`[SAMPLER] Failed to cleanup workspace: ${workspace}`, e);
    }
  }
}

/**
 * Pick best candidate based on test progress.
 *
 * Primary: highest testsPassing
 * Tiebreaker: lowest index (first sample)
 */
export function pickBestCandidate(candidates: CandidateResult[]): CandidateResult {
  if (candidates.length === 0) {
    throw new Error("No candidates to pick from");
  }

  // Sort by progress (descending), then by index (ascending)
  const sorted = [...candidates].sort((a, b) => {
    if (b.testsPassing !== a.testsPassing) {
      return b.testsPassing - a.testsPassing;
    }
    return a.index - b.index;
  });

  return sorted[0];
}

/**
 * Calculate statistics for sampling result.
 */
export function calculateSamplingStats(
  candidates: CandidateResult[],
  previousBest: number = 0
): { best: CandidateResult; averageProgress: number; improvement: number } {
  const best = pickBestCandidate(candidates);
  const averageProgress = candidates.reduce((sum, c) => sum + c.progress, 0) / candidates.length;
  const improvement = best.progress - previousBest;

  return { best, averageProgress, improvement };
}

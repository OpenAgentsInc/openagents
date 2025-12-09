/**
 * Sampling Orchestrator - Integrates parallel sampling with MAP
 *
 * This module provides the high-level interface for running parallel sampling
 * within the MAP orchestrator workflow.
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { writeFileSync } from "fs";
import { join } from "path";
import { evaluateProgressWithDocker, type EvaluatorResult } from "./evaluator.js";
import {
  type SamplingOptions,
  type CandidateResult,
  type SamplingResult,
  createSampleWorkspaces,
  cleanupSampleWorkspaces,
  generateVariationPrompts,
  generateTemperatures,
  calculateSamplingStats,
} from "./parallel-sampler.js";

/**
 * Run parallel sampling for a specific subtask turn.
 *
 * Flow:
 * 1. Generate N prompts with variations
 * 2. Sample N candidates from FM
 * 3. Write candidates to N workspaces
 * 4. Verify all in parallel
 * 5. Pick best based on test progress
 * 6. Cleanup temp workspaces
 * 7. Apply best to main workspace
 *
 * @param getCandidateFn - Function to generate one candidate (given variation hint and temperature)
 * @param options - Sampling configuration
 * @returns Best candidate and stats
 */
export async function runParallelSampling(
  getCandidateFn: (variation: string, temperature: number, index: number) => Promise<string | null>,
  options: SamplingOptions & {
    currentBestProgress?: number;
    solutionFilename?: string; // e.g., "regex.txt"
  }
): Promise<SamplingResult> {
  const {
    numSamples,
    baseWorkspace,
    task,
    temperatureRange = [0.3, 0.7],
    currentBestProgress = 0,
    solutionFilename = "regex.txt",
  } = options;

  // Generate variation prompts and temperatures
  const variations = generateVariationPrompts(numSamples);
  const temperatures = generateTemperatures(numSamples, temperatureRange);

  console.log(`[SAMPLER] Sampling ${numSamples} candidates...`);

  // Step 1: Sample N candidates from FM in parallel
  const candidateSolutions = await Promise.all(
    variations.map((variation, i) =>
      getCandidateFn(variation, temperatures[i], i).catch((e) => {
        console.warn(`[SAMPLER] Candidate ${i} failed: ${e}`);
        return null;
      })
    )
  );

  // Filter out failed candidates
  const validCandidates = candidateSolutions
    .map((solution, i) => ({ solution, index: i }))
    .filter((c) => c.solution !== null) as { solution: string; index: number }[];

  if (validCandidates.length === 0) {
    throw new Error("All candidates failed to generate");
  }

  console.log(`[SAMPLER] Generated ${validCandidates.length}/${numSamples} valid candidates`);

  // Step 2: Create temp workspaces for each candidate
  const sampleWorkspaces = createSampleWorkspaces(baseWorkspace, validCandidates.length);

  // Step 3: Write each candidate to its workspace
  for (let i = 0; i < validCandidates.length; i++) {
    const { solution } = validCandidates[i];
    const workspace = sampleWorkspaces[i];
    const solutionPath = join(workspace, solutionFilename);
    writeFileSync(solutionPath, solution, "utf-8");
  }

  // Step 4: Verify all candidates in parallel
  console.log(`[SAMPLER] Verifying ${validCandidates.length} candidates in parallel...`);

  const verificationResults = await Promise.all(
    sampleWorkspaces.map(async (workspace, i) => {
      try {
        const result = await Effect.runPromise(
          evaluateProgressWithDocker(task, workspace).pipe(Effect.provide(BunContext.layer))
        );
        return {
          workspace,
          ...result,
        };
      } catch (e) {
        console.warn(`[SAMPLER] Verification ${i} failed: ${e}`);
        return {
          workspace,
          passed: false,
          progress: 0,
          testsPassing: 0,
          testsTotal: 0,
        };
      }
    })
  );

  // Step 5: Build candidate results
  const candidateResults: CandidateResult[] = validCandidates.map((c, i) => {
    const result: CandidateResult = {
      index: c.index,
      temperature: temperatures[c.index],
      variationHint: variations[c.index],
      workspace: sampleWorkspaces[i],
      passed: verificationResults[i].passed,
      progress: verificationResults[i].progress,
      testsPassing: verificationResults[i].testsPassing,
      testsTotal: verificationResults[i].testsTotal,
      solution: c.solution,
    };
    // failedTests is optional, only add if present
    const verificationResult = verificationResults[i] as EvaluatorResult & { failedTests?: string[] };
    if (verificationResult.failedTests && Array.isArray(verificationResult.failedTests)) {
      result.failedTests = verificationResult.failedTests;
    }
    return result;
  });

  // Step 6: Calculate stats and pick best
  const { best, averageProgress, improvement } = calculateSamplingStats(
    candidateResults,
    currentBestProgress
  );

  console.log(`[SAMPLER] Best: ${best.testsPassing}/${best.testsTotal} (${(best.progress * 100).toFixed(1)}%)`);
  console.log(`[SAMPLER] Average: ${(averageProgress * 100).toFixed(1)}%`);
  console.log(`[SAMPLER] Improvement: +${(improvement * 100).toFixed(1)}%`);

  // Log all candidates for analysis
  for (const c of candidateResults) {
    console.log(
      `[SAMPLER]   Candidate ${c.index}: ${c.testsPassing}/${c.testsTotal} ` +
      `(${(c.progress * 100).toFixed(1)}%, temp=${c.temperature.toFixed(2)})`
    );
  }

  // Step 7: Apply best candidate to main workspace
  if (best.solution) {
    const mainSolutionPath = join(baseWorkspace, solutionFilename);
    writeFileSync(mainSolutionPath, best.solution, "utf-8");
    console.log(`[SAMPLER] Applied best candidate to main workspace`);
  }

  // Step 8: Cleanup temp workspaces
  cleanupSampleWorkspaces(sampleWorkspaces);

  // Sort all by progress for result
  const all = candidateResults.sort((a, b) => b.progress - a.progress);

  return {
    best,
    all,
    averageProgress,
    improvement,
  };
}

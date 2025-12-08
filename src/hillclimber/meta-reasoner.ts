/**
 * HillClimber Meta-Reasoner
 *
 * Uses OpenRouter models to propose config changes based on run results.
 * Primary: x-ai/grok-4.1-fast:free (free, unlimited)
 * Backup: openrouter/auto (every 10th run for deeper analysis)
 */

import { Effect, Layer } from "effect";
import {
  OpenRouterInference,
  OpenRouterInferenceLive,
} from "../llm/openrouter-inference.js";
import { openRouterLive } from "../llm/openrouter-http.js";
import { InferenceStoreLive } from "../llm/inference-store.js";
import type {
  HillClimberConfig,
  HillClimberConfigInput,
  ConfigChange,
  TaskRunResult,
  HillClimberRun,
} from "./types.js";
import type { TerminalBenchTask } from "../bench/terminal-bench.js";

// ============================================================================
// Historical Context
// ============================================================================

/**
 * Historical context from the database for smarter meta-reasoning.
 */
export interface HistoricalContext {
  /** Recent runs for this task (newest first) */
  recentRuns: HillClimberRun[];
  /** Total runs for this task */
  totalRuns: number;
  /** Total passes for this task */
  totalPasses: number;
  /** Best score ever achieved */
  bestScore: number;
  /** Best hint that achieved the best score (if any) */
  bestHint: string | null;
  /** All unique hints that have been tried */
  triedHints: string[];
}

// ============================================================================
// Constants
// ============================================================================

import { DEFAULT_FREE_MODEL } from "../llm/openrouter-inference.js";

/** Free model for meta-reasoning (uses DEFAULT_FREE_MODEL from inference service) */
export const FREE_MODEL = DEFAULT_FREE_MODEL;

/** Auto model for deeper analysis (use sparingly) */
export const AUTO_MODEL = "openrouter/auto";

/** How often to use auto model (every Nth run) */
export const AUTO_MODEL_FREQUENCY = 10;

// ============================================================================
// Prompt Templates
// ============================================================================

/**
 * Build historical context section for the prompt.
 */
const buildHistorySection = (history: HistoricalContext | null): string => {
  if (!history || history.totalRuns === 0) {
    return "History: This is the first run for this task.";
  }

  const passRate = ((history.totalPasses / history.totalRuns) * 100).toFixed(0);

  let historyText = `History:
- Total runs: ${history.totalRuns}, Passes: ${history.totalPasses} (${passRate}% pass rate)
- Best score ever: ${history.bestScore}`;

  if (history.bestHint) {
    historyText += `\n- Best performing hint: "${history.bestHint}"`;
  }

  if (history.triedHints.length > 0) {
    const hintsList = history.triedHints
      .filter(h => h) // Filter out null/empty
      .slice(0, 5) // Show max 5
      .map(h => `"${h.slice(0, 50)}${h.length > 50 ? '...' : ''}"`)
      .join(", ");
    if (hintsList) {
      historyText += `\n- Previously tried hints: ${hintsList}`;
    }
  }

  // Show recent run outcomes
  if (history.recentRuns.length > 0) {
    const recentOutcomes = history.recentRuns
      .slice(0, 5)
      .map(r => r.passed ? "PASS" : "FAIL")
      .join(", ");
    historyText += `\n- Recent outcomes (newest first): ${recentOutcomes}`;
  }

  return historyText;
};

/**
 * Build the meta-reasoning prompt for suggesting config changes.
 */
const buildMetaPrompt = (
  task: TerminalBenchTask,
  config: HillClimberConfig,
  result: TaskRunResult,
  history: HistoricalContext | null,
): string => {
  const stepSummaryText =
    result.stepSummary.length > 0
      ? result.stepSummary.join("\n")
      : "No step summary available";

  const historySection = buildHistorySection(history);

  return `You are a tiny tuning agent for a coding benchmark.

Task ID: ${task.id}
Task description: ${task.description.slice(0, 300)}${task.description.length > 300 ? "..." : ""}

Current hint: ${config.hint || "none"}
Last run: ${result.passed ? "PASSED" : "FAILED"} in ${result.turns} turns
${result.errorMessage ? `Error: ${result.errorMessage}` : ""}

Step summary:
${stepSummaryText}

${historySection}

Based on this, suggest ONE small change to the hint that might improve performance.
Reply with ONLY the new hint text (1-2 sentences max), or "KEEP" if no change needed.

Guidelines:
- If the task passed, only suggest changes if turns > 15 (to improve efficiency)
- If the task failed, analyze the step summary to understand what went wrong
- Keep hints concise and actionable
- Focus on the specific task requirements
- DO NOT repeat hints that have already been tried and failed
- If a hint worked before (led to PASS), consider building on it

Examples of good hints:
- "Write the regex directly to /app/regex.txt without reading any files first."
- "Use grep to count matching lines rather than parsing manually."
- "Create the output file with the exact format: one number per line."

Reply:`;
};

// ============================================================================
// Meta-Reasoning
// ============================================================================

/**
 * Propose a config change based on run results.
 *
 * @param task The TB task that was run
 * @param config The current config
 * @param result The run result
 * @param runNumber The current run number (to decide which model to use)
 * @returns Effect that resolves to the proposed config change
 */
export const proposeConfigChange = (
  task: TerminalBenchTask,
  config: HillClimberConfig,
  result: TaskRunResult,
  runNumber: number,
): Effect.Effect<ConfigChange, Error> =>
  Effect.gen(function* () {
    const inference = yield* OpenRouterInference;

    // Use auto model every Nth run for deeper analysis
    const useAuto = runNumber % AUTO_MODEL_FREQUENCY === 0;
    const model = useAuto ? AUTO_MODEL : FREE_MODEL;

    console.log(
      `[MetaReasoner] Using ${useAuto ? "auto" : "free"} model for run #${runNumber}`,
    );

    const prompt = buildMetaPrompt(task, config, result);

    const response = yield* inference.send(
      model,
      [{ role: "user", content: prompt }],
      { temperature: 0.3, maxTokens: 200 },
    );

    const content = response.choices[0]?.message?.content?.trim() ?? "";

    console.log(`[MetaReasoner] Response: ${content.slice(0, 100)}...`);

    // Parse the response
    if (
      content.toUpperCase() === "KEEP" ||
      content.toUpperCase().includes("KEEP THE CURRENT") ||
      content.toUpperCase().includes("NO CHANGE")
    ) {
      return {
        type: "keep" as const,
        reasoning: content,
      };
    }

    // Extract the new hint (the response should be just the hint text)
    // Remove any quotes around the hint
    let newHint = content.replace(/^["']|["']$/g, "").trim();

    // If response is too long, it might not be a valid hint
    if (newHint.length > 500) {
      console.log(`[MetaReasoner] Response too long, keeping current config`);
      return {
        type: "keep" as const,
        reasoning: "Response too long, likely not a valid hint",
      };
    }

    // If response is empty, keep current
    if (!newHint) {
      return {
        type: "keep" as const,
        reasoning: "Empty response",
      };
    }

    return {
      type: "update_hint" as const,
      newHint,
      reasoning: content,
    };
  }).pipe(
    Effect.provide(
      OpenRouterInferenceLive.pipe(
        Layer.provideMerge(openRouterLive),
        Layer.provideMerge(InferenceStoreLive),
      ),
    ),
  );

/**
 * Apply a config change to create a new config input.
 */
export const applyConfigChange = (
  config: HillClimberConfig,
  change: ConfigChange,
): HillClimberConfigInput => {
  switch (change.type) {
    case "keep":
      return {
        taskId: config.taskId,
        hint: config.hint,
        useSkills: config.useSkills,
        maxTurnsOverride: config.maxTurnsOverride,
      };

    case "update_hint":
      return {
        taskId: config.taskId,
        hint: change.newHint ?? config.hint,
        useSkills: config.useSkills,
        maxTurnsOverride: config.maxTurnsOverride,
      };

    case "toggle_skills":
      return {
        taskId: config.taskId,
        hint: config.hint,
        useSkills: change.newUseSkills ?? !config.useSkills,
        maxTurnsOverride: config.maxTurnsOverride,
      };

    case "adjust_turns":
      return {
        taskId: config.taskId,
        hint: config.hint,
        useSkills: config.useSkills,
        maxTurnsOverride: change.newMaxTurns ?? config.maxTurnsOverride,
      };

    default:
      return {
        taskId: config.taskId,
        hint: config.hint,
        useSkills: config.useSkills,
        maxTurnsOverride: config.maxTurnsOverride,
      };
  }
};

// ============================================================================
// Heuristic Fallback
// ============================================================================

/**
 * Default hints for specific tasks when no better hint is available.
 */
const DEFAULT_TASK_HINTS: Record<string, string> = {
  "regex-log":
    "Write the regex directly to /app/regex.txt. The regex should match dates in YYYY-MM-DD format.",
};

/**
 * Propose a config change using heuristics (no LLM).
 * Used as fallback when OpenRouter is unavailable.
 */
export const proposeHeuristicChange = (
  task: TerminalBenchTask,
  config: HillClimberConfig,
  result: TaskRunResult,
): ConfigChange => {
  // If never passed and hint is empty, add a default hint
  if (!result.passed && !config.hint) {
    const defaultHint = DEFAULT_TASK_HINTS[task.id];
    if (defaultHint) {
      return {
        type: "update_hint",
        newHint: defaultHint,
        reasoning: "Added default hint for task",
      };
    }
  }

  // If passed but took many turns, suggest efficiency hint
  if (result.passed && result.turns > 20) {
    return {
      type: "update_hint",
      newHint: (config.hint || "") + " Be direct and efficient.",
      reasoning: "Task passed but took too many turns",
    };
  }

  // If failed with specific error patterns, adjust hint
  const errorText = (result.errorMessage || "") + result.stepSummary.join(" ");

  if (errorText.includes("file not found") || errorText.includes("no such file")) {
    return {
      type: "update_hint",
      newHint: (config.hint || "") + " Check file paths carefully.",
      reasoning: "File not found error detected",
    };
  }

  if (errorText.includes("permission denied")) {
    return {
      type: "update_hint",
      newHint: (config.hint || "") + " Ensure you have write permissions.",
      reasoning: "Permission denied error detected",
    };
  }

  if (errorText.includes("syntax error") || errorText.includes("parse error")) {
    return {
      type: "update_hint",
      newHint: (config.hint || "") + " Double-check syntax and escaping.",
      reasoning: "Syntax error detected",
    };
  }

  // No change needed
  return {
    type: "keep",
    reasoning: "No heuristic change suggested",
  };
};

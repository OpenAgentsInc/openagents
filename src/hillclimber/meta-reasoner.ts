/**
 * HillClimber Meta-Reasoner
 *
 * Uses OpenRouter models to propose config changes based on run results.
 * Features:
 * - JSON-formatted prompts with strict constraints
 * - Task-specific guardrails (forbidden/required strings)
 * - Hint validation and sanitization
 * - Change gating (only update if meaningfully different)
 * - Primary: Default free model via free:true option
 * - Backup: openrouter/auto (every 10th run for deeper analysis)
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
import { log, logError } from "./logger.js";

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

// Note: We use free:true option instead of hardcoding model names
// This lets the OpenRouter service automatically select the default free model

/** Auto model for deeper analysis (use sparingly) */
export const AUTO_MODEL = "openrouter/auto";

/** How often to use auto model (every Nth run) */
export const AUTO_MODEL_FREQUENCY = 10;

/** Maximum hint length in characters */
export const MAX_HINT_LENGTH = 150;

// ============================================================================
// Task-Specific Constraints
// ============================================================================

/**
 * Task-specific constraints for hint generation.
 * Prevents invalid hints like reading forbidden files or using unavailable tools.
 */
interface TaskConstraints {
  /** Forbidden strings that must not appear in hints */
  forbidden: string[];
  /** Required strings that should appear in hints (if applicable) */
  required?: string[];
  /** Example of a good hint for this task */
  example?: string;
}

const TASK_CONSTRAINTS: Record<string, TaskConstraints> = {
  "path-tracing": {
    forbidden: ["image.ppm", "/app/image.ppm", "read image", "read the image"],
    example: "Write image.c that generates a PPM with same dimensions and color range as a typical Doom frame; you can hardcode simple scene.",
  },
  "regex-log": {
    forbidden: ["python", "read", "parse", "log file"],
    required: ["/app/regex.txt", "write"],
    example: "Write regex to /app/regex.txt that matches IPv4 addresses and YYYY-MM-DD dates.",
  },
  "video-processing": {
    forbidden: ["primer3", "python", "ssh", "sudo"],
    example: "Use OpenCV to compute frame differences; detect jump via large changes; write results to /app/output.toml.",
  },
  "dna-assembly": {
    forbidden: ["primer3", "python", "external tool"],
    example: "Write a script that finds overlapping DNA sequences and assembles them into a single sequence.",
  },
  "model-extraction-relu-logits": {
    forbidden: ["weights = 1.0", "all weights", "initialized to 1"],
    example: "Extract the neural network architecture and weights from the provided model file.",
  },
};

/**
 * Get constraints for a specific task, or default constraints.
 */
const getTaskConstraints = (taskId: string): TaskConstraints => {
  return TASK_CONSTRAINTS[taskId] ?? {
    forbidden: ["primer3", "python", "ssh", "sudo", "external tool"],
  };
};

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
 * Uses strict JSON format with task-specific constraints.
 */
const buildMetaPrompt = (
  task: TerminalBenchTask,
  config: HillClimberConfig,
  result: TaskRunResult,
  history: HistoricalContext | null,
): string => {
  const stepSummaryText =
    result.stepSummary.length > 0
      ? result.stepSummary.slice(-3).join("\n  * ") // Last 3 steps
      : "No step summary available";

  const historySection = buildHistorySection(history);
  const constraints = getTaskConstraints(task.id);

  const forbiddenList = constraints.forbidden.map(f => `"${f}"`).join(", ");
  const requiredList = constraints.required?.map(r => `"${r}"`).join(", ") ?? "none";

  return `You are tuning a very dumb coding agent with limited tools: write_file, read_file, run_command, edit_file.

Task ID: ${task.id}
Task description (truncated):
* ${task.description.slice(0, 200)}${task.description.length > 200 ? "..." : ""}

Last run summary:
* Verification: ${result.passed ? "PASSED" : "FAILED"}
* Turns used: ${result.turns}
${result.errorMessage ? `* Error: ${result.errorMessage}` : ""}
* Last 3 steps:
  * ${stepSummaryText}

Existing hint:
* ${config.hint || "none"}

${historySection}

CONSTRAINTS:
* The agent CANNOT read /app/image.ppm for path-tracing tasks.
* It CANNOT rely on tools like primer3/python in this environment.
* Hint must be <= ${MAX_HINT_LENGTH} characters.
* Hint MUST be directly actionable given its tools (e.g. "write a regex to /app/regex.txt that does X").
* DO NOT invent network architecture constants unless the task explicitly states them.
* FORBIDDEN strings (must NOT appear): ${forbiddenList}
${constraints.required ? `* REQUIRED strings (should appear): ${requiredList}` : ""}
${constraints.example ? `* Example good hint: "${constraints.example}"` : ""}

Respond ONLY with a JSON object:
{ "hint": "<one-sentence hint>", "reason": "<brief why>" }

If no change is needed, respond with:
{ "hint": "KEEP", "reason": "<why>" }`;
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
    const model = useAuto ? AUTO_MODEL : "openrouter/auto"; // Model string (free:true will override to default free model)

    log(
      `[MetaReasoner] Using ${useAuto ? "auto" : "free"} model for run #${runNumber} (model: ${model}, free: ${!useAuto})`,
    );

    const prompt = buildMetaPrompt(task, config, result);
    log(`[MetaReasoner] Prompt length: ${prompt.length} chars`);

    let response;
    try {
      response = yield* inference.send(
        model,
        [{ role: "user", content: prompt }],
        {
          free: !useAuto, // Use free model when not using auto
          temperature: 0.3,
          maxTokens: 200
        },
      );
    } catch (error) {
      logError(`[MetaReasoner] API call failed`, error instanceof Error ? error : new Error(String(error)));
      throw error;
    }

    // Log full response structure for debugging
    log(`[MetaReasoner] Raw response structure: ${JSON.stringify({
      id: response.id,
      model: response.model,
      choicesCount: response.choices?.length ?? 0,
      firstChoice: response.choices?.[0] ? {
        messageRole: response.choices[0].message?.role,
        contentLength: response.choices[0].message?.content?.length ?? 0,
        contentPreview: response.choices[0].message?.content?.slice(0, 50) ?? "(null/empty)"
      } : null
    }, null, 2)}`);

    const content = response.choices[0]?.message?.content?.trim() ?? "";
    const actualModel = response.model ?? model; // Get the actual model that was used

    // Always log the FULL response content (not truncated)
    if (content.length === 0) {
      logError(`[MetaReasoner] Response: (EMPTY) - no content from model ${actualModel}`);
      logError(`[MetaReasoner] Usage: ${JSON.stringify(response.usage, null, 2)}`);
      logError(`[MetaReasoner] This model may be a reasoning-only model. Trying fallback...`);

      // If we got tokens but no content, this might be a reasoning model issue
      // Try using a different free model that actually returns content
      if (response.usage?.completion_tokens && response.usage.completion_tokens > 0) {
        logError(`[MetaReasoner] Model generated ${response.usage.completion_tokens} tokens but returned empty content. This is likely a model issue.`);
      }

      // Don't throw - let it fall through to "keep" behavior
    } else {
      log(`[MetaReasoner] Response (${content.length} chars, model: ${actualModel}):`);
      log(`[MetaReasoner] ${content}`);
    }

    // Parse JSON response
    let parsedResponse: { hint?: string; reason?: string } | null = null;
    try {
      // Try to parse as JSON first
      parsedResponse = JSON.parse(content);
    } catch {
      // If not JSON, treat as plain text (backward compatibility)
      log(`[MetaReasoner] Response is not JSON, parsing as plain text`);
    }

    let newHint: string;
    let reason: string;

    if (parsedResponse && typeof parsedResponse.hint === "string") {
      newHint = parsedResponse.hint.trim();
      reason = parsedResponse.reason || "No reason provided";
    } else {
      // Fallback: parse as plain text
      if (
        content.toUpperCase() === "KEEP" ||
        content.toUpperCase().includes("KEEP THE CURRENT") ||
        content.toUpperCase().includes("NO CHANGE")
      ) {
        return {
          type: "keep" as const,
          reasoning: content,
          model: actualModel,
        };
      }
      newHint = content.replace(/^["']|["']$/g, "").trim();
      reason = content;
    }

    // Check if it's a KEEP response
    if (newHint.toUpperCase() === "KEEP") {
      return {
        type: "keep" as const,
        reasoning: reason,
        model: actualModel,
      };
    }

    // Validate and sanitize the hint
    const validation = validateHint(newHint, task.id);
    if (!validation.valid) {
      logError(`[MetaReasoner] Hint validation failed: ${validation.reason}`);
      return {
        type: "keep" as const,
        reasoning: `Invalid hint rejected: ${validation.reason}`,
        model: actualModel,
      };
    }

    newHint = validation.sanitized;

    // Check if hint is meaningfully different from current
    if (config.hint && !isHintMeaningfullyDifferent(config.hint, newHint)) {
      log(`[MetaReasoner] New hint is too similar to current, keeping current config`);
      return {
        type: "keep" as const,
        reasoning: "New hint too similar to current hint",
        model: actualModel,
      };
    }

    return {
      type: "update_hint" as const,
      newHint,
      reasoning: reason,
      model: actualModel,
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
// Hint Validation
// ============================================================================

/**
 * Validation result for a hint.
 */
interface HintValidation {
  valid: boolean;
  reason?: string;
  sanitized: string;
}

/**
 * Validate and sanitize a hint according to task constraints.
 */
const validateHint = (hint: string, taskId: string): HintValidation => {
  // Check length
  if (hint.length > MAX_HINT_LENGTH) {
    return {
      valid: false,
      reason: `Hint too long (${hint.length} > ${MAX_HINT_LENGTH} chars)`,
      sanitized: hint.slice(0, MAX_HINT_LENGTH),
    };
  }

  if (hint.length === 0) {
    return {
      valid: false,
      reason: "Hint is empty",
      sanitized: "",
    };
  }

  const constraints = getTaskConstraints(taskId);
  const hintLower = hint.toLowerCase();

  // Check for forbidden strings
  for (const forbidden of constraints.forbidden) {
    if (hintLower.includes(forbidden.toLowerCase())) {
      return {
        valid: false,
        reason: `Contains forbidden string: "${forbidden}"`,
        sanitized: hint,
      };
    }
  }

  // Check for required strings (if any)
  if (constraints.required) {
    const hasRequired = constraints.required.some(req =>
      hintLower.includes(req.toLowerCase())
    );
    if (!hasRequired) {
      log(`[MetaReasoner] Warning: Hint missing required strings, but accepting anyway`);
    }
  }

  // Sanitize: remove extra whitespace
  const sanitized = hint.replace(/\s+/g, " ").trim();

  return {
    valid: true,
    sanitized,
  };
};

/**
 * Check if two hints are meaningfully different.
 * Returns true if they differ significantly (not just minor word changes).
 */
const isHintMeaningfullyDifferent = (oldHint: string, newHint: string): boolean => {
  // Normalize hints for comparison
  const normalize = (h: string) => h.toLowerCase().replace(/\s+/g, " ").trim();
  const oldNorm = normalize(oldHint);
  const newNorm = normalize(newHint);

  // If identical after normalization, not different
  if (oldNorm === newNorm) {
    return false;
  }

  // Calculate word overlap
  const oldWords = new Set(oldNorm.split(/\s+/));
  const newWords = new Set(newNorm.split(/\s+/));
  const intersection = new Set([...oldWords].filter(w => newWords.has(w)));
  const union = new Set([...oldWords, ...newWords]);

  // If more than 80% word overlap, consider them too similar
  const similarity = intersection.size / union.size;
  if (similarity > 0.8) {
    return false;
  }

  // Check character-level difference
  const charDiff = Math.abs(oldNorm.length - newNorm.length) / Math.max(oldNorm.length, newNorm.length);
  if (charDiff < 0.2 && similarity > 0.6) {
    return false; // Too similar
  }

  return true;
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

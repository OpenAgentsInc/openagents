/**
 * TestGen Meta-Reasoner
 *
 * Uses OpenRouter models to propose test generation config changes based on run results.
 * Similar to HillClimber meta-reasoner but focused on test generation quality.
 */

import { Effect } from "effect"
import { InferenceStoreLive } from "../llm/inference-store.js"
import { openRouterLive } from "../llm/openrouter-http.js"
import {
    OpenRouterInference, OpenRouterInferenceLive
} from "../llm/openrouter-inference.js"
import { log, logError } from "./logger.js"
import { FREE_MODELS } from "./meta-reasoner.js"

import type { TestGenAnalysis } from "./testgen-analyzer.js"

import type {
  TestGenConfig,
  TestGenConfigInput,
  TestGenConfigChange,
  TestGenRun,
} from "./testgen-types.js";
// ============================================================================
// Prompt Building
// ============================================================================

/**
 * Build meta-reasoning prompt for test generation config optimization.
 */
const buildTestGenMetaPrompt = (
  config: TestGenConfig,
  recentRuns: TestGenRun[],
  lastAnalysis: TestGenAnalysis,
  taskType: string,
): string => {
  const recentScores = recentRuns.map((r) => r.score).slice(0, 5);
  const avgScore = recentScores.length > 0
    ? recentScores.reduce((sum, s) => sum + s, 0) / recentScores.length
    : 0;

  const patterns: string[] = [];
  if (lastAnalysis.categoryBalance < 0.6) {
    patterns.push("- Category balance is low (tests are unevenly distributed)");
  }
  if (lastAnalysis.antiCheatCoverage < 0.7) {
    patterns.push("- Anti-cheat coverage is low (missing tests for prohibited tools)");
  }
  if (lastAnalysis.tokenEfficiency < 0.5) {
    patterns.push("- Token efficiency is low (spending too many tokens for quality)");
  }
  if (lastAnalysis.reflectionEffectiveness < 0.5) {
    patterns.push("- Reflections are not leading to new tests");
  }

  return `You are optimizing a test generation system. Your goal is to improve test quality over time.

Current config:
- Temperature: ${config.temperature}
- Min tests per category: ${config.minTestsPerCategory}
- Max tests per category: ${config.maxTestsPerCategory}
- Max rounds per category: ${config.maxRoundsPerCategory}
- Environment weight: ${config.environmentWeight}
- Anti-cheat weight: ${config.antiCheatWeight}
- Precision weight: ${config.precisionWeight}
- Primary model: ${config.primaryModel}
- Reflection model: ${config.reflectionModel}

Recent performance (last ${recentRuns.length} runs):
${recentRuns.slice(0, 5).map((r, i) =>
    `- Run ${i + 1}: Score ${r.score}, comprehensiveness=${r.comprehensivenessScore?.toFixed(1) ?? "N/A"}, balance=${r.categoryBalance?.toFixed(2) ?? "N/A"}, anti-cheat=${r.antiCheatCoverage?.toFixed(2) ?? "N/A"}, efficiency=${r.tokenEfficiency?.toFixed(2) ?? "N/A"}`
  ).join("\n")}

Average score: ${avgScore.toFixed(0)}/1000

Last run analysis:
- Category balance: ${lastAnalysis.categoryBalance.toFixed(2)} (target: 0.8+)
- Anti-cheat coverage: ${lastAnalysis.antiCheatCoverage.toFixed(2)} (target: 0.9+)
- Parameter discovery: ${lastAnalysis.parameterDiscovery.toFixed(2)} (target: 0.8+)
- Reflection effectiveness: ${lastAnalysis.reflectionEffectiveness.toFixed(2)} (target: 0.7+)
- Token efficiency: ${lastAnalysis.tokenEfficiency.toFixed(2)} (target: 0.6+)

Patterns observed:
${patterns.length > 0 ? patterns.join("\n") : "- No major issues detected"}

Task type: ${taskType}

What should we change to improve overall quality? Consider:
1. Adjusting temperature (lower = more focused, higher = more diverse)
2. Changing min/max tests per category (more tests = better coverage but more tokens)
3. Adjusting max rounds per category (more rounds = more refinement but slower)
4. Changing strategy weights (environment vs description, anti-cheat emphasis)
5. Model selection (local vs claude for different phases)

**IMPORTANT: Guardrail Constraints**
To ensure stable evolution, changes must be incremental:
- Temperature: Can change by ±0.1 maximum (e.g., 0.3 → 0.2 or 0.3 → 0.4)
- Min/Max tests per category: Can change by ±1 maximum (e.g., 2 → 3 or 5 → 4)
- Max rounds per category: Can change by ±1 maximum (e.g., 3 → 4 or 5 → 4)
- Weights: Can change by ±0.1 maximum (e.g., 0.7 → 0.8 or 0.7 → 0.6)

**Examples of VALID changes:**
- minTestsPerCategory: 2 → 3 (change of +1, within limit)
- maxTestsPerCategory: 5 → 6 (change of +1, within limit)
- temperature: 0.3 → 0.4 (change of +0.1, within limit)
- environmentWeight: 0.7 → 0.8 (change of +0.1, within limit)
- antiCheatWeight: 0.8 → 0.9 (change of +0.1, within limit)

**Examples of INVALID changes (will be rejected):**
- minTestsPerCategory: 2 → 4 (change of +2, exceeds ±1 limit)
- maxTestsPerCategory: 5 → 8 (change of +3, exceeds ±1 limit)
- temperature: 0.3 → 0.6 (change of +0.3, exceeds ±0.1 limit)

**When to propose changes:**
- If scores are stagnant or declining, propose small adjustments
- If specific metrics are low (balance, anti-cheat, efficiency), adjust relevant weights
- If token efficiency is low, consider reducing test counts or rounds
- If comprehensiveness is low, consider increasing test counts or rounds
- Temperature adjustments can help with diversity vs focus tradeoff

Propose SMALL, INCREMENTAL changes. If you want a larger change, propose it in steps over multiple iterations. Don't be too conservative - small improvements add up over time.

Return JSON with this exact structure:
{
  "type": "keep" | "update_params" | "update_prompts" | "update_weights",
  "changes": {
    "temperature": 0.3,
    "minTestsPerCategory": 2,
    "maxTestsPerCategory": 5,
    "maxRoundsPerCategory": 3,
    "environmentWeight": 0.7,
    "antiCheatWeight": 0.8,
    "precisionWeight": 0.6,
    "primaryModel": "local",
    "reflectionModel": "local"
  },
  "reasoning": "Explanation of why these changes should improve quality"
}

Only include fields in "changes" that you want to modify. If no changes are needed, use type "keep" with empty changes.`;
};

// ============================================================================
// Response Parsing
// ============================================================================

/**
 * Parse LLM response into config change proposal.
 */
const parseTestGenConfigChange = (
  response: string,
  model: string,
): TestGenConfigChange => {
  try {
    // Extract JSON from response (handle markdown code blocks)
    let jsonStr = response.trim();
    if (jsonStr.includes("```")) {
      const match = jsonStr.match(/```(?:json)?\s*(\{[\s\S]*\})\s*```/);
      if (match) {
        jsonStr = match[1];
      }
    }

    const parsed = JSON.parse(jsonStr);

    if (parsed.type === "keep") {
      return {
        type: "keep",
        reasoning: parsed.reasoning || "No changes needed",
        model,
      };
    }

    // Validate changes object
    const changes: Partial<TestGenConfigInput> = {};
    if (parsed.changes) {
      if (parsed.changes.temperature !== undefined) {
        changes.temperature = Math.max(0, Math.min(1, Number(parsed.changes.temperature)));
      }
      if (parsed.changes.minTestsPerCategory !== undefined) {
        changes.minTestsPerCategory = Math.max(1, Number(parsed.changes.minTestsPerCategory));
      }
      if (parsed.changes.maxTestsPerCategory !== undefined) {
        changes.maxTestsPerCategory = Math.max(1, Number(parsed.changes.maxTestsPerCategory));
      }
      if (parsed.changes.maxRoundsPerCategory !== undefined) {
        changes.maxRoundsPerCategory = Math.max(1, Number(parsed.changes.maxRoundsPerCategory));
      }
      if (parsed.changes.environmentWeight !== undefined) {
        changes.environmentWeight = Math.max(0, Math.min(1, Number(parsed.changes.environmentWeight)));
      }
      if (parsed.changes.antiCheatWeight !== undefined) {
        changes.antiCheatWeight = Math.max(0, Math.min(1, Number(parsed.changes.antiCheatWeight)));
      }
      if (parsed.changes.precisionWeight !== undefined) {
        changes.precisionWeight = Math.max(0, Math.min(1, Number(parsed.changes.precisionWeight)));
      }
      if (parsed.changes.primaryModel !== undefined) {
        changes.primaryModel = parsed.changes.primaryModel === "claude" ? "claude" : "local";
      }
      if (parsed.changes.reflectionModel !== undefined) {
        changes.reflectionModel = parsed.changes.reflectionModel === "claude" ? "claude" : "local";
      }
    }

    const result: TestGenConfigChange = {
      type: (parsed.type || "update_params") as TestGenConfigChange["type"],
      reasoning: parsed.reasoning || "No reasoning provided",
      model,
    };
    if (Object.keys(changes).length > 0) {
      result.changes = changes;
    }
    return result;
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    logError("Failed to parse meta-reasoner response", error instanceof Error ? error : new Error(String(error)));
    return {
      type: "keep" as const,
      reasoning: `Parse error: ${errorMessage}`,
      model,
    };
  }
};

// ============================================================================
// Main Meta-Reasoning Function
// ============================================================================

/**
 * Propose a config change based on recent runs and analysis.
 */
export const proposeTestGenConfigChange = (
  config: TestGenConfig,
  recentRuns: TestGenRun[],
  lastAnalysis: TestGenAnalysis,
  taskType: string = "_global_",
  modelOverride?: string,
): Effect.Effect<TestGenConfigChange, Error> =>
  Effect.gen(function* () {
    const inference = yield* OpenRouterInference;

    const prompt = buildTestGenMetaPrompt(config, recentRuns, lastAnalysis, taskType);

    let model = modelOverride || FREE_MODELS[0];
    let modelIndex = 0;
    const maxAttempts = modelOverride ? 1 : FREE_MODELS.length;
    const maxRetriesPerModel = 3;
    const baseDelayMs = 5000; // Start with 5 seconds
    const maxDelayMs = 60000; // Max 60 seconds

    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      for (let retry = 0; retry < maxRetriesPerModel; retry++) {
        try {
          if (retry > 0) {
            // Exponential backoff: 5s, 10s, 20s, capped at 60s
            const delayMs = Math.min(
              baseDelayMs * Math.pow(2, retry - 1),
              maxDelayMs
            );
            log(`[TestGenMetaReasoner] Retrying ${model} after ${delayMs}ms (retry ${retry}/${maxRetriesPerModel})`);
            yield* Effect.sleep(delayMs);
          } else {
            log(`[TestGenMetaReasoner] Using model: ${model} (attempt ${attempt + 1}/${maxAttempts})`);
          }

          const response = yield* inference.send(
            model,
            [{ role: "user", content: prompt }],
            {
              temperature: 0.3,
              maxTokens: 1000,
              // Note: responseFormat is not directly supported, but json_object
              // should be handled by the model's native support
            },
          );

          if (!response || !response.choices || response.choices.length === 0) {
            throw new Error("Empty response from model");
          }

          const messageContent = response.choices[0]?.message?.content;
          const content = typeof messageContent === "string"
            ? messageContent
            : null;

          if (!content) {
            throw new Error("No content in response");
          }

          const change = parseTestGenConfigChange(content, model);
          log(`[TestGenMetaReasoner] Proposed change: ${change.type}`);
          return change;
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : String(error);
          const isRateLimit = errorMessage.includes("429") ||
            errorMessage.includes("rate") ||
            errorMessage.includes("rate limit") ||
            errorMessage.includes("rate-limited");

          const errorObj = error instanceof Error ? error : new Error(String(error));
          if (isRateLimit && retry < maxRetriesPerModel - 1) {
            logError(`[TestGenMetaReasoner] Rate limited on ${model}, will retry`, errorObj);
            // Continue to next retry with backoff
            continue;
          } else if (isRateLimit) {
            logError(`[TestGenMetaReasoner] Rate limited on ${model}, exhausted retries`, errorObj);
            // Fall through to try next model or return "keep"
          } else {
            logError(`[TestGenMetaReasoner] Model ${model} failed`, errorObj);
            // Non-rate-limit error: try next model immediately
            break;
          }
        }
      }

      // Try next model if available
      if (modelIndex < FREE_MODELS.length - 1) {
        modelIndex++;
        model = FREE_MODELS[modelIndex];
      } else {
        // All models exhausted, return "keep" (no change)
        log(`[TestGenMetaReasoner] All models exhausted, returning "keep" (no config change)`);
        return {
          type: "keep" as const,
          reasoning: "All models rate-limited or failed, keeping current config",
        };
      }
    }

    // Fallback: return "keep" if we somehow get here
    log(`[TestGenMetaReasoner] Fallback: returning "keep" (no config change)`);
    return {
      type: "keep" as const,
      reasoning: "Meta-reasoning failed, keeping current config",
    };
  }).pipe(
    Effect.provide(OpenRouterInferenceLive),
    Effect.provide(openRouterLive),
    Effect.provide(InferenceStoreLive),
  );

/**
 * Validate config changes against guardrails.
 * Returns null if validation passes, or an error message if it fails.
 */
const validateConfigChange = (
  currentConfig: TestGenConfig,
  change: TestGenConfigChange,
): string | null => {
  if (change.type === "keep" || !change.changes) {
    return null; // No changes to validate
  }

  const changes = change.changes;

  // Delta caps: temperature ±0.1, tests ±1, rounds ±1
  if (changes.temperature !== undefined) {
    const delta = Math.abs(changes.temperature - currentConfig.temperature);
    if (delta > 0.1) {
      return `Temperature change too large: ${delta.toFixed(3)} > 0.1 (capped at ±0.1)`;
    }
  }

  if (changes.minTestsPerCategory !== undefined) {
    const delta = Math.abs(changes.minTestsPerCategory - currentConfig.minTestsPerCategory);
    if (delta > 1) {
      return `Min tests per category change too large: ${delta} > 1 (capped at ±1)`;
    }
    // Hard minimum: 2 per category
    if (changes.minTestsPerCategory < 2) {
      return `Min tests per category too low: ${changes.minTestsPerCategory} < 2 (minimum: 2)`;
    }
  }

  if (changes.maxTestsPerCategory !== undefined) {
    const delta = Math.abs(changes.maxTestsPerCategory - currentConfig.maxTestsPerCategory);
    if (delta > 1) {
      return `Max tests per category change too large: ${delta} > 1 (capped at ±1)`;
    }
    // Ensure max >= min
    const minTests = changes.minTestsPerCategory ?? currentConfig.minTestsPerCategory;
    if (changes.maxTestsPerCategory < minTests) {
      return `Max tests per category (${changes.maxTestsPerCategory}) < min (${minTests})`;
    }
  }

  if (changes.maxRoundsPerCategory !== undefined) {
    const delta = Math.abs(changes.maxRoundsPerCategory - currentConfig.maxRoundsPerCategory);
    if (delta > 1) {
      return `Max rounds per category change too large: ${delta} > 1 (capped at ±1)`;
    }
  }

  // Hard minimum: 10 total tests (minTestsPerCategory * 5 categories)
  const minTestsPerCategory = changes.minTestsPerCategory ?? currentConfig.minTestsPerCategory;
  const minTotalTests = minTestsPerCategory * 5; // 5 categories
  if (minTotalTests < 10) {
    return `Total minimum tests too low: ${minTotalTests} < 10 (need at least 2 per category × 5 categories)`;
  }

  // Token limits are checked at runtime, not in config validation
  // (soft ceiling: warn at 80k, hard-stop at 100k)

  return null; // Validation passed
};

/**
 * Apply a config change to create a new config.
 * Includes guardrail validation.
 */
export const applyConfigChange = (
  currentConfig: TestGenConfig,
  change: TestGenConfigChange,
): TestGenConfigInput => {
  // Validate guardrails
  const validationError = validateConfigChange(currentConfig, change);
  if (validationError) {
    log(`[TestGenMetaReasoner] Guardrail violation: ${validationError}. Keeping current config.`);
    // Return current config (no changes) if validation fails
    return {
      version: incrementVersion(currentConfig.version),
      temperature: currentConfig.temperature,
      maxTokens: currentConfig.maxTokens,
      minTestsPerCategory: currentConfig.minTestsPerCategory,
      maxTestsPerCategory: currentConfig.maxTestsPerCategory,
      maxRoundsPerCategory: currentConfig.maxRoundsPerCategory,
      environmentWeight: currentConfig.environmentWeight,
      antiCheatWeight: currentConfig.antiCheatWeight,
      precisionWeight: currentConfig.precisionWeight,
      categoryOrder: currentConfig.categoryOrder,
      primaryModel: currentConfig.primaryModel,
      reflectionModel: currentConfig.reflectionModel,
      minComprehensivenessScore: currentConfig.minComprehensivenessScore,
      targetComprehensivenessScore: currentConfig.targetComprehensivenessScore,
    };
  }
  // Helper to build config input, handling optional properties correctly
  const buildConfigInput = (overrides: Partial<TestGenConfigInput> = {}): TestGenConfigInput => {
    const input: TestGenConfigInput = {
      version: incrementVersion(currentConfig.version),
      temperature: overrides.temperature ?? currentConfig.temperature,
      maxTokens: overrides.maxTokens ?? currentConfig.maxTokens,
      minTestsPerCategory: overrides.minTestsPerCategory ?? currentConfig.minTestsPerCategory,
      maxTestsPerCategory: overrides.maxTestsPerCategory ?? currentConfig.maxTestsPerCategory,
      maxRoundsPerCategory: overrides.maxRoundsPerCategory ?? currentConfig.maxRoundsPerCategory,
      environmentWeight: overrides.environmentWeight ?? currentConfig.environmentWeight,
      antiCheatWeight: overrides.antiCheatWeight ?? currentConfig.antiCheatWeight,
      precisionWeight: overrides.precisionWeight ?? currentConfig.precisionWeight,
      categoryOrder: overrides.categoryOrder ?? currentConfig.categoryOrder,
      primaryModel: overrides.primaryModel ?? currentConfig.primaryModel,
      reflectionModel: overrides.reflectionModel ?? currentConfig.reflectionModel,
      minComprehensivenessScore: overrides.minComprehensivenessScore ?? currentConfig.minComprehensivenessScore,
      targetComprehensivenessScore: overrides.targetComprehensivenessScore ?? currentConfig.targetComprehensivenessScore,
    };

    // Handle optional string properties (only include if defined)
    const categoryPromptsValue = overrides.categoryPrompts ?? currentConfig.categoryPrompts;
    if (categoryPromptsValue !== undefined) {
      input.categoryPrompts = categoryPromptsValue;
    }
    const antiCheatPromptValue = overrides.antiCheatPrompt ?? currentConfig.antiCheatPrompt;
    if (antiCheatPromptValue !== undefined) {
      input.antiCheatPrompt = antiCheatPromptValue;
    }
    const reflectionPromptValue = overrides.reflectionPrompt ?? currentConfig.reflectionPrompt;
    if (reflectionPromptValue !== undefined) {
      input.reflectionPrompt = reflectionPromptValue;
    }

    return input;
  };

  if (change.type === "keep" || !change.changes) {
    // No changes, return current config as input (will be deduplicated by hash)
    return buildConfigInput();
  }

  // Merge changes with current config
  return buildConfigInput(change.changes);
};

/**
 * Increment version string (e.g., "1.0.0" -> "1.0.1").
 */
const incrementVersion = (version: string): string => {
  const parts = version.split(".");
  if (parts.length === 3) {
    const patch = parseInt(parts[2], 10) + 1;
    return `${parts[0]}.${parts[1]}.${patch}`;
  }
  return version;
};

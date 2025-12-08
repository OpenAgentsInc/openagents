/**
 * TestGen Meta-Reasoner
 *
 * Uses OpenRouter models to propose test generation config changes based on run results.
 * Similar to HillClimber meta-reasoner but focused on test generation quality.
 */

import { Effect } from "effect";
import {
  OpenRouterInference,
  OpenRouterInferenceLive,
} from "../llm/openrouter-inference.js";
import { openRouterLive } from "../llm/openrouter-http.js";
import { InferenceStoreLive } from "../llm/inference-store.js";
import { TestGenStore, TestGenStoreLive } from "./testgen-store.js";
import type {
  TestGenConfig,
  TestGenConfigInput,
  TestGenConfigChange,
  TestGenRun,
  TestGenAnalysis,
} from "./testgen-types.js";
import { FREE_MODELS } from "./meta-reasoner.js";
import { log, logError } from "./logger.js";

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

    return {
      type: parsed.type || "update_params",
      changes: Object.keys(changes).length > 0 ? changes : undefined,
      reasoning: parsed.reasoning || "No reasoning provided",
      model,
    };
  } catch (error) {
    logError("Failed to parse meta-reasoner response", error);
    return {
      type: "keep",
      reasoning: `Parse error: ${error}`,
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

    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      try {
        log(`[TestGenMetaReasoner] Using model: ${model} (attempt ${attempt + 1}/${maxAttempts})`);

        const response = yield* inference.send(
          model,
          [{ role: "user", content: prompt }],
          {
            temperature: 0.3,
            max_tokens: 1000,
            response_format: { type: "json_object" },
          },
        );

        if (!response || !response.content) {
          throw new Error("Empty response from model");
        }

        const content = Array.isArray(response.content)
          ? response.content[0]?.text || ""
          : typeof response.content === "string"
          ? response.content
          : "";

        if (!content) {
          throw new Error("No content in response");
        }

        const change = parseTestGenConfigChange(content, model);
        log(`[TestGenMetaReasoner] Proposed change: ${change.type}`);
        return change;
      } catch (error) {
        logError(`[TestGenMetaReasoner] Model ${model} failed`, error);

        if (modelIndex < FREE_MODELS.length - 1) {
          modelIndex++;
          model = FREE_MODELS[modelIndex];
        } else {
          throw error;
        }
      }
    }

    throw new Error("All models failed");
  }).pipe(
    Effect.provide(OpenRouterInferenceLive),
    Effect.provide(openRouterLive),
    Effect.provide(InferenceStoreLive),
  );

/**
 * Apply a config change to create a new config.
 */
export const applyConfigChange = (
  currentConfig: TestGenConfig,
  change: TestGenConfigChange,
): TestGenConfigInput => {
  if (change.type === "keep" || !change.changes) {
    // No changes, return current config as input (will be deduplicated by hash)
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
      categoryPrompts: currentConfig.categoryPrompts,
      antiCheatPrompt: currentConfig.antiCheatPrompt,
      reflectionPrompt: currentConfig.reflectionPrompt,
      primaryModel: currentConfig.primaryModel,
      reflectionModel: currentConfig.reflectionModel,
      minComprehensivenessScore: currentConfig.minComprehensivenessScore,
      targetComprehensivenessScore: currentConfig.targetComprehensivenessScore,
    };
  }

  // Merge changes with current config
  return {
    version: incrementVersion(currentConfig.version),
    temperature: change.changes.temperature ?? currentConfig.temperature,
    maxTokens: change.changes.maxTokens ?? currentConfig.maxTokens,
    minTestsPerCategory: change.changes.minTestsPerCategory ?? currentConfig.minTestsPerCategory,
    maxTestsPerCategory: change.changes.maxTestsPerCategory ?? currentConfig.maxTestsPerCategory,
    maxRoundsPerCategory: change.changes.maxRoundsPerCategory ?? currentConfig.maxRoundsPerCategory,
    environmentWeight: change.changes.environmentWeight ?? currentConfig.environmentWeight,
    antiCheatWeight: change.changes.antiCheatWeight ?? currentConfig.antiCheatWeight,
    precisionWeight: change.changes.precisionWeight ?? currentConfig.precisionWeight,
    categoryOrder: change.changes.categoryOrder ?? currentConfig.categoryOrder,
    categoryPrompts: change.changes.categoryPrompts ?? currentConfig.categoryPrompts,
    antiCheatPrompt: change.changes.antiCheatPrompt ?? currentConfig.antiCheatPrompt,
    reflectionPrompt: change.changes.reflectionPrompt ?? currentConfig.reflectionPrompt,
    primaryModel: change.changes.primaryModel ?? currentConfig.primaryModel,
    reflectionModel: change.changes.reflectionModel ?? currentConfig.reflectionModel,
    minComprehensivenessScore: change.changes.minComprehensivenessScore ?? currentConfig.minComprehensivenessScore,
    targetComprehensivenessScore: change.changes.targetComprehensivenessScore ?? currentConfig.targetComprehensivenessScore,
  };
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


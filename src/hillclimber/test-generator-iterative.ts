/**
 * Iterative Test Generator
 *
 * Generates tests one at a time with category-based iteration and reflection.
 * Streams tests to UI in real-time as they're generated.
 */

import { Effect, Layer } from "effect";
import {
  FMService,
  FMServiceLive,
} from "../fm/service.js";
import {
  AnthropicClient,
  anthropicClientLive,
  anthropicConfigLayer,
} from "../llm/anthropic.js";
import { log } from "./logger.js";
import type { EnvironmentInfo } from "./environment-info.js";
import type {
  GeneratedTest,
  TestCategory,
  TestGeneratorOptions,
  EnvironmentAwareTestResult,
} from "./test-generator.js";
import type {
  TestGenStartMessage,
  TestGenTestMessage,
  TestGenCompleteMessage,
  TestGenErrorMessage,
  TestGenProgressMessage,
  TestGenReflectionMessage,
} from "../hud/protocol.js";

// ============================================================================
// Types
// ============================================================================

/**
 * Extended emitter interface for iterative generation.
 */
export interface IterativeTestGenEmitter {
  onStart: (msg: TestGenStartMessage) => void;
  onTest: (msg: TestGenTestMessage) => void;
  onProgress: (msg: TestGenProgressMessage) => void;
  onReflection: (msg: TestGenReflectionMessage) => void;
  onComplete: (msg: TestGenCompleteMessage) => void;
  onError: (msg: TestGenErrorMessage) => void;
}

/**
 * State tracker for multi-round generation.
 */
interface GeneratorState {
  antiCheatTests: GeneratedTest[];
  existenceTests: GeneratedTest[];
  correctnessTests: GeneratedTest[];
  boundaryTests: GeneratedTest[];
  integrationTests: GeneratedTest[];

  descriptionRequirements: string[];
  environmentRequirements: string[];
  uncertainties: string[];

  currentPhase: "category_generation" | "global_refinement" | "complete";
  currentCategory: IterativeTestCategory | null;
  categoryRoundNumber: Record<string, number>;
  globalRoundNumber: number;

  totalRounds: number;
  totalTokensUsed: number;
  comprehensivenessScore: number | null;
}

/**
 * Iteration settings.
 */
interface IterationConfig {
  minTestsPerCategory: number;
  targetTestsPerCategory: number;
  maxRoundsPerCategory: number;
  enableGlobalRefinement: boolean;
  minComprehensivenessScore: number;
  maxGlobalRefinementRounds: number;
  minTotalTests: number;
  targetTotalTests: number;
  maxTotalRounds: number;
  maxTotalTokens: number;
  maxTotalTimeMs: number;
}

const DEFAULT_CONFIG: IterationConfig = {
  minTestsPerCategory: 2,
  targetTestsPerCategory: 5,
  maxRoundsPerCategory: 3,
  enableGlobalRefinement: true,
  minComprehensivenessScore: 8,
  maxGlobalRefinementRounds: 2,
  minTotalTests: 15,
  targetTotalTests: 30,
  maxTotalRounds: 12,
  maxTotalTokens: 100000, // Hard stop at 100k (soft warning at 80k)
  maxTotalTimeMs: 180000,
};

// Category order (priority) - using string literals since TestCategory doesn't include anti_cheat
const CATEGORIES = [
  "anti_cheat",
  "existence",
  "correctness",
  "boundary",
  "integration",
] as const;

type IterativeTestCategory = typeof CATEGORIES[number];

// ============================================================================
// Helper Functions
// ============================================================================

function initializeState(): GeneratorState {
  return {
    antiCheatTests: [],
    existenceTests: [],
    correctnessTests: [],
    boundaryTests: [],
    integrationTests: [],
    descriptionRequirements: [],
    environmentRequirements: [],
    uncertainties: [],
    currentPhase: "category_generation",
    currentCategory: null,
    categoryRoundNumber: {
      anti_cheat: 0,
      existence: 0,
      correctness: 0,
      boundary: 0,
      integration: 0,
      format: 0,
      happy_path: 0,
      edge_case: 0,
      invalid_input: 0,
    },
    globalRoundNumber: 0,
    totalRounds: 0,
    totalTokensUsed: 0,
    comprehensivenessScore: null,
  };
}

function getTestsForCategory(
  state: GeneratorState,
  category: IterativeTestCategory,
): GeneratedTest[] {
  switch (category) {
    case "anti_cheat":
      return state.antiCheatTests;
    case "existence":
      return state.existenceTests;
    case "correctness":
      return state.correctnessTests;
    case "boundary":
      return state.boundaryTests;
    case "integration":
      return state.integrationTests;
    default:
      return [];
  }
}

function setTestsForCategory(
  state: GeneratorState,
  category: IterativeTestCategory,
  tests: GeneratedTest[],
): void {
  switch (category) {
    case "anti_cheat":
      state.antiCheatTests = tests;
      break;
    case "existence":
      state.existenceTests = tests;
      break;
    case "correctness":
      state.correctnessTests = tests;
      break;
    case "boundary":
      state.boundaryTests = tests;
      break;
    case "integration":
      state.integrationTests = tests;
      break;
  }
}

function countAllTests(state: GeneratorState): number {
  return (
    state.antiCheatTests.length +
    state.existenceTests.length +
    state.correctnessTests.length +
    state.boundaryTests.length +
    state.integrationTests.length
  );
}

function isCategoryComplete(
  state: GeneratorState,
  category: IterativeTestCategory,
  config: IterationConfig,
): boolean {
  const tests = getTestsForCategory(state, category);
  const rounds = state.categoryRoundNumber[category];
  return (
    tests.length >= config.minTestsPerCategory ||
    rounds >= config.maxRoundsPerCategory
  );
}

function shouldReflect(
  state: GeneratorState,
  category: IterativeTestCategory,
  config: IterationConfig,
): boolean {
  const tests = getTestsForCategory(state, category);
  const rounds = state.categoryRoundNumber[category];
  return (
    rounds < config.maxRoundsPerCategory &&
    tests.length < config.targetTestsPerCategory
  );
}

function convertStateToResult(
  state: GeneratorState,
  model: string,
  durationMs: number,
): EnvironmentAwareTestResult {
  return {
    descriptionRequirements: state.descriptionRequirements,
    environmentRequirements: state.environmentRequirements,
    antiCheatTests: state.antiCheatTests,
    existenceTests: state.existenceTests,
    correctnessTests: state.correctnessTests,
    boundaryTests: state.boundaryTests,
    integrationTests: state.integrationTests,
    uncertainties: state.uncertainties,
    model,
    durationMs,
  };
}

// ============================================================================
// Prompt Building
// ============================================================================

function buildCategoryPrompt(
  taskDescription: string,
  taskId: string,
  category: IterativeTestCategory,
  environment: EnvironmentInfo,
  existingTests: GeneratedTest[],
  round: number,
): string {
  const existingTestsText =
    existingTests.length > 0
      ? `\n## Existing Tests for ${category}\n${existingTests
          .map(
            (t, i) =>
              `${i + 1}. ${t.id}: ${t.input} → ${t.expectedOutput ?? "null"}`,
          )
          .join("\n")}`
      : "";

  const reflectionPrompt =
    round > 1
      ? `\n## Reflection\nYou've already generated ${existingTests.length} tests for ${category}. What edge cases or scenarios are still missing? Generate 1-3 additional tests that fill gaps.`
      : "";

  return `You are a QA engineer generating tests for category: ${category}

## Task ID
${taskId}

## Task Description
${taskDescription}

## Category: ${category}
${getCategoryDescription(category)}

## Environment Context
- Platform: ${environment.platform.type}
- Prohibited Tools: ${environment.tools.prohibited.map((t) => t.name).join(", ") || "None"}
- Files: ${environment.files.listing.length} files, ${environment.files.taskFiles.length} previews
${existingTestsText}${reflectionPrompt}

## Output Format
Generate 2-5 tests as JSON array:
[
  {
    "id": "${category}_1",
    "input": "test input command or data",
    "expectedOutput": "expected output or null",
    "reasoning": "why this test is important",
    "confidence": 0.9
  }
]

Respond with valid JSON array only. No markdown, no explanation.`;
}

function getCategoryDescription(category: IterativeTestCategory): string {
  switch (category) {
    case "anti_cheat":
      return "Verify prohibited tools/approaches are NOT used. Critical for catching gaming behavior.";
    case "existence":
      return "Test that required output files are created in correct paths and are non-empty.";
    case "correctness":
      return "Happy path tests for main functionality. Verify output format matches expectations.";
    case "boundary":
      return "Test min/max values, range constraints, and limits mentioned in description.";
    case "integration":
      return "System-level behavior, multi-step verification, integration with existing files.";
    default:
      return "Test for this category.";
  }
}

function buildComprehensivenessPrompt(
  state: GeneratorState,
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
): string {
  const allTests = [
    ...state.antiCheatTests,
    ...state.existenceTests,
    ...state.correctnessTests,
    ...state.boundaryTests,
    ...state.integrationTests,
  ];

  return `Review all generated tests and assess comprehensiveness.

## Task: ${taskId}
${taskDescription}

## Generated Tests (${allTests.length} total)
Anti-cheat: ${state.antiCheatTests.length}
Existence: ${state.existenceTests.length}
Correctness: ${state.correctnessTests.length}
Boundary: ${state.boundaryTests.length}
Integration: ${state.integrationTests.length}

## Your Assessment
Rate comprehensiveness 1-10 (10 = comprehensive, 1 = missing critical tests).
Identify gaps: what test categories or scenarios are missing?

Respond with JSON:
{
  "score": 8,
  "gaps": ["missing boundary tests for parameter X", "need more anti-cheat coverage"],
  "recommendations": ["generate 2 more boundary tests", "add anti-cheat test for tool Y"]
}`;
}

// ============================================================================
// Generation Functions
// ============================================================================

async function generateTestsForCategoryRound(
  category: IterativeTestCategory,
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  existingTests: GeneratedTest[],
  round: number,
  model: "local" | "claude",
): Promise<{ tests: GeneratedTest[]; tokens: number }> {
  const prompt = buildCategoryPrompt(
    taskDescription,
    taskId,
    category,
    environment,
    existingTests,
    round,
  );

  if (model === "local") {
    return generateWithLocalFM(prompt, category);
  } else {
    return generateWithClaude(prompt, category);
  }
}

async function generateWithLocalFM(
  prompt: string,
  category: IterativeTestCategory,
): Promise<{ tests: GeneratedTest[]; tokens: number }> {
  const result = await Effect.runPromise(
    Effect.gen(function* () {
      const fm = yield* FMService;
      yield* fm.ensureRunning();

      const response = yield* fm.chat({
        messages: [{ role: "user", content: prompt }],
        temperature: 0.3,
        maxTokens: 2048,
        responseFormat: {
          type: "json_schema",
          schema_type: "test_generation",
        },
      });

      const content = response.choices[0]?.message?.content ?? "";
      const tests = parseTestsResponse(content, category);
      const tokens = response.usage?.total_tokens ?? 0;
      return { tests, tokens };
    }).pipe(Effect.provide(FMServiceLive)),
  );

  return result;
}

async function generateWithClaude(
  prompt: string,
  category: IterativeTestCategory,
): Promise<{ tests: GeneratedTest[]; tokens: number }> {
  const result = await Effect.runPromise(
    Effect.gen(function* () {
      const anthropic = yield* AnthropicClient;

      const response = yield* anthropic.chat({
        model: "claude-sonnet-4-20250514",
        messages: [{ role: "user", content: prompt }],
        temperature: 0.3,
        maxTokens: 2048,
      });

      const content = response.choices[0]?.message?.content ?? "";
      const tests = parseTestsResponse(content, category);
      const tokens = response.usage?.total_tokens ?? 0;
      return { tests, tokens };
    }).pipe(
      Effect.provide(Layer.mergeAll(anthropicConfigLayer, anthropicClientLive)),
    ) as Effect.Effect<{ tests: GeneratedTest[]; tokens: number }, Error>,
  );

  return result;
}

function parseTestsResponse(
  content: string,
  category: IterativeTestCategory,
): GeneratedTest[] {
  let jsonContent = content.trim();
  if (jsonContent.startsWith("```")) {
    jsonContent = jsonContent.replace(/^```(?:json)?\s*\n?/, "");
    jsonContent = jsonContent.replace(/\n?```\s*$/, "");
    jsonContent = jsonContent.trim();
  }

  try {
    const parsed = JSON.parse(jsonContent);
    const tests = Array.isArray(parsed) ? parsed : parsed.tests || [];

    return tests
      .filter((t: unknown) => {
        const test = t as Record<string, unknown>;
        return test && typeof test.id === "string" && typeof test.input === "string";
      })
      .map((t: unknown) => {
        const test = t as Record<string, unknown>;
        return {
          id: test.id as string,
          input: test.input as string,
          expectedOutput: (test.expectedOutput as string | null) ?? null,
          reasoning: (test.reasoning as string) ?? "",
          category: category as TestCategory,
          confidence: typeof test.confidence === "number" ? test.confidence : 0.5,
        };
      });
  } catch (e) {
    log(`[TestGen] JSON parse error: ${e instanceof Error ? e.message : String(e)}`);
    return [];
  }
}

async function reflectOnCategory(
  category: IterativeTestCategory,
  existingTests: GeneratedTest[],
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  model: "local" | "claude",
): Promise<{ reflection: string; tokens: number }> {
  const prompt = `Review the ${existingTests.length} existing tests for category ${category}:

${existingTests.map((t, i) => `${i + 1}. ${t.id}: ${t.reasoning}`).join("\n")}

What edge cases or scenarios are missing? Provide a brief reflection (1-2 sentences).`;

  if (model === "local") {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const fm = yield* FMService;
        yield* fm.ensureRunning();

        const response = yield* fm.chat({
          messages: [{ role: "user", content: prompt }],
          temperature: 0.3,
          maxTokens: 512,
        });

        const reflection = response.choices[0]?.message?.content ?? "";
        const tokens = response.usage?.total_tokens ?? 0;
        return { reflection, tokens };
      }).pipe(Effect.provide(FMServiceLive)),
    );
    return result;
  } else {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const anthropic = yield* AnthropicClient;

        const response = yield* anthropic.chat({
          model: "claude-sonnet-4-20250514",
          messages: [{ role: "user", content: prompt }],
          temperature: 0.3,
          maxTokens: 512,
        });

        const reflection = response.choices[0]?.message?.content ?? "";
        const tokens = response.usage?.total_tokens ?? 0;
        return { reflection, tokens };
      }).pipe(
        Effect.provide(Layer.mergeAll(anthropicConfigLayer, anthropicClientLive)),
      ) as Effect.Effect<{ reflection: string; tokens: number }, Error>,
    );
    return result;
  }
}

async function assessComprehensiveness(
  state: GeneratorState,
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  model: "local" | "claude",
): Promise<{ score: number; gaps: string[]; recommendations: string[]; tokens: number }> {
  const prompt = buildComprehensivenessPrompt(
    state,
    taskDescription,
    taskId,
    environment,
  );

  let content: string;
  let tokens = 0;
  if (model === "local") {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const fm = yield* FMService;
        yield* fm.ensureRunning();

        const response = yield* fm.chat({
          messages: [{ role: "user", content: prompt }],
          temperature: 0.3,
          maxTokens: 1024,
        });

        const text = response.choices[0]?.message?.content ?? "";
        const tokenCount = response.usage?.total_tokens ?? 0;
        return { text, tokens: tokenCount };
      }).pipe(Effect.provide(FMServiceLive)),
    );
    content = result.text;
    tokens = result.tokens;
  } else {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const anthropic = yield* AnthropicClient;

        const response = yield* anthropic.chat({
          model: "claude-sonnet-4-20250514",
          messages: [{ role: "user", content: prompt }],
          temperature: 0.3,
          maxTokens: 1024,
        });

        const text = response.choices[0]?.message?.content ?? "";
        const tokenCount = response.usage?.total_tokens ?? 0;
        return { text, tokens: tokenCount };
      }).pipe(
        Effect.provide(Layer.mergeAll(anthropicConfigLayer, anthropicClientLive)),
      ) as Effect.Effect<{ text: string; tokens: number }, Error>,
    );
    content = result.text;
    tokens = result.tokens;
  }

  let jsonContent = content.trim();
  if (jsonContent.startsWith("```")) {
    jsonContent = jsonContent.replace(/^```(?:json)?\s*\n?/, "");
    jsonContent = jsonContent.replace(/\n?```\s*$/, "");
    jsonContent = jsonContent.trim();
  }

  try {
    const parsed = JSON.parse(jsonContent);
    return {
      score: typeof parsed.score === "number" ? parsed.score : 5,
      gaps: Array.isArray(parsed.gaps) ? parsed.gaps : [],
      recommendations: Array.isArray(parsed.recommendations)
        ? parsed.recommendations
        : [],
      tokens,
    };
  } catch {
    return { score: 5, gaps: [], recommendations: [], tokens };
  }
}

// ============================================================================
// Main API
// ============================================================================

/**
 * Generate tests iteratively with category-based rounds and reflection.
 */
export async function generateTestsIteratively(
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  emitter: IterativeTestGenEmitter,
  options: TestGeneratorOptions = {},
  config: Partial<IterationConfig> = {},
): Promise<EnvironmentAwareTestResult> {
  const finalConfig = { ...DEFAULT_CONFIG, ...config };
  const state = initializeState();
  const startTime = Date.now();
  const modelOption = options.model ?? "local";
  const model: "local" | "claude" = modelOption === "both" ? "local" : modelOption;

  log(`[TestGen] Starting iterative test generation for task: ${taskId}`);
  log(`[TestGen] Model: ${model}`);

  try {
    // Phase 2: Category-based iteration
    for (const category of CATEGORIES) {
      state.currentCategory = category;
      let round = 1;

      while (
        !isCategoryComplete(state, category, finalConfig) &&
        round <= finalConfig.maxRoundsPerCategory &&
        state.totalRounds < finalConfig.maxTotalRounds &&
        state.totalTokensUsed < finalConfig.maxTotalTokens
      ) {
        emitter.onProgress({
          type: "testgen_progress",
          sessionId: "", // Will be set by caller
          phase: "category_generation",
          currentCategory: category,
          roundNumber: round,
          status: `Generating ${category} tests (round ${round})...`,
        });

        // Generate tests for this category/round
        const result = await generateTestsForCategoryRound(
          category,
          taskDescription,
          taskId,
          environment,
          getTestsForCategory(state, category),
          round,
          model,
        );

        // Update token usage
        state.totalTokensUsed += result.tokens;

        // Token limit guardrails
        if (state.totalTokensUsed >= finalConfig.maxTotalTokens) {
          log(`[TestGen] Hard token limit reached: ${state.totalTokensUsed} >= ${finalConfig.maxTotalTokens}. Stopping generation.`);
          break; // Exit category loop
        } else if (state.totalTokensUsed >= 80000) {
          log(`[TestGen] Warning: Token usage at ${state.totalTokensUsed}, approaching limit of ${finalConfig.maxTotalTokens}`);
        }

        // Emit each test immediately
        for (const test of result.tests) {
          emitter.onTest({
            type: "testgen_test",
            sessionId: "", // Will be set by caller
            test: {
              id: test.id,
              category,
              input: test.input,
              expectedOutput: test.expectedOutput ?? null,
              reasoning: test.reasoning,
              confidence: test.confidence,
            },
          });

          // Add to state
          const existing = getTestsForCategory(state, category);
          setTestsForCategory(state, category, [...existing, test]);
        }

        // Reflect if needed
        if (shouldReflect(state, category, finalConfig)) {
          const reflectionResult = await reflectOnCategory(
            category,
            getTestsForCategory(state, category),
            taskDescription,
            taskId,
            environment,
            model,
          );

          // Update token usage
          state.totalTokensUsed += reflectionResult.tokens;

          emitter.onReflection({
            type: "testgen_reflection",
            sessionId: "", // Will be set by caller
            category,
            reflectionText: reflectionResult.reflection,
            action: "refining",
          });
        }

        state.categoryRoundNumber[category] = round;
        round++;
        state.totalRounds++;

        // Check limits
        if (Date.now() - startTime > finalConfig.maxTotalTimeMs) {
          log(`[TestGen] Time limit reached`);
          break;
        }
      }
    }

    // Phase 3: Global refinement
    if (finalConfig.enableGlobalRefinement) {
      state.currentPhase = "global_refinement";

      emitter.onProgress({
        type: "testgen_progress",
        sessionId: "",
        phase: "global_refinement",
        roundNumber: state.globalRoundNumber + 1,
        status: "Assessing comprehensiveness...",
      });

      const assessment = await assessComprehensiveness(
        state,
        taskDescription,
        taskId,
        environment,
        model,
      );

      // Update token usage
      state.totalTokensUsed += assessment.tokens;
      state.comprehensivenessScore = assessment.score;

      emitter.onReflection({
        type: "testgen_reflection",
        sessionId: "",
        reflectionText: `Comprehensiveness score: ${assessment.score}/10. Gaps: ${assessment.gaps.join("; ")}`,
        action: "assessing",
      });

      // Generate additional tests if score is low
      if (
        assessment.score < finalConfig.minComprehensivenessScore &&
        state.globalRoundNumber < finalConfig.maxGlobalRefinementRounds &&
        state.totalRounds < finalConfig.maxTotalRounds
      ) {
        // Generate tests based on recommendations (simplified - just generate more)
        const additionalResult = await generateTestsForCategoryRound(
          "correctness", // Default category for additional tests
          taskDescription,
          taskId,
          environment,
          [],
          1,
          model,
        );

        // Update token usage
        state.totalTokensUsed += additionalResult.tokens;

        for (const test of additionalResult.tests) {
          const testCategory = test.category as IterativeTestCategory;
          emitter.onTest({
            type: "testgen_test",
            sessionId: "",
            test: {
              id: test.id,
              category: testCategory,
              input: test.input,
              expectedOutput: test.expectedOutput ?? null,
              reasoning: test.reasoning,
              confidence: test.confidence,
            },
          });

          // Only add if it's one of our iterative categories
          if (CATEGORIES.includes(testCategory as any)) {
            const existing = getTestsForCategory(state, testCategory);
            setTestsForCategory(state, testCategory, [...existing, test]);
          }
        }

        state.globalRoundNumber++;
        state.totalRounds++;
      }
    }

    state.currentPhase = "complete";

    const durationMs = Date.now() - startTime;
    const totalTests = countAllTests(state);

    emitter.onComplete({
      type: "testgen_complete",
      sessionId: "", // Will be set by caller
      totalTests,
      totalRounds: state.totalRounds,
      categoryRounds: state.categoryRoundNumber as Record<TestCategory, number>,
      comprehensivenessScore: state.comprehensivenessScore,
      totalTokensUsed: state.totalTokensUsed,
      durationMs,
      uncertainties: state.uncertainties,
    });

    return convertStateToResult(
      state,
      `iterative-${model}`,
      durationMs,
    );
  } catch (error) {
    emitter.onError({
      type: "testgen_error",
      sessionId: "",
      error: error instanceof Error ? error.message : String(error),
    });
    throw error;
  }
}

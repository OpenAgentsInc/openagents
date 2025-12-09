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

  // Extract task-specific edge cases and format for this category
  const edgeCases = extractTaskEdgeCases(taskDescription, taskId);
  const edgeCaseGuidance = formatEdgeCasesForCategory(edgeCases, category);

  return `You are a QA engineer generating tests for category: ${category}

## Task ID
${taskId}

## Task Description
${taskDescription}

## Category: ${category}
${getCategoryDescription(category)}
${edgeCaseGuidance}

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

// ============================================================================
// Task-Specific Edge Case Extraction
// ============================================================================

export interface TaskEdgeCases {
  ipv4?: {
    validRanges: string;
    invalidExamples: string[];
    boundaryRequirements: string[];
  };
  date?: {
    format: string;
    validRanges: { month: string; day: string };
    invalidExamples: string[];
  };
  regex?: {
    captureGroup: string;
    multipleMatches: string;
  };
  generic: string[];
}

/**
 * Extract task-specific edge cases from the task description.
 * This helps TestGen generate tests that match what TB2 actually tests.
 */
export function extractTaskEdgeCases(taskDescription: string, taskId: string): TaskEdgeCases {
  const edgeCases: TaskEdgeCases = { generic: [] };
  const desc = taskDescription.toLowerCase();

  // IPv4 address detection
  if (desc.includes("ipv4") || desc.includes("ip address") || desc.includes("ip ")) {
    edgeCases.ipv4 = {
      validRanges: "Each octet must be 0-255 (not 256+, not negative)",
      invalidExamples: [
        "256.1.1.1 (first octet > 255)",
        "1.2.3.999 (fourth octet > 255)",
        "1.2.3.4a (alphanumeric suffix - not a valid IP)",
        "a1.2.3.4 (alphanumeric prefix - not a valid IP)",
        "1.2.3 (missing octet)",
        "1.2.3.4.5 (extra octet)",
      ],
      boundaryRequirements: [
        "IP must have word boundaries (not adjacent to alphanumeric)",
        "Test IPs at start/middle/end of line",
        "Test multiple IPs on same line",
      ],
    };
  }

  // Date detection (YYYY-MM-DD format)
  if (desc.includes("date") || desc.includes("yyyy-mm-dd") || desc.includes("yyyy/mm/dd")) {
    edgeCases.date = {
      format: "YYYY-MM-DD where month is 01-12 and day is 01-31",
      validRanges: {
        month: "01-12 (not 00, not 13+)",
        day: "01-31 (not 00, not 32+)",
      },
      invalidExamples: [
        "2024-00-15 (month 00 invalid)",
        "2024-13-15 (month 13 invalid)",
        "2024-01-00 (day 00 invalid)",
        "2024-01-32 (day 32 invalid)",
        "2024-02-30 (Feb 30 doesn't exist)",
        "2024-04-31 (Apr 31 doesn't exist)",
        "20240115 (missing dashes)",
        "2024-1-15 (single digit month)",
      ],
    };
  }

  // Regex-specific patterns
  if (desc.includes("regex") || desc.includes("regular expression") || desc.includes("pattern")) {
    edgeCases.regex = {
      captureGroup: "Test that the correct capture group is returned, not the whole match",
      multipleMatches: "When multiple matches exist, test which one is captured (first/last/all)",
    };
  }

  // Log file patterns
  if (desc.includes("log") || taskId.includes("log")) {
    edgeCases.generic.push(
      "Test multi-line log entries",
      "Test lines with multiple potential matches",
      "Test lines that look like matches but aren't (false positives)",
      "Test empty lines and lines with only whitespace",
    );
  }

  // Generic patterns from description
  if (desc.includes("last") || desc.includes("final")) {
    edgeCases.generic.push("When multiple matches exist, verify the LAST one is selected");
  }
  if (desc.includes("first")) {
    edgeCases.generic.push("When multiple matches exist, verify the FIRST one is selected");
  }
  if (desc.includes("only") || desc.includes("must")) {
    edgeCases.generic.push("Test lines that should NOT match to verify false positive rejection");
  }

  return edgeCases;
}

/**
 * Format edge cases as additional prompt guidance for a category.
 */
export function formatEdgeCasesForCategory(
  edgeCases: TaskEdgeCases,
  category: IterativeTestCategory,
): string {
  const lines: string[] = [];

  if (category === "boundary") {
    // Boundary tests should specifically target validation edge cases
    // IMPORTANT: Tests must include BOTH components (IP and date) to properly test the regex
    if (edgeCases.ipv4 && edgeCases.date) {
      lines.push("\n### Combined IP + Date Boundary Cases (CRITICAL)");
      lines.push("IMPORTANT: Each test MUST include both an IP and a date!");
      lines.push("The regex requires BOTH a valid IP AND a valid date to match.");
      lines.push("");
      lines.push("Test with INVALID IPs (valid date, should NOT match):");
      lines.push("  - Input: '256.1.1.1 2024-01-15' → expectedOutput: null (invalid IP)");
      lines.push("  - Input: '1.2.3.999 2024-01-15' → expectedOutput: null (invalid IP)");
      lines.push("  - Input: '1.2.3.4a 2024-01-15' → expectedOutput: null (alphanumeric suffix)");
      lines.push("  - Input: 'a1.2.3.4 2024-01-15' → expectedOutput: null (alphanumeric prefix)");
      lines.push("");
      lines.push("Test with INVALID dates (valid IP, should NOT match):");
      lines.push("  - Input: '192.168.1.1 2024-00-15' → expectedOutput: null (month 00)");
      lines.push("  - Input: '192.168.1.1 2024-13-15' → expectedOutput: null (month 13)");
      lines.push("  - Input: '192.168.1.1 2024-01-00' → expectedOutput: null (day 00)");
      lines.push("  - Input: '192.168.1.1 2024-01-32' → expectedOutput: null (day 32)");
      lines.push("  - Input: '192.168.1.1 2024-02-30' → expectedOutput: null (Feb 30)");
      lines.push("");
      lines.push("Test with VALID IP and date (should match):");
      lines.push("  - Input: '192.168.1.1 2024-01-15' → expectedOutput: '2024-01-15'");
      lines.push("  - Input: '10.0.0.1 2024-12-31' → expectedOutput: '2024-12-31'");
    } else if (edgeCases.ipv4) {
      lines.push("\n### IPv4 Boundary Cases (CRITICAL)");
      lines.push(`Valid range: ${edgeCases.ipv4.validRanges}`);
      lines.push("Test these INVALID IPs (should NOT match):");
      for (const example of edgeCases.ipv4.invalidExamples) {
        lines.push(`  - ${example}`);
      }
      lines.push("Boundary requirements:");
      for (const req of edgeCases.ipv4.boundaryRequirements) {
        lines.push(`  - ${req}`);
      }
    } else if (edgeCases.date) {
      lines.push("\n### Date Boundary Cases (CRITICAL)");
      lines.push(`Format: ${edgeCases.date.format}`);
      lines.push(`Month range: ${edgeCases.date.validRanges.month}`);
      lines.push(`Day range: ${edgeCases.date.validRanges.day}`);
      lines.push("Test these INVALID dates (should NOT match):");
      for (const example of edgeCases.date.invalidExamples) {
        lines.push(`  - ${example}`);
      }
    }
  }

  if (category === "correctness" && edgeCases.regex) {
    lines.push("\n### Regex Correctness Cases");
    lines.push(`- ${edgeCases.regex.captureGroup}`);
    lines.push(`- ${edgeCases.regex.multipleMatches}`);
  }

  if (category === "integration" && edgeCases.generic.length > 0) {
    lines.push("\n### Task-Specific Integration Cases");
    for (const g of edgeCases.generic) {
      lines.push(`- ${g}`);
    }
  }

  // Anti-cheat should test that invalid inputs are properly rejected
  if (category === "anti_cheat") {
    lines.push("\n### Anti-Cheat: False Positive Prevention");
    lines.push("Verify the solution does NOT match invalid inputs:");
    if (edgeCases.ipv4) {
      lines.push("- Lines with invalid IPs (256+, alphanumeric adjacent) should NOT match");
    }
    if (edgeCases.date) {
      lines.push("- Lines with invalid dates (month 00/13, day 00/32) should NOT match");
    }
  }

  return lines.join("\n");
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
        // Handle "null" string as actual null (LLM sometimes outputs "null" instead of null)
        let expectedOutput = test.expectedOutput as string | null;
        if (expectedOutput === "null" || expectedOutput === "None" || expectedOutput === "") {
          expectedOutput = null;
        }
        return {
          id: test.id as string,
          input: test.input as string,
          expectedOutput: expectedOutput ?? null,
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

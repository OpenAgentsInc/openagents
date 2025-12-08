/**
 * Test Generator for Blind Verification
 *
 * Generates test cases from task descriptions WITHOUT seeing actual benchmark tests.
 * This is the core component for proving generalization rather than gaming.
 *
 * Key principle: The agent must be BLIND to actual test cases.
 * It reasons about requirements and generates its own tests.
 */

import { Effect, Layer } from "effect";
import { AnthropicClient, anthropicConfigLayer, anthropicClientLive } from "../llm/anthropic.js";
import { FMService, FMServiceLive, type FMServiceError } from "../fm/service.js";
import type { ChatMessage } from "../llm/openrouter.js";
import { log } from "./logger.js";

// ============================================================================
// Types
// ============================================================================

/**
 * Category of generated test case.
 * Based on distilled patterns from TB2 analysis.
 */
export type TestCategory =
  | "existence"      // File/output created
  | "format"         // Structure valid
  | "happy_path"     // Basic correct behavior
  | "boundary"       // Min/max limits
  | "edge_case"      // Tricky scenarios
  | "invalid_input"  // Should fail/reject
  | "integration";   // System-level

/**
 * A single generated test case.
 */
export interface GeneratedTest {
  /** Unique identifier */
  id: string;
  /** Test input (e.g., log line for regex-log) */
  input: string;
  /** Expected output (e.g., matched date or null for no match) */
  expectedOutput: string | null;
  /** Why this test exists - reasoning from the generator */
  reasoning: string;
  /** Category of test */
  category: TestCategory;
  /** Confidence score 0-1 (how certain the generator is) */
  confidence: number;
}

/**
 * Result of test generation.
 */
export interface TestGenerationResult {
  /** Generated test cases */
  tests: GeneratedTest[];
  /** Requirements parsed from description */
  requirements: string[];
  /** Assumptions the generator made */
  assumptions: string[];
  /** Things the generator was uncertain about */
  uncertainties: string[];
  /** Model used for generation */
  model: string;
  /** Generation duration in ms */
  durationMs: number;
}

/**
 * Options for test generation.
 */
export interface TestGeneratorOptions {
  /** Minimum number of tests to generate */
  minTests?: number;
  /** Maximum number of tests to generate */
  maxTests?: number;
  /** Model to use: 'claude', 'local', or 'both' */
  model?: "claude" | "local" | "both";
  /** Temperature for generation (0-1) */
  temperature?: number;
  /** Enable verbose logging */
  verbose?: boolean;
}

// ============================================================================
// Prompt Template
// ============================================================================

/**
 * Build the test generation prompt.
 * This is the core prompt that drives test generation quality.
 */
const buildTestGenPrompt = (
  taskDescription: string,
  taskId: string,
  options: TestGeneratorOptions,
): string => {
  const minTests = options.minTests ?? 15;
  const maxTests = options.maxTests ?? 30;

  return `You are a QA engineer designing tests for a programming task. You must create
tests that would verify a correct implementation WITHOUT seeing the actual test suite.

## Task ID
${taskId}

## Task Description
${taskDescription}

## Your Mission
Generate ${minTests}-${maxTests} test cases that a correct implementation MUST pass.
Think like a QA engineer:
1. What are the explicit requirements stated in the description?
2. What edge cases are hinted at but not explicitly stated?
3. What boundaries exist (min/max, valid/invalid)?
4. What would a naive implementation get wrong?

## Test Categories (generate at least 2 per applicable category):

### 1. Existence Tests
- Does the output file exist?
- Is it non-empty?

### 2. Format Tests
- Does the output have correct structure?
- Headers, magic bytes, encoding correct?

### 3. Happy Path Tests
- Basic valid input → expected output
- The "golden path" through the requirements

### 4. Boundary Tests
For each numeric/range constraint in the description:
- Minimum valid value
- Maximum valid value
- Just below minimum (should fail/be rejected)
- Just above maximum (should fail/be rejected)

### 5. Edge Case Tests
For each constraint mentioned:
- What about the edge between valid/invalid?
- What about multiple instances?
- What about order/sequence?
- What about empty input?

### 6. Invalid Input Tests
What inputs should be rejected or produce no output?
- Invalid formats
- Out-of-range values
- Malformed data

## Output Format
Respond ONLY with a JSON object in this exact format:
{
  "requirements": ["requirement 1", "requirement 2", ...],
  "assumptions": ["assumption 1", "assumption 2", ...],
  "uncertainties": ["uncertainty 1", ...],
  "tests": [
    {
      "id": "happy_path_1",
      "input": "the test input string",
      "expectedOutput": "expected output or null",
      "reasoning": "why this test matters",
      "category": "happy_path",
      "confidence": 0.95
    },
    ...
  ]
}

## Critical Rules
1. You do NOT have access to the real test suite - you must REASON
2. Generate at least ${minTests} tests, up to ${maxTests}
3. Include tests you're uncertain about (mark confidence low)
4. Think adversarially: what would break a naive implementation?
5. For regex/pattern tasks: include both valid matches and invalid near-matches
6. For file format tasks: include malformed headers and edge cases
7. For numeric tasks: include boundary values (0, -1, max, min)

Respond with valid JSON only. No markdown, no explanation outside the JSON.`;
};

// ============================================================================
// JSON Parsing
// ============================================================================

/**
 * Parse the JSON response from the model.
 */
const parseTestGenResponse = (
  content: string,
): { tests: GeneratedTest[]; requirements: string[]; assumptions: string[]; uncertainties: string[] } | null => {
  // Strip markdown code blocks if present
  let jsonContent = content.trim();
  if (jsonContent.startsWith("```")) {
    jsonContent = jsonContent.replace(/^```(?:json)?\s*\n?/, "");
    jsonContent = jsonContent.replace(/\n?```\s*$/, "");
    jsonContent = jsonContent.trim();
  }

  try {
    const parsed = JSON.parse(jsonContent);

    // Validate structure
    if (!Array.isArray(parsed.tests)) {
      log("[TestGen] Invalid response: tests is not an array");
      return null;
    }

    // Validate and normalize each test
    const tests: GeneratedTest[] = [];
    for (const t of parsed.tests) {
      if (!t.id || typeof t.input !== "string" || !t.category) {
        log(`[TestGen] Skipping invalid test: ${JSON.stringify(t)}`);
        continue;
      }

      tests.push({
        id: t.id,
        input: t.input,
        expectedOutput: t.expectedOutput ?? null,
        reasoning: t.reasoning ?? "",
        category: t.category as TestCategory,
        confidence: typeof t.confidence === "number" ? t.confidence : 0.5,
      });
    }

    return {
      tests,
      requirements: Array.isArray(parsed.requirements) ? parsed.requirements : [],
      assumptions: Array.isArray(parsed.assumptions) ? parsed.assumptions : [],
      uncertainties: Array.isArray(parsed.uncertainties) ? parsed.uncertainties : [],
    };
  } catch (e) {
    log(`[TestGen] JSON parse error: ${e instanceof Error ? e.message : String(e)}`);
    log(`[TestGen] Content was: ${content.slice(0, 500)}...`);
    return null;
  }
};

// ============================================================================
// Generation with Claude
// ============================================================================

/**
 * Generate tests using Claude (Anthropic API).
 */
export const generateTestsWithClaude = (
  taskDescription: string,
  taskId: string,
  options: TestGeneratorOptions = {},
): Effect.Effect<TestGenerationResult, Error> =>
  Effect.gen(function* () {
    const startTime = Date.now();
    const anthropic = yield* AnthropicClient;

    const prompt = buildTestGenPrompt(taskDescription, taskId, options);

    if (options.verbose) {
      log(`[TestGen] Generating tests with Claude for task: ${taskId}`);
      log(`[TestGen] Prompt length: ${prompt.length} chars`);
    }

    const response = yield* anthropic.chat({
      model: "claude-sonnet-4-20250514",
      messages: [
        { role: "user", content: prompt },
      ],
      temperature: options.temperature ?? 0.3,
      maxTokens: 4096,
    });

    const content = response.choices[0]?.message?.content ?? "";

    if (options.verbose) {
      log(`[TestGen] Response length: ${content.length} chars`);
    }

    const parsed = parseTestGenResponse(content);

    if (!parsed) {
      return yield* Effect.fail(new Error("Failed to parse test generation response"));
    }

    if (options.verbose) {
      log(`[TestGen] Generated ${parsed.tests.length} tests`);
      log(`[TestGen] Requirements: ${parsed.requirements.length}`);
      log(`[TestGen] Uncertainties: ${parsed.uncertainties.length}`);
    }

    return {
      tests: parsed.tests,
      requirements: parsed.requirements,
      assumptions: parsed.assumptions,
      uncertainties: parsed.uncertainties,
      model: "claude-sonnet-4-20250514",
      durationMs: Date.now() - startTime,
    };
  }).pipe(
    Effect.provide(
      Layer.mergeAll(anthropicConfigLayer, anthropicClientLive),
    ),
  );

// ============================================================================
// Generation with Local FM
// ============================================================================

/**
 * Generate tests using local Foundation Model (Apple FM).
 */
export const generateTestsWithLocalFM = (
  taskDescription: string,
  taskId: string,
  options: TestGeneratorOptions = {},
): Effect.Effect<TestGenerationResult, FMServiceError> =>
  Effect.gen(function* () {
    const startTime = Date.now();
    const fm = yield* FMService;

    // Ensure FM is running
    yield* fm.ensureRunning();

    const prompt = buildTestGenPrompt(taskDescription, taskId, options);

    if (options.verbose) {
      log(`[TestGen] Generating tests with local FM for task: ${taskId}`);
      log(`[TestGen] Prompt length: ${prompt.length} chars`);
    }

    const response = yield* fm.chat({
      messages: [
        { role: "user", content: prompt },
      ],
      temperature: options.temperature ?? 0.3,
      maxTokens: 4096,
    });

    const content = response.choices[0]?.message?.content ?? "";

    if (options.verbose) {
      log(`[TestGen] Response length: ${content.length} chars`);
    }

    const parsed = parseTestGenResponse(content);

    if (!parsed) {
      return yield* Effect.fail({
        _tag: "FMServiceError" as const,
        reason: "parse_error",
        message: "Failed to parse test generation response",
        retryable: false,
        retryCount: 0,
      } as unknown as FMServiceError);
    }

    if (options.verbose) {
      log(`[TestGen] Generated ${parsed.tests.length} tests`);
    }

    return {
      tests: parsed.tests,
      requirements: parsed.requirements,
      assumptions: parsed.assumptions,
      uncertainties: parsed.uncertainties,
      model: "local-fm",
      durationMs: Date.now() - startTime,
    };
  }).pipe(
    Effect.provide(FMServiceLive),
  );

// ============================================================================
// Main API
// ============================================================================

/**
 * Generate test cases from a task description.
 *
 * This is the main entry point for test generation.
 * The agent MUST NOT have access to actual benchmark tests.
 *
 * @param taskDescription The task description (from Terminal-Bench)
 * @param taskId The task identifier
 * @param options Generation options
 * @returns Generated tests and metadata
 */
export async function generateTestsFromDescription(
  taskDescription: string,
  taskId: string,
  options: TestGeneratorOptions = {},
): Promise<TestGenerationResult> {
  const model = options.model ?? "claude";

  log(`[TestGen] Starting test generation for task: ${taskId}`);
  log(`[TestGen] Model: ${model}`);

  if (model === "claude") {
    return Effect.runPromise(
      generateTestsWithClaude(taskDescription, taskId, options),
    );
  } else if (model === "local") {
    return Effect.runPromise(
      generateTestsWithLocalFM(taskDescription, taskId, options),
    );
  } else if (model === "both") {
    // Try Claude first, fall back to local if it fails
    try {
      const claudeResult = await Effect.runPromise(
        generateTestsWithClaude(taskDescription, taskId, options),
      );
      log(`[TestGen] Claude succeeded with ${claudeResult.tests.length} tests`);
      return claudeResult;
    } catch (claudeError) {
      log(`[TestGen] Claude failed, trying local FM: ${claudeError}`);
      return Effect.runPromise(
        generateTestsWithLocalFM(taskDescription, taskId, options),
      );
    }
  } else {
    throw new Error(`Unknown model: ${model}`);
  }
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Summarize test categories for logging.
 */
export function summarizeCategories(tests: GeneratedTest[]): string {
  const counts: Record<TestCategory, number> = {
    existence: 0,
    format: 0,
    happy_path: 0,
    boundary: 0,
    edge_case: 0,
    invalid_input: 0,
    integration: 0,
  };

  for (const test of tests) {
    counts[test.category]++;
  }

  return Object.entries(counts)
    .filter(([_, count]) => count > 0)
    .map(([cat, count]) => `${cat}:${count}`)
    .join(", ");
}

/**
 * Filter tests by minimum confidence.
 */
export function filterByConfidence(
  tests: GeneratedTest[],
  minConfidence: number,
): GeneratedTest[] {
  return tests.filter((t) => t.confidence >= minConfidence);
}

/**
 * Get tests by category.
 */
export function getTestsByCategory(
  tests: GeneratedTest[],
  category: TestCategory,
): GeneratedTest[] {
  return tests.filter((t) => t.category === category);
}

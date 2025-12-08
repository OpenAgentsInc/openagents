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
import { log } from "./logger.js";
import type { EnvironmentInfo } from "./environment-info.js";

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
 * Uses guided generation for guaranteed valid output structure.
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
      log(`[TestGen] Using guided generation with test_generation schema`);
    }

    // Use guided generation with the pre-defined test_generation schema
    // This constrains the model to output valid TestGenerationResult structure
    const response = yield* fm.chat({
      messages: [
        { role: "user", content: prompt },
      ],
      temperature: options.temperature ?? 0.3,
      maxTokens: 4096,
      responseFormat: {
        type: "json_schema",
        schema_type: "test_generation",
      },
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
      model: "local-fm-guided",
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

// ============================================================================
// Environment-Aware Test Generation
// ============================================================================

/**
 * Result of environment-aware test generation.
 * Includes categorized tests with anti-cheat support.
 */
export interface EnvironmentAwareTestResult {
  /** Requirements from task description */
  descriptionRequirements: string[];
  /** Requirements inferred from environment */
  environmentRequirements: string[];
  /** Anti-cheat tests (prohibited tools/patterns NOT used) */
  antiCheatTests: GeneratedTest[];
  /** File existence and structure tests */
  existenceTests: GeneratedTest[];
  /** Correctness tests from description */
  correctnessTests: GeneratedTest[];
  /** Boundary tests from environment */
  boundaryTests: GeneratedTest[];
  /** Integration tests */
  integrationTests: GeneratedTest[];
  /** Uncertainties and assumptions */
  uncertainties: string[];
  /** Model used */
  model: string;
  /** Generation duration in ms */
  durationMs: number;
}

/**
 * Build the environment-aware test generation prompt.
 * Includes environment context for anti-cheat and parameter discovery.
 */
const buildEnvironmentAwarePrompt = (
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  options: TestGeneratorOptions,
): string => {
  // Format languages section
  const languagesSection = formatLanguages(environment);

  // Format tools section
  const toolsSection = formatTools(environment);

  // Format files section
  const filesSection = formatFiles(environment);

  // Format file previews section
  const previewsSection = formatPreviews(environment);

  return `You are a QA engineer designing tests for a programming task. You have access to:
1. The task description (what the agent is supposed to do)
2. The execution environment (what's actually available in the container)

## Task ID
${taskId}

## Task Description
${taskDescription}

## Environment Context

### Platform
- Type: ${environment.platform.type}
${environment.platform.containerImage ? `- Container Image: ${environment.platform.containerImage}` : ""}
${environment.platform.osDistro ? `- OS: ${environment.platform.osDistro} ${environment.platform.osVersion || ""}` : ""}

### Available Languages
${languagesSection}

### Available Tools
${toolsSection}

### Prohibited Tools (ANTI-CHEAT)
${environment.tools.prohibited.length > 0
  ? environment.tools.prohibited.map(t =>
      `- ${t.name}: ${t.reason} (found: ${t.found})`
    ).join("\n")
  : "None specified"
}

### Working Directory: ${environment.files.workdir}

### Files in Workspace
${filesSection}

### File Previews (Key Files)
${previewsSection}

### Resources
- Memory: ${environment.resources.memoryLimitMB ?? "unknown"} MB
- CPUs: ${environment.resources.cpuCount ?? "unknown"}

## Your Mission

Generate comprehensive tests in these categories. For each category, generate 2-5 tests based on what you see in the environment and description.

### 1. Anti-Cheat Tests (CRITICAL)
Based on the task description, what tools/approaches should be PROHIBITED?
- If this is a conversion task (e.g., R to Python), verify the original tool is NOT used
- If this is an "implement from scratch" task, verify no pre-built solutions exist
- Think: "What would a lazy implementation do that we should catch?"
${environment.tools.prohibited.length > 0
  ? `\nProhibited tools to check: ${environment.tools.prohibited.map(t => t.name).join(", ")}`
  : "\nInfer prohibited tools from the task description."
}

### 2. Existence Tests
Based on the workspace:
- Test that required output files are created in correct paths
- Test that outputs are non-empty
- Test file permissions if relevant

### 3. Correctness Tests
Based on task requirements:
- Happy path tests for main functionality
- Verify output format matches expectations
- Test all parameters mentioned in description AND visible in file previews

### 4. Boundary Tests
Based on environment constraints and file previews:
- If you see numeric parameters, test min/max values
- If you see range constraints, test boundaries
- Test limits mentioned in description

### 5. Integration Tests
System-level behavior:
- Multi-step verification if applicable
- Verify the solution integrates correctly with existing files

## Output Format
Respond ONLY with a JSON object in this exact format:
{
  "descriptionRequirements": ["requirement 1", ...],
  "environmentRequirements": ["requirement inferred from env", ...],
  "antiCheatTests": [
    {
      "id": "anti_cheat_1",
      "input": "which R 2>/dev/null || echo 'not found'",
      "expectedOutput": "not found",
      "reasoning": "R should not be installed for R→Python conversion",
      "category": "anti_cheat",
      "confidence": 0.95
    }
  ],
  "existenceTests": [...],
  "correctnessTests": [...],
  "boundaryTests": [...],
  "integrationTests": [...],
  "uncertainties": ["uncertainty 1", ...]
}

## Critical Rules
1. Generate anti-cheat tests FIRST - these catch gaming behavior
2. Use file previews to identify ALL parameters (not just those in description)
3. If you see variables like alpha, sigma, rho, beta in previews, test ALL of them
4. Think adversarially: what would a lazy implementation miss?
5. Be specific - use actual paths and values from the environment

Respond with valid JSON only. No markdown, no explanation outside the JSON.`;
};

/**
 * Format languages section for prompt.
 */
function formatLanguages(env: EnvironmentInfo): string {
  const langs: string[] = [];
  const { languages } = env;

  if (languages.python) {
    langs.push(`- Python ${languages.python.version}`);
    if (languages.python.packages.length > 0) {
      const topPackages = languages.python.packages.slice(0, 10);
      langs.push(`  Packages: ${topPackages.map(p => `${p.name}==${p.version}`).join(", ")}`);
      if (languages.python.packages.length > 10) {
        langs.push(`  ... and ${languages.python.packages.length - 10} more`);
      }
    }
  }

  if (languages.node) {
    langs.push(`- Node ${languages.node.version}`);
    if (languages.node.npmVersion) langs.push(`  npm: ${languages.node.npmVersion}`);
  }

  if (languages.r) {
    langs.push(`- R ${languages.r.version}`);
    if (languages.r.packages.length > 0) {
      langs.push(`  Packages: ${languages.r.packages.slice(0, 5).map(p => p.name).join(", ")}...`);
    }
  }

  if (languages.rust) langs.push(`- Rust ${languages.rust.version}`);
  if (languages.go) langs.push(`- Go ${languages.go.version}`);
  if (languages.java) langs.push(`- Java ${languages.java.version}`);

  return langs.length > 0 ? langs.join("\n") : "None detected";
}

/**
 * Format tools section for prompt.
 */
function formatTools(env: EnvironmentInfo): string {
  const { tools } = env;
  if (tools.available.length === 0) return "None detected";
  return tools.available.map(t =>
    t.version ? `- ${t.name} (${t.version})` : `- ${t.name}`
  ).join("\n");
}

/**
 * Format files section for prompt.
 */
function formatFiles(env: EnvironmentInfo): string {
  const { files } = env;
  if (files.listing.length === 0) return "Empty directory";
  return files.listing.slice(0, 20).map(f =>
    `- ${f.name} (${f.type}, ${f.size} bytes)`
  ).join("\n") +
    (files.listing.length > 20 ? `\n... and ${files.listing.length - 20} more` : "");
}

/**
 * Format file previews section for prompt.
 */
function formatPreviews(env: EnvironmentInfo): string {
  const { files } = env;
  if (files.taskFiles.length === 0) return "No previews available";

  return files.taskFiles.map(f => {
    const header = `=== ${f.path} (${f.detectedType ?? f.extension}, ${f.lineCount} lines) ===`;

    // Include extracted structure if available
    let structure = "";
    if (f.structure) {
      const parts: string[] = [];
      if (f.structure.variables?.length) parts.push(`Variables: ${f.structure.variables.join(", ")}`);
      if (f.structure.functions?.length) parts.push(`Functions: ${f.structure.functions.join(", ")}`);
      if (f.structure.parameters?.length) parts.push(`Parameters: ${f.structure.parameters.join(", ")}`);
      if (f.structure.columns?.length) parts.push(`Columns: ${f.structure.columns.join(", ")}`);
      if (parts.length > 0) structure = `\n[Structure: ${parts.join("; ")}]`;
    }

    return `${header}${structure}\n${f.preview}`;
  }).join("\n\n");
}

/**
 * Parse environment-aware test generation response.
 */
const parseEnvironmentAwareResponse = (
  content: string,
): Omit<EnvironmentAwareTestResult, "model" | "durationMs"> | null => {
  // Strip markdown code blocks if present
  let jsonContent = content.trim();
  if (jsonContent.startsWith("```")) {
    jsonContent = jsonContent.replace(/^```(?:json)?\s*\n?/, "");
    jsonContent = jsonContent.replace(/\n?```\s*$/, "");
    jsonContent = jsonContent.trim();
  }

  try {
    const parsed = JSON.parse(jsonContent);

    // Helper to normalize test array
    const normalizeTests = (tests: unknown[]): GeneratedTest[] => {
      if (!Array.isArray(tests)) return [];
      return tests.filter((t: unknown) => {
        const test = t as Record<string, unknown>;
        return test && typeof test.id === "string" && typeof test.input === "string";
      }).map((t: unknown) => {
        const test = t as Record<string, unknown>;
        return {
          id: test.id as string,
          input: test.input as string,
          expectedOutput: (test.expectedOutput as string | null) ?? null,
          reasoning: (test.reasoning as string) ?? "",
          category: (test.category as TestCategory) ?? "happy_path",
          confidence: typeof test.confidence === "number" ? test.confidence : 0.5,
        };
      });
    };

    return {
      descriptionRequirements: Array.isArray(parsed.descriptionRequirements) ? parsed.descriptionRequirements : [],
      environmentRequirements: Array.isArray(parsed.environmentRequirements) ? parsed.environmentRequirements : [],
      antiCheatTests: normalizeTests(parsed.antiCheatTests || []),
      existenceTests: normalizeTests(parsed.existenceTests || []),
      correctnessTests: normalizeTests(parsed.correctnessTests || []),
      boundaryTests: normalizeTests(parsed.boundaryTests || []),
      integrationTests: normalizeTests(parsed.integrationTests || []),
      uncertainties: Array.isArray(parsed.uncertainties) ? parsed.uncertainties : [],
    };
  } catch (e) {
    log(`[TestGen] JSON parse error: ${e instanceof Error ? e.message : String(e)}`);
    log(`[TestGen] Content was: ${content.slice(0, 500)}...`);
    return null;
  }
};

/**
 * Generate tests using environment context with local FM.
 * Uses guided generation with environment_aware_test_generation schema.
 */
export const generateTestsWithEnvironmentLocalFM = (
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  options: TestGeneratorOptions = {},
): Effect.Effect<EnvironmentAwareTestResult, FMServiceError> =>
  Effect.gen(function* () {
    const startTime = Date.now();
    const fm = yield* FMService;

    yield* fm.ensureRunning();

    const prompt = buildEnvironmentAwarePrompt(taskDescription, taskId, environment, options);

    if (options.verbose) {
      log(`[TestGen] Generating environment-aware tests for task: ${taskId}`);
      log(`[TestGen] Prompt length: ${prompt.length} chars`);
      log(`[TestGen] Prohibited tools: ${environment.tools.prohibited.map(t => t.name).join(", ") || "none"}`);
      log(`[TestGen] File previews: ${environment.files.taskFiles.length}`);
    }

    const response = yield* fm.chat({
      messages: [{ role: "user", content: prompt }],
      temperature: options.temperature ?? 0.3,
      maxTokens: 8192,
      responseFormat: {
        type: "json_schema",
        schema_type: "environment_aware_test_generation",
      },
    });

    const content = response.choices[0]?.message?.content ?? "";

    if (options.verbose) {
      log(`[TestGen] Response length: ${content.length} chars`);
    }

    const parsed = parseEnvironmentAwareResponse(content);

    if (!parsed) {
      return yield* Effect.fail({
        _tag: "FMServiceError" as const,
        reason: "parse_error",
        message: "Failed to parse environment-aware test generation response",
        retryable: false,
        retryCount: 0,
      } as unknown as FMServiceError);
    }

    const totalTests =
      parsed.antiCheatTests.length +
      parsed.existenceTests.length +
      parsed.correctnessTests.length +
      parsed.boundaryTests.length +
      parsed.integrationTests.length;

    if (options.verbose) {
      log(`[TestGen] Generated ${totalTests} tests:`);
      log(`[TestGen]   Anti-cheat: ${parsed.antiCheatTests.length}`);
      log(`[TestGen]   Existence: ${parsed.existenceTests.length}`);
      log(`[TestGen]   Correctness: ${parsed.correctnessTests.length}`);
      log(`[TestGen]   Boundary: ${parsed.boundaryTests.length}`);
      log(`[TestGen]   Integration: ${parsed.integrationTests.length}`);
    }

    return {
      ...parsed,
      model: "local-fm-env-aware",
      durationMs: Date.now() - startTime,
    };
  }).pipe(
    Effect.provide(FMServiceLive),
  );

/**
 * Generate tests using environment context with Claude.
 */
export const generateTestsWithEnvironmentClaude = (
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  options: TestGeneratorOptions = {},
): Effect.Effect<EnvironmentAwareTestResult, Error> =>
  Effect.gen(function* () {
    const startTime = Date.now();
    const anthropic = yield* AnthropicClient;

    const prompt = buildEnvironmentAwarePrompt(taskDescription, taskId, environment, options);

    if (options.verbose) {
      log(`[TestGen] Generating environment-aware tests with Claude for task: ${taskId}`);
    }

    const response = yield* anthropic.chat({
      model: "claude-sonnet-4-20250514",
      messages: [{ role: "user", content: prompt }],
      temperature: options.temperature ?? 0.3,
      maxTokens: 8192,
    });

    const content = response.choices[0]?.message?.content ?? "";
    const parsed = parseEnvironmentAwareResponse(content);

    if (!parsed) {
      return yield* Effect.fail(new Error("Failed to parse environment-aware test generation response"));
    }

    return {
      ...parsed,
      model: "claude-sonnet-env-aware",
      durationMs: Date.now() - startTime,
    };
  }).pipe(
    Effect.provide(Layer.mergeAll(anthropicConfigLayer, anthropicClientLive)),
  );

/**
 * Generate tests from task description AND environment context.
 *
 * This is the main entry point for environment-aware test generation.
 * Uses environment information for:
 * - Anti-cheat tests (prohibited tools)
 * - Parameter discovery (from file previews)
 * - Boundary detection (from resources)
 *
 * @param taskDescription The task description
 * @param taskId The task identifier
 * @param environment The introspected environment
 * @param options Generation options
 * @returns Environment-aware test result with categorized tests
 */
export async function generateTestsFromEnvironment(
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  options: TestGeneratorOptions = {},
): Promise<EnvironmentAwareTestResult> {
  const model = options.model ?? "local";

  log(`[TestGen] Starting environment-aware test generation for task: ${taskId}`);
  log(`[TestGen] Model: ${model}`);
  log(`[TestGen] Environment: ${environment.platform.type}`);

  if (model === "claude") {
    return Effect.runPromise(
      generateTestsWithEnvironmentClaude(taskDescription, taskId, environment, options),
    );
  } else if (model === "local") {
    return Effect.runPromise(
      generateTestsWithEnvironmentLocalFM(taskDescription, taskId, environment, options),
    );
  } else if (model === "both") {
    // Try local FM first (guided generation), fall back to Claude
    try {
      const localResult = await Effect.runPromise(
        generateTestsWithEnvironmentLocalFM(taskDescription, taskId, environment, options),
      );
      log(`[TestGen] Local FM succeeded`);
      return localResult;
    } catch (localError) {
      log(`[TestGen] Local FM failed, trying Claude: ${localError}`);
      return Effect.runPromise(
        generateTestsWithEnvironmentClaude(taskDescription, taskId, environment, options),
      );
    }
  } else {
    throw new Error(`Unknown model: ${model}`);
  }
}

/**
 * Get all tests from an environment-aware result as a flat array.
 */
export function getAllTestsFromEnvironmentResult(
  result: EnvironmentAwareTestResult,
): GeneratedTest[] {
  return [
    ...result.antiCheatTests,
    ...result.existenceTests,
    ...result.correctnessTests,
    ...result.boundaryTests,
    ...result.integrationTests,
  ];
}

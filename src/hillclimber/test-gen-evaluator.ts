/**
 * Test Generation Evaluator
 *
 * Compares generated tests to actual TB2 tests to measure quality.
 * This enables hill-climbing on the test generator itself.
 *
 * Key metrics:
 * - Coverage: What percentage of actual test categories are covered?
 * - Accuracy: Are the expected values correct?
 * - Edge Case Detection: Did we anticipate the hard cases?
 * - Category Balance: Do we have the right mix of test types?
 */

import { Effect } from "effect"
import { BunContext } from "@effect/platform-bun"
import {
    loadTerminalBenchSuite, type TerminalBenchTask
} from "../bench/terminal-bench.js"

import type { GeneratedTest, TestCategory, TestGenerationResult } from "./test-generator.js";

// ============================================================================
// Types
// ============================================================================

/**
 * A test case extracted from actual TB2 tests.
 */
export interface ActualTestCase {
  /** Test ID/name from pytest */
  id: string;
  /** Test input */
  input: string;
  /** Expected output */
  expectedOutput: string | null;
  /** Category inferred from test name/content */
  category: TestCategory;
  /** Difficulty: easy tests pass naively, hard ones require edge case handling */
  difficulty: "easy" | "medium" | "hard";
  /** Source line in test file */
  sourceLine?: number;
}

/**
 * Extracted test suite from a TB2 task's test_outputs.py.
 */
export interface ActualTestSuite {
  taskId: string;
  tests: ActualTestCase[];
  rawTestContent: string;
  extractionErrors: string[];
}

/**
 * Comparison result for a single test case.
 */
export interface TestComparison {
  /** Generated test */
  generated: GeneratedTest;
  /** Matched actual test (if any) */
  matchedActual: ActualTestCase | null;
  /** How well did it match */
  matchType: "exact" | "partial" | "category_only" | "no_match";
  /** Was the expected output correct? */
  expectedCorrect: boolean;
  /** Notes about the comparison */
  notes: string;
}

/**
 * Overall quality score for generated tests.
 */
export interface TestGenerationScore {
  /** Overall score 0-100 */
  overall: number;
  /** Coverage: % of actual test categories covered */
  coverage: number;
  /** Accuracy: % of generated tests with correct expectations */
  accuracy: number;
  /** Edge case detection: % of hard cases anticipated */
  edgeCaseDetection: number;
  /** Category balance: deviation from ideal distribution */
  categoryBalance: number;
  /** Number of exact matches */
  exactMatches: number;
  /** Number of partial matches */
  partialMatches: number;
  /** Number of false positives (generated tests that don't correspond to real ones) */
  falsePositives: number;
  /** Number of missed tests (real tests not anticipated) */
  missedTests: number;
  /** Detailed comparisons */
  comparisons: TestComparison[];
}

// ============================================================================
// Test Extraction from Python Files
// ============================================================================

/**
 * Extract test cases from a TB2 test_outputs.py file.
 *
 * This parser handles common patterns in TB2 tests:
 * - pytest test functions with assertions
 * - Test data defined as lists/dicts
 * - Expected values in assertions
 */
export async function extractActualTests(
  testFilePath: string,
  taskId: string,
): Promise<ActualTestSuite> {
  const fs = await import("node:fs");

  if (!fs.existsSync(testFilePath)) {
    return {
      taskId,
      tests: [],
      rawTestContent: "",
      extractionErrors: [`Test file not found: ${testFilePath}`],
    };
  }

  const content = fs.readFileSync(testFilePath, "utf-8");
  const tests: ActualTestCase[] = [];
  const errors: string[] = [];

  // Task-specific extraction
  if (taskId === "regex-log") {
    const result = extractRegexLogTests(content, taskId);
    tests.push(...result.tests);
    errors.push(...result.errors);
  } else if (taskId === "path-tracing") {
    const result = extractPathTracingTests(content, taskId);
    tests.push(...result.tests);
    errors.push(...result.errors);
  } else if (taskId === "dna-assembly") {
    const result = extractDnaAssemblyTests(content, taskId);
    tests.push(...result.tests);
    errors.push(...result.errors);
  } else {
    // Generic extraction for other tasks
    const result = extractGenericTests(content, taskId);
    tests.push(...result.tests);
    errors.push(...result.errors);
  }

  return {
    taskId,
    tests,
    rawTestContent: content,
    extractionErrors: errors,
  };
}

/**
 * Extract tests from regex-log test file.
 */
function extractRegexLogTests(
  content: string,
  taskId: string,
): { tests: ActualTestCase[]; errors: string[] } {
  const tests: ActualTestCase[] = [];
  const errors: string[] = [];

  // Look for test input lines and expected dates
  const testLinesMatch = content.match(/test_lines\s*=\s*\[([\s\S]*?)\]/);
  const expectedDatesMatch = content.match(/expected_dates\s*=\s*\[([\s\S]*?)\]/);

  if (!testLinesMatch || !expectedDatesMatch) {
    errors.push("Could not extract test_lines or expected_dates from regex-log");
    return { tests, errors };
  }

  // Parse test lines (strings in a Python list)
  const testLinesStr = testLinesMatch[1];
  const testLines = parseStringArray(testLinesStr);

  // Parse expected dates
  const expectedDatesStr = expectedDatesMatch[1];
  const expectedDates = parseStringArray(expectedDatesStr);

  // Create test cases
  let expectedIdx = 0;
  for (let i = 0; i < testLines.length; i++) {
    const line = testLines[i];

    // Determine if this line should match (has valid IP and date)
    // Lines with valid dates have them in expected_dates
    // This is a heuristic - actual behavior depends on regex
    const shouldMatch = expectedIdx < expectedDates.length && lineContainsValidIPAndDate(line);

    if (shouldMatch) {
      tests.push({
        id: `regex_log_${i + 1}`,
        input: line,
        expectedOutput: expectedDates[expectedIdx],
        category: determineCategory(line, expectedDates[expectedIdx]),
        difficulty: determineDifficulty(line),
      });
      expectedIdx++;
    } else {
      tests.push({
        id: `regex_log_${i + 1}`,
        input: line,
        expectedOutput: null,
        category: "invalid_input",
        difficulty: determineDifficulty(line),
      });
    }
  }

  return { tests, errors };
}

/**
 * Extract tests from path-tracing test file.
 */
function extractPathTracingTests(
  content: string,
  _taskId: string,
): { tests: ActualTestCase[]; errors: string[] } {
  const tests: ActualTestCase[] = [];
  const errors: string[] = [];

  // Path tracing tests check:
  // 1. File exists
  // 2. Compiles
  // 3. Size constraint
  // 4. Similarity threshold

  tests.push({
    id: "existence",
    input: "image.c",
    expectedOutput: "file exists",
    category: "existence",
    difficulty: "easy",
  });

  tests.push({
    id: "compilation",
    input: "image.c",
    expectedOutput: "compiles successfully",
    category: "format",
    difficulty: "easy",
  });

  tests.push({
    id: "size_constraint",
    input: "image.c compiled",
    expectedOutput: "< 2100 bytes compressed",
    category: "boundary",
    difficulty: "medium",
  });

  tests.push({
    id: "similarity",
    input: "image.ppm output",
    expectedOutput: ">= 0.99 cosine similarity",
    category: "happy_path",
    difficulty: "hard",
  });

  return { tests, errors };
}

/**
 * Extract tests from dna-assembly test file.
 */
function extractDnaAssemblyTests(
  content: string,
  _taskId: string,
): { tests: ActualTestCase[]; errors: string[] } {
  const tests: ActualTestCase[] = [];
  const errors: string[] = [];

  // DNA assembly tests check biological constraints
  tests.push({
    id: "primer_count",
    input: "primers.fasta",
    expectedOutput: "16 primers (8 pairs for 4 templates)",
    category: "format",
    difficulty: "easy",
  });

  tests.push({
    id: "bsai_sites",
    input: "all primers",
    expectedOutput: "contain ggtctc BsaI site",
    category: "happy_path",
    difficulty: "medium",
  });

  tests.push({
    id: "tm_range",
    input: "all primers",
    expectedOutput: "Tm 58-72°C",
    category: "boundary",
    difficulty: "medium",
  });

  tests.push({
    id: "tm_difference",
    input: "primer pairs",
    expectedOutput: "≤ 5°C difference",
    category: "boundary",
    difficulty: "medium",
  });

  tests.push({
    id: "overhang_match",
    input: "adjacent primers",
    expectedOutput: "overhangs match reverse-complement",
    category: "edge_case",
    difficulty: "hard",
  });

  tests.push({
    id: "unique_overhangs",
    input: "all junctions",
    expectedOutput: "4 unique overhangs",
    category: "edge_case",
    difficulty: "hard",
  });

  tests.push({
    id: "circular_assembly",
    input: "assembled sequence",
    expectedOutput: "forms closed circle",
    category: "integration",
    difficulty: "hard",
  });

  return { tests, errors };
}

/**
 * Generic test extraction for tasks without specific parsers.
 */
function extractGenericTests(
  content: string,
  taskId: string,
): { tests: ActualTestCase[]; errors: string[] } {
  const tests: ActualTestCase[] = [];
  const errors: string[] = [];

  // Look for pytest test functions
  const testFunctionPattern = /def (test_\w+)\([^)]*\):/g;
  let match;

  while ((match = testFunctionPattern.exec(content)) !== null) {
    const testName = match[1];
    tests.push({
      id: testName,
      input: `(function ${testName})`,
      expectedOutput: "(assertion passes)",
      category: inferCategoryFromName(testName),
      difficulty: "medium",
    });
  }

  if (tests.length === 0) {
    errors.push(`No test functions found in ${taskId}`);
  }

  return { tests, errors };
}

// ============================================================================
// Comparison Logic
// ============================================================================

/**
 * Compare generated tests to actual tests and compute quality score.
 */
export function compareTests(
  generated: TestGenerationResult,
  actual: ActualTestSuite,
): TestGenerationScore {
  const comparisons: TestComparison[] = [];

  // Track which actual tests were matched
  const matchedActualIds = new Set<string>();

  // Compare each generated test
  for (const gen of generated.tests) {
    const comparison = findBestMatch(gen, actual.tests, matchedActualIds);
    comparisons.push(comparison);

    if (comparison.matchedActual) {
      matchedActualIds.add(comparison.matchedActual.id);
    }
  }

  // Count metrics
  const exactMatches = comparisons.filter((c) => c.matchType === "exact").length;
  const partialMatches = comparisons.filter((c) => c.matchType === "partial").length;
  const falsePositives = comparisons.filter((c) => c.matchType === "no_match").length;
  const missedTests = actual.tests.filter((t) => !matchedActualIds.has(t.id)).length;

  // Calculate category coverage
  const actualCategories = new Set(actual.tests.map((t) => t.category));
  const generatedCategories = new Set(generated.tests.map((t) => t.category));
  const coveredCategories = [...actualCategories].filter((c) => generatedCategories.has(c));
  const coverage = actualCategories.size > 0
    ? (coveredCategories.length / actualCategories.size) * 100
    : 100;

  // Calculate accuracy (% with correct expected output)
  const accuracyComparisons = comparisons.filter((c) => c.matchedActual !== null);
  const accuracy = accuracyComparisons.length > 0
    ? (accuracyComparisons.filter((c) => c.expectedCorrect).length / accuracyComparisons.length) * 100
    : 0;

  // Calculate edge case detection
  const hardActualTests = actual.tests.filter((t) => t.difficulty === "hard");
  const detectedHardTests = hardActualTests.filter((t) => matchedActualIds.has(t.id));
  const edgeCaseDetection = hardActualTests.length > 0
    ? (detectedHardTests.length / hardActualTests.length) * 100
    : 100;

  // Calculate category balance (deviation from ideal)
  const idealCategories: Record<TestCategory, number> = {
    existence: 0.05,
    format: 0.10,
    happy_path: 0.25,
    boundary: 0.20,
    edge_case: 0.25,
    invalid_input: 0.10,
    integration: 0.05,
  };

  const generatedCounts: Record<TestCategory, number> = {
    existence: 0,
    format: 0,
    happy_path: 0,
    boundary: 0,
    edge_case: 0,
    invalid_input: 0,
    integration: 0,
  };

  for (const test of generated.tests) {
    generatedCounts[test.category]++;
  }

  const totalGenerated = generated.tests.length;
  let balanceDeviation = 0;
  for (const [cat, ideal] of Object.entries(idealCategories)) {
    const actual = totalGenerated > 0 ? generatedCounts[cat as TestCategory] / totalGenerated : 0;
    balanceDeviation += Math.abs(ideal - actual);
  }
  const categoryBalance = Math.max(0, 100 - balanceDeviation * 100);

  // Overall score (weighted average)
  const overall = (
    coverage * 0.25 +
    accuracy * 0.30 +
    edgeCaseDetection * 0.25 +
    categoryBalance * 0.20
  );

  return {
    overall,
    coverage,
    accuracy,
    edgeCaseDetection,
    categoryBalance,
    exactMatches,
    partialMatches,
    falsePositives,
    missedTests,
    comparisons,
  };
}

/**
 * Find the best matching actual test for a generated test.
 */
function findBestMatch(
  generated: GeneratedTest,
  actualTests: ActualTestCase[],
  alreadyMatched: Set<string>,
): TestComparison {
  let bestMatch: ActualTestCase | null = null;
  let bestMatchType: "exact" | "partial" | "category_only" | "no_match" = "no_match";

  for (const actual of actualTests) {
    if (alreadyMatched.has(actual.id)) continue;

    // Check for exact input match
    if (generated.input === actual.input) {
      bestMatch = actual;
      bestMatchType = "exact";
      break;
    }

    // Check for partial match (similar input)
    if (inputsSimilar(generated.input, actual.input)) {
      if (bestMatchType === "no_match" || bestMatchType === "category_only") {
        bestMatch = actual;
        bestMatchType = "partial";
      }
    }

    // Check for category-only match
    if (generated.category === actual.category && bestMatchType === "no_match") {
      bestMatch = actual;
      bestMatchType = "category_only";
    }
  }

  const expectedCorrect = bestMatch !== null &&
    (generated.expectedOutput === bestMatch.expectedOutput ||
      (generated.expectedOutput === null && bestMatch.expectedOutput === null));

  let notes = "";
  if (bestMatch) {
    if (expectedCorrect) {
      notes = `Matched ${bestMatch.id} with correct expected value`;
    } else {
      notes = `Matched ${bestMatch.id} but expected "${generated.expectedOutput}" vs actual "${bestMatch.expectedOutput}"`;
    }
  } else {
    notes = "No matching actual test found";
  }

  return {
    generated,
    matchedActual: bestMatch,
    matchType: bestMatchType,
    expectedCorrect,
    notes,
  };
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Parse a Python string array like: ["a", "b", "c"]
 */
function parseStringArray(content: string): string[] {
  const result: string[] = [];
  const stringPattern = /"([^"\\]*(?:\\.[^"\\]*)*)"|'([^'\\]*(?:\\.[^'\\]*)*)'/g;
  let match;

  while ((match = stringPattern.exec(content)) !== null) {
    result.push(match[1] || match[2]);
  }

  return result;
}

/**
 * Check if a line contains a valid IP and date (heuristic).
 */
function lineContainsValidIPAndDate(line: string): boolean {
  // Simple heuristic: contains something that looks like an IP and a date
  const hasIP = /\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}/.test(line);
  const hasDate = /\d{4}-\d{2}-\d{2}/.test(line);
  return hasIP && hasDate;
}

/**
 * Determine category from test line content.
 */
function determineCategory(line: string, expectedDate: string | null): TestCategory {
  if (!expectedDate) return "invalid_input";

  // Look for edge case indicators
  if (line.includes("256") || line.includes("999")) return "boundary";
  if (line.includes("abc") || line.includes("xyz")) return "edge_case";
  if (line.includes("0.0.0.0") || line.includes("255.255.255.255")) return "boundary";

  return "happy_path";
}

/**
 * Determine test difficulty from content.
 */
function determineDifficulty(line: string): "easy" | "medium" | "hard" {
  // Hard: tricky edge cases
  if (line.includes("256") || line.includes("abc1") || line.includes("1abc")) {
    return "hard";
  }

  // Medium: boundary cases
  if (line.includes("0.0.0.0") || line.includes("255")) {
    return "medium";
  }

  return "easy";
}

/**
 * Infer category from test function name.
 */
function inferCategoryFromName(name: string): TestCategory {
  const nameLower = name.toLowerCase();

  if (nameLower.includes("exist") || nameLower.includes("file")) return "existence";
  if (nameLower.includes("format") || nameLower.includes("struct")) return "format";
  if (nameLower.includes("bound") || nameLower.includes("limit") || nameLower.includes("range")) return "boundary";
  if (nameLower.includes("edge") || nameLower.includes("corner")) return "edge_case";
  if (nameLower.includes("invalid") || nameLower.includes("error")) return "invalid_input";
  if (nameLower.includes("integrat")) return "integration";

  return "happy_path";
}

/**
 * Check if two inputs are similar.
 */
function inputsSimilar(a: string, b: string): boolean {
  // Normalize and compare
  const normalize = (s: string) => s.toLowerCase().replace(/\s+/g, " ").trim();
  const na = normalize(a);
  const nb = normalize(b);

  if (na === nb) return true;

  // Check for significant overlap
  const wordsA = new Set(na.split(/\s+/));
  const wordsB = new Set(nb.split(/\s+/));
  const intersection = [...wordsA].filter((w) => wordsB.has(w));

  return intersection.length / Math.max(wordsA.size, wordsB.size) > 0.5;
}

// ============================================================================
// Main API
// ============================================================================

/**
 * Evaluate test generation quality for a task.
 */
export async function evaluateTestGeneration(
  taskId: string,
  generated: TestGenerationResult,
  tb2Path: string,
): Promise<{ score: TestGenerationScore; actual: ActualTestSuite }> {
  const testFilePath = `${tb2Path}/${taskId}/tests/test_outputs.py`;

  const actual = await extractActualTests(testFilePath, taskId);
  const score = compareTests(generated, actual);

  return { score, actual };
}

/**
 * Load the TB2 suite from the default path.
 */
export async function loadTB2Tasks(): Promise<TerminalBenchTask[]> {
  const suitePath = "tasks/terminal-bench-2.json";
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite(suitePath).pipe(Effect.provide(BunContext.layer)),
  );
  return [...suite.tasks];
}

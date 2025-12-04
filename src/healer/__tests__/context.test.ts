/**
 * Tests for Healer Context Builder
 */
import { describe, test, expect } from "bun:test";
import {
  detectErrorPatterns,
  buildHeuristics,
} from "../context.js";

// ============================================================================
// detectErrorPatterns Tests
// ============================================================================

describe("detectErrorPatterns", () => {
  test("returns empty array for null input", () => {
    expect(detectErrorPatterns(null)).toEqual([]);
  });

  test("returns empty array for empty string", () => {
    expect(detectErrorPatterns("")).toEqual([]);
  });

  test("detects TypeScript compilation errors", () => {
    const output = "error TS2345: Argument of type 'string' is not assignable";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("TypeScript compilation error");
  });

  test("detects missing module or name errors", () => {
    const output = "Cannot find module '@/utils/helper'";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Missing module or name");
  });

  test("detects property access errors", () => {
    const output = "Property 'foo' does not exist on type 'Bar'";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Property access error");
  });

  test("detects type assignment errors", () => {
    const output = "Type 'string' is not assignable to type 'number'";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Type assignment error");
  });

  test("detects test failures", () => {
    const output = "5 tests failed";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Test failures");
  });

  test("detects assertion failures", () => {
    const output = "expect(result).toBe(true)";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Assertion failure");
  });

  test("detects import resolution errors", () => {
    const output = "import { foo } from './missing' - module not found";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Import resolution error");
  });

  test("detects export not found errors", () => {
    const output = "export 'MyComponent' not found in './components'";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Export not found");
  });

  test("detects TypeError", () => {
    const output = "TypeError: Cannot read property 'x' of undefined";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Runtime type error");
  });

  test("detects ReferenceError", () => {
    const output = "ReferenceError: foo is not defined";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Reference error");
  });

  test("detects SyntaxError", () => {
    const output = "SyntaxError: Unexpected token '}'";
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("Syntax error");
  });

  test("detects multiple error patterns", () => {
    const output = `
      error TS2345: Argument of type 'string' is not assignable
      TypeError: Cannot read property 'x' of undefined
      5 tests failed
    `;
    const patterns = detectErrorPatterns(output);
    expect(patterns).toContain("TypeScript compilation error");
    expect(patterns).toContain("Type assignment error"); // Also triggered by "is not assignable"
    expect(patterns).toContain("Runtime type error");
    expect(patterns).toContain("Test failures");
    expect(patterns.length).toBe(4);
  });
});

// ============================================================================
// buildHeuristics Tests
// ============================================================================

describe("buildHeuristics", () => {
  test("builds heuristics with scenario and failure count", () => {
    const heuristics = buildHeuristics("SubtaskFailed", null, 2);

    expect(heuristics.scenario).toBe("SubtaskFailed");
    expect(heuristics.failureCount).toBe(2);
    expect(heuristics.isFlaky).toBe(false);
    expect(heuristics.previousAttempts).toBe(0);
  });

  test("detects missing imports from error patterns", () => {
    const output = "Cannot find module '@/utils/helper'";
    const heuristics = buildHeuristics("InitScriptTypecheckFailure", output, 1);

    expect(heuristics.hasMissingImports).toBe(true);
    expect(heuristics.errorPatterns).toContain("Missing module or name");
  });

  test("detects type errors from error patterns", () => {
    const output = "error TS2345: Type 'string' is not assignable";
    const heuristics = buildHeuristics("InitScriptTypecheckFailure", output, 1);

    expect(heuristics.hasTypeErrors).toBe(true);
    expect(heuristics.errorPatterns).toContain("TypeScript compilation error");
    expect(heuristics.errorPatterns).toContain("Type assignment error");
  });

  test("detects test assertions from error patterns", () => {
    const output = `
      expect(result).toBe(true)
      3 tests failed
    `;
    const heuristics = buildHeuristics("InitScriptTestFailure", output, 1);

    expect(heuristics.hasTestAssertions).toBe(true);
    expect(heuristics.errorPatterns).toContain("Assertion failure");
    expect(heuristics.errorPatterns).toContain("Test failures");
  });

  test("handles null error output", () => {
    const heuristics = buildHeuristics("RuntimeError", null, 0);

    expect(heuristics.hasMissingImports).toBe(false);
    expect(heuristics.hasTypeErrors).toBe(false);
    expect(heuristics.hasTestAssertions).toBe(false);
    expect(heuristics.errorPatterns).toEqual([]);
  });
});

/**
 * TestGen to Pytest Converter
 *
 * Converts testgen output (GeneratedTest[]) to pytest format for verification.
 */

import type { GeneratedTest } from "./test-generator.js";

export interface TestGenToPytestOptions {
  /** Name of the module being tested (e.g., "regex") */
  moduleName: string;
  /** Path to the solution file (e.g., "/app/regex.txt") */
  solutionPath: string;
  /** Type of task (affects test structure) */
  taskType: "regex" | "script" | "code";
}

/**
 * Convert testgen tests to pytest Python code.
 *
 * Generates a test_outputs.py file that pytest can run.
 * Each testgen test becomes a pytest function.
 */
export function convertTestsToPytest(
  tests: GeneratedTest[],
  options: TestGenToPytestOptions
): string {
  const lines: string[] = [];

  // Header
  lines.push("#!/usr/bin/env python3");
  lines.push('"""');
  lines.push("Generated test file from TestGen");
  lines.push(`Total tests: ${tests.length}`);
  lines.push('"""');
  lines.push("");

  // Imports based on task type
  if (options.taskType === "regex") {
    lines.push("import re");
    lines.push("import pytest");
    lines.push("");

    // Load regex pattern
    lines.push("# Load regex pattern from solution");
    lines.push(`pattern_path = "${options.solutionPath}"`);
    lines.push("with open(pattern_path, 'r') as f:");
    lines.push("    pattern_text = f.read().strip()");
    lines.push("");
  }

  // Generate test functions
  for (const test of tests) {
    const functionName = sanitizeFunctionName(test.id || test.category);

    lines.push(`def test_${functionName}():`);
    lines.push(`    """`);
    lines.push(`    ${test.category.toUpperCase()}: ${test.reasoning}`);
    lines.push(`    Confidence: ${test.confidence}`);
    lines.push(`    """`);

    if (options.taskType === "regex") {
      // Regex test
      // CRITICAL: Strip any leading/trailing quotes that LLM may have added
      let inputValue = test.input;
      if (typeof inputValue === "string") {
        inputValue = inputValue.replace(/^["']+|["']+$/g, "");
      }
      lines.push(`    input_text = ${pythonStringLiteral(inputValue)}`);

      if (test.expectedOutput === null || test.expectedOutput === undefined) {
        // Should NOT match
        lines.push(`    matches = re.findall(pattern_text, input_text, re.MULTILINE)`);
        lines.push(`    assert len(matches) == 0, f"Expected no match, but got {matches}"`);
      } else {
        // Should match and return expected output
        // CRITICAL: Strip any leading/trailing quotes that LLM may have added
        // Handle cases like: '"2023-10-01"', '"""2023-10-01"""', ""2023-10-01"", etc.
        let expectedValue = test.expectedOutput;
        if (typeof expectedValue === "string") {
          // Remove ALL leading/trailing quote characters (single, double, or triple)
          // This handles: '"value"', '"""value"""', ""value"", etc.
          expectedValue = expectedValue.replace(/^["']+|["']+$/g, "");
          // Also handle triple quotes specifically
          expectedValue = expectedValue.replace(/^"""|"""$/g, "");
          expectedValue = expectedValue.replace(/^'''|'''$/g, "");
          // Final cleanup: remove any remaining leading/trailing quotes
          expectedValue = expectedValue.replace(/^["']+|["']+$/g, "");
        }
        lines.push(`    matches = re.findall(pattern_text, input_text, re.MULTILINE)`);
        lines.push(`    expected = ${pythonStringLiteral(expectedValue)}`);
        lines.push(`    assert len(matches) > 0, f"Expected match '{expected}', but got no matches"`);
        lines.push(`    assert matches[0] == expected, f"Expected '{expected}', got '{matches[0]}'"`);
      }
    }

    lines.push("");
  }

  return lines.join("\n");
}

/**
 * Sanitize test ID/category for use as Python function name.
 */
function sanitizeFunctionName(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9_]/g, "_")
    .replace(/_+/g, "_")
    .replace(/^_|_$/g, "");
}

/**
 * Convert a string to a Python string literal with proper escaping.
 *
 * IMPORTANT: This function assumes the input string has already had
 * leading/trailing quotes stripped. It will NOT add quotes if the string
 * already contains quotes that would cause syntax errors.
 */
function pythonStringLiteral(str: string): string {
  // Handle empty string
  if (str === "") {
    return '""';
  }

  // Use triple-quoted string for multi-line strings
  if (str.includes("\n")) {
    return '"""' + str.replace(/"""/g, '\\"\\"\\"') + '"""';
  }

  // For strings containing quotes, prefer single quotes if possible
  if (str.includes('"') && !str.includes("'")) {
    return "'" + str.replace(/\\/g, "\\\\").replace(/'/g, "\\'") + "'";
  }

  // For strings containing single quotes, use double quotes
  if (str.includes("'") && !str.includes('"')) {
    return '"' + str.replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"';
  }

  // For strings with both quote types, use triple quotes
  if (str.includes('"') || str.includes("'")) {
    return '"""' + str.replace(/"""/g, '\\"\\"\\"') + '"""';
  }

  // Simple string with no quotes - use double quotes
  return '"' + str.replace(/\\/g, "\\\\") + '"';
}

/**
 * Generate pytest conftest.py for custom configuration.
 */
export function generatePytestConftest(): string {
  return `#!/usr/bin/env python3
"""
pytest configuration for testgen-generated tests
"""
import pytest

def pytest_configure(config):
    """Configure pytest."""
    config.addinivalue_line(
        "markers", "testgen: mark test as generated by testgen"
    )
`;
}

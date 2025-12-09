/**
 * Unit tests for TestGen edge case extraction
 */

import { describe, test, expect } from "bun:test";
import {
  extractTaskEdgeCases,
  formatEdgeCasesForCategory,
  type TaskEdgeCases,
} from "./test-generator-iterative.js";

describe("extractTaskEdgeCases", () => {
  test("extracts IPv4 edge cases when description mentions IP", () => {
    const description =
      "Write a regex to match lines containing an IPv4 address and a date";
    const edgeCases = extractTaskEdgeCases(description, "regex-log");

    expect(edgeCases.ipv4).toBeDefined();
    expect(edgeCases.ipv4!.invalidExamples.length).toBeGreaterThan(0);
    expect(edgeCases.ipv4!.invalidExamples.some((e) => e.includes("256"))).toBe(
      true
    );
  });

  test("extracts date edge cases when description mentions date", () => {
    const description =
      "Match the last date (YYYY-MM-DD format) on each line with a valid IP";
    const edgeCases = extractTaskEdgeCases(description, "regex-log");

    expect(edgeCases.date).toBeDefined();
    expect(edgeCases.date!.invalidExamples.length).toBeGreaterThan(0);
    expect(
      edgeCases.date!.invalidExamples.some((e) => e.includes("month 00"))
    ).toBe(true);
    expect(
      edgeCases.date!.invalidExamples.some((e) => e.includes("month 13"))
    ).toBe(true);
    expect(
      edgeCases.date!.invalidExamples.some((e) => e.includes("day 00"))
    ).toBe(true);
    expect(
      edgeCases.date!.invalidExamples.some((e) => e.includes("day 32"))
    ).toBe(true);
  });

  test("extracts regex edge cases when description mentions regex", () => {
    const description = "Write a regex pattern to extract dates from log files";
    const edgeCases = extractTaskEdgeCases(description, "regex-log");

    expect(edgeCases.regex).toBeDefined();
    expect(edgeCases.regex!.captureGroup).toContain("capture group");
    expect(edgeCases.regex!.multipleMatches).toContain("multiple");
  });

  test("extracts log-specific edge cases from taskId", () => {
    const description = "Parse server access records";
    const edgeCases = extractTaskEdgeCases(description, "regex-log");

    expect(edgeCases.generic.length).toBeGreaterThan(0);
    expect(edgeCases.generic.some((g) => g.includes("multi-line"))).toBe(true);
  });

  test("extracts 'last match' requirement when description mentions 'last'", () => {
    const description = "Capture the LAST date appearing on lines with IPv4";
    const edgeCases = extractTaskEdgeCases(description, "test-task");

    expect(
      edgeCases.generic.some((g) => g.toLowerCase().includes("last"))
    ).toBe(true);
  });

  test("returns minimal edge cases for generic task", () => {
    const description = "Convert CSV to JSON format";
    const edgeCases = extractTaskEdgeCases(description, "csv-convert");

    expect(edgeCases.ipv4).toBeUndefined();
    expect(edgeCases.date).toBeUndefined();
    expect(edgeCases.regex).toBeUndefined();
  });
});

describe("formatEdgeCasesForCategory", () => {
  const regexLogEdgeCases: TaskEdgeCases = {
    ipv4: {
      validRanges: "Each octet must be 0-255",
      invalidExamples: ["256.1.1.1", "1.2.3.4a"],
      boundaryRequirements: ["IP must have word boundaries"],
    },
    date: {
      format: "YYYY-MM-DD",
      validRanges: { month: "01-12", day: "01-31" },
      invalidExamples: ["2024-00-15", "2024-13-15"],
    },
    generic: ["Test multi-line entries"],
  };

  test("includes combined IP+date validation for boundary category", () => {
    const output = formatEdgeCasesForCategory(regexLogEdgeCases, "boundary");

    // Uses combined format when both IP and date are present
    expect(output).toContain("Combined IP + Date Boundary Cases");
    expect(output).toContain("MUST include both an IP and a date");

    // Invalid IPs with valid dates
    expect(output).toContain("256.1.1.1 2024-01-15");
    expect(output).toContain("1.2.3.4a 2024-01-15");

    // Valid IPs with invalid dates
    expect(output).toContain("192.168.1.1 2024-00-15");
    expect(output).toContain("192.168.1.1 2024-13-15");
  });

  test("includes false positive prevention for anti_cheat category", () => {
    const output = formatEdgeCasesForCategory(regexLogEdgeCases, "anti_cheat");

    expect(output).toContain("False Positive Prevention");
    expect(output).toContain("invalid IPs");
    expect(output).toContain("invalid dates");
  });

  test("includes generic cases for integration category", () => {
    const output = formatEdgeCasesForCategory(regexLogEdgeCases, "integration");

    expect(output).toContain("Task-Specific Integration Cases");
    expect(output).toContain("multi-line");
  });

  test("returns empty string for existence category (no special cases)", () => {
    const output = formatEdgeCasesForCategory(regexLogEdgeCases, "existence");

    // existence category has no special formatting
    expect(output).toBe("");
  });
});

describe("regex-log task edge cases", () => {
  // These tests validate that the regex-log task gets proper edge case guidance
  const REGEX_LOG_DESCRIPTION = `Write a regex pattern that matches lines containing a valid IPv4 address,
and captures the LAST date (YYYY-MM-DD format) appearing on that line.
The regex should be stored in /app/regex.txt.`;

  test("regex-log description extracts all relevant edge cases", () => {
    const edgeCases = extractTaskEdgeCases(REGEX_LOG_DESCRIPTION, "regex-log");

    // Should detect IPv4
    expect(edgeCases.ipv4).toBeDefined();
    expect(edgeCases.ipv4!.invalidExamples).toContain(
      "256.1.1.1 (first octet > 255)"
    );

    // Should detect date
    expect(edgeCases.date).toBeDefined();
    expect(edgeCases.date!.validRanges.month).toBe("01-12 (not 00, not 13+)");
    expect(edgeCases.date!.validRanges.day).toBe("01-31 (not 00, not 32+)");

    // Should detect regex
    expect(edgeCases.regex).toBeDefined();

    // Should detect "last" requirement
    expect(edgeCases.generic.some((g) => g.includes("LAST"))).toBe(true);

    // Should detect log-related edge cases
    expect(
      edgeCases.generic.some((g) => g.includes("multi-line") || g.includes("log"))
    ).toBe(true);
  });

  test("boundary category prompt includes critical validation tests with both IP and date", () => {
    const edgeCases = extractTaskEdgeCases(REGEX_LOG_DESCRIPTION, "regex-log");
    const boundaryPrompt = formatEdgeCasesForCategory(edgeCases, "boundary");

    // Critical: Tests must include BOTH IP and date
    expect(boundaryPrompt).toContain("Combined IP + Date");
    expect(boundaryPrompt).toContain("MUST include both an IP and a date");

    // Invalid IPs with valid dates
    expect(boundaryPrompt).toContain("256.1.1.1 2024-01-15");
    expect(boundaryPrompt).toContain("1.2.3.4a 2024-01-15");

    // Valid IPs with invalid dates
    expect(boundaryPrompt).toContain("192.168.1.1 2024-00-15");
    expect(boundaryPrompt).toContain("192.168.1.1 2024-13-15");
    expect(boundaryPrompt).toContain("192.168.1.1 2024-02-30");

    // Valid cases
    expect(boundaryPrompt).toContain("192.168.1.1 2024-01-15");
  });
});

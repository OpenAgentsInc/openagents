/**
 * Guardrails Schema Tests
 */

import { describe, test, expect } from "bun:test";
import {
  createResult,
  aggregateResults,
  matchesBlockedPattern,
  getRulesByCategory,
  getEnabledRules,
  DEFAULT_GUARDRAILS_CONFIG,
  BUILTIN_RULES,
  type GuardrailResult,
  type GuardrailRule,
} from "./schema.js";

describe("createResult", () => {
  test("creates passing result", () => {
    const result = createResult("test-rule", true, "Test passed");
    expect(result.ruleId).toBe("test-rule");
    expect(result.passed).toBe(true);
    expect(result.message).toBe("Test passed");
    expect(result.timestamp).toBeDefined();
  });

  test("creates failing result with options", () => {
    const result = createResult("test-rule", false, "Test failed", {
      severity: "error",
      action: "Fix it",
      context: { value: 123 },
    });
    expect(result.passed).toBe(false);
    expect(result.severity).toBe("error");
    expect(result.action).toBe("Fix it");
    expect(result.context?.value).toBe(123);
  });
});

describe("aggregateResults", () => {
  test("aggregates all passing", () => {
    const results: GuardrailResult[] = [
      createResult("rule-1", true, "OK"),
      createResult("rule-2", true, "OK"),
    ];
    const status = aggregateResults(results);
    expect(status.allPassed).toBe(true);
    expect(status.warnings).toBe(0);
    expect(status.errors).toBe(0);
    expect(status.shouldBlock).toBe(false);
  });

  test("counts warnings", () => {
    const results: GuardrailResult[] = [
      createResult("rule-1", true, "OK"),
      createResult("rule-2", false, "Warning", { severity: "warning" }),
      createResult("rule-3", false, "Warning", { severity: "warning" }),
    ];
    const status = aggregateResults(results);
    expect(status.allPassed).toBe(false);
    expect(status.warnings).toBe(2);
    expect(status.shouldBlock).toBe(false);
  });

  test("counts errors and blocks", () => {
    const results: GuardrailResult[] = [
      createResult("rule-1", true, "OK"),
      createResult("rule-2", false, "Error", { severity: "error" }),
    ];
    const status = aggregateResults(results);
    expect(status.errors).toBe(1);
    expect(status.shouldBlock).toBe(true);
  });

  test("counts critical and blocks", () => {
    const results: GuardrailResult[] = [
      createResult("rule-1", false, "Critical", { severity: "critical" }),
    ];
    const status = aggregateResults(results);
    expect(status.critical).toBe(1);
    expect(status.shouldBlock).toBe(true);
  });
});

describe("matchesBlockedPattern", () => {
  test("matches suffix patterns", () => {
    expect(matchesBlockedPattern("config.env", ["*.env"])).toBe(true);
    expect(matchesBlockedPattern("/path/to/file.env", ["*.env"])).toBe(true);
    expect(matchesBlockedPattern("config.json", ["*.env"])).toBe(false);
  });

  test("matches prefix patterns", () => {
    expect(matchesBlockedPattern("~/.ssh/id_rsa", ["~/.ssh/*"])).toBe(true);
    expect(matchesBlockedPattern("/home/user/.ssh/config", ["~/.ssh/*"])).toBe(false);
  });

  test("matches contains patterns", () => {
    expect(matchesBlockedPattern("my-credentials.json", ["*credentials*"])).toBe(true);
    expect(matchesBlockedPattern("/path/to/credentials/file", ["*credentials*"])).toBe(true);
    expect(matchesBlockedPattern("config.json", ["*credentials*"])).toBe(false);
  });

  test("matches exact patterns", () => {
    expect(matchesBlockedPattern("secrets.txt", ["secrets"])).toBe(true);
    expect(matchesBlockedPattern("/path/secrets/file", ["secrets"])).toBe(true);
    expect(matchesBlockedPattern("config.txt", ["secrets"])).toBe(false);
  });

  test("case insensitive matching", () => {
    expect(matchesBlockedPattern("CONFIG.ENV", ["*.env"])).toBe(true);
    expect(matchesBlockedPattern("SECRETS.txt", ["*secrets*"])).toBe(true);
  });
});

describe("getRulesByCategory", () => {
  test("filters by category", () => {
    const resourceRules = getRulesByCategory(BUILTIN_RULES, "resource");
    expect(resourceRules.length).toBeGreaterThan(0);
    expect(resourceRules.every(r => r.category === "resource")).toBe(true);
  });

  test("returns empty for unknown category", () => {
    const rules = getRulesByCategory(BUILTIN_RULES, "unknown" as any);
    expect(rules).toEqual([]);
  });
});

describe("getEnabledRules", () => {
  test("filters to enabled rules", () => {
    const rules: GuardrailRule[] = [
      { ...BUILTIN_RULES[0], enabled: true },
      { ...BUILTIN_RULES[1], enabled: false },
      { ...BUILTIN_RULES[2], enabled: true },
    ];
    const enabled = getEnabledRules(rules);
    expect(enabled.length).toBe(2);
    expect(enabled.every(r => r.enabled)).toBe(true);
  });
});

describe("DEFAULT_GUARDRAILS_CONFIG", () => {
  test("has expected limits", () => {
    expect(DEFAULT_GUARDRAILS_CONFIG.maxTokensPerTask).toBe(50000);
    expect(DEFAULT_GUARDRAILS_CONFIG.maxDurationPerTask).toBe(300000);
    expect(DEFAULT_GUARDRAILS_CONFIG.maxRetriesPerTask).toBe(3);
    expect(DEFAULT_GUARDRAILS_CONFIG.maxTokensPerRun).toBe(1000000);
    expect(DEFAULT_GUARDRAILS_CONFIG.maxDurationPerRun).toBe(3600000);
  });

  test("has quality thresholds", () => {
    expect(DEFAULT_GUARDRAILS_CONFIG.minSuccessRate).toBe(0.1);
    expect(DEFAULT_GUARDRAILS_CONFIG.maxConsecutiveFailures).toBe(10);
  });

  test("has blocked patterns", () => {
    expect(DEFAULT_GUARDRAILS_CONFIG.blockedPatterns).toContain("*.env");
    expect(DEFAULT_GUARDRAILS_CONFIG.blockedPatterns).toContain("*.pem");
    expect(DEFAULT_GUARDRAILS_CONFIG.blockedPatterns).toContain("*credentials*");
  });

  test("has strict mode disabled by default", () => {
    expect(DEFAULT_GUARDRAILS_CONFIG.strictMode).toBe(false);
  });
});

describe("BUILTIN_RULES", () => {
  test("has resource rules", () => {
    const resourceRules = BUILTIN_RULES.filter(r => r.category === "resource");
    expect(resourceRules.length).toBeGreaterThan(0);
  });

  test("has safety rules", () => {
    const safetyRules = BUILTIN_RULES.filter(r => r.category === "safety");
    expect(safetyRules.length).toBeGreaterThan(0);
  });

  test("has quality rules", () => {
    const qualityRules = BUILTIN_RULES.filter(r => r.category === "quality");
    expect(qualityRules.length).toBeGreaterThan(0);
  });

  test("all rules have required fields", () => {
    for (const rule of BUILTIN_RULES) {
      expect(rule.id).toBeDefined();
      expect(rule.name).toBeDefined();
      expect(rule.description).toBeDefined();
      expect(rule.category).toBeDefined();
      expect(rule.severity).toBeDefined();
      expect(typeof rule.enabled).toBe("boolean");
      expect(rule.checkFn).toBeDefined();
    }
  });
});

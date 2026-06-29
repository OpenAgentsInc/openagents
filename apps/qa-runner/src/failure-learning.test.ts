// QA failure learning (#6195) — report-side strategies + capture. Pure, no IO.
//
// Proves: a failed/REFUTED run produces a captured, public-safe failure pattern
// + a default `suggest_in_report` snippet; the mutating strategies are
// config-gated (default OFF -> downgraded; armed -> plan only, never executed);
// a clean pass produces NO capture (no fabricated failure, no fake green).

import { describe, expect, test } from "bun:test";

import {
  captureFailurePattern,
  captureLowEvalPattern,
  detectRunFailure,
  learnFromRun,
  resolveFailureLearningStrategy,
  suggestFromPattern,
} from "./failure-learning";
import { assertPublicSafeResult, type QaRunResult } from "./result";

const baseResult = (overrides: Partial<QaRunResult> = {}): QaRunResult => ({
  schemaVersion: "openagents.qa_runner.result.v1",
  status: "pass",
  target: { name: "openagents.com", baseUrl: "https://example.test" },
  brain: "scripted",
  backend: "local",
  startedAt: "2026-06-24T00:00:00.000Z",
  endedAt: "2026-06-24T00:00:01.000Z",
  durationMs: 1000,
  steps: [],
  artifacts: { screenshots: [] },
  ...overrides,
});

const refutedResult = (): QaRunResult =>
  baseResult({
    status: "fail",
    failure: 'assertion failed: "redirects away from /login"',
    steps: [
      { index: 0, kind: "navigate", label: "open /login", status: "ok" },
      {
        index: 1,
        kind: "assert",
        label: "redirects away from /login (intentionally wrong)",
        status: "failed",
      },
    ],
    verify: {
      verdict: "REFUTED",
      observed: true,
      findings: [
        {
          id: "claims-redirect",
          claim: "/login redirects away from /login (FALSE claim under test)",
          verdict: "REFUTED",
          evidenceSummary:
            'observed step "redirects away from /login (intentionally wrong)" = failed (contradicting evidence)',
        },
      ],
    },
  });

describe("detectRunFailure (read result only)", () => {
  test("a clean pass is NOT a learning trigger", () => {
    expect(detectRunFailure(baseResult())).toEqual({ isFailure: false });
  });

  test("status=fail is a run_failed trigger", () => {
    const r = baseResult({ status: "fail", failure: "boom" });
    expect(detectRunFailure(r)).toEqual({ isFailure: true, source: "run_failed" });
  });

  test("verify REFUTED is a verify_refuted trigger", () => {
    expect(detectRunFailure(refutedResult())).toEqual({
      isFailure: true,
      source: "verify_refuted",
    });
  });

  test("an INCONCLUSIVE verify is NOT a trigger (uncertainty is never rounded to a failure)", () => {
    const r = baseResult({
      verify: { verdict: "INCONCLUSIVE", observed: false, findings: [] },
    });
    expect(detectRunFailure(r)).toEqual({ isFailure: false });
  });
});

describe("captureFailurePattern", () => {
  test("a clean pass captures NOTHING (no fabricated failure)", () => {
    expect(captureFailurePattern(baseResult())).toBeUndefined();
  });

  test("a REFUTED run captures the contradicting commitment finding (public-safe)", () => {
    const pattern = captureFailurePattern(refutedResult());
    expect(pattern).toBeDefined();
    expect(pattern!.source).toBe("verify_refuted");
    expect(pattern!.verdict).toBe("REFUTED");
    expect(pattern!.findings).toHaveLength(1);
    expect(pattern!.findings[0]!.id).toBe("claims-redirect");
    expect(pattern!.patternRef).toContain("failure_pattern:qa_runner:openagents-com:");
    // public-safe by construction
    expect(() => assertPublicSafeResult(pattern)).not.toThrow();
  });

  test("a plain failed run with no commitments falls back to the failed steps", () => {
    const r = baseResult({
      status: "fail",
      failure: "the login form did not render",
      steps: [
        { index: 0, kind: "navigate", label: "open /login", status: "ok" },
        { index: 1, kind: "wait-for", label: "sign-in form renders", status: "failed" },
      ],
    });
    const pattern = captureFailurePattern(r);
    expect(pattern!.source).toBe("run_failed");
    expect(pattern!.findings).toHaveLength(1);
    expect(pattern!.findings[0]!.id).toBe("step:1");
    expect(pattern!.findings[0]!.claim).toBe("sign-in form renders");
  });

  test("the same result captures a stable patternRef (deterministic)", () => {
    const a = captureFailurePattern(refutedResult())!;
    const b = captureFailurePattern(refutedResult())!;
    expect(a.patternRef).toBe(b.patternRef);
  });
});

describe("captureLowEvalPattern", () => {
  test("a variant below the floor is captured", () => {
    const pattern = captureLowEvalPattern({
      evalId: "login-mcp-compare",
      variantId: "candidate",
      passRate: 0.4,
      passRateFloor: 0.8,
      target: { name: "ci", baseUrl: "https://example.test" },
    });
    expect(pattern!.source).toBe("low_eval_score");
    expect(pattern!.findings[0]!.evidenceSummary).toContain("0.40 < floor 0.80");
  });

  test("a variant that meets the floor captures NOTHING", () => {
    expect(
      captureLowEvalPattern({
        evalId: "e",
        variantId: "v",
        passRate: 1,
        passRateFloor: 0.8,
        target: { name: "ci", baseUrl: "https://example.test" },
      }),
    ).toBeUndefined();
  });
});

describe("resolveFailureLearningStrategy (config-gated, default suggest-only)", () => {
  test("default is suggest_in_report", () => {
    expect(resolveFailureLearningStrategy()).toEqual({ strategy: "suggest_in_report" });
  });

  test("auto_commit without armMutations is downgraded to suggest_in_report", () => {
    const r = resolveFailureLearningStrategy({ strategy: "auto_commit" });
    expect(r.strategy).toBe("suggest_in_report");
    expect("downgradedFrom" in r && r.downgradedFrom).toBe("auto_commit");
  });

  test("open_pr without armMutations is downgraded to suggest_in_report", () => {
    const r = resolveFailureLearningStrategy({ strategy: "open_pr" });
    expect(r.strategy).toBe("suggest_in_report");
    expect("downgradedFrom" in r && r.downgradedFrom).toBe("open_pr");
  });

  test("auto_commit WITH armMutations resolves to a plan-only auto_commit", () => {
    const r = resolveFailureLearningStrategy({ strategy: "auto_commit", armMutations: true });
    expect(r).toEqual({ strategy: "auto_commit", planOnly: true });
  });

  test("open_pr WITH armMutations resolves to a plan-only open_pr", () => {
    const r = resolveFailureLearningStrategy({ strategy: "open_pr", armMutations: true });
    expect(r).toEqual({ strategy: "open_pr", planOnly: true });
  });
});

describe("suggestFromPattern", () => {
  test("default produces a suggest-only snippet (no mutation plan)", () => {
    const pattern = captureFailurePattern(refutedResult())!;
    const s = suggestFromPattern(pattern);
    expect(s.resolved.strategy).toBe("suggest_in_report");
    expect(s.mutationPlan).toBeUndefined();
    expect(s.snippet).toContain("Captured failure pattern");
    expect(s.snippet).toContain("claims-redirect");
  });

  test("armed auto_commit produces a PLAN that is never executed", () => {
    const pattern = captureFailurePattern(refutedResult())!;
    const s = suggestFromPattern(pattern, { strategy: "auto_commit", armMutations: true });
    expect(s.mutationPlan!.kind).toBe("auto_commit");
    expect(s.mutationPlan!.executed).toBe(false);
  });

  test("armed open_pr produces a draft-PR PLAN that is never executed", () => {
    const pattern = captureFailurePattern(refutedResult())!;
    const s = suggestFromPattern(pattern, { strategy: "open_pr", armMutations: true });
    expect(s.mutationPlan!.kind).toBe("open_pr");
    expect(s.mutationPlan!.executed).toBe(false);
  });
});

describe("learnFromRun", () => {
  test("a clean pass learns nothing", () => {
    expect(learnFromRun(baseResult())).toBeUndefined();
  });

  test("a REFUTED run yields a pattern + a suggestion", () => {
    const learned = learnFromRun(refutedResult());
    expect(learned).toBeDefined();
    expect(learned!.pattern.source).toBe("verify_refuted");
    expect(learned!.suggestion.patternRef).toBe(learned!.pattern.patternRef);
  });
});

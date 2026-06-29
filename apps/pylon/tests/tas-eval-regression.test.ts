import { describe, expect, test } from "bun:test"

import {
  detectRegression,
  summarizeBucket,
  type EvalBucketResult,
} from "../src/tas/eval-regression"

describe("tas eval regression", () => {
  test("summarizes bucket math", () => {
    const results: EvalBucketResult[] = [
      { caseRef: "eval.fixture.checkout", passed: true },
      { caseRef: "eval.fixture.redaction", passed: false },
      { caseRef: "eval.fixture.budget", passed: true },
      { caseRef: "eval.fixture.policy", passed: false },
    ]

    expect(summarizeBucket(results)).toEqual({
      total: 4,
      passed: 2,
      failed: 2,
      passRate: 0.5,
    })
  })

  test("detects a regression when a baseline pass now fails", () => {
    const baseline: EvalBucketResult[] = [
      { caseRef: "eval.fixture.checkout", passed: true },
      { caseRef: "eval.fixture.redaction", passed: true },
    ]
    const current: EvalBucketResult[] = [
      { caseRef: "eval.fixture.checkout", passed: false },
      { caseRef: "eval.fixture.redaction", passed: true },
    ]

    expect(detectRegression(baseline, current)).toEqual({
      regressed: ["eval.fixture.checkout"],
      newlyPassing: [],
      isRegression: true,
    })
  })

  test("tracks newly-passing cases", () => {
    const baseline: EvalBucketResult[] = [
      { caseRef: "eval.fixture.checkout", passed: true },
      { caseRef: "eval.fixture.redaction", passed: false },
    ]
    const current: EvalBucketResult[] = [
      { caseRef: "eval.fixture.checkout", passed: true },
      { caseRef: "eval.fixture.redaction", passed: true },
    ]

    expect(detectRegression(baseline, current)).toEqual({
      regressed: [],
      newlyPassing: ["eval.fixture.redaction"],
      isRegression: false,
    })
  })

  test("reports no regression for unchanged results", () => {
    const baseline: EvalBucketResult[] = [
      { caseRef: "eval.fixture.checkout", passed: true },
      { caseRef: "eval.fixture.redaction", passed: false },
    ]
    const current: EvalBucketResult[] = [
      { caseRef: "eval.fixture.checkout", passed: true },
      { caseRef: "eval.fixture.redaction", passed: false },
    ]

    expect(detectRegression(baseline, current)).toEqual({
      regressed: [],
      newlyPassing: [],
      isRegression: false,
    })
  })
})

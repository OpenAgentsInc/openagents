import { describe, expect, test } from "bun:test"

import {
  evaluateIssueCloseSafe,
  extractClosesIssueNumbers,
  ISSUE_CLOSE_SAFE_EVIDENCE,
  issueIsEpic,
  prBodyClosesIssue,
  type IssueCloseSafeInputs,
} from "./issue-close-safe.js"

const baseInputs: IssueCloseSafeInputs = {
  issueNumber: 6648,
  issueLabels: ["bug"],
  parentEpicNumber: null,
  prNumber: 7000,
  prBody: "Implements the gate.\n\nCloses #6648.",
}

describe("issue-close-safe: closing-keyword parsing", () => {
  test("extracts numbers across the standard closing keywords", () => {
    expect(
      extractClosesIssueNumbers("Closes #1, fixes #2, resolved #3, close #4"),
    ).toEqual([1, 2, 3, 4])
  })

  test("ignores plain issue references with no closing keyword", () => {
    expect(extractClosesIssueNumbers("See #99 and related #100")).toEqual([])
  })

  test("prBodyClosesIssue matches the exact issue number only", () => {
    expect(prBodyClosesIssue("Closes #6648", 6648)).toBe(true)
    expect(prBodyClosesIssue("Closes #6648", 664)).toBe(false)
  })
})

describe("issue-close-safe: happy path", () => {
  test("a non-epic issue with a matching Closes reaches SAFE_TO_CLOSE", () => {
    const result = evaluateIssueCloseSafe(baseInputs)
    expect(result.state).toBe("SAFE_TO_CLOSE")
    expect(result.canClose).toBe(true)
    expect(result.locked).toBe(false)
    expect(result.missingEvidence).toEqual([])
    expect(result.satisfiedEvidence).toEqual([
      ISSUE_CLOSE_SAFE_EVIDENCE.labels,
      ISSUE_CLOSE_SAFE_EVIDENCE.parentEpicCheck,
      ISSUE_CLOSE_SAFE_EVIDENCE.prBodyContainsCloses,
    ])
  })

  test("the last open sub-issue of an epic may close", () => {
    const result = evaluateIssueCloseSafe({
      ...baseInputs,
      parentEpicNumber: 6637,
      isLastOpenSubIssue: true,
    })
    expect(result.state).toBe("SAFE_TO_CLOSE")
    expect(result.canClose).toBe(true)
  })
})

describe("issue-close-safe: EPIC protection (the #6376 failure class)", () => {
  test("a sub-PR must NOT be able to close an epic", () => {
    // The issue under consideration is itself an EPIC, and the PR body would
    // (incorrectly) try to close it. Without the higher-risk-ceiling guard the
    // gate must refuse and must NOT reach SAFE_TO_CLOSE.
    const result = evaluateIssueCloseSafe({
      issueNumber: 6637,
      issueLabels: ["EPIC", "observability"],
      parentEpicNumber: null,
      prNumber: 7001,
      prBody: "Sub-PR work.\n\nCloses #6637.",
    })
    expect(result.isEpic).toBe(true)
    expect(result.canClose).toBe(false)
    expect(result.state).not.toBe("SAFE_TO_CLOSE")
    expect(result.locked).toBe(true)
    expect(result.lockedAt).toBe("EPIC_SAFE")
  })

  test("an epic can only close via the higher-risk-ceiling guard", () => {
    const inputs: IssueCloseSafeInputs = {
      issueNumber: 6637,
      issueLabels: ["epic"],
      parentEpicNumber: null,
      prNumber: 7002,
      prBody: "Closes #6637.",
      epicCloseAuthorized: true,
    }
    const result = evaluateIssueCloseSafe(inputs)
    expect(result.isEpic).toBe(true)
    expect(result.canClose).toBe(true)
    expect(result.state).toBe("SAFE_TO_CLOSE")
  })

  test("a sub-issue that is not the last open one locks at EPIC_SAFE", () => {
    const result = evaluateIssueCloseSafe({
      ...baseInputs,
      parentEpicNumber: 6637,
      isLastOpenSubIssue: false,
    })
    expect(result.canClose).toBe(false)
    expect(result.locked).toBe(true)
    expect(result.lockedAt).toBe("EPIC_SAFE")
    expect(result.state).toBe("LABELS_READ")
  })

  test("a sub-issue with unknown sibling state cannot advance", () => {
    const result = evaluateIssueCloseSafe({
      ...baseInputs,
      parentEpicNumber: 6637,
      // isLastOpenSubIssue intentionally omitted (evidence missing).
    })
    expect(result.canClose).toBe(false)
    expect(result.lockedAt).toBe("EPIC_SAFE")
  })
})

describe("issue-close-safe: closes-verification gate", () => {
  test("a missing Closes keyword locks at CLOSE_VERIFIED", () => {
    const result = evaluateIssueCloseSafe({
      ...baseInputs,
      prBody: "No closing keyword here, just prose referencing #6648.",
    })
    expect(result.canClose).toBe(false)
    expect(result.locked).toBe(true)
    expect(result.lockedAt).toBe("CLOSE_VERIFIED")
    expect(result.state).toBe("EPIC_SAFE")
    expect(result.missingEvidence).toContain(
      ISSUE_CLOSE_SAFE_EVIDENCE.prBodyContainsCloses,
    )
  })

  test("a Closes keyword for a different issue does not satisfy the gate", () => {
    const result = evaluateIssueCloseSafe({
      ...baseInputs,
      prBody: "Closes #9999.",
    })
    expect(result.canClose).toBe(false)
    expect(result.lockedAt).toBe("CLOSE_VERIFIED")
  })
})

describe("issue-close-safe: epic inference", () => {
  test("issueIsEpic infers from labels case-insensitively", () => {
    expect(
      issueIsEpic({ ...baseInputs, issueLabels: ["Epic"] }),
    ).toBe(true)
    expect(issueIsEpic({ ...baseInputs, issueLabels: ["bug"] })).toBe(false)
  })

  test("explicit isEpic flag overrides label inference", () => {
    expect(
      issueIsEpic({ ...baseInputs, issueLabels: ["epic"], isEpic: false }),
    ).toBe(false)
  })
})

import { describe, expect, test } from "bun:test"

import {
  authorizeIssueClosingBody,
  closingBodyCandidate,
  downgradeClosingKeywords,
} from "./codex-pr-publisher.js"

describe("codex PR publisher issue-close gate", () => {
  test("downgrades generated closing keywords before the issue-close gate", () => {
    expect(downgradeClosingKeywords("Fixes #7927\n\nBody", 7927)).toBe(
      "Addresses #7927\n\nBody",
    )
  })

  test("builds the closing body candidate only for the authorized edit path", () => {
    expect(closingBodyCandidate("Addresses #7927.\n\nBody", 7927)).toContain(
      "Closes #7927",
    )
  })

  test("gate-denied issue close leaves the closing body unconstructible", () => {
    const denied = authorizeIssueClosingBody({
      body: "Addresses #7927.\n\nBody",
      issueLabels: ["epic"],
      issueNumber: 7927,
      prNumber: 9001,
    })

    expect(denied.ok).toBe(false)
    if (!denied.ok) {
      expect(denied.reason).toContain("EPIC")
    }
  })

  test("gate-ok issue close returns the exact evaluated issue and PR body", () => {
    const allowed = authorizeIssueClosingBody({
      body: "Addresses #7927.\n\nBody",
      issueLabels: ["bug"],
      issueNumber: 7927,
      prNumber: 9001,
    })

    expect(allowed.ok).toBe(true)
    if (allowed.ok) {
      expect(allowed.body).toContain("Closes #7927")
    }
  })
})

import { describe, expect, test } from "bun:test"

import { buildWorkRequestFromIssue } from "../src/coordinator/backlog-faucet"

describe("backlog faucet work request builder", () => {
  test("builds a NIP-LBR work request from a budgeted issue", () => {
    const result = buildWorkRequestFromIssue({
      issueNumber: 4781,
      title: "P5 backlog faucet",
      body: "Budgeted issues become work requests.",
      budgetSats: 1_500,
      labels: ["p5", "budgeted", "backlog"],
    })

    expect(result).toEqual({
      ok: true,
      request: {
        kind: "nip-lbr.work_request",
        issueRef: "github.issue.4781",
        title: "P5 backlog faucet",
        summary: "Budgeted issues become work requests.",
        budgetSats: 1_500,
        createdFromLabels: ["p5", "budgeted", "backlog"],
      },
    })
  })

  test("trims titles before emitting the request", () => {
    const result = buildWorkRequestFromIssue({
      issueNumber: 42,
      title: "  Ship faucet bridge  ",
      body: "Bridge budgeted backlog issues.",
      budgetSats: 100,
      labels: ["budgeted"],
    })

    expect(result).toMatchObject({
      ok: true,
      request: {
        title: "Ship faucet bridge",
      },
    })
  })

  test("uses the title as summary when the body is empty", () => {
    const result = buildWorkRequestFromIssue({
      issueNumber: 43,
      title: "Fill empty body summary",
      body: "   ",
      budgetSats: 100,
      labels: ["budgeted"],
    })

    expect(result).toMatchObject({
      ok: true,
      request: {
        summary: "Fill empty body summary",
      },
    })
  })

  test("copies labels into the emitted request", () => {
    const labels = ["budgeted", "coordination"]
    const result = buildWorkRequestFromIssue({
      issueNumber: 44,
      title: "Copy labels",
      body: "Keep provenance labels.",
      budgetSats: 100,
      labels,
    })

    labels.push("mutated")

    expect(result).toMatchObject({
      ok: true,
      request: {
        createdFromLabels: ["budgeted", "coordination"],
      },
    })
  })

  test("rejects issues without the budgeted label", () => {
    const result = buildWorkRequestFromIssue({
      issueNumber: 45,
      title: "Unfunded backlog item",
      body: "Should stay in backlog.",
      budgetSats: 100,
      labels: ["backlog"],
    })

    expect(result).toEqual({ ok: false, reason: "issue is not budgeted" })
  })

  test("rejects non-positive budgets", () => {
    expect(buildWorkRequestFromIssue({
      issueNumber: 46,
      title: "Zero budget",
      body: "No spend authorized.",
      budgetSats: 0,
      labels: ["budgeted"],
    })).toEqual({ ok: false, reason: "budgetSats must be positive" })

    expect(buildWorkRequestFromIssue({
      issueNumber: 47,
      title: "Negative budget",
      body: "Invalid spend.",
      budgetSats: -1,
      labels: ["budgeted"],
    })).toEqual({ ok: false, reason: "budgetSats must be positive" })
  })

  test("rejects empty titles", () => {
    const result = buildWorkRequestFromIssue({
      issueNumber: 48,
      title: "   ",
      body: "A title is required.",
      budgetSats: 100,
      labels: ["budgeted"],
    })

    expect(result).toEqual({ ok: false, reason: "title is required" })
  })
})

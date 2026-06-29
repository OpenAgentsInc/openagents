import { describe, expect, test } from "bun:test"

import { projectApprovals, type ApprovalRow } from "./approvals-view.js"

describe("approvals view projection", () => {
  test("projects pending approvals with approve deny answer verbs", () => {
    expect(projectApprovals([
      {
        requestId: "decision.req.001",
        actionRef: "action.deploy.001",
        state: "pending",
        resolvedVerb: null,
        expiresAtMs: 9_999_999_999_999,
      },
    ])).toEqual([
      {
        requestId: "decision.req.001",
        actionRef: "action.deploy.001",
        state: "pending",
        resolvedVerb: null,
        availableVerbs: ["approve", "deny", "answer"],
        expired: false,
      },
    ] satisfies ApprovalRow[])
  })

  test("projects resolved approvals without available verbs", () => {
    expect(projectApprovals([
      {
        requestId: "decision.req.002",
        actionRef: "action.deploy.002",
        state: "resolved",
        resolvedVerb: "approve",
        expiresAtMs: 9_999_999_999_999,
      },
    ])).toEqual([
      {
        requestId: "decision.req.002",
        actionRef: "action.deploy.002",
        state: "resolved",
        resolvedVerb: "approve",
        availableVerbs: [],
        expired: false,
      },
    ] satisfies ApprovalRow[])
  })

  test("projects expired approvals without available verbs", () => {
    expect(projectApprovals([
      {
        requestId: "decision.req.003",
        actionRef: "action.deploy.003",
        state: "expired",
        resolvedVerb: null,
        expiresAtMs: 0,
      },
    ])).toEqual([
      {
        requestId: "decision.req.003",
        actionRef: "action.deploy.003",
        state: "expired",
        resolvedVerb: null,
        availableVerbs: [],
        expired: true,
      },
    ] satisfies ApprovalRow[])
  })

  test("projects cancelled approvals as read-only", () => {
    expect(projectApprovals([
      {
        requestId: "decision.req.004",
        actionRef: "action.deploy.004",
        state: "cancelled",
        resolvedVerb: null,
      },
    ])).toEqual([
      {
        requestId: "decision.req.004",
        actionRef: "action.deploy.004",
        state: "cancelled",
        resolvedVerb: null,
        availableVerbs: [],
        expired: false,
      },
    ] satisfies ApprovalRow[])
  })

  test("skips non-object entries and keeps stable fallbacks", () => {
    expect(projectApprovals([
      "bad",
      null,
      {
        state: "pending",
      },
    ])).toEqual([
      {
        requestId: "",
        actionRef: "",
        state: "pending",
        resolvedVerb: null,
        availableVerbs: ["approve", "deny", "answer"],
        expired: false,
      },
    ] satisfies ApprovalRow[])
  })

  test("treats unknown states as expired and read-only", () => {
    expect(projectApprovals([
      {
        requestId: "decision.req.005",
        actionRef: "action.deploy.005",
        state: "paused",
        resolvedVerb: "deny",
      },
    ])).toEqual([
      {
        requestId: "decision.req.005",
        actionRef: "action.deploy.005",
        state: "expired",
        resolvedVerb: "deny",
        availableVerbs: [],
        expired: true,
      },
    ] satisfies ApprovalRow[])
  })

  test("returns an empty list for bad input", () => {
    expect(projectApprovals(undefined as unknown as unknown[])).toEqual([])
    expect(projectApprovals(null as unknown as unknown[])).toEqual([])
    expect(projectApprovals({ records: [] } as unknown as unknown[])).toEqual([])
  })
})

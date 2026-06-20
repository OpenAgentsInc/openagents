import { describe, expect, test } from "bun:test"

import { projectApprovalQueue } from "./approval-queue-view.js"

describe("approval queue view projection", () => {
  test("projects a pending approvals list", () => {
    expect(projectApprovalQueue([
      {
        ref: "approval.deploy.001",
        kind: "deploy",
        prompt: "Approve production deploy?",
        extra: "ignored",
      },
      {
        ref: "approval.input.002",
        kind: "answer",
        prompt: "Provide missing environment name.",
      },
    ])).toEqual({
      pending: [
        {
          ref: "approval.deploy.001",
          kind: "deploy",
          prompt: "Approve production deploy?",
        },
        {
          ref: "approval.input.002",
          kind: "answer",
          prompt: "Provide missing environment name.",
        },
      ],
      count: 2,
    })
  })

  test("projects a wrapped pending approvals list", () => {
    expect(projectApprovalQueue({
      pending: [
        {
          ref: "approval.pr.003",
          kind: "pull_request",
          prompt: "Open a PR draft?",
        },
      ],
    })).toEqual({
      pending: [
        {
          ref: "approval.pr.003",
          kind: "pull_request",
          prompt: "Open a PR draft?",
        },
      ],
      count: 1,
    })
  })

  test("drops malformed rows", () => {
    expect(projectApprovalQueue([
      null,
      "bad",
      {
        ref: "approval.missing-kind.004",
        prompt: "Missing kind.",
      },
      {
        ref: "approval.bad-prompt.005",
        kind: "answer",
        prompt: 42,
      },
      {
        ref: "approval.good.006",
        kind: "deny",
        prompt: "Deny requested action?",
      },
    ])).toEqual({
      pending: [
        {
          ref: "approval.good.006",
          kind: "deny",
          prompt: "Deny requested action?",
        },
      ],
      count: 1,
    })
  })

  test("returns an empty queue for bad input", () => {
    expect(projectApprovalQueue(undefined)).toEqual({ pending: [], count: 0 })
    expect(projectApprovalQueue(null)).toEqual({ pending: [], count: 0 })
    expect(projectApprovalQueue({ records: [] })).toEqual({ pending: [], count: 0 })
  })

  test("derives count from projected pending rows", () => {
    expect(projectApprovalQueue({
      pending: [
        {
          ref: "approval.valid.007",
          kind: "approve",
          prompt: "Approve this action?",
        },
        {
          ref: "approval.invalid.008",
          kind: "approve",
        },
      ],
      count: 99,
    })).toEqual({
      pending: [
        {
          ref: "approval.valid.007",
          kind: "approve",
          prompt: "Approve this action?",
        },
      ],
      count: 1,
    })
  })

  test("returns a read-only snapshot of source rows", () => {
    const source = [
      {
        ref: "approval.snapshot.009",
        kind: "answer",
        prompt: "Answer now?",
      },
    ]

    const projected = projectApprovalQueue(source)
    source[0]!.prompt = "Changed after projection."

    expect(projected).toEqual({
      pending: [
        {
          ref: "approval.snapshot.009",
          kind: "answer",
          prompt: "Answer now?",
        },
      ],
      count: 1,
    })
  })
})

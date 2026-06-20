import { describe, expect, test } from "bun:test"

import { buildApprovalResponse } from "./approval-answer.js"

describe("approval answer response builder", () => {
  test("builds approve responses without an answer payload", () => {
    expect(buildApprovalResponse({
      ref: "approval-1",
      decision: "approve",
    })).toEqual({
      ref: "approval-1",
      decision: "approve",
      payload: {},
      readOnlyViolation: false,
      ok: true,
    })
  })

  test("builds deny responses without an answer payload", () => {
    expect(buildApprovalResponse({
      ref: "approval-2",
      decision: "deny",
    })).toEqual({
      ref: "approval-2",
      decision: "deny",
      payload: {},
      readOnlyViolation: false,
      ok: true,
    })
  })

  test("ignores answers on approve decisions", () => {
    expect(buildApprovalResponse({
      ref: "approval-3",
      decision: "approve",
      answer: "approved with note",
    })).toEqual({
      ref: "approval-3",
      decision: "approve",
      payload: {},
      readOnlyViolation: false,
      ok: true,
    })
  })

  test("ignores answers on deny decisions", () => {
    expect(buildApprovalResponse({
      ref: "approval-4",
      decision: "deny",
      answer: "denied with note",
    })).toEqual({
      ref: "approval-4",
      decision: "deny",
      payload: {},
      readOnlyViolation: false,
      ok: true,
    })
  })

  test("builds answer responses with a non-empty answer", () => {
    expect(buildApprovalResponse({
      ref: "approval-5",
      decision: "answer",
      answer: "Use the staging database.",
    })).toEqual({
      ref: "approval-5",
      decision: "answer",
      payload: {
        answer: "Use the staging database.",
      },
      readOnlyViolation: false,
      ok: true,
    })
  })

  test("marks missing answers invalid for answer decisions", () => {
    expect(buildApprovalResponse({
      ref: "approval-6",
      decision: "answer",
    })).toEqual({
      ref: "approval-6",
      decision: "answer",
      payload: {},
      readOnlyViolation: false,
      ok: false,
    })
  })

  test("marks empty answers invalid for answer decisions", () => {
    expect(buildApprovalResponse({
      ref: "approval-7",
      decision: "answer",
      answer: "",
    })).toEqual({
      ref: "approval-7",
      decision: "answer",
      payload: {
        answer: "",
      },
      readOnlyViolation: false,
      ok: false,
    })
  })
})

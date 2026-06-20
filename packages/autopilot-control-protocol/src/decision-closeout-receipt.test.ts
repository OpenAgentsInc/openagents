import { describe, expect, test } from "bun:test"

import type { ActionOutcome } from "./action-receipt.js"
import {
  buildDecisionCloseoutReceipt,
  isTerminalDecisionOutcome,
  validateDecisionCloseoutReceipt,
  type DecisionClient,
  type TerminalDecisionOutcome,
} from "./decision-closeout-receipt.js"

const TERMINAL_OUTCOMES: ReadonlyArray<TerminalDecisionOutcome> = [
  "applied",
  "duplicate",
  "expired",
  "revoked",
  "stale",
  "unauthorized",
  "unsupported",
  "error",
]

const CLIENTS: ReadonlyArray<DecisionClient> = ["desktop", "web", "expo"]

describe("decision closeout receipt", () => {
  test("builds an applied approve receipt with the canonical line", () => {
    expect(buildDecisionCloseoutReceipt({
      requestId: "dec-1",
      actionRef: "continue",
      verb: "approve",
      outcome: "applied",
      client: "desktop",
      actor: "owner",
      decidedAt: "2026-06-20T12:00:00.000Z",
    })).toEqual({
      kind: "decision_closeout_receipt",
      requestId: "dec-1",
      actionRef: "continue",
      verb: "approve",
      outcome: "applied",
      client: "desktop",
      actor: "owner",
      decidedAt: "2026-06-20T12:00:00.000Z",
      hasAnswer: false,
      line: "Decision dec-1 (continue) approve closed out as applied on desktop by owner at 2026-06-20T12:00:00.000Z.",
    })
  })

  test("hasAnswer is true only for an answer verb with non-empty text", () => {
    const withAnswer = buildDecisionCloseoutReceipt({
      requestId: "dec-2",
      actionRef: "provide_context",
      verb: "answer",
      outcome: "applied",
      client: "web",
      actor: "autopilot",
      decidedAt: "2026-06-20T12:01:00.000Z",
      answer: "use the staging DB",
    })
    expect(withAnswer.hasAnswer).toBe(true)
    expect(withAnswer.line).toContain("answer with answer closed out")
  })

  test("answer verb with empty text does not set hasAnswer", () => {
    expect(buildDecisionCloseoutReceipt({
      requestId: "dec-3",
      actionRef: "provide_context",
      verb: "answer",
      outcome: "applied",
      client: "expo",
      actor: "owner",
      decidedAt: "2026-06-20T12:02:00.000Z",
      answer: "",
    }).hasAnswer).toBe(false)
  })

  test("approve/deny verbs never set hasAnswer even if answer leaks in", () => {
    expect(buildDecisionCloseoutReceipt({
      requestId: "dec-4",
      actionRef: "stop",
      verb: "deny",
      outcome: "applied",
      client: "web",
      actor: "owner",
      decidedAt: "2026-06-20T12:03:00.000Z",
      answer: "ignored",
    }).hasAnswer).toBe(false)
  })

  test("builds and validates a receipt for every terminal outcome", () => {
    for (const outcome of TERMINAL_OUTCOMES) {
      const receipt = buildDecisionCloseoutReceipt({
        requestId: `dec-${outcome}`,
        actionRef: "rerun_tests",
        verb: "approve",
        outcome,
        client: "desktop",
        actor: "owner",
        decidedAt: "2026-06-20T12:04:00.000Z",
      })
      expect(receipt.outcome).toBe(outcome)
      expect(validateDecisionCloseoutReceipt(receipt)).toBe(true)
    }
  })

  test("builds and validates a receipt for every client surface", () => {
    for (const client of CLIENTS) {
      const receipt = buildDecisionCloseoutReceipt({
        requestId: `dec-${client}`,
        actionRef: "retry",
        verb: "approve",
        outcome: "applied",
        client,
        actor: "owner",
        decidedAt: "2026-06-20T12:05:00.000Z",
      })
      expect(receipt.client).toBe(client)
      expect(validateDecisionCloseoutReceipt(receipt)).toBe(true)
    }
  })

  test("isTerminalDecisionOutcome accepts terminal and rejects transient outcomes", () => {
    for (const outcome of TERMINAL_OUTCOMES) {
      expect(isTerminalDecisionOutcome(outcome)).toBe(true)
    }
    const transient: ReadonlyArray<ActionOutcome> = ["offline", "overloaded"]
    for (const outcome of transient) {
      expect(isTerminalDecisionOutcome(outcome)).toBe(false)
    }
  })
})

describe("decision closeout receipt validation", () => {
  const base = buildDecisionCloseoutReceipt({
    requestId: "dec-valid",
    actionRef: "continue",
    verb: "answer",
    outcome: "applied",
    client: "expo",
    actor: "owner",
    decidedAt: "2026-06-20T12:06:00.000Z",
    answer: "go",
  })

  test("accepts an untouched receipt", () => {
    expect(validateDecisionCloseoutReceipt(base)).toBe(true)
  })

  test("rejects non-records", () => {
    expect(validateDecisionCloseoutReceipt(null)).toBe(false)
    expect(validateDecisionCloseoutReceipt("nope")).toBe(false)
    expect(validateDecisionCloseoutReceipt([base])).toBe(false)
  })

  test("rejects a wrong kind", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, kind: "approval_receipt" })).toBe(false)
  })

  test("rejects an unknown verb", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, verb: "maybe" })).toBe(false)
  })

  test("rejects a transient outcome as a closeout", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, outcome: "offline" })).toBe(false)
    expect(validateDecisionCloseoutReceipt({ ...base, outcome: "overloaded" })).toBe(false)
  })

  test("rejects an unknown client surface", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, client: "ios" })).toBe(false)
  })

  test("rejects hasAnswer:true on a non-answer verb", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, verb: "approve", hasAnswer: true })).toBe(false)
  })

  test("rejects a tampered actionRef (line no longer reconstructs)", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, actionRef: "stop" })).toBe(false)
  })

  test("rejects a tampered outcome (line no longer reconstructs)", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, outcome: "duplicate" })).toBe(false)
  })

  test("rejects a tampered actor (line no longer reconstructs)", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, actor: "attacker" })).toBe(false)
  })

  test("rejects a tampered decidedAt (line no longer reconstructs)", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, decidedAt: "2026-01-01T00:00:00.000Z" })).toBe(false)
  })

  test("rejects a directly rewritten line", () => {
    expect(validateDecisionCloseoutReceipt({ ...base, line: "totally legit" })).toBe(false)
  })

  test("rejects a missing line", () => {
    const { line: _line, ...withoutLine } = base
    expect(validateDecisionCloseoutReceipt(withoutLine)).toBe(false)
  })
})

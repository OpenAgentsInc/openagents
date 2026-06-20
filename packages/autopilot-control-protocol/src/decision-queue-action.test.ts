import { describe, expect, test } from "bun:test"

import {
  buildDecisionQueueCommand,
  DECISION_QUEUE_ACTION_SPECS,
  DECISION_QUEUE_ACTIONS,
  decisionQueueIdempotencyKey,
  isDecisionQueueAction,
  type DecisionQueueAction,
} from "./decision-queue-action.js"
import { buildDecisionResolve } from "./decision-resolve-command.js"

describe("decision-queue action enum (#5004)", () => {
  test("exposes exactly the eight promised actions", () => {
    expect([...DECISION_QUEUE_ACTIONS].sort()).toEqual(
      [
        "accept",
        "continue",
        "create_follow_up",
        "provide_context",
        "rerun_tests",
        "retry_with_another_account",
        "steer",
        "stop",
      ],
    )
  })

  test("every action lowers to a valid wire verb", () => {
    for (const action of DECISION_QUEUE_ACTIONS) {
      expect(["approve", "deny", "answer"]).toContain(DECISION_QUEUE_ACTION_SPECS[action].verb)
    }
  })

  test("requiresOwnerApproval is derived from authority", () => {
    for (const action of DECISION_QUEUE_ACTIONS) {
      const spec = DECISION_QUEUE_ACTION_SPECS[action]
      expect(spec.requiresOwnerApproval).toBe(spec.authority !== "none")
    }
  })

  test("only stop and provide_context are authority-free", () => {
    const free = DECISION_QUEUE_ACTIONS.filter(
      (a) => !DECISION_QUEUE_ACTION_SPECS[a].requiresOwnerApproval,
    ).sort()
    expect(free).toEqual(["provide_context", "stop"])
  })

  test("isDecisionQueueAction guards unknown strings", () => {
    expect(isDecisionQueueAction("continue")).toBe(true)
    expect(isDecisionQueueAction("delete_everything")).toBe(false)
  })
})

describe("decisionQueueIdempotencyKey", () => {
  test("is deterministic per request + action and trims the ref", () => {
    expect(decisionQueueIdempotencyKey("  req-1  ", "continue")).toBe("dq:req-1:continue")
    expect(decisionQueueIdempotencyKey("req-1", "continue")).toBe(
      decisionQueueIdempotencyKey("req-1", "continue"),
    )
  })

  test("differs across actions on the same request", () => {
    expect(decisionQueueIdempotencyKey("req-1", "continue")).not.toBe(
      decisionQueueIdempotencyKey("req-1", "stop"),
    )
  })
})

describe("buildDecisionQueueCommand", () => {
  test("builds a continue command once owner-approved", () => {
    expect(
      buildDecisionQueueCommand({ requestId: "req-1", action: "continue", ownerApproved: true }),
    ).toEqual({
      ok: true,
      errors: [],
      command: {
        type: "decision.resolve",
        ref: "req-1",
        choice: "approve",
        action: "continue",
        idempotencyKey: "dq:req-1:continue",
        authority: "continuation",
        requiresOwnerApproval: true,
      },
    })
  })

  test("stop needs no owner approval and lowers to deny", () => {
    const result = buildDecisionQueueCommand({ requestId: "req-2", action: "stop" })
    expect(result.ok).toBe(true)
    expect(result.command?.choice).toBe("deny")
    expect(result.command?.requiresOwnerApproval).toBe(false)
  })

  test("steer carries its guidance into the wire answer", () => {
    const result = buildDecisionQueueCommand({
      requestId: "req-3",
      action: "steer",
      payload: "  focus on the failing test  ",
      ownerApproved: true,
    })
    expect(result.ok).toBe(true)
    expect(result.command?.choice).toBe("answer")
    expect(result.command?.answer).toBe("focus on the failing test")
  })

  test("rejects an authority-bearing action without owner approval", () => {
    const result = buildDecisionQueueCommand({
      requestId: "req-4",
      action: "retry_with_another_account",
    })
    expect(result.ok).toBe(false)
    expect(result.command).toBeNull()
    expect(result.errors).toContain("owner approval is required for retry_with_another_account (account)")
  })

  test("rejects a required payload that is missing", () => {
    const result = buildDecisionQueueCommand({
      requestId: "req-5",
      action: "create_follow_up",
      ownerApproved: true,
    })
    expect(result.ok).toBe(false)
    expect(result.errors).toContain("title is required for create_follow_up")
  })

  test("rejects a blank requestId and an unknown action", () => {
    expect(buildDecisionQueueCommand({ requestId: "  ", action: "stop" }).errors).toContain(
      "requestId is required",
    )
    const unknown = buildDecisionQueueCommand({ requestId: "req-6", action: "frobnicate" })
    expect(unknown.ok).toBe(false)
    expect(unknown.command).toBeNull()
    expect(unknown.errors[0]).toContain("action must be one of")
  })

  test("retry_with_another_account is optional-payload and may carry an accountRef", () => {
    const withRef = buildDecisionQueueCommand({
      requestId: "req-7",
      action: "retry_with_another_account",
      payload: "acct-backup",
      ownerApproved: true,
    })
    expect(withRef.ok).toBe(true)
    expect(withRef.command?.answer).toBe("acct-backup")

    const withoutRef = buildDecisionQueueCommand({
      requestId: "req-7",
      action: "retry_with_another_account",
      ownerApproved: true,
    })
    expect(withoutRef.ok).toBe(true)
    expect(withoutRef.command?.answer).toBeUndefined()
  })

  test("the lowered wire command matches buildDecisionResolve for the same verb", () => {
    const queueCmd = buildDecisionQueueCommand({
      requestId: "req-8",
      action: "steer",
      payload: "narrow the scope",
      ownerApproved: true,
    })
    const wire = buildDecisionResolve({ ref: "req-8", choice: "answer", answer: "narrow the scope" })
    expect(wire.ok).toBe(true)
    expect(queueCmd.command?.ref).toBe(wire.command.ref)
    expect(queueCmd.command?.choice).toBe(wire.command.choice)
    expect(queueCmd.command?.answer).toBe(wire.command.answer)
  })

  test("all eight actions build (with approval + payload) — full coverage", () => {
    const payloadFor: Record<DecisionQueueAction, string | undefined> = {
      continue: undefined,
      steer: "x",
      provide_context: "x",
      rerun_tests: undefined,
      retry_with_another_account: undefined,
      stop: undefined,
      accept: undefined,
      create_follow_up: "x",
    }
    for (const action of DECISION_QUEUE_ACTIONS) {
      const result = buildDecisionQueueCommand({
        requestId: "req-all",
        action,
        ownerApproved: true,
        ...(payloadFor[action] === undefined ? {} : { payload: payloadFor[action] }),
      })
      expect(result.ok).toBe(true)
      expect(result.command?.idempotencyKey).toBe(`dq:req-all:${action}`)
    }
  })
})

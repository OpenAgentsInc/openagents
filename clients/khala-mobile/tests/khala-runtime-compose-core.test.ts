import { describe, expect, test } from "bun:test"
import type { RuntimeTurnEntity } from "@openagentsinc/khala-sync"

import {
  buildAppendUserMessageIntentArgs,
  buildChatAppendMessageArgs,
  buildContinueTurnIntentArgs,
  buildInterruptTurnIntentArgs,
  buildRetryTurnIntentArgs,
  buildStartTurnIntentArgs,
  chatMessageBodyRef,
  findActiveTurn,
  findRecoverableTurn,
  mostRecentTurnLane
} from "../src/sync/khala-runtime-compose-core"

const turn = (overrides: Partial<RuntimeTurnEntity>): RuntimeTurnEntity => ({
  createdAt: "2026-01-01T00:00:00Z",
  eventCount: 0,
  lane: "codex_app_server",
  latestIntentId: null,
  ownerUserId: "user1",
  settledAt: null,
  startedAt: null,
  status: "queued",
  threadId: "t1",
  turnId: "turn0001",
  updatedAt: "2026-01-01T00:00:00Z",
  ...overrides
})

describe("findActiveTurn", () => {
  test("returns undefined for an empty list", () => {
    expect(findActiveTurn([])).toBeUndefined()
  })

  test("returns undefined when every turn has settled", () => {
    const turns = [turn({ turnId: "turn0001", status: "completed" }), turn({ turnId: "turn0002", status: "failed" })]
    expect(findActiveTurn(turns)).toBeUndefined()
  })

  test("returns the running turn when only one is active", () => {
    const turns = [turn({ turnId: "turn0001", status: "completed" }), turn({ turnId: "turn0002", status: "running" })]
    expect(findActiveTurn(turns)?.turnId).toBe("turn0002")
  })

  test("picks the most recent active turn by turnId order, ignoring older settled ones", () => {
    const turns = [
      turn({ turnId: "turn0001", status: "running" }),
      turn({ turnId: "turn0002", status: "completed" }),
      turn({ turnId: "turn0003", status: "queued" })
    ]
    expect(findActiveTurn(turns)?.turnId).toBe("turn0003")
  })
})

describe("findRecoverableTurn", () => {
  test("returns undefined when no interrupted or failed turn exists", () => {
    expect(findRecoverableTurn([turn({ status: "completed" }), turn({ status: "closed", turnId: "turn0002" })])).toBeUndefined()
  })

  test("returns the latest interrupted or failed turn by turn id order", () => {
    const turns = [
      turn({ status: "interrupted", turnId: "turn0001" }),
      turn({ status: "completed", turnId: "turn0002" }),
      turn({ status: "failed", turnId: "turn0003" })
    ]
    expect(findRecoverableTurn(turns)?.turnId).toBe("turn0003")
  })
})

describe("mostRecentTurnLane", () => {
  test("returns undefined for a thread with no turns yet", () => {
    expect(mostRecentTurnLane([])).toBeUndefined()
  })

  test("returns the lane of the single turn", () => {
    expect(mostRecentTurnLane([turn({ lane: "claude_pylon", turnId: "turn0001" })])).toBe("claude_pylon")
  })

  test("picks the lane of the most recent turn by turnId order, regardless of status", () => {
    const turns = [
      turn({ lane: "codex_app_server", status: "completed", turnId: "turn0001" }),
      turn({ lane: "claude_pylon", status: "completed", turnId: "turn0002" })
    ]
    expect(mostRecentTurnLane(turns)).toBe("claude_pylon")
  })
})

describe("chatMessageBodyRef", () => {
  test("formats a safe ref pointing at the chat_message entity", () => {
    expect(chatMessageBodyRef("msg123")).toBe("chat_message.msg123")
  })
})

describe("buildChatAppendMessageArgs", () => {
  test("carries the raw text through untouched", () => {
    expect(buildChatAppendMessageArgs({ body: "hello", messageId: "m1", threadId: "t1" })).toEqual({
      body: "hello",
      messageId: "m1",
      threadId: "t1"
    })
  })
})

describe("buildStartTurnIntentArgs", () => {
  test("builds a turn.start control intent referencing the chat message body", () => {
    const args = buildStartTurnIntentArgs({
      bodyRef: chatMessageBodyRef("m1"),
      nowIso: "2026-01-01T00:00:00Z",
      target: { lane: "codex_app_server" },
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.kind).toBe("turn.start")
    expect(args.bodyRef).toBe("chat_message.m1")
    expect(args.turnId).toBe("turn0001")
    expect(args.origin).toEqual({ lane: "khala_sync_mobile_control", surface: "mobile" })
    expect(args.target).toEqual({ lane: "codex_app_server" })
    expect(args.visibility).toBe("private")
    expect(args).not.toHaveProperty("body")
  })

  test("threads a claude_pylon target through instead of the old hardcoded Codex-only value (#8405)", () => {
    const args = buildStartTurnIntentArgs({
      bodyRef: chatMessageBodyRef("m1"),
      nowIso: "2026-01-01T00:00:00Z",
      target: { lane: "claude_pylon" },
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.target).toEqual({ lane: "claude_pylon" })
  })

  test("threads an account-specific execution target through for CX-4 receipts", () => {
    const args = buildStartTurnIntentArgs({
      bodyRef: chatMessageBodyRef("m1"),
      nowIso: "2026-01-01T00:00:00Z",
      target: { executionTargetId: "codex:owner-account-ref-hash", lane: "codex_app_server" },
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.target).toEqual({
      executionTargetId: "codex:owner-account-ref-hash",
      lane: "codex_app_server"
    })
  })
})

describe("buildAppendUserMessageIntentArgs", () => {
  test("builds a message.append control intent tied to the running turn", () => {
    const args = buildAppendUserMessageIntentArgs({
      bodyRef: chatMessageBodyRef("m2"),
      messageId: "m2",
      nowIso: "2026-01-01T00:01:00Z",
      target: { lane: "codex_app_server" },
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.kind).toBe("message.append")
    expect(args.turnId).toBe("turn0001")
    expect(args.messageId).toBe("m2")
    expect(args.bodyRef).toBe("chat_message.m2")
    expect(args.target).toEqual({ lane: "codex_app_server" })
  })

  test("carries a claude_pylon target through for a Claude-lane active turn", () => {
    const args = buildAppendUserMessageIntentArgs({
      bodyRef: chatMessageBodyRef("m2"),
      messageId: "m2",
      nowIso: "2026-01-01T00:01:00Z",
      target: { lane: "claude_pylon" },
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.target).toEqual({ lane: "claude_pylon" })
  })
})

describe("buildInterruptTurnIntentArgs", () => {
  test("builds a turn.interrupt control intent with no message/body ref", () => {
    const args = buildInterruptTurnIntentArgs({
      nonce: "abc123",
      nowIso: "2026-01-01T00:02:00Z",
      target: { lane: "codex_app_server" },
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.kind).toBe("turn.interrupt")
    expect(args.turnId).toBe("turn0001")
    expect(args.messageId).toBeUndefined()
    expect(args.bodyRef).toBeUndefined()
    expect(args.target).toEqual({ lane: "codex_app_server" })
  })

  test("two interrupt taps with different nonces produce distinct idempotency keys", () => {
    const a = buildInterruptTurnIntentArgs({
      nonce: "n1",
      nowIso: "t",
      target: { lane: "codex_app_server" },
      threadId: "t1",
      turnId: "turn0001"
    })
    const b = buildInterruptTurnIntentArgs({
      nonce: "n2",
      nowIso: "t",
      target: { lane: "codex_app_server" },
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(a.idempotencyKey).not.toBe(b.idempotencyKey)
  })
})

describe("buildContinueTurnIntentArgs", () => {
  test("builds a turn.continue control intent for the same turn and lane", () => {
    const args = buildContinueTurnIntentArgs({
      nonce: "resume1",
      nowIso: "2026-01-01T00:03:00Z",
      target: { lane: "codex_app_server" },
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.kind).toBe("turn.continue")
    expect(args.turnId).toBe("turn0001")
    expect(args.target).toEqual({ lane: "codex_app_server" })
    expect(args.bodyRef).toBeUndefined()
  })
})

describe("buildRetryTurnIntentArgs", () => {
  test("builds a turn.retry control intent for the same turn and lane", () => {
    const args = buildRetryTurnIntentArgs({
      nonce: "retry1",
      nowIso: "2026-01-01T00:04:00Z",
      target: { lane: "claude_pylon" },
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.kind).toBe("turn.retry")
    expect(args.turnId).toBe("turn0001")
    expect(args.target).toEqual({ lane: "claude_pylon" })
    expect(args.bodyRef).toBeUndefined()
  })
})

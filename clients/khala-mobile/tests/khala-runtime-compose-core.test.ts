import { describe, expect, test } from "bun:test"
import type { RuntimeTurnEntity } from "@openagentsinc/khala-sync"

import {
  buildAppendUserMessageIntentArgs,
  buildChatAppendMessageArgs,
  buildInterruptTurnIntentArgs,
  buildStartTurnIntentArgs,
  chatMessageBodyRef,
  findActiveTurn
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
})

describe("buildAppendUserMessageIntentArgs", () => {
  test("builds a message.append control intent tied to the running turn", () => {
    const args = buildAppendUserMessageIntentArgs({
      bodyRef: chatMessageBodyRef("m2"),
      messageId: "m2",
      nowIso: "2026-01-01T00:01:00Z",
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.kind).toBe("message.append")
    expect(args.turnId).toBe("turn0001")
    expect(args.messageId).toBe("m2")
    expect(args.bodyRef).toBe("chat_message.m2")
  })
})

describe("buildInterruptTurnIntentArgs", () => {
  test("builds a turn.interrupt control intent with no message/body ref", () => {
    const args = buildInterruptTurnIntentArgs({
      nonce: "abc123",
      nowIso: "2026-01-01T00:02:00Z",
      threadId: "t1",
      turnId: "turn0001"
    })
    expect(args.kind).toBe("turn.interrupt")
    expect(args.turnId).toBe("turn0001")
    expect(args.messageId).toBeUndefined()
    expect(args.bodyRef).toBeUndefined()
  })

  test("two interrupt taps with different nonces produce distinct idempotency keys", () => {
    const a = buildInterruptTurnIntentArgs({ nonce: "n1", nowIso: "t", threadId: "t1", turnId: "turn0001" })
    const b = buildInterruptTurnIntentArgs({ nonce: "n2", nowIso: "t", threadId: "t1", turnId: "turn0001" })
    expect(a.idempotencyKey).not.toBe(b.idempotencyKey)
  })
})

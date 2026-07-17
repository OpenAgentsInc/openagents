import { describe, expect, test } from "vite-plus/test"

import {
  classifyRuntimeControlReplay,
  decodeRuntimeControlIntent,
  decodeRuntimeControlOutcome,
  type RuntimeControlIntent,
} from "./thread-control.js"

// background_agents.fast_follow.owner_admitted_expansion.v1
const common = {
  schema: "openagents.runtime_control_intent.v2" as const,
  intentRef: "intent.desktop.thread_control.1",
  idempotencyKey: "idem.desktop.thread_control.1",
  threadRef: "thread.desktop.1",
  targetGeneration: { state: "known" as const, value: 7 },
  orderingKey: "order.thread.desktop.1",
  createdAt: "2026-07-16T20:00:00.000Z",
  expiresAt: "2026-07-16T20:05:00.000Z",
  origin: { surface: "desktop" as const, lane: "owner_local" as const },
}

describe("openagents.runtime_control_intent.v2", () => {
  test("keeps queue, steer, and interrupt distinct and ref-only", () => {
    const intents = [
      decodeRuntimeControlIntent({ ...common, kind: "turn.queue", messageRef: "message.desktop.1" }),
      decodeRuntimeControlIntent({ ...common, intentRef: "intent.desktop.thread_control.2", idempotencyKey: "idem.desktop.thread_control.2", kind: "turn.steer", turnRef: "turn.codex.7", messageRef: "message.desktop.2" }),
      decodeRuntimeControlIntent({ ...common, intentRef: "intent.desktop.thread_control.3", idempotencyKey: "idem.desktop.thread_control.3", kind: "turn.interrupt", turnRef: "turn.codex.7" }),
    ]

    expect(intents.map(intent => intent.kind)).toEqual([
      "turn.queue",
      "turn.steer",
      "turn.interrupt",
    ])
    expect(JSON.stringify(intents)).not.toContain('"message"')
    expect(JSON.stringify(intents)).not.toContain('"body"')
  })

  test("rejects missing kind-specific identity, raw body, and malformed refs", () => {
    expect(() => decodeRuntimeControlIntent({ ...common, kind: "turn.steer", messageRef: "message.desktop.1" })).toThrow()
    expect(() => decodeRuntimeControlIntent({ ...common, kind: "turn.queue" })).toThrow()
    expect(() => decodeRuntimeControlIntent({ ...common, kind: "turn.interrupt" })).toThrow()
    expect(() => decodeRuntimeControlIntent({ ...common, kind: "turn.queue", messageRef: "message.desktop.1", body: "raw prompt" })).toThrow()
    expect(() => decodeRuntimeControlIntent({ ...common, kind: "turn.queue", messageRef: "../escape" })).toThrow()
    expect(() => decodeRuntimeControlIntent({ ...common, kind: "turn.queue", messageRef: "message.desktop.1", expiresAt: common.createdAt })).toThrow()
  })

  test("classifies an exact lost-ACK retry and conflicting identity reuse", () => {
    const first = decodeRuntimeControlIntent({
      ...common,
      kind: "turn.steer",
      turnRef: "turn.codex.7",
      messageRef: "message.desktop.1",
    })
    const exactRetry = decodeRuntimeControlIntent({ ...first })
    const conflict = decodeRuntimeControlIntent({ ...first, messageRef: "message.desktop.changed" })
    const next = decodeRuntimeControlIntent({
      ...first,
      intentRef: "intent.desktop.thread_control.next",
      idempotencyKey: "idem.desktop.thread_control.next",
    })

    expect(classifyRuntimeControlReplay(first, exactRetry)).toBe("exact_retry")
    expect(classifyRuntimeControlReplay(first, conflict)).toBe("conflicting_reuse")
    expect(classifyRuntimeControlReplay(first, next)).toBe("new")
  })

  test("keeps admission, delivery, and terminal observation separate", () => {
    const outcome = decodeRuntimeControlOutcome({
      schema: "openagents.runtime_control_outcome.v1",
      outcomeRef: "outcome.desktop.thread_control.1",
      intentRef: common.intentRef,
      idempotencyKey: common.idempotencyKey,
      observedAt: "2026-07-16T20:00:01.000Z",
      admission: { status: "accepted", acceptedAt: "2026-07-16T20:00:00.500Z" },
      delivery: { status: "queued", queueRef: "queue.desktop.1" },
      terminal: { status: "pending" },
    })

    expect(outcome).toMatchObject({
      admission: { status: "accepted" },
      delivery: { status: "queued" },
      terminal: { status: "pending" },
    })
  })

  test("types the golden fixture as the exported intent union", () => {
    const intent: RuntimeControlIntent = decodeRuntimeControlIntent({
      ...common,
      targetGeneration: { state: "unknown", reason: "not_observed" },
      kind: "turn.queue",
      messageRef: "message.desktop.unknown_generation",
    })
    expect(intent.targetGeneration).toEqual({ state: "unknown", reason: "not_observed" })
  })
})

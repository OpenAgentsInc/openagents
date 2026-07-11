import { describe, expect, test } from "bun:test"
import { MutationId } from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import {
  buildAppendUserMessageIntent,
  buildInterruptTurnIntent,
  buildStartTurnIntent,
  createKhalaSyncRuntimeCommands,
  createRuntimeClientMutators,
} from "./runtime.js"
import type { KhalaSyncSession } from "./session.js"

const context = {
  nowIso: "2026-07-11T12:00:00.000Z",
  surface: "desktop" as const,
  target: { lane: "codex_app_server" as const },
}

describe("shared runtime command contract", () => {
  test("builds exact deterministic start, follow-up, and interrupt identities", () => {
    expect(buildStartTurnIntent({
      context,
      messageRef: "message.shared.1",
      threadRef: "thread.shared.1",
      turnRef: "run.shared.1",
    })).toMatchObject({
      bodyRef: "chat_message.message.shared.1",
      idempotencyKey: "idem.start.run.shared.1",
      intentId: "intent.start.run.shared.1",
      kind: "turn.start",
      threadId: "thread.shared.1",
      turnId: "run.shared.1",
    })
    expect(buildAppendUserMessageIntent({
      context: { ...context, surface: "mobile" },
      messageRef: "message.shared.2",
      threadRef: "thread.shared.1",
      turnRef: "run.shared.1",
    })).toMatchObject({
      bodyRef: "chat_message.message.shared.2",
      idempotencyKey: "idem.append.message.shared.2",
      intentId: "intent.append.message.shared.2",
      kind: "message.append",
      messageId: "message.shared.2",
      threadId: "thread.shared.1",
      turnId: "run.shared.1",
    })
    expect(buildInterruptTurnIntent({
      commandRef: "control.shared.1",
      context: { ...context, surface: "mobile" },
      threadRef: "thread.shared.1",
      turnRef: "run.shared.1",
    })).toMatchObject({
      idempotencyKey: "idem.interrupt.control.shared.1",
      intentId: "intent.interrupt.control.shared.1",
      kind: "turn.interrupt",
      threadId: "thread.shared.1",
      turnId: "run.shared.1",
    })
  })

  test("keeps runtime truth confirmed-only while queuing through the shared session", () => {
    const mutations: Array<{ name: string; intentId: string }> = []
    const session = {
      mutate: (mutator: { name: string }, intent: { intentId: string }) =>
        Effect.sync(() => {
          mutations.push({ name: String(mutator.name), intentId: intent.intentId })
          return MutationId.make(41)
        }),
    } as unknown as KhalaSyncSession
    const mutators = createRuntimeClientMutators()
    const commands = createKhalaSyncRuntimeCommands({ mutators, session })
    const start = buildStartTurnIntent({
      context,
      messageRef: "message.shared.1",
      threadRef: "thread.shared.1",
      turnRef: "run.shared.1",
    })

    expect(mutators.startTurn.apply(start, { get: () => undefined, list: () => [] })).toEqual([])
    expect(Effect.runSync(commands.startTurn(start))).toBe(MutationId.make(41))
    expect(mutations).toEqual([{
      name: "runtime.startTurn",
      intentId: "intent.start.run.shared.1",
    }])
  })
})

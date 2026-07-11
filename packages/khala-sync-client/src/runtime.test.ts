import { describe, expect, test } from "bun:test"
import {
  ChangelogEntry,
  EntityId,
  EntityType,
  MutationEnvelope,
  MutationId,
  MutatorName,
  RUNTIME_CONTROL_INTENT_ENTITY_TYPE,
  SyncVersion,
  canonicalJson,
  decodeRuntimeControlIntentEntity,
  encodeRuntimeControlIntentEntity,
  threadScope,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  buildAppendUserMessageIntent,
  buildInterruptTurnIntent,
  buildStartTurnIntent,
  createKhalaSyncRuntimeCommands,
  createRuntimeClientMutators,
} from "./runtime.js"
import type { KhalaSyncSession } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const context = {
  nowIso: "2026-07-11T12:00:00.000Z",
  surface: "desktop" as const,
  target: { lane: "codex_app_server" as const },
}

describe("shared runtime command contract", () => {
  test("builds exact deterministic start, follow-up, and interrupt identities", () => {
    expect(buildStartTurnIntent({
      context: { ...context, expiresAtIso: "2026-07-11T12:05:00.000Z" },
      messageRef: "message.shared.1",
      threadRef: "thread.shared.1",
      turnRef: "run.shared.1",
    })).toMatchObject({
      bodyRef: "chat_message.message.shared.1",
      idempotencyKey: "idem.start.run.shared.1",
      intentId: "intent.start.run.shared.1",
      expiresAt: "2026-07-11T12:05:00.000Z",
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

  test("reads pending identity before ACK and the same expired terminal result after local restart", async () => {
    const intent = buildStartTurnIntent({
      context: { ...context, expiresAtIso: "2026-07-11T12:05:00.000Z" },
      messageRef: "message.shared.expiry",
      threadRef: "thread.shared.expiry",
      turnRef: "run.shared.expiry",
    })
    const pendingMutation = new MutationEnvelope({
      argsJson: canonicalJson(intent),
      mutationId: MutationId.make(9),
      name: MutatorName.make("runtime.startTurn"),
    })
    const pendingSession = {
      pending: () => [pendingMutation],
    } as unknown as KhalaSyncSession
    const pending = createKhalaSyncRuntimeCommands({
      mutators: createRuntimeClientMutators(),
      session: pendingSession,
    })
    expect(Effect.runSync(pending.outcome({
      intentId: intent.intentId,
      threadRef: intent.threadId,
    }))).toEqual({
      commandRef: intent.intentId,
      mutationId: 9,
      runRef: intent.turnId ?? null,
      status: "pending",
      threadRef: intent.threadId,
      updatedAt: null,
      version: null,
    })

    const root = mkdtempSync(join(tmpdir(), "khala-runtime-command-"))
    const database = join(root, "runtime-command.sqlite")
    const scope = threadScope(intent.threadId)
    const entity = decodeRuntimeControlIntentEntity({
      createdAt: context.nowIso,
      intent,
      intentId: intent.intentId,
      kind: intent.kind,
      ownerUserId: "owner.shared.expiry",
      status: "expired",
      threadId: intent.threadId,
      turnId: intent.turnId ?? null,
      updatedAt: "2026-07-11T12:06:00.000Z",
    })
    const initial = openKhalaSyncStore(database)
    Effect.runSync(initial.applyConfirmed(scope, [new ChangelogEntry({
      committedAt: "2026-07-11T12:06:00.000Z",
      entityId: EntityId.make(intent.intentId),
      entityType: EntityType.make(RUNTIME_CONTROL_INTENT_ENTITY_TYPE),
      mutationRef: "mutation.runtime-command.expired",
      op: "upsert",
      postImageJson: canonicalJson(encodeRuntimeControlIntentEntity(entity)),
      scope,
      version: SyncVersion.make(4),
    })], SyncVersion.make(4)))
    Effect.runSync(initial.close())

    const restarted = openKhalaSyncStore(database)
    try {
      const terminal = createKhalaSyncRuntimeCommands({
        mutators: createRuntimeClientMutators(),
        session: pendingSession,
        store: restarted,
      })
      expect(await Effect.runPromise(terminal.outcome({
        intentId: intent.intentId,
        threadRef: intent.threadId,
      }))).toEqual({
        commandRef: intent.intentId,
        mutationId: null,
        runRef: intent.turnId ?? null,
        status: "expired",
        threadRef: intent.threadId,
        updatedAt: "2026-07-11T12:06:00.000Z",
        version: 4,
      })
    } finally {
      Effect.runSync(restarted.close())
      rmSync(root, { force: true, recursive: true })
    }
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

  test("carries bounded Desktop operation correlation into Sync causality refs", () => {
    const correlationRefs = [
      "operation.desktop.1",
      "session.desktop.1",
      "correlation.desktop.1",
    ]
    expect(buildStartTurnIntent({
      context,
      correlationRefs,
      messageRef: "message.shared.1",
      threadRef: "thread.shared.1",
      turnRef: "run.shared.1",
    }).causalityRefs).toEqual([...correlationRefs, "message.shared.1"])
    expect(buildInterruptTurnIntent({
      commandRef: "control.shared.1",
      context,
      correlationRefs,
      threadRef: "thread.shared.1",
      turnRef: "run.shared.1",
    }).causalityRefs).toEqual([...correlationRefs, "run.shared.1"])
  })
})

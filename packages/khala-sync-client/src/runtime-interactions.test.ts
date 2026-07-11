import { describe, expect, test } from "bun:test"

import {
  ChangelogEntry,
  EntityId,
  EntityType,
  MutationId,
  RUNTIME_INTERACTION_ENTITY_TYPE,
  SyncVersion,
  canonicalJson,
  decodeRuntimeInteractionEntity,
  encodeRuntimeInteractionEntity,
  threadScope,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import {
  buildRuntimeInteractionDecisionCommand,
  createKhalaSyncRuntimeInteractions,
  createRuntimeInteractionClientMutator,
} from "./runtime-interactions.js"
import type { KhalaSyncSession } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const threadRef = "thread.interaction.client.1"
const turnRef = "turn.interaction.client.1"

const interactionEntry = new ChangelogEntry({
  scope: threadScope(threadRef),
  version: SyncVersion.make(1),
  entityType: EntityType.make(RUNTIME_INTERACTION_ENTITY_TYPE),
  entityId: EntityId.make("interaction.client.1"),
  op: "upsert",
  postImageJson: canonicalJson(encodeRuntimeInteractionEntity(
    decodeRuntimeInteractionEntity({
      interactionRef: "interaction.client.1",
      threadId: threadRef,
      turnId: turnRef,
      ownerUserId: "owner.private.1",
      kind: "provider_question",
      status: "pending",
      interaction: {
        schema: "openagents.runtime_interaction.v1",
        interactionRef: "interaction.client.1",
        threadId: threadRef,
        turnId: turnRef,
        requestedSequence: 2,
        requestedAt: "2026-07-11T22:00:00.000Z",
        expiresAt: "2026-07-11T22:05:00.000Z",
        source: { lane: "claude_pylon", surface: "server" },
        visibility: "private",
        redactionClass: "private_ref",
        causalityRefs: [],
        payload: {
          kind: "provider_question",
          displayTitle: "Choose verification",
          questions: [{
            questionRef: "question.client.1",
            displayText: "Which verification should run?",
            multiSelect: false,
            options: [{ optionRef: "option.tests", label: "Tests" }],
          }],
        },
        lifecycle: { status: "pending" },
      },
      createdAt: "2026-07-11T22:00:00.000Z",
      updatedAt: "2026-07-11T22:00:00.000Z",
    }),
  )),
  mutationRef: "mutation.interaction.client.1",
  committedAt: "2026-07-11T22:00:00.000Z",
})

describe("Khala Sync runtime interaction client", () => {
  test("projects only confirmed exact-thread interactions while authority is live", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(
        threadScope(threadRef),
        [interactionEntry],
        SyncVersion.make(1),
      ))
      const liveSession = {
        state: () => ({ phase: "live", cursor: SyncVersion.make(1) }),
        pending: () => [],
      } as unknown as KhalaSyncSession
      const live = createKhalaSyncRuntimeInteractions({
        store,
        session: liveSession,
        mutator: createRuntimeInteractionClientMutator(),
      })
      expect(Effect.runSync(live.list(threadRef))).toEqual([
        expect.objectContaining({
          interactionRef: "interaction.client.1",
          kind: "provider_question",
          status: "pending",
          questions: [expect.objectContaining({ questionRef: "question.client.1" })],
          version: 1,
        }),
      ])
      expect(Effect.runSync(live.list("thread.interaction.other"))).toEqual([])

      const staleSession = {
        state: () => ({ phase: "must_refetch", reason: "retention_gap" }),
        pending: () => [],
      } as unknown as KhalaSyncSession
      const stale = createKhalaSyncRuntimeInteractions({
        store,
        session: staleSession,
        mutator: createRuntimeInteractionClientMutator(),
      })
      expect(Effect.runSync(stale.list(threadRef))).toEqual([])
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("queues a confirmed-only exact decision command through the shared session", () => {
    const mutations: Array<{ name: string; command: unknown }> = []
    const session = {
      state: () => ({ phase: "live", cursor: SyncVersion.make(1) }),
      pending: () => [],
      mutate: (mutator: { name: string }, command: unknown) => Effect.sync(() => {
        mutations.push({ name: String(mutator.name), command })
        return MutationId.make(7)
      }),
    } as unknown as KhalaSyncSession
    const mutator = createRuntimeInteractionClientMutator()
    const command = buildRuntimeInteractionDecisionCommand({
      interactionRef: "interaction.client.1",
      threadRef,
      turnRef,
      envelope: {
        decisionRef: "decision.client.1",
        idempotencyKey: "idem.decision.client.1",
        decidedAt: "2026-07-11T22:01:00.000Z",
        surface: "mobile",
        decision: {
          kind: "provider_question",
          answers: [{
            questionRef: "question.client.1",
            optionRefs: ["option.tests"],
          }],
        },
      },
    })
    expect(mutator.apply(command, { get: () => undefined, list: () => [] })).toEqual([])
    const store = openKhalaSyncStore(":memory:")
    try {
      const client = createKhalaSyncRuntimeInteractions({ store, session, mutator })
      expect(Effect.runSync(client.decide(command))).toBe(MutationId.make(7))
      expect(mutations).toEqual([{
        name: "runtime.decideInteraction",
        command,
      }])
    } finally {
      Effect.runSync(store.close())
    }
  })
})

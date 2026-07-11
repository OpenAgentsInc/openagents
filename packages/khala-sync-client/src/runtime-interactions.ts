import {
  decodeRuntimeInteractionDecisionEnvelope,
  decodeRuntimeInteractionEntity,
  projectRuntimeInteraction,
  RUNTIME_INTERACTION_ENTITY_TYPE,
  threadScope,
  MutatorName,
  type MutationId,
  type RuntimeInteractionDecisionEnvelope,
  type RuntimeInteractionProjection,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import type { ClientMutator, OverlayError } from "./overlay.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import type {
  ConfirmedEntity,
  KhalaSyncClientStoreError,
  KhalaSyncLocalStore,
} from "./store.js"

export const RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME =
  "runtime.decideInteraction"

export type RuntimeInteractionDecisionCommand = Readonly<{
  interactionRef: string
  threadId: string
  turnId: string
  envelope: RuntimeInteractionDecisionEnvelope
}>

export const buildRuntimeInteractionDecisionCommand = (input: Readonly<{
  interactionRef: string
  threadRef: string
  turnRef: string
  envelope: RuntimeInteractionDecisionEnvelope
}>): RuntimeInteractionDecisionCommand => ({
  interactionRef: input.interactionRef,
  threadId: input.threadRef,
  turnId: input.turnRef,
  envelope: decodeRuntimeInteractionDecisionEnvelope(input.envelope),
})

export const createRuntimeInteractionClientMutator = (): ClientMutator<
  RuntimeInteractionDecisionCommand
> => ({
  // Decisions are never optimistic interaction truth. Pending stays visible
  // until the exact confirmed post-image resolves, expires, or is revoked.
  apply: () => [],
  name: MutatorName.make(RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME),
})

export type ConfirmedRuntimeInteraction = RuntimeInteractionProjection &
  Readonly<{
    requestedSequence: number
    requestedAt: string
    version: number
  }>

export const confirmedRuntimeInteractions = (
  threadRef: string,
  rows: ReadonlyArray<ConfirmedEntity>,
): ReadonlyArray<ConfirmedRuntimeInteraction> => {
  const byRef = new Map<string, ConfirmedRuntimeInteraction>()
  for (const row of rows) {
    try {
      const entity = decodeRuntimeInteractionEntity(
        JSON.parse(row.postImageJson) as unknown,
      )
      if (entity.threadId !== threadRef) continue
      const projection = {
        ...projectRuntimeInteraction(entity.interaction),
        requestedSequence: entity.interaction.requestedSequence,
        requestedAt: entity.interaction.requestedAt,
        version: Number(row.version),
      }
      const previous = byRef.get(projection.interactionRef)
      if (previous === undefined || previous.version < projection.version) {
        byRef.set(projection.interactionRef, projection)
      }
    } catch {
      // Malformed/pre-contract rows are withheld until confirmed replacement.
    }
  }
  return [...byRef.values()].sort((left, right) =>
    left.interactionRef.localeCompare(right.interactionRef))
}

export type KhalaSyncRuntimeInteractions = Readonly<{
  status: (threadRef: string) => ScopeSyncState
  list: (threadRef: string) => Effect.Effect<
    ReadonlyArray<ConfirmedRuntimeInteraction>,
    KhalaSyncClientStoreError
  >
  decide: (
    command: RuntimeInteractionDecisionCommand,
  ) => Effect.Effect<MutationId, OverlayError>
}>

export const createKhalaSyncRuntimeInteractions = (input: Readonly<{
  store: KhalaSyncLocalStore
  session: KhalaSyncSession
  mutator: ClientMutator<RuntimeInteractionDecisionCommand>
}>): KhalaSyncRuntimeInteractions => ({
  status: threadRef => input.session.state(threadScope(threadRef)),
  list: threadRef => {
    if (input.session.state(threadScope(threadRef)).phase !== "live") {
      return Effect.succeed([])
    }
    return Effect.map(
      input.store.readEntities(
        threadScope(threadRef),
        RUNTIME_INTERACTION_ENTITY_TYPE,
      ),
      rows => confirmedRuntimeInteractions(threadRef, rows),
    )
  },
  decide: command => input.session.mutate(input.mutator, command),
})

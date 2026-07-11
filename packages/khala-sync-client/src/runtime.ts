import {
  AGENT_RUN_ENTITY_TYPE,
  decodeAgentRunEntity,
  decodeKhalaRuntimeControlIntent,
  decodeRuntimeControlIntentEntity,
  MutatorName,
  RUNTIME_CONTROL_INTENT_ENTITY_TYPE,
  threadScope,
  type MutationId,
  type KhalaRuntimeControlIntent,
  type KhalaRuntimeLane,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import type { ClientMutator } from "./overlay.js"
import type { OverlayError } from "./overlay.js"
import type { KhalaSyncSession } from "./session.js"
import type {
  KhalaSyncClientStoreError,
  KhalaSyncLocalStore,
} from "./store.js"

export const RUNTIME_START_TURN_MUTATOR_NAME = "runtime.startTurn"
export const RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME =
  "runtime.appendUserMessage"
export const RUNTIME_INTERRUPT_TURN_MUTATOR_NAME = "runtime.interruptTurn"
export const RUNTIME_CONTINUE_TURN_MUTATOR_NAME = "runtime.continueTurn"
export const RUNTIME_RETRY_TURN_MUTATOR_NAME = "runtime.retryTurn"
export const RUNTIME_CLOSE_TURN_MUTATOR_NAME = "runtime.closeTurn"

export type RuntimeCommandSurface = "desktop" | "mobile"
export type RuntimeCommandTarget = Readonly<{
  lane: KhalaRuntimeLane
  executionTargetId?: string
}>

export type RuntimeCommandContext = Readonly<{
  surface: RuntimeCommandSurface
  target: RuntimeCommandTarget
  nowIso: string
  expiresAtIso?: string
}>

const expiry = (context: RuntimeCommandContext): Readonly<{ expiresAt?: string }> =>
  context.expiresAtIso === undefined ? {} : { expiresAt: context.expiresAtIso }

export const chatMessageBodyRef = (messageRef: string): string =>
  `chat_message.${messageRef}`

const origin = (surface: RuntimeCommandSurface) => ({
  lane: "khala_sync_mobile_control" as const,
  surface,
})

export const buildStartTurnIntent = (input: Readonly<{
  context: RuntimeCommandContext
  threadRef: string
  turnRef: string
  messageRef: string
  correlationRefs?: ReadonlyArray<string>
}>): KhalaRuntimeControlIntent =>
  decodeKhalaRuntimeControlIntent({
    bodyRef: chatMessageBodyRef(input.messageRef),
    causalityRefs: [...(input.correlationRefs ?? []), input.messageRef],
    createdAt: input.context.nowIso,
    ...expiry(input.context),
    idempotencyKey: `idem.start.${input.turnRef}`,
    intentId: `intent.start.${input.turnRef}`,
    kind: "turn.start",
    origin: origin(input.context.surface),
    redactionClass: "private_ref",
    schema: "openagents.khala_runtime_control_intent.v1",
    target: input.context.target,
    threadId: input.threadRef,
    turnId: input.turnRef,
    visibility: "private",
  })

export const buildAppendUserMessageIntent = (input: Readonly<{
  context: RuntimeCommandContext
  threadRef: string
  turnRef: string
  messageRef: string
}>): KhalaRuntimeControlIntent =>
  decodeKhalaRuntimeControlIntent({
    bodyRef: chatMessageBodyRef(input.messageRef),
    causalityRefs: [input.turnRef, input.messageRef],
    createdAt: input.context.nowIso,
    ...expiry(input.context),
    idempotencyKey: `idem.append.${input.messageRef}`,
    intentId: `intent.append.${input.messageRef}`,
    kind: "message.append",
    messageId: input.messageRef,
    origin: origin(input.context.surface),
    redactionClass: "private_ref",
    schema: "openagents.khala_runtime_control_intent.v1",
    target: input.context.target,
    threadId: input.threadRef,
    turnId: input.turnRef,
    visibility: "private",
  })

export const buildInterruptTurnIntent = (input: Readonly<{
  commandRef: string
  context: RuntimeCommandContext
  threadRef: string
  turnRef: string
  correlationRefs?: ReadonlyArray<string>
}>): KhalaRuntimeControlIntent =>
  decodeKhalaRuntimeControlIntent({
    causalityRefs: [...(input.correlationRefs ?? []), input.turnRef],
    createdAt: input.context.nowIso,
    ...expiry(input.context),
    idempotencyKey: `idem.interrupt.${input.commandRef}`,
    intentId: `intent.interrupt.${input.commandRef}`,
    kind: "turn.interrupt",
    origin: origin(input.context.surface),
    redactionClass: "private_ref",
    schema: "openagents.khala_runtime_control_intent.v1",
    target: input.context.target,
    threadId: input.threadRef,
    turnId: input.turnRef,
    visibility: "private",
  })

type ExistingTurnCommandInput = Readonly<{
  commandRef: string
  context: RuntimeCommandContext
  threadRef: string
  turnRef: string
  correlationRefs?: ReadonlyArray<string>
}>

const buildExistingTurnIntent = (
  input: ExistingTurnCommandInput & Readonly<{
    kind: "turn.continue" | "turn.retry" | "turn.close"
    retryMessageRef?: string
  }>,
): KhalaRuntimeControlIntent => {
  const action = input.kind.slice("turn.".length)
  const causalityRefs = [
    ...(input.correlationRefs ?? []),
    input.turnRef,
    ...(input.retryMessageRef === undefined ? [] : [input.retryMessageRef]),
  ]
  return decodeKhalaRuntimeControlIntent({
    causalityRefs,
    createdAt: input.context.nowIso,
    ...expiry(input.context),
    idempotencyKey: `idem.${action}.${input.commandRef}`,
    intentId: `intent.${action}.${input.commandRef}`,
    kind: input.kind,
    origin: origin(input.context.surface),
    redactionClass: "private_ref",
    schema: "openagents.khala_runtime_control_intent.v1",
    target: input.context.target,
    threadId: input.threadRef,
    turnId: input.turnRef,
    visibility: "private",
    ...(input.retryMessageRef === undefined
      ? {}
      : { bodyRef: chatMessageBodyRef(input.retryMessageRef) }),
  })
}

export const buildContinueTurnIntent = (
  input: ExistingTurnCommandInput,
): KhalaRuntimeControlIntent => buildExistingTurnIntent({
  ...input,
  kind: "turn.continue",
})

export const buildRetryTurnIntent = (input: ExistingTurnCommandInput & Readonly<{
  retryMessageRef?: string
}>): KhalaRuntimeControlIntent => buildExistingTurnIntent({
  ...input,
  kind: "turn.retry",
})

export const buildCloseTurnIntent = (
  input: ExistingTurnCommandInput,
): KhalaRuntimeControlIntent => buildExistingTurnIntent({
  ...input,
  kind: "turn.close",
})

export type RuntimeClientMutators = Readonly<{
  startTurn: ClientMutator<KhalaRuntimeControlIntent>
  appendUserMessage: ClientMutator<KhalaRuntimeControlIntent>
  interruptTurn: ClientMutator<KhalaRuntimeControlIntent>
  continueTurn: ClientMutator<KhalaRuntimeControlIntent>
  retryTurn: ClientMutator<KhalaRuntimeControlIntent>
  closeTurn: ClientMutator<KhalaRuntimeControlIntent>
}>

const confirmedOnlyMutator = (
  name: string,
): ClientMutator<KhalaRuntimeControlIntent> => ({
  // Runtime admission and lifecycle never become optimistic truth. The
  // durable queue is visible through pendingMutationCount until the server's
  // canonical runtime/agent-run projection arrives.
  apply: () => [],
  name: MutatorName.make(name),
})

export const createRuntimeClientMutators = (): RuntimeClientMutators => ({
  appendUserMessage: confirmedOnlyMutator(
    RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
  ),
  closeTurn: confirmedOnlyMutator(RUNTIME_CLOSE_TURN_MUTATOR_NAME),
  continueTurn: confirmedOnlyMutator(RUNTIME_CONTINUE_TURN_MUTATOR_NAME),
  interruptTurn: confirmedOnlyMutator(RUNTIME_INTERRUPT_TURN_MUTATOR_NAME),
  retryTurn: confirmedOnlyMutator(RUNTIME_RETRY_TURN_MUTATOR_NAME),
  startTurn: confirmedOnlyMutator(RUNTIME_START_TURN_MUTATOR_NAME),
})

export type KhalaSyncRuntimeCommands = Readonly<{
  startTurn: (intent: KhalaRuntimeControlIntent) => Effect.Effect<MutationId, OverlayError>
  appendUserMessage: (intent: KhalaRuntimeControlIntent) => Effect.Effect<MutationId, OverlayError>
  interruptTurn: (intent: KhalaRuntimeControlIntent) => Effect.Effect<MutationId, OverlayError>
  continueTurn: (intent: KhalaRuntimeControlIntent) => Effect.Effect<MutationId, OverlayError>
  retryTurn: (intent: KhalaRuntimeControlIntent) => Effect.Effect<MutationId, OverlayError>
  closeTurn: (intent: KhalaRuntimeControlIntent) => Effect.Effect<MutationId, OverlayError>
  outcome: (input: Readonly<{
    intentId: string
    threadRef: string
  }>) => Effect.Effect<RuntimeCommandOutcome | null, KhalaSyncClientStoreError>
}>

export type RuntimeCommandOutcomeStatus =
  | "pending"
  | "accepted"
  | "settled"
  | "expired"
  | "failed"
  | "canceled"

export type RuntimeCommandOutcome = Readonly<{
  commandRef: string
  threadRef: string
  runRef: string | null
  status: RuntimeCommandOutcomeStatus
  mutationId: number | null
  version: number | null
  updatedAt: string | null
}>

const pendingRuntimeCommand = (
  session: KhalaSyncSession,
  intentId: string,
  threadRef: string,
): RuntimeCommandOutcome | null => {
  for (const mutation of session.pending()) {
    try {
      const intent = decodeKhalaRuntimeControlIntent(
        JSON.parse(mutation.argsJson) as unknown,
      )
      if (intent.intentId !== intentId || intent.threadId !== threadRef) continue
      return {
        commandRef: intent.intentId,
        mutationId: Number(mutation.mutationId),
        runRef: intent.turnId ?? null,
        status: "pending",
        threadRef,
        updatedAt: null,
        version: null,
      }
    } catch {
      // A pending mutation for another domain is not a runtime command.
    }
  }
  return null
}

const confirmedRuntimeCommand = (
  store: KhalaSyncLocalStore,
  intentId: string,
  threadRef: string,
): Effect.Effect<RuntimeCommandOutcome | null, KhalaSyncClientStoreError> => {
  const scope = threadScope(threadRef)
  return Effect.map(Effect.all([
    store.readEntities(scope, RUNTIME_CONTROL_INTENT_ENTITY_TYPE),
    store.readEntities(scope, AGENT_RUN_ENTITY_TYPE),
  ]), ([intentRows, runRows]) => {
    const row = intentRows.find(candidate => candidate.entityId === intentId)
    if (row === undefined) return null
    try {
      const entity = decodeRuntimeControlIntentEntity(
        JSON.parse(row.postImageJson) as unknown,
      )
      if (entity.intentId !== intentId || entity.threadId !== threadRef) return null
      let status: RuntimeCommandOutcomeStatus = entity.status
      if (entity.status === "accepted" && entity.turnId !== null) {
        for (const runRow of runRows) {
          try {
            const run = decodeAgentRunEntity(JSON.parse(runRow.postImageJson) as unknown)
            if (run.runId !== entity.turnId) continue
            status = run.status === "failed"
              ? "failed"
              : run.status === "canceled"
                ? "canceled"
                : run.status === "completed"
                  ? "settled"
                  : "accepted"
            break
          } catch {
            // Ignore malformed/pre-contract rows; confirmed replacement heals.
          }
        }
      }
      return {
        commandRef: entity.intentId,
        mutationId: null,
        runRef: entity.turnId,
        status,
        threadRef,
        updatedAt: entity.updatedAt,
        version: Number(row.version),
      }
    } catch {
      return null
    }
  })
}

export const createKhalaSyncRuntimeCommands = (input: Readonly<{
  mutators: RuntimeClientMutators
  session: KhalaSyncSession
  store?: KhalaSyncLocalStore
}>): KhalaSyncRuntimeCommands => ({
  appendUserMessage: intent =>
    input.session.mutate(input.mutators.appendUserMessage, intent),
  closeTurn: intent => input.session.mutate(input.mutators.closeTurn, intent),
  continueTurn: intent =>
    input.session.mutate(input.mutators.continueTurn, intent),
  interruptTurn: intent =>
    input.session.mutate(input.mutators.interruptTurn, intent),
  outcome: query => input.store === undefined
    ? Effect.succeed(pendingRuntimeCommand(input.session, query.intentId, query.threadRef))
    : Effect.map(
        confirmedRuntimeCommand(input.store, query.intentId, query.threadRef),
        confirmed => confirmed ??
          pendingRuntimeCommand(input.session, query.intentId, query.threadRef),
      ),
  retryTurn: intent => input.session.mutate(input.mutators.retryTurn, intent),
  startTurn: intent => input.session.mutate(input.mutators.startTurn, intent),
})

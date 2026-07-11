import {
  decodeKhalaRuntimeControlIntent,
  MutatorName,
  type MutationId,
  type KhalaRuntimeControlIntent,
  type KhalaRuntimeLane,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import type { ClientMutator } from "./overlay.js"
import type { OverlayError } from "./overlay.js"
import type { KhalaSyncSession } from "./session.js"

export const RUNTIME_START_TURN_MUTATOR_NAME = "runtime.startTurn"
export const RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME =
  "runtime.appendUserMessage"
export const RUNTIME_INTERRUPT_TURN_MUTATOR_NAME = "runtime.interruptTurn"

export type RuntimeCommandSurface = "desktop" | "mobile"
export type RuntimeCommandTarget = Readonly<{
  lane: KhalaRuntimeLane
  executionTargetId?: string
}>

export type RuntimeCommandContext = Readonly<{
  surface: RuntimeCommandSurface
  target: RuntimeCommandTarget
  nowIso: string
}>

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

export type RuntimeClientMutators = Readonly<{
  startTurn: ClientMutator<KhalaRuntimeControlIntent>
  appendUserMessage: ClientMutator<KhalaRuntimeControlIntent>
  interruptTurn: ClientMutator<KhalaRuntimeControlIntent>
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
  interruptTurn: confirmedOnlyMutator(RUNTIME_INTERRUPT_TURN_MUTATOR_NAME),
  startTurn: confirmedOnlyMutator(RUNTIME_START_TURN_MUTATOR_NAME),
})

export type KhalaSyncRuntimeCommands = Readonly<{
  startTurn: (intent: KhalaRuntimeControlIntent) => Effect.Effect<MutationId, OverlayError>
  appendUserMessage: (intent: KhalaRuntimeControlIntent) => Effect.Effect<MutationId, OverlayError>
  interruptTurn: (intent: KhalaRuntimeControlIntent) => Effect.Effect<MutationId, OverlayError>
}>

export const createKhalaSyncRuntimeCommands = (input: Readonly<{
  mutators: RuntimeClientMutators
  session: KhalaSyncSession
}>): KhalaSyncRuntimeCommands => ({
  appendUserMessage: intent =>
    input.session.mutate(input.mutators.appendUserMessage, intent),
  interruptTurn: intent =>
    input.session.mutate(input.mutators.interruptTurn, intent),
  startTurn: intent => input.session.mutate(input.mutators.startTurn, intent),
})

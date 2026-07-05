import type { RuntimeTurnEntity } from "@openagentsinc/khala-sync"

/**
 * Pure composer logic — building the exact mutation payloads a "send" or
 * "stop" tap needs. No native/RN imports, no ids generated internally (the
 * caller passes every id/timestamp so this stays deterministic and
 * unit-testable). Sequencing (mutationId, actually POSTing) lives in
 * use-khala-sync-push.ts.
 */

const ACTIVE_TURN_STATUSES: ReadonlySet<string> = new Set([
  "queued",
  "running",
  "waiting_for_input"
])

/** The most recent turn for a thread that hasn't settled yet, or undefined
 * if the thread is idle. Turn ids are UUIDv7 (time-ordered), so lexicographic
 * sort gives chronological order. */
export const findActiveTurn = (
  turns: ReadonlyArray<RuntimeTurnEntity>
): RuntimeTurnEntity | undefined => {
  const sorted = [...turns].sort((a, b) => a.turnId.localeCompare(b.turnId))
  for (let i = sorted.length - 1; i >= 0; i--) {
    const turn = sorted[i]
    if (turn !== undefined && ACTIVE_TURN_STATUSES.has(turn.status)) return turn
  }
  return undefined
}

export type ChatAppendMessageArgs = Readonly<{
  threadId: string
  messageId: string
  body: string
}>

export const buildChatAppendMessageArgs = (input: {
  threadId: string
  messageId: string
  body: string
}): ChatAppendMessageArgs => ({
  body: input.body,
  messageId: input.messageId,
  threadId: input.threadId
})

/** bodyRef convention: raw prompt text lives in the chat_message entity
 * (already-proven mutator), the runtime control intent only references it —
 * control intents forbid inline `body` by design. The runtime dispatch
 * consumer (#8388) resolves this ref by reading the chat_message entity with
 * this messageId from the thread's scope. */
export const chatMessageBodyRef = (messageId: string): string => `chat_message.${messageId}`

const RUNTIME_ORIGIN = { lane: "khala_sync_mobile_control", surface: "mobile" } as const
const RUNTIME_TARGET = { lane: "codex_app_server" } as const

export type RuntimeControlIntentArgs = Readonly<{
  schema: "openagents.khala_runtime_control_intent.v1"
  intentId: string
  kind: "turn.start" | "message.append" | "turn.interrupt"
  threadId: string
  turnId?: string
  messageId?: string
  createdAt: string
  origin: typeof RUNTIME_ORIGIN
  target: typeof RUNTIME_TARGET
  visibility: "private"
  redactionClass: "private_ref"
  idempotencyKey: string
  causalityRefs: ReadonlyArray<string>
  bodyRef?: string
}>

export const buildStartTurnIntentArgs = (input: {
  threadId: string
  turnId: string
  bodyRef: string
  nowIso: string
}): RuntimeControlIntentArgs => ({
  bodyRef: input.bodyRef,
  causalityRefs: [],
  createdAt: input.nowIso,
  idempotencyKey: `idem.start.${input.turnId}`,
  intentId: `intent.start.${input.turnId}`,
  kind: "turn.start",
  origin: RUNTIME_ORIGIN,
  redactionClass: "private_ref",
  schema: "openagents.khala_runtime_control_intent.v1",
  target: RUNTIME_TARGET,
  threadId: input.threadId,
  turnId: input.turnId,
  visibility: "private"
})

/** "Steer" — attach a follow-up message to the turn that's already running,
 * without interrupting it. */
export const buildAppendUserMessageIntentArgs = (input: {
  threadId: string
  turnId: string
  messageId: string
  bodyRef: string
  nowIso: string
}): RuntimeControlIntentArgs => ({
  bodyRef: input.bodyRef,
  causalityRefs: [],
  createdAt: input.nowIso,
  idempotencyKey: `idem.append.${input.messageId}`,
  intentId: `intent.append.${input.messageId}`,
  kind: "message.append",
  messageId: input.messageId,
  origin: RUNTIME_ORIGIN,
  redactionClass: "private_ref",
  schema: "openagents.khala_runtime_control_intent.v1",
  target: RUNTIME_TARGET,
  threadId: input.threadId,
  turnId: input.turnId,
  visibility: "private"
})

/** "Queue" — a brand-new turn that sits `queued` until the dispatch consumer
 * finishes whatever is currently running for this thread. Reuses
 * buildStartTurnIntentArgs; a distinct name documents the intent. */
export const buildQueueTurnIntentArgs = buildStartTurnIntentArgs

/** "Stop" — hard-abort the active turn. Partial output is kept (marked
 * interrupted), matching the runtime.interruptTurn mutator's settled=true
 * transition and the transcript reducer's turn-status handling. `nonce`
 * only needs to be unique per interrupt tap (no retry of the same turn
 * would otherwise collide on intentId/idempotencyKey). */
export const buildInterruptTurnIntentArgs = (input: {
  threadId: string
  turnId: string
  nowIso: string
  nonce: string
}): RuntimeControlIntentArgs => ({
  causalityRefs: [],
  createdAt: input.nowIso,
  idempotencyKey: `idem.interrupt.${input.turnId}.${input.nonce}`,
  intentId: `intent.interrupt.${input.turnId}.${input.nonce}`,
  kind: "turn.interrupt",
  origin: RUNTIME_ORIGIN,
  redactionClass: "private_ref",
  schema: "openagents.khala_runtime_control_intent.v1",
  target: RUNTIME_TARGET,
  threadId: input.threadId,
  turnId: input.turnId,
  visibility: "private"
})

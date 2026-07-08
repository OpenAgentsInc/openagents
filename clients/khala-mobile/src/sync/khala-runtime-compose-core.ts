import type { KhalaRuntimeLane, RuntimeTurnEntity } from "@openagentsinc/khala-sync"

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

/** The default lane a NEW turn's picker should preselect (#8405): whichever
 * lane the thread's most recent turn (settled or not) actually used, so a
 * user who always talks to Claude in a thread doesn't have to re-pick it
 * every time; `undefined` for a thread with no turns yet, letting the caller
 * fall back to whatever this app's overall default lane is. Turn ids are
 * UUIDv7 (time-ordered), so lexicographic sort gives chronological order —
 * same technique as `findActiveTurn`, just without the active-status filter. */
export const mostRecentTurnLane = (
  turns: ReadonlyArray<RuntimeTurnEntity>
): KhalaRuntimeLane | undefined => {
  if (turns.length === 0) return undefined
  const sorted = [...turns].sort((a, b) => a.turnId.localeCompare(b.turnId))
  return sorted[sorted.length - 1]?.lane
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

/** The server-hosted Khala lane is the app-wide default (#8467): a thread
 * with no turns yet, or a caller that hasn't threaded a `target` at all,
 * preselects `hosted_khala`. Unlike `codex_app_server` / `claude_pylon`
 * (which require the user's own local Pylon to consume the queued turn),
 * `hosted_khala` is drained SERVER-SIDE by the Cloud Run dispatch consumer
 * (`apps/openagents.com/workers/api/src/khala-hosted-runtime-dispatch.ts`),
 * so a plain mobile chat turn gets a real model answer with no local runtime.
 * Users who want their own Codex/Claude runtime still pick it explicitly. */
export const DEFAULT_RUNTIME_LANE: KhalaRuntimeLane = "hosted_khala"

export type RuntimeControlIntentTarget = Readonly<{
  executionTargetId?: string
  lane: KhalaRuntimeLane
}>

export type RuntimeControlIntentArgs = Readonly<{
  schema: "openagents.khala_runtime_control_intent.v1"
  intentId: string
  kind: "turn.start" | "message.append" | "turn.interrupt"
  threadId: string
  turnId?: string
  messageId?: string
  createdAt: string
  origin: typeof RUNTIME_ORIGIN
  target: RuntimeControlIntentTarget
  visibility: "private"
  redactionClass: "private_ref"
  idempotencyKey: string
  causalityRefs: ReadonlyArray<string>
  bodyRef?: string
}>

/** Starts a brand-new turn (#8405: `target` now names WHICH provider — Codex
 * vs Claude — should run it, from the composer's lane picker, instead of the
 * old hardcoded Codex-only constant). */
export const buildStartTurnIntentArgs = (input: {
  threadId: string
  turnId: string
  bodyRef: string
  nowIso: string
  target: RuntimeControlIntentTarget
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
  target: input.target,
  threadId: input.threadId,
  turnId: input.turnId,
  visibility: "private"
})

/** "Steer" — attach a follow-up message to the turn that's already running,
 * without interrupting it. `target` here is NOT a fresh user choice — a
 * running turn's provider is already fixed, so the caller must pass the
 * ACTIVE turn's own lane (`RuntimeTurnEntity.lane`), never the composer's
 * lane picker (that picker only applies to a brand-new turn; switching an
 * in-flight turn's provider is cross-agent delegation, #8407, out of scope
 * here). */
export const buildAppendUserMessageIntentArgs = (input: {
  threadId: string
  turnId: string
  messageId: string
  bodyRef: string
  nowIso: string
  target: RuntimeControlIntentTarget
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
  target: input.target,
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
 * would otherwise collide on intentId/idempotencyKey). `target` must be the
 * turn being interrupted's own lane, for the same reason as
 * `buildAppendUserMessageIntentArgs` above — you can't retarget a turn
 * that's already running on a specific provider. */
export const buildInterruptTurnIntentArgs = (input: {
  threadId: string
  turnId: string
  nowIso: string
  nonce: string
  target: RuntimeControlIntentTarget
}): RuntimeControlIntentArgs => ({
  causalityRefs: [],
  createdAt: input.nowIso,
  idempotencyKey: `idem.interrupt.${input.turnId}.${input.nonce}`,
  intentId: `intent.interrupt.${input.turnId}.${input.nonce}`,
  kind: "turn.interrupt",
  origin: RUNTIME_ORIGIN,
  redactionClass: "private_ref",
  schema: "openagents.khala_runtime_control_intent.v1",
  target: input.target,
  threadId: input.threadId,
  turnId: input.turnId,
  visibility: "private"
})

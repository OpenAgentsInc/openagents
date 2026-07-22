// Server-side hosted-Khala runtime dispatch consumer (#8467 follow-up).
//
// THE GAP THIS CLOSES. `runtime.startTurn` (mobile Khala Code) durably
// records a `turn.start` control intent and a `queued` `runtime_turn`, then
// NOTHING on the server executes it. The only existing dispatch consumer is
// Pylon-side (`apps/pylon/src/orchestration/runtime-intent-enforcement.ts`),
// which drives REAL local Codex/Claude coding turns on a contributor's own
// machine. A mobile chat turn from a user with no local Pylon running (the
// common case — e.g. "Explain this codebase") therefore sits `queued`
// forever and no assistant ever answers.
//
// This module is the SERVER-OWNED consumer for the `hosted_khala` lane: a
// per-minute cron task (wired into the Worker's `scheduled()` table, run on
// the Cloud Run monolith) that claims `queued` `hosted_khala` turns, resolves
// the user's prompt from the `chat_message` the `bodyRef` points at, drives
// hosted inference (Gemini via `artanisMindComplete`, pure HTTP — no
// Cloudflare bindings, no D1), and writes the assistant response back into
// the private thread scope as `runtime.recordEvent` mutations
// (`turn.started` → `text.delta` → `text.completed` → `turn.finished`). Those
// events sync straight to the mobile client through the Khala Sync changelog
// / LiveHub tail, exactly like a Pylon-driven turn.
//
// STORAGE: authoritative Postgres only, through the same
// transaction-mode-safe `KHALA_SYNC_DB` client discipline the push route uses
// (`defaultMakeKhalaSyncSqlClient`, `max: 1`, `prepare: false`). Reads are
// bounded single statements; every mutation goes through the sanctioned
// `executePush` engine so the changelog appends and mutation ledger stay
// consistent.
//
// EXACTLY-ONCE / RACE SAFETY: claiming a turn IS recording its `turn.started`
// event. `runtime.recordEvent` takes `FOR UPDATE` on the turn row and rejects
// a duplicate `(turn_id, sequence)`, so two overlapping cron ticks serialize
// on the turn row and only one wins the claim (the loser gets an in-band
// `runtime_event_exists` rejection and skips without running inference). A
// claimed turn leaves `queued` status immediately, so the next tick's
// `status = 'queued'` filter never re-selects it.
//
// USAGE: a completed turn records exact provider usage without charging.
// `artanisMindComplete` returns Gemini's `usageMetadata` receipt; on success
// this consumer writes one exact `token_usage_events` row (owner-attributed,
// lane `hosted_khala`) and emits a `usage.recorded` runtime event. Recording is fail-soft:
// it never drops the assistant answer or wedges the turn. See
// `khala-hosted-runtime-metering.ts`.
//
// RESTART RECONCILIATION (#8689): every tick first settles stale `running`
// hosted turns as `turn.interrupted`. A process that dies after its durable
// `turn.started` claim is therefore never re-run (which could duplicate the
// provider call or assistant text), and the shared timeline eventually shows
// one honest terminal outcome. The `(turn_id, sequence)` event constraint
// serializes the sweep against a late original finalizer.
// - The `hosted_khala` lane is also listed in the Pylon consumer's supported
//   lanes (fail-closed there in production). If an owner runs a Pylon AND
//   sends a `hosted_khala` turn, both could act. This consumer's claim
//   protects the server side from double-inference; making `hosted_khala`
//   strictly server-owned (dropping it from the Pylon dispatch set) is the
//   recommended follow-up.
import {
  type ChatMessageImageAttachment,
  type KhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
  type MutationResult,
  decodeKhalaRuntimeEvent,
  decodePushRequest,
} from '@openagentsinc/khala-sync'
import {
  type MutatorRegistry,
  type SyncSql,
  executePush as executePushEngine,
  makeMutatorRegistry,
  readChatMessageById as readChatMessageByIdFromPostgres,
  runtimeMutators,
} from '@openagentsinc/khala-sync-server'
import { sanitizeSarahConversationResponse } from '@openagentsinc/sarah'

import { DEFAULT_GEMMA4_MODEL_ID } from './inference/gemma4-model'
import { parseJsonUnknown } from './json-boundary'
import {
  HOSTED_KHALA_PROVIDER,
  type HostedTurnMeteringInput,
  type HostedTurnMeteringOutcome,
  type HostedTurnUsage,
  hostedKhalaUsageRef,
} from './khala-hosted-runtime-metering'
import type { RuntimeNotifyEventKind } from './push/push-notify-events'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'
import type { SarahAgentToolActivity } from './sarah-agent-runtime'

export type { HostedTurnUsage } from './khala-hosted-runtime-metering'

/** Typed failure for the hosted dispatch push path (the zero-debt
 * architecture check forbids generic `throw new Error` in Worker modules). */
class HostedRuntimeDispatchError extends Error {
  readonly _tag = 'HostedRuntimeDispatchError'
}

/** The single lane this server consumer owns. */
export const HOSTED_RUNTIME_LANE = 'hosted_khala'

/** Default hosted model (matches `artanisMindComplete`'s own default). */
export const DEFAULT_HOSTED_RUNTIME_MODEL = DEFAULT_GEMMA4_MODEL_ID

/** Provider ref stamped on the runtime events' `source`/metadata. */
export const HOSTED_RUNTIME_PROVIDER_REF = HOSTED_KHALA_PROVIDER

/** Base prefix for this consumer's mutation-ledger client group. */
export const HOSTED_RUNTIME_DISPATCH_CLIENT_GROUP_ID =
  'server.hosted-runtime-dispatch'

/**
 * Per-OWNER client group. A Khala Sync client group never migrates between
 * users (the mutation ledger rejects cross-user reuse with "client group is
 * bound to a different user"). A single shared group therefore binds to the
 * FIRST owner whose turn is dispatched and then throws for every other owner —
 * silently orphaning their hosted chat turns (no assistant reply ever lands).
 * Scoping the group by owner keeps each owner on their own group. (Same fix as
 * the #8477 writeback recorder.)
 */
export const hostedRuntimeDispatchClientGroupIdForOwner = (
  ownerUserId: string,
): string => `${HOSTED_RUNTIME_DISPATCH_CLIENT_GROUP_ID}.${ownerUserId}`

/** Default per-tick turn budget (bounds Postgres + inference fan-out). */
export const DEFAULT_HOSTED_RUNTIME_DISPATCH_LIMIT = 8

/** A hosted provider invocation older than this without any durable runtime
 * event is treated as abandoned by its worker generation. */
export const DEFAULT_HOSTED_RUNTIME_STALE_AFTER_MS = 5 * 60_000

/** Default system prompt for a hosted Khala chat turn. */
export const DEFAULT_HOSTED_RUNTIME_SYSTEM_PROMPT =
  'You are Khala, the OpenAgents coding and reasoning assistant. Answer the ' +
  "user's message directly, clearly, and concisely. Use Markdown for code."

const CHAT_MESSAGE_BODY_REF_PREFIX = 'chat_message.'

/** A `queued` `hosted_khala` turn awaiting dispatch. */
export type QueuedHostedTurn = Readonly<{
  turnId: string
  threadId: string
  ownerUserId: string
  eventCount: number
}>

/** Result of a single hosted completion. `usage` carries Gemini's exact
 * `usageMetadata`-derived token counts when the provider reported them; absent
 * usage means the turn records no metering row (exact-only money path). */
export type HostedRuntimeCompletion =
  | {
      readonly ok: true
      readonly text: string
      readonly usage?: HostedTurnUsage | undefined
    }
  | { readonly ok: false; readonly detail: string }

/** Records one completed hosted turn's exact usage. Fail-soft by contract. */
export type HostedRuntimeRecordUsageFn = (
  input: HostedTurnMeteringInput,
) => Promise<HostedTurnMeteringOutcome>

/** The two terminal notify-event kinds this dispatch tick can honestly
 * produce. `turn_needs_input` has no analog here — a hosted chat turn always
 * resolves to a definite answer or a definite failure, it never pauses
 * mid-turn for owner input (SARAH-PUSH-2 #9063; see the runtime-interaction
 * "needs input" route for that lifecycle instead). */
export type HostedRuntimeNotifyKind = Extract<
  RuntimeNotifyEventKind,
  'turn_completed' | 'turn_failed'
>

/** Fire-and-forget push notification on a terminal turn outcome. Injected so
 * the dispatch tick never imports Postgres/KV bindings directly — the real
 * implementation (SARAH-PUSH-2) resolves the owner's devices/preference and
 * calls `dispatchNotifyEvent` in-process, no HTTP hop, no admin bearer. */
export type HostedRuntimeNotifyFn = (
  input: Readonly<{
    kind: HostedRuntimeNotifyKind
    ownerUserId: string
    threadId: string
    turnId: string
  }>,
) => Promise<unknown>

/** Injectable inference seam (default: Gemini via `artanisMindComplete`). */
export type HostedRuntimeCompleteFn = (input: {
  readonly system: string
  readonly prompt: string
  readonly images?: ReadonlyArray<ChatMessageImageAttachment>
  readonly turn: QueuedHostedTurn
  readonly responsePresentation?: 'owner_conversation' | undefined
  readonly onToolActivity: (activity: SarahAgentToolActivity) => Promise<void>
}) => Promise<HostedRuntimeCompletion>

export type HostedRuntimePrepareTurnFn = (
  input: Readonly<{
    turn: QueuedHostedTurn
    system: string
    prompt: string
  }>,
) => Promise<
  Readonly<{
    system: string
    prompt: string
    responsePresentation?: 'owner_conversation'
  }>
>

/** Injectable push-engine seam so tests never need the real engine. */
export type HostedRuntimeExecutePushFn = typeof executePushEngine

export type HostedRuntimeDispatchDependencies = Readonly<{
  /** Root Postgres handle (transaction-mode-safe client from KHALA_SYNC_DB). */
  sql: SyncSql
  /** Drives the assistant answer for one prompt. */
  complete: HostedRuntimeCompleteFn
  /** Optional owner/persona context projection before inference. */
  prepareTurn?: HostedRuntimePrepareTurnFn | undefined
  /** Per-tick turn budget. Default {@link DEFAULT_HOSTED_RUNTIME_DISPATCH_LIMIT}. */
  limit?: number | undefined
  /** Restart-reconciliation age bound. Default: five minutes. */
  staleAfterMs?: number | undefined
  /** Model ref stamped on events. Default {@link DEFAULT_HOSTED_RUNTIME_MODEL}. */
  model?: string | undefined
  /** System prompt. Default {@link DEFAULT_HOSTED_RUNTIME_SYSTEM_PROMPT}. */
  systemPrompt?: string | undefined
  /** Mutator registry (default: the runtime mutators only). */
  registry?: MutatorRegistry | undefined
  /** Push-engine seam (default: the real engine). */
  executePush?: HostedRuntimeExecutePushFn | undefined
  /** Clock (default: real wall clock). */
  now?: (() => string) | undefined
  /** Id generator (default: `crypto.randomUUID`). */
  uuid?: (() => string) | undefined
  /** Structured logger for per-turn outcomes. */
  log?: ((line: string, fields?: Record<string, unknown>) => void) | undefined
  /** Records exact usage on a completed turn. */
  recordUsage?: HostedRuntimeRecordUsageFn | undefined
  /** Fail-soft push notification on turn_completed/turn_failed (#9063).
   * Absent by default so every existing caller/test is unchanged. */
  notify?: HostedRuntimeNotifyFn | undefined
}>

export type HostedRuntimeDispatchSummary = Readonly<{
  scanned: number
  claimed: number
  answered: number
  failed: number
  skipped: number
}>

type ResolvedDeps = Readonly<{
  sql: SyncSql
  complete: HostedRuntimeCompleteFn
  limit: number
  staleAfterMs: number
  model: string
  systemPrompt: string
  registry: MutatorRegistry
  executePush: HostedRuntimeExecutePushFn
  now: () => string
  uuid: () => string
  log: (line: string, fields?: Record<string, unknown>) => void
  recordUsage: HostedRuntimeRecordUsageFn | undefined
  prepareTurn: HostedRuntimePrepareTurnFn | undefined
  notify: HostedRuntimeNotifyFn | undefined
}>

const resolveDeps = (
  deps: HostedRuntimeDispatchDependencies,
): ResolvedDeps => ({
  complete: deps.complete,
  executePush: deps.executePush ?? executePushEngine,
  limit:
    deps.limit !== undefined &&
    Number.isSafeInteger(deps.limit) &&
    deps.limit > 0
      ? deps.limit
      : DEFAULT_HOSTED_RUNTIME_DISPATCH_LIMIT,
  staleAfterMs:
    deps.staleAfterMs !== undefined &&
    Number.isSafeInteger(deps.staleAfterMs) &&
    deps.staleAfterMs > 0
      ? deps.staleAfterMs
      : DEFAULT_HOSTED_RUNTIME_STALE_AFTER_MS,
  log: deps.log ?? (() => undefined),
  model: deps.model ?? DEFAULT_HOSTED_RUNTIME_MODEL,
  notify: deps.notify,
  now: deps.now ?? currentIsoTimestamp,
  prepareTurn: deps.prepareTurn,
  recordUsage: deps.recordUsage,
  registry: deps.registry ?? makeMutatorRegistry([...runtimeMutators]),
  sql: deps.sql,
  systemPrompt: deps.systemPrompt ?? DEFAULT_HOSTED_RUNTIME_SYSTEM_PROMPT,
  uuid: deps.uuid ?? randomUuid,
})

type RuntimeTurnQueueRow = Readonly<{
  turn_id: string
  thread_id: string
  owner_user_id: string
  event_count: string | number
}>

/**
 * Oldest-first bounded page of `queued` `hosted_khala` turns. A turn leaves
 * `queued` the instant it is claimed (its `turn.started` event flips it to
 * `running`), so this never re-selects an in-flight or finished turn.
 */
export const readQueuedHostedTurns = async (
  sql: SyncSql,
  limit: number,
): Promise<ReadonlyArray<QueuedHostedTurn>> => {
  const rows: Array<RuntimeTurnQueueRow> = await sql`
    SELECT turn_id, thread_id, owner_user_id, event_count
    FROM khala_sync_runtime_turns
    WHERE status = 'queued' AND lane = ${HOSTED_RUNTIME_LANE}
    ORDER BY created_at ASC
    LIMIT ${limit}
  `
  return rows.map(row => ({
    eventCount: Number(row.event_count),
    ownerUserId: row.owner_user_id,
    threadId: row.thread_id,
    turnId: row.turn_id,
  }))
}

/** Stale `running` rows use the same bounded identity needed to append the
 * next dense runtime event. They are intentionally never re-queued. */
export const readStaleRunningHostedTurns = async (
  sql: SyncSql,
  cutoffIso: string,
  limit: number,
): Promise<ReadonlyArray<QueuedHostedTurn>> => {
  const rows: Array<RuntimeTurnQueueRow> = await sql`
    SELECT turn_id, thread_id, owner_user_id, event_count
    FROM khala_sync_runtime_turns
    WHERE status = 'running'
      AND lane = ${HOSTED_RUNTIME_LANE}
      AND updated_at <= ${cutoffIso}
    ORDER BY updated_at ASC
    LIMIT ${limit}
  `
  return rows.map(row => ({
    eventCount: Number(row.event_count),
    ownerUserId: row.owner_user_id,
    threadId: row.thread_id,
    turnId: row.turn_id,
  }))
}

type RuntimeStartIntentRow = Readonly<{ intent_json: unknown }>

/**
 * The `bodyRef` recorded by the turn's `turn.start` control intent (the
 * `chat_message.<messageId>` convention). `null` when no `turn.start` intent
 * exists for the turn or it carried no `bodyRef`.
 */
export const readTurnStartBodyRef = async (
  sql: SyncSql,
  turnId: string,
): Promise<string | null> => {
  const rows: Array<RuntimeStartIntentRow> = await sql`
    SELECT intent_json
    FROM khala_sync_runtime_control_intents
    WHERE turn_id = ${turnId} AND kind = 'turn.start'
    ORDER BY seq ASC
    LIMIT 1
  `
  // `intent_json` may come back as a real object OR as a JSON-encoded STRING
  // (some writers/adapters store the control intent double-encoded, i.e. a
  // JSON string inside the jsonb column: `"{\"bodyRef\":...}"`). A string form
  // would previously fall through the `typeof !== 'object'` guard and orphan
  // EVERY hosted turn as `prompt_unresolved` (no assistant reply ever lands).
  // Parse the string form so both encodings resolve the bodyRef.
  let intentJson = rows[0]?.intent_json
  if (typeof intentJson === 'string') {
    try {
      intentJson = parseJsonUnknown(intentJson)
    } catch {
      return null
    }
  }
  if (intentJson === null || typeof intentJson !== 'object') return null
  const bodyRef = (intentJson as { bodyRef?: unknown }).bodyRef
  return typeof bodyRef === 'string' && bodyRef.length > 0 ? bodyRef : null
}

/**
 * Resolve the user's prompt text for a queued turn: read its `turn.start`
 * `bodyRef`, strip the `chat_message.` prefix, and read that message's body.
 * `null` when the ref is missing/malformed or the message does not exist —
 * the caller treats that as a dispatch failure, never a silent skip.
 */
export const resolveHostedTurnMessage = async (
  sql: SyncSql,
  turn: QueuedHostedTurn,
): Promise<Readonly<{
  prompt: string
  images?: ReadonlyArray<ChatMessageImageAttachment>
}> | null> => {
  const bodyRef = await readTurnStartBodyRef(sql, turn.turnId)
  if (bodyRef === null || !bodyRef.startsWith(CHAT_MESSAGE_BODY_REF_PREFIX)) {
    return null
  }
  const messageId = bodyRef.slice(CHAT_MESSAGE_BODY_REF_PREFIX.length)
  if (messageId.length === 0) return null
  const message = await readChatMessageByIdFromPostgres(sql, {
    messageId,
    threadId: turn.threadId,
  })
  if (message === null) return null
  const body = message.body
  if (typeof body !== 'string' || body.length === 0) return null
  return {
    prompt: body,
    ...(message.attachments === undefined || message.attachments.length === 0
      ? {}
      : {
          images:
            message.attachments as ReadonlyArray<ChatMessageImageAttachment>,
        }),
  }
}

/** Backward-compatible text-only projection used by existing callers/tests. */
export const resolveHostedTurnPrompt = async (
  sql: SyncSql,
  turn: QueuedHostedTurn,
): Promise<string | null> =>
  (await resolveHostedTurnMessage(sql, turn))?.prompt ?? null

const runtimeSource = (model: string) => ({
  adapterKind: 'openagents_native' as const,
  lane: HOSTED_RUNTIME_LANE,
  modelRef: model,
  providerRef: HOSTED_RUNTIME_PROVIDER_REF,
  surface: 'server' as const,
})

const buildRuntimeEvent = (
  deps: ResolvedDeps,
  turn: QueuedHostedTurn,
  sequence: number,
  extra: Record<string, unknown>,
): KhalaRuntimeEvent =>
  decodeKhalaRuntimeEvent({
    causalityRefs: [],
    eventId: deps.uuid(),
    observedAt: deps.now(),
    redactionClass: 'private_ref',
    schema: 'openagents.khala_runtime_event.v1',
    sequence,
    source: runtimeSource(deps.model),
    threadId: turn.threadId,
    turnId: turn.turnId,
    visibility: 'private',
    ...extra,
  })

const runtimeSafeRef = (prefix: string, value: string): string =>
  `${prefix}.${value.replaceAll(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 120)}`

/**
 * Fail-soft push notification on a terminal turn outcome (#9063). NEVER
 * throws or rejects — a notify failure (the owner has no devices, the push
 * provider is down, an unexpected error) must never affect the turn's own
 * already-recorded outcome, which is durable by the time this is called.
 */
const notifyTurnOutcomeFailSoft = async (
  resolved: ResolvedDeps,
  kind: HostedRuntimeNotifyKind,
  turn: QueuedHostedTurn,
): Promise<void> => {
  if (resolved.notify === undefined) return
  try {
    await resolved.notify({
      kind,
      ownerUserId: turn.ownerUserId,
      threadId: turn.threadId,
      turnId: turn.turnId,
    })
  } catch (error) {
    resolved.log('hosted_runtime_dispatch_notify_failed', {
      detail: error instanceof Error ? error.message : 'unknown',
      kind,
      turnId: turn.turnId,
    })
  }
}

const sarahToolAuthority = (activity: SarahAgentToolActivity) => ({
  allowed: activity.authorityAllowed ?? true,
  authorityRef:
    activity.authorityReceiptRef ??
    runtimeSafeRef('authority.sarah.tool.selected', activity.toolCallId),
  blockerRefs:
    activity.authorityAllowed === false
      ? activity.resultRefs
          .filter(ref => ref.startsWith('blocker.'))
          .map(ref => runtimeSafeRef('blocker.sarah.tool', ref))
      : [],
  decisionRef: runtimeSafeRef('decision.sarah.tool', activity.toolCallId),
  policyRef: 'policy.sarah.owner_orchestrator.rev3',
  status:
    activity.authorityAllowed === false
      ? ('denied' as const)
      : ('allowed' as const),
  toolRef: runtimeSafeRef('tool.sarah', activity.toolName),
})

const recordHostedRuntimeEvent = (
  resolved: ResolvedDeps,
  turn: QueuedHostedTurn,
  clientId: string,
  mutationId: number,
  event: KhalaRuntimeEvent,
): Promise<MutationResult> => {
  const clientGroupId = hostedRuntimeDispatchClientGroupIdForOwner(
    turn.ownerUserId,
  )
  return resolved
    .executePush({
      registry: resolved.registry,
      request: decodePushRequest({
        clientGroupId,
        clientId,
        mutations: [
          {
            argsJson: JSON.stringify(event),
            mutationId,
            name: 'runtime.recordEvent',
          },
        ],
        protocolVersion: 1,
        schemaVersion: 1,
      }),
      sql: resolved.sql,
      userId: turn.ownerUserId,
    })
    .then(response => {
      const result = response.results[0]
      if (result === undefined) {
        throw new HostedRuntimeDispatchError(
          'executePush returned no result for runtime.recordEvent',
        )
      }
      return result
    })
}

/**
 * Reconcile process death without replaying provider work. A late original
 * worker and this sweep contend on the same next `(turn_id, sequence)`; only
 * one durable event wins. Partial assistant text remains exactly as recorded,
 * followed by one visible interrupted terminal when the sweep wins.
 */
export const recoverStaleRunningHostedTurns = async (
  deps: HostedRuntimeDispatchDependencies,
): Promise<number> => {
  const resolved = resolveDeps(deps)
  let cutoffIso: string
  try {
    cutoffIso = isoTimestampAfterIso(resolved.now(), -resolved.staleAfterMs)
  } catch (cause) {
    throw new HostedRuntimeDispatchError(
      `hosted runtime clock returned an invalid timestamp: ${cause instanceof Error ? cause.name : 'unknown'}`,
    )
  }
  const stale = await readStaleRunningHostedTurns(
    resolved.sql,
    cutoffIso,
    resolved.limit,
  )
  let recovered = 0
  for (const turn of stale) {
    const clientId = `${hostedRuntimeDispatchClientGroupIdForOwner(turn.ownerUserId)}.${turn.turnId}.recovery.${resolved.uuid()}`
    const result = await recordHostedRuntimeEvent(
      resolved,
      turn,
      clientId,
      1,
      buildRuntimeEvent(resolved, turn, turn.eventCount, {
        kind: 'turn.interrupted',
        reasonRef: 'worker_generation_lost',
      }),
    )
    if (result.status === 'applied') {
      recovered += 1
      resolved.log('hosted_runtime_dispatch_recovered_interrupted', {
        turnId: turn.turnId,
      })
    }
  }
  return recovered
}

/**
 * Dispatch a single queued hosted turn end-to-end. Returns the outcome so the
 * batch runner can tally it. NEVER throws for an ordinary per-turn failure —
 * inference/resolution errors are reported to the user as a terminal
 * `turn.finished` (`finishReason: "error"`).
 */
export const dispatchHostedRuntimeTurn = async (
  deps: HostedRuntimeDispatchDependencies,
  turn: QueuedHostedTurn,
): Promise<'answered' | 'failed' | 'skipped'> => {
  const resolved = resolveDeps(deps)
  const ownerId = turn.ownerUserId
  // Owner-scoped client group so different owners never collide on one group
  // (a shared group binds to the first owner and throws for the rest).
  const clientGroupId = hostedRuntimeDispatchClientGroupIdForOwner(ownerId)
  const clientId = `${clientGroupId}.${turn.turnId}.${resolved.uuid()}`
  let seq = turn.eventCount

  // Every event is recorded as the turn's OWNER: `runtime.recordEvent`
  // resolves thread-scope ownership from `ctx.userId`, which `executePush`
  // takes as `userId`. A fresh per-attempt `clientId` keeps concurrent cron
  // ticks off each other's mutation ledger; `(turn_id, sequence)` is the real
  // dedupe.
  const record = (
    mutationId: number,
    event: KhalaRuntimeEvent,
  ): Promise<MutationResult> =>
    recordHostedRuntimeEvent(resolved, turn, clientId, mutationId, event)

  // 1. CLAIM: record turn.started. This is the atomic claim — the loser of a
  // race gets an in-band rejection (runtime_event_exists / turn moved).
  const startEvent = buildRuntimeEvent(resolved, turn, seq, {
    kind: 'turn.started',
  })
  const startResult = await record(1, startEvent)
  if (startResult.status !== 'applied') {
    resolved.log('hosted_runtime_dispatch_claim_skipped', {
      errorCode: startResult.errorCode,
      turnId: turn.turnId,
    })
    return 'skipped'
  }
  seq += 1
  let mutationId = 2

  const onToolActivity = async (
    activity: SarahAgentToolActivity,
  ): Promise<void> => {
    const authority = sarahToolAuthority(activity)
    const extra: Record<string, unknown> =
      activity.phase === 'started'
        ? {
            authority,
            kind: 'tool.call',
            toolCallId: runtimeSafeRef('call.sarah', activity.toolCallId),
            toolName: activity.toolName,
          }
        : activity.phase === 'succeeded'
          ? {
              authority,
              kind: 'tool.result',
              providerExecuted: true,
              resultRef: runtimeSafeRef(
                'result.sarah.tool',
                activity.resultRefs[0] ?? activity.toolCallId,
              ),
              toolCallId: runtimeSafeRef('call.sarah', activity.toolCallId),
              toolName: activity.toolName,
            }
          : {
              authority,
              errorRef: runtimeSafeRef(
                'error.sarah.tool',
                activity.resultRefs[0] ?? activity.toolCallId,
              ),
              kind: 'tool.error',
              messageSafe: activity.summary.slice(0, 500),
              providerExecuted: true,
              toolCallId: runtimeSafeRef('call.sarah', activity.toolCallId),
              toolName: activity.toolName,
            }
    const result = await record(
      mutationId,
      buildRuntimeEvent(resolved, turn, seq, extra),
    )
    if (result.status !== 'applied') {
      throw new HostedRuntimeDispatchError(
        `Sarah tool activity write rejected: ${result.errorCode ?? 'unknown'}`,
      )
    }
    mutationId += 1
    seq += 1
  }

  // 2. Resolve the prompt and drive inference.
  let completion: HostedRuntimeCompletion
  let responsePresentation: 'owner_conversation' | undefined
  try {
    const message = await resolveHostedTurnMessage(resolved.sql, turn)
    if (message === null) {
      completion = { detail: 'prompt_unresolved', ok: false }
    } else {
      const prepared =
        resolved.prepareTurn === undefined
          ? { prompt: message.prompt, system: resolved.systemPrompt }
          : await resolved.prepareTurn({
              prompt: message.prompt,
              system: resolved.systemPrompt,
              turn,
            })
      responsePresentation = prepared.responsePresentation
      completion = await resolved.complete({
        onToolActivity,
        prompt: prepared.prompt,
        responsePresentation,
        system: prepared.system,
        turn,
        ...(message.images === undefined ? {} : { images: message.images }),
      })
    }
  } catch (error) {
    completion = {
      detail: error instanceof Error ? error.message : 'inference_threw',
      ok: false,
    }
  }

  if (completion.ok && responsePresentation === 'owner_conversation') {
    completion = {
      ...completion,
      text: sanitizeSarahConversationResponse(completion.text),
    }
  }
  // A terminal success without assistant text leaves the mobile composer
  // looking complete even though Sarah (or an ordinary hosted chat) never
  // answered. Keep that outcome on the existing explicit failure path so the
  // client receives terminal truth instead of a silent `turn.finished(stop)`.
  if (completion.ok && completion.text.trim() === '') {
    completion = { detail: 'empty_response', ok: false }
  }

  // 3. On failure, still settle the turn so the client stops spinning.
  if (!completion.ok) {
    resolved.log('hosted_runtime_dispatch_failed', {
      detail: completion.detail,
      turnId: turn.turnId,
    })
    const failedResult = await record(
      mutationId,
      buildRuntimeEvent(resolved, turn, seq, {
        finishReason: 'error' satisfies KhalaRuntimeFinishReason,
        kind: 'turn.finished',
      }),
    )
    if (failedResult.status !== 'applied') return 'skipped'
    await notifyTurnOutcomeFailSoft(resolved, 'turn_failed', turn)
    return 'failed'
  }

  // 4. Success: stream the answer as one text.delta + text.completed, then
  // finish the turn.
  const messageId = resolved.uuid()
  if (completion.text.length > 0) {
    const deltaResult = await record(
      mutationId,
      buildRuntimeEvent(resolved, turn, seq, {
        chunkId: resolved.uuid(),
        kind: 'text.delta',
        messageId,
        text: completion.text,
      }),
    )
    if (deltaResult.status !== 'applied') return 'skipped'
    seq += 1
    mutationId += 1
    const completedResult = await record(
      mutationId,
      buildRuntimeEvent(resolved, turn, seq, {
        kind: 'text.completed',
        messageId,
      }),
    )
    if (completedResult.status !== 'applied') return 'skipped'
    seq += 1
    mutationId += 1
  }

  // 5. USAGE. With exact Gemini `usageMetadata`, record the exact
  // `token_usage_events` row, then emit a `usage.recorded` runtime event. All fail-soft — a recording
  // failure never drops the assistant answer or wedges the turn. A turn with no
  // provider usage records no row (exact-only; never a fabricated receipt).
  const usage = completion.usage
  let turnUsage: HostedTurnUsage | undefined
  if (usage !== undefined) {
    turnUsage = usage
    let meterOutcome: HostedTurnMeteringOutcome | undefined
    if (resolved.recordUsage !== undefined) {
      try {
        meterOutcome = await resolved.recordUsage({
          observedAt: resolved.now(),
          ownerUserId: ownerId,
          threadId: turn.threadId,
          turnId: turn.turnId,
          usage,
        })
      } catch (error) {
        resolved.log('hosted_runtime_dispatch_metering_threw', {
          detail: error instanceof Error ? error.message : 'unknown',
          turnId: turn.turnId,
        })
      }
    }
    const usageResult = await record(
      mutationId,
      buildRuntimeEvent(resolved, turn, seq, {
        kind: 'usage.recorded',
        usage: {
          ...(usage.cacheReadTokens > 0
            ? { cacheReadInputTokens: usage.cacheReadTokens }
            : {}),
          inputTokens: usage.inputTokens,
          outputTokens: usage.outputTokens,
          reasoningTokens: usage.reasoningTokens,
          totalTokens: usage.totalTokens,
          usageRef: meterOutcome?.usageRef ?? hostedKhalaUsageRef(turn.turnId),
        },
      }),
    )
    if (usageResult.status !== 'applied') return 'skipped'
    seq += 1
    mutationId += 1
    resolved.log('hosted_runtime_dispatch_usage_recorded', {
      insertedTokenUsage: meterOutcome?.insertedTokenUsage,
      tokensServed: meterOutcome?.tokensServed,
      turnId: turn.turnId,
    })
  }

  const finishedResult = await record(
    mutationId,
    buildRuntimeEvent(resolved, turn, seq, {
      finishReason: 'stop' satisfies KhalaRuntimeFinishReason,
      kind: 'turn.finished',
      ...(turnUsage === undefined
        ? {}
        : {
            usage: {
              ...(turnUsage.cacheReadTokens > 0
                ? { cacheReadInputTokens: turnUsage.cacheReadTokens }
                : {}),
              inputTokens: turnUsage.inputTokens,
              outputTokens: turnUsage.outputTokens,
              reasoningTokens: turnUsage.reasoningTokens,
              totalTokens: turnUsage.totalTokens,
              usageRef: hostedKhalaUsageRef(turn.turnId),
            },
          }),
    }),
  )
  if (finishedResult.status !== 'applied') return 'skipped'
  await notifyTurnOutcomeFailSoft(resolved, 'turn_completed', turn)
  resolved.log('hosted_runtime_dispatch_answered', {
    responseChars: completion.text.length,
    turnId: turn.turnId,
  })
  return 'answered'
}

/**
 * One cron tick: claim and answer up to `limit` queued `hosted_khala` turns.
 * Failure-isolated — one bad turn can never wedge the batch. Returns a tally.
 */
export const runHostedRuntimeTurnDispatch = async (
  deps: HostedRuntimeDispatchDependencies,
): Promise<HostedRuntimeDispatchSummary> => {
  const resolved = resolveDeps(deps)
  // Restart reconciliation precedes new claims: abandoned work becomes one
  // durable interrupted terminal and is never sent to the provider again.
  await recoverStaleRunningHostedTurns(deps)
  const turns = await readQueuedHostedTurns(resolved.sql, resolved.limit)
  let answered = 0
  let failed = 0
  let claimed = 0
  let skipped = 0
  for (const turn of turns) {
    try {
      const outcome = await dispatchHostedRuntimeTurn(deps, turn)
      if (outcome === 'answered') {
        answered += 1
        claimed += 1
      } else if (outcome === 'failed') {
        failed += 1
        claimed += 1
      } else {
        skipped += 1
      }
    } catch (error) {
      // A thrown dispatch (e.g. an unexpected storage error) must not wedge
      // the batch — log and move to the next turn.
      failed += 1
      resolved.log('hosted_runtime_dispatch_threw', {
        detail: error instanceof Error ? error.message : 'unknown',
        turnId: turn.turnId,
      })
    }
  }
  return { answered, claimed, failed, scanned: turns.length, skipped }
}

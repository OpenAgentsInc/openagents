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
// METERING (#8555): a completed turn now records EXACT usage + debits credits.
// `artanisMindComplete` returns Gemini's `usageMetadata` receipt; on success
// this consumer writes one exact `token_usage_events` row (owner-attributed,
// lane `hosted_khala`), debits the owner's credit balance through the live
// ledger metering hook (which also projects the `scope.user.<userId>`
// `credit_balance` delta), and emits a `usage.recorded` runtime event. A
// pre-inference balance gate refuses a zero/negative-credit owner (fail-closed,
// but never blocking on an undetermined balance). All metering is fail-soft:
// it never drops the assistant answer or wedges the turn. See
// `khala-hosted-runtime-metering.ts`.
//
// KNOWN GAPS (honest, not papered over):
// - A process that dies AFTER claiming (`turn.started` recorded) but BEFORE
//   `turn.finished` leaves the turn stuck `running`. There is no requeue
//   watchdog here; a future timeout sweep should re-open stale `running`
//   hosted turns. (Ordinary failures ARE handled: inference errors record a
//   terminal `turn.finished` with `finishReason: "error"`.)
// - The `hosted_khala` lane is also listed in the Pylon consumer's supported
//   lanes (fail-closed there in production). If an owner runs a Pylon AND
//   sends a `hosted_khala` turn, both could act. This consumer's claim
//   protects the server side from double-inference; making `hosted_khala`
//   strictly server-owned (dropping it from the Pylon dispatch set) is the
//   recommended follow-up.

import {
  decodeKhalaRuntimeEvent,
  decodePushRequest,
  type KhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
  type MutationResult,
} from '@openagentsinc/khala-sync'
import {
  executePush as executePushEngine,
  makeMutatorRegistry,
  readChatMessageById as readChatMessageByIdFromPostgres,
  runtimeMutators,
  type MutatorRegistry,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { parseJsonUnknown } from './json-boundary'
import {
  hostedKhalaUsageRef,
  type HostedTurnMeteringInput,
  type HostedTurnMeteringOutcome,
  type HostedTurnUsage,
} from './khala-hosted-runtime-metering'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

export type { HostedTurnUsage } from './khala-hosted-runtime-metering'

/** Typed failure for the hosted dispatch push path (the zero-debt
 * architecture check forbids generic `throw new Error` in Worker modules). */
class HostedRuntimeDispatchError extends Error {
  readonly _tag = 'HostedRuntimeDispatchError'
}

/** The single lane this server consumer owns. */
export const HOSTED_RUNTIME_LANE = 'hosted_khala'

/** Default hosted model (matches `artanisMindComplete`'s own default). */
export const DEFAULT_HOSTED_RUNTIME_MODEL = 'gemini-3.5-flash'

/** Provider ref stamped on the runtime events' `source`/metadata. */
export const HOSTED_RUNTIME_PROVIDER_REF = 'openagents-khala'

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

/** Reads the owner's available (spendable) msat credit balance for the
 * pre-inference balance gate. Returns `null` when the balance is UNDETERMINED
 * (a read failure) — the gate must NOT block on undetermined balance. */
export type HostedRuntimeReadOwnerBalanceMsatFn = (
  ownerUserId: string,
) => Promise<number | null>

/** Records one completed hosted turn's exact usage + debits the owner's
 * credits. Fail-soft by contract (never throws). Default: absent (no metering,
 * used by tests and any deployment without the credits/ledger bindings). */
export type HostedRuntimeRecordUsageFn = (
  input: HostedTurnMeteringInput,
) => Promise<HostedTurnMeteringOutcome>

/** Injectable inference seam (default: Gemini via `artanisMindComplete`). */
export type HostedRuntimeCompleteFn = (input: {
  readonly system: string
  readonly prompt: string
}) => Promise<HostedRuntimeCompletion>

/** Injectable push-engine seam so tests never need the real engine. */
export type HostedRuntimeExecutePushFn = typeof executePushEngine

export type HostedRuntimeDispatchDependencies = Readonly<{
  /** Root Postgres handle (transaction-mode-safe client from KHALA_SYNC_DB). */
  sql: SyncSql
  /** Drives the assistant answer for one prompt. */
  complete: HostedRuntimeCompleteFn
  /** Per-tick turn budget. Default {@link DEFAULT_HOSTED_RUNTIME_DISPATCH_LIMIT}. */
  limit?: number | undefined
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
  /** Balance gate: read the owner's spendable credit balance before running
   * inference. Absent => no gate (hosted turns run without a pre-check;
   * metering still charges after). */
  readOwnerBalanceMsat?: HostedRuntimeReadOwnerBalanceMsatFn | undefined
  /** Records exact usage + debits credits on a completed turn. Absent => no
   * metering (the pre-#8555 free behavior; kept for tests / unconfigured
   * deployments). */
  recordUsage?: HostedRuntimeRecordUsageFn | undefined
}>

export type HostedRuntimeDispatchSummary = Readonly<{
  scanned: number
  claimed: number
  answered: number
  failed: number
  skipped: number
  /** Turns claimed then refused by the zero-balance gate (never ran inference). */
  refused: number
}>

type ResolvedDeps = Readonly<{
  sql: SyncSql
  complete: HostedRuntimeCompleteFn
  limit: number
  model: string
  systemPrompt: string
  registry: MutatorRegistry
  executePush: HostedRuntimeExecutePushFn
  now: () => string
  uuid: () => string
  log: (line: string, fields?: Record<string, unknown>) => void
  readOwnerBalanceMsat: HostedRuntimeReadOwnerBalanceMsatFn | undefined
  recordUsage: HostedRuntimeRecordUsageFn | undefined
}>

const resolveDeps = (deps: HostedRuntimeDispatchDependencies): ResolvedDeps => ({
  complete: deps.complete,
  executePush: deps.executePush ?? executePushEngine,
  limit:
    deps.limit !== undefined && Number.isSafeInteger(deps.limit) && deps.limit > 0
      ? deps.limit
      : DEFAULT_HOSTED_RUNTIME_DISPATCH_LIMIT,
  log: deps.log ?? (() => undefined),
  model: deps.model ?? DEFAULT_HOSTED_RUNTIME_MODEL,
  now: deps.now ?? currentIsoTimestamp,
  readOwnerBalanceMsat: deps.readOwnerBalanceMsat,
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
export const resolveHostedTurnPrompt = async (
  sql: SyncSql,
  turn: QueuedHostedTurn,
): Promise<string | null> => {
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
  return typeof body === 'string' && body.length > 0 ? body : null
}

const runtimeSource = (model: string) =>
  ({
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

/**
 * Dispatch a single queued hosted turn end-to-end. Returns the outcome so the
 * batch runner can tally it. NEVER throws for an ordinary per-turn failure —
 * inference/resolution errors are reported to the user as a terminal
 * `turn.finished` (`finishReason: "error"`).
 */
export const dispatchHostedRuntimeTurn = async (
  deps: HostedRuntimeDispatchDependencies,
  turn: QueuedHostedTurn,
): Promise<'answered' | 'failed' | 'skipped' | 'refused'> => {
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
    resolved.executePush({
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
      userId: ownerId,
    }).then(response => {
      const result = response.results[0]
      if (result === undefined) {
        throw new HostedRuntimeDispatchError(
          'executePush returned no result for runtime.recordEvent',
        )
      }
      return result
    })

  // 1. CLAIM: record turn.started. This is the atomic claim — the loser of a
  // race gets an in-band rejection (runtime_event_exists / turn moved).
  const startEvent = buildRuntimeEvent(resolved, turn, seq, { kind: 'turn.started' })
  const startResult = await record(1, startEvent)
  if (startResult.status !== 'applied') {
    resolved.log('hosted_runtime_dispatch_claim_skipped', {
      errorCode: startResult.errorCode,
      turnId: turn.turnId,
    })
    return 'skipped'
  }
  seq += 1

  // 1b. BALANCE GATE (#8555). Everything on this lane uses credits (#8467), so
  // a user with zero/negative spendable credit must be refused, not served for
  // free. Fail-CLOSED on a determinable zero balance, but NEVER block on an
  // UNDETERMINED balance (a read failure => `null` => proceed), per the
  // onboarding contract. The gate is skipped entirely when no balance reader is
  // wired (tests / unconfigured deployments).
  if (resolved.readOwnerBalanceMsat !== undefined) {
    const balanceMsat = await resolved.readOwnerBalanceMsat(ownerId).catch(() => null)
    if (balanceMsat !== null && balanceMsat <= 0) {
      resolved.log('hosted_runtime_dispatch_refused', {
        reason: 'insufficient_credit',
        turnId: turn.turnId,
      })
      await record(
        2,
        buildRuntimeEvent(resolved, turn, seq, {
          finishReason: 'error' satisfies KhalaRuntimeFinishReason,
          kind: 'turn.finished',
        }),
      )
      return 'refused'
    }
  }

  // 2. Resolve the prompt and drive inference.
  let completion: HostedRuntimeCompletion
  try {
    const prompt = await resolveHostedTurnPrompt(resolved.sql, turn)
    if (prompt === null) {
      completion = { detail: 'prompt_unresolved', ok: false }
    } else {
      completion = await resolved.complete({
        prompt,
        system: resolved.systemPrompt,
      })
    }
  } catch (error) {
    completion = {
      detail: error instanceof Error ? error.message : 'inference_threw',
      ok: false,
    }
  }

  // 3. On failure, still settle the turn so the client stops spinning.
  if (!completion.ok) {
    resolved.log('hosted_runtime_dispatch_failed', {
      detail: completion.detail,
      turnId: turn.turnId,
    })
    await record(
      2,
      buildRuntimeEvent(resolved, turn, seq, {
        finishReason: 'error' satisfies KhalaRuntimeFinishReason,
        kind: 'turn.finished',
      }),
    )
    return 'failed'
  }

  // 4. Success: stream the answer as one text.delta + text.completed, then
  // finish the turn.
  const messageId = resolved.uuid()
  let mutationId = 2
  if (completion.text.length > 0) {
    await record(
      mutationId,
      buildRuntimeEvent(resolved, turn, seq, {
        chunkId: resolved.uuid(),
        kind: 'text.delta',
        messageId,
        text: completion.text,
      }),
    )
    seq += 1
    mutationId += 1
    await record(
      mutationId,
      buildRuntimeEvent(resolved, turn, seq, {
        kind: 'text.completed',
        messageId,
      }),
    )
    seq += 1
    mutationId += 1
  }

  // 5. METERING (#8555). With exact Gemini `usageMetadata`, record the exact
  // `token_usage_events` row + debit the owner's credits + project the balance
  // delta, then emit a `usage.recorded` runtime event so the client sees the
  // exact usage (and its charge receipt) live. All fail-soft — a metering
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
    const chargeReceiptRef = meterOutcome?.chargeReceiptRef ?? null
    await record(
      mutationId,
      buildRuntimeEvent(resolved, turn, seq, {
        kind: 'usage.recorded',
        usage: {
          ...(usage.cacheReadTokens > 0
            ? { cacheReadInputTokens: usage.cacheReadTokens }
            : {}),
          ...(chargeReceiptRef === null ? {} : { costRef: chargeReceiptRef }),
          inputTokens: usage.inputTokens,
          outputTokens: usage.outputTokens,
          reasoningTokens: usage.reasoningTokens,
          totalTokens: usage.totalTokens,
          usageRef: meterOutcome?.usageRef ?? hostedKhalaUsageRef(turn.turnId),
        },
      }),
    )
    seq += 1
    mutationId += 1
    resolved.log('hosted_runtime_dispatch_metered', {
      chargeUsdCents: meterOutcome?.chargeUsdCents,
      failureReason: meterOutcome?.failureReason,
      insertedTokenUsage: meterOutcome?.insertedTokenUsage,
      metered: meterOutcome?.metered,
      tokensServed: meterOutcome?.tokensServed,
      turnId: turn.turnId,
    })
  }

  await record(
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
  const turns = await readQueuedHostedTurns(resolved.sql, resolved.limit)
  let answered = 0
  let failed = 0
  let claimed = 0
  let skipped = 0
  let refused = 0
  for (const turn of turns) {
    try {
      const outcome = await dispatchHostedRuntimeTurn(deps, turn)
      if (outcome === 'answered') {
        answered += 1
        claimed += 1
      } else if (outcome === 'failed') {
        failed += 1
        claimed += 1
      } else if (outcome === 'refused') {
        refused += 1
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
  return { answered, claimed, failed, refused, scanned: turns.length, skipped }
}

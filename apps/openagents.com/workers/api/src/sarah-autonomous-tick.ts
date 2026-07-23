// SARAH-AUTONOMOUS-1: a scheduled autonomous heartbeat for Sarah.
//
// THE GAP THIS CLOSES. Until now Sarah only ever runs when the owner sends a
// message on her thread: an owner push admits a `runtime.startTurn` mutation
// (`/api/sync/push`) which kicks `runHostedRuntimeTurnDispatchForEnv`, and the
// hosted runtime answers that ONE queued turn. With no owner message there is
// no turn, so Sarah never self-initiates — she can sit idle for days. This
// module gives her a timer-driven trigger: on the existing per-minute cron
// drive (`scheduled()` -> Cloud Scheduler `/internal/cron`), gated by a
// default-OFF env flag, Sarah wakes, reads the SAME bounded business context,
// runs ONE ordinary Sarah turn whose objective is "assess company state and
// take the single next best admitted action, or report the top blocker" — using
// her EXISTING gated tools — and posts the owner a proactive, receipt-backed
// update. No owner message required.
//
// WHAT THIS IS NOT. This adds a trigger and an objective around the existing
// turn machinery. It adds NO new Sarah power. Every mutation the turn performs
// still travels through `makeSarahRuntimeTools` -> `authorizeSarahOperation` ->
// the admitted `SARAH_RUNTIME_AUTHORITY_PROFILE`, so a reserved action (stable
// release without independent verification, spend beyond budget, financial
// custody, self-amplification, ...) refuses with a receipt exactly as it does
// on an owner-triggered turn. Redaction, immutability, and owner-scope gates are
// untouched.
//
// BOUNDED BY CONSTRUCTION.
//  - Flag default OFF (`SARAH_AUTONOMOUS_TICK_ENABLED`): with it unset there is
//    zero behavior change — no owner is resolved, no turn runs, no store opened.
//  - At most one tick per owner per interval, even across concurrent Cloud Run
//    instances: each interval bucket is atomically claimed in
//    `sarah_autonomous_tick_runs` (migration 0095) with
//    `INSERT ... ON CONFLICT DO NOTHING RETURNING`. The loser cleanly no-ops.
//  - One turn per tick. The turn's own tool-round budget
//    (`SARAH_AGENT_MAX_TOOL_ROUNDS`) bounds tool calls; the objective asks for a
//    SINGLE next action or a clean top-blocker report.
//  - A bounded owner fan-out per tick (`SARAH_AUTONOMOUS_TICK_MAX_OWNERS`).
//  - FAIL-SOFT: nothing here may throw into the cron tick. A per-owner failure
//    is isolated and reported as a value; a claim/turn/append/push failure never
//    breaks the cron or any other scheduled work.

import {
  type KhalaRuntimeControlIntent,
  type KhalaRuntimeEvent,
  decodeKhalaRuntimeControlIntent,
  decodeKhalaRuntimeEvent,
  decodePushRequest,
} from '@openagentsinc/khala-sync'
import {
  type MutatorRegistry,
  type SyncSql,
  executePush as executePushEngine,
  makeMutatorRegistry,
  runtimeMutators,
} from '@openagentsinc/khala-sync-server'

import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import type { SarahOperationAuthorityOutcome } from './sarah-owner-routes'
import { hasSarahThreadAuthority } from './sarah-owner-routes'

// ---------------------------------------------------------------------------
// Flag + interval + budget configuration
// ---------------------------------------------------------------------------

/** Default-OFF master gate. With it unset the tick is completely inert. */
export const SARAH_AUTONOMOUS_TICK_FLAG = 'SARAH_AUTONOMOUS_TICK_ENABLED' as const
/** Optional interval override (whole minutes), clamped to the safe band. */
export const SARAH_AUTONOMOUS_TICK_INTERVAL_FLAG =
  'SARAH_AUTONOMOUS_TICK_INTERVAL_MINUTES' as const

export const SARAH_AUTONOMOUS_TICK_DEFAULT_INTERVAL_MINUTES = 15
export const SARAH_AUTONOMOUS_TICK_MIN_INTERVAL_MINUTES = 5
export const SARAH_AUTONOMOUS_TICK_MAX_INTERVAL_MINUTES = 240
/** Bounded owner fan-out per tick: the admitted owner set is tiny (a single
 * owner today), but this caps the work a single cron tick can ever start. */
export const SARAH_AUTONOMOUS_TICK_MAX_OWNERS = 4

export const isSarahAutonomousTickEnabled = (
  env: Readonly<{ SARAH_AUTONOMOUS_TICK_ENABLED?: string | undefined }> | undefined,
): boolean => {
  const value = env?.SARAH_AUTONOMOUS_TICK_ENABLED
  return value === 'true' || value === '1' || value === 'on'
}

export const resolveSarahAutonomousTickIntervalMinutes = (
  env:
    | Readonly<{ SARAH_AUTONOMOUS_TICK_INTERVAL_MINUTES?: string | undefined }>
    | undefined,
): number => {
  const raw = env?.SARAH_AUTONOMOUS_TICK_INTERVAL_MINUTES
  if (raw === undefined || raw.trim() === '') {
    return SARAH_AUTONOMOUS_TICK_DEFAULT_INTERVAL_MINUTES
  }
  const parsed = Number(raw)
  if (!Number.isFinite(parsed) || !Number.isInteger(parsed)) {
    return SARAH_AUTONOMOUS_TICK_DEFAULT_INTERVAL_MINUTES
  }
  return Math.min(
    SARAH_AUTONOMOUS_TICK_MAX_INTERVAL_MINUTES,
    Math.max(SARAH_AUTONOMOUS_TICK_MIN_INTERVAL_MINUTES, parsed),
  )
}

/** The fixed autonomous objective. Not model-authored: it is a bounded,
 * one-action-per-tick instruction that keeps Sarah inside her admitted powers
 * and her honest-reporting contract. It never asks for a reserved action and it
 * forbids claiming an action ran without a target receipt. */
export const SARAH_AUTONOMOUS_TICK_OBJECTIVE = [
  'This is an autonomous check-in that fired on a timer, not a message from the owner. The owner is away and did not ask anything right now.',
  'Assess the current company state from your owner-scoped business context and your read tools.',
  'Then take the single next best action that is already within your admitted authority to move the company forward — for example: delegate one bounded coding task to your Codex workers, read the Full Auto projection and pause or resume an existing run if that is clearly right, or draft a blog, document, or Forum communication for owner review.',
  'Take at most ONE action this tick. If nothing is clearly actionable, or you are blocked, do not force an action — instead report the single most important blocker or the top thing the owner should decide.',
  'Before you report a blocker, compare it with the recent owner-thread conversation in your cited context. Never repeat an unchanged capacity blocker that an earlier autonomous check-in already reported. If coding capacity is unavailable and that blocker is already present, take a different admitted action such as a draft or a justified Full Auto control; if no different action is useful, say only that there is no new action instead of repeating the blocker.',
  'Do not take any reserved action (no stable release without the independent verification gate, no spend beyond budget, no new authority, no financial, legal, or employment move). Those refuse by design.',
  'Finish with a brief, plain-language update to the owner: what you observed, and what you did or decided this tick. Never claim an action ran unless a tool returned a receipt confirming it.',
].join('\n')

// ---------------------------------------------------------------------------
// Interval bucketing + deterministic tick ref
// ---------------------------------------------------------------------------

export const sarahAutonomousTickIntervalBucket = (
  nowMs: number,
  intervalMinutes: number,
): number => Math.floor(nowMs / (intervalMinutes * 60_000))

const THREAD_SUFFIX_PATTERN = /^thread\.sarah\.([0-9a-f]{24})$/

/** Deterministic per-owner-per-interval id. Derived from the OPAQUE Sarah
 * thread ref (already a hash of the owner id) plus the interval bucket, so the
 * raw owner id never enters the ref and the id is stable for every instance
 * racing on the same bucket. */
export const sarahAutonomousTickRef = (
  threadRef: string,
  bucket: number,
): string => {
  const match = THREAD_SUFFIX_PATTERN.exec(threadRef)
  const suffix = match === null ? 'unknown' : match[1]
  return `tick.sarah.${suffix}.b${bucket}`
}

// ---------------------------------------------------------------------------
// Owner resolution (reuses the exact hosted authority gate)
// ---------------------------------------------------------------------------

export type SarahAutonomousTickOwner = Readonly<{
  ownerUserId: string
  threadRef: string
}>

export type ResolveSarahAutonomousTickOwnersDeps = Readonly<{
  sql: SyncSql
  /** Injectable for tests; defaults to the real hosted authority gate. */
  hasThreadAuthority?:
    | ((sql: SyncSql, ownerUserId: string, threadRef: string) => Promise<boolean>)
    | undefined
  maxOwners?: number | undefined
}>

/**
 * The admitted owner set for the autonomous tick. Candidates come from the
 * durable Sarah bootstrap receipts (`sarah_authority_decision_receipts`,
 * action `maintain_owner_contact`, outcome `succeeded`) — the same rows the
 * owner route writes when it admits Sarah for an owner+thread. Each candidate
 * is then RE-VERIFIED through `hasSarahThreadAuthority` (active admin identity +
 * live bootstrap receipt), so a stale row can never re-admit a revoked owner.
 */
export const resolveSarahAutonomousTickOwners = async (
  deps: ResolveSarahAutonomousTickOwnersDeps,
): Promise<ReadonlyArray<SarahAutonomousTickOwner>> => {
  const hasThreadAuthority = deps.hasThreadAuthority ?? hasSarahThreadAuthority
  const maxOwners = Math.max(
    1,
    Math.min(deps.maxOwners ?? SARAH_AUTONOMOUS_TICK_MAX_OWNERS, SARAH_AUTONOMOUS_TICK_MAX_OWNERS),
  )
  const candidates: ReadonlyArray<{ owner_user_id: string; thread_ref: string }> =
    await deps.sql`
      SELECT DISTINCT owner_user_id, thread_ref
        FROM sarah_authority_decision_receipts
       WHERE action_ref = 'maintain_owner_contact'
         AND outcome = 'succeeded'
       ORDER BY owner_user_id, thread_ref
    `
  const admitted: Array<SarahAutonomousTickOwner> = []
  for (const candidate of candidates) {
    if (admitted.length >= maxOwners) break
    const ok = await hasThreadAuthority(
      deps.sql,
      candidate.owner_user_id,
      candidate.thread_ref,
    ).catch(() => false)
    if (ok) {
      admitted.push({
        ownerUserId: candidate.owner_user_id,
        threadRef: candidate.thread_ref,
      })
    }
  }
  return admitted
}

// ---------------------------------------------------------------------------
// Interval claim + settle (atomic, cross-instance-safe)
// ---------------------------------------------------------------------------

/**
 * Atomically claim this owner's interval bucket. Returns the deterministic
 * `tickRef` when THIS caller won the bucket, or `undefined` when the bucket was
 * already claimed (by an earlier tick or a concurrent Cloud Run instance) — a
 * safe no-op. `ON CONFLICT (tick_ref) DO NOTHING RETURNING` is the whole
 * at-most-once guarantee.
 */
export const claimSarahAutonomousTickInterval = async (
  sql: SyncSql,
  input: Readonly<{
    ownerUserId: string
    threadRef: string
    bucket: number
    nowIso: string
  }>,
): Promise<string | undefined> => {
  const tickRef = sarahAutonomousTickRef(input.threadRef, input.bucket)
  const rows: ReadonlyArray<{ tick_ref: string }> = await sql`
    INSERT INTO sarah_autonomous_tick_runs
      (tick_ref, owner_user_id, thread_ref, interval_bucket, started_at)
    VALUES
      (${tickRef}, ${input.ownerUserId}, ${input.threadRef}, ${input.bucket}, ${input.nowIso})
    ON CONFLICT (tick_ref) DO NOTHING
    RETURNING tick_ref
  `
  return rows[0]?.tick_ref
}

/** Record the tick's own settled outcome + authority receipt ref for audit.
 * Best-effort: a lost settle row never affects the already-emitted target
 * receipts or the owner-thread update. */
export const settleSarahAutonomousTickRun = async (
  sql: SyncSql,
  input: Readonly<{
    tickRef: string
    outcome: string
    receiptRef?: string | undefined
    nowIso: string
  }>,
): Promise<void> => {
  await sql`
    UPDATE sarah_autonomous_tick_runs
       SET outcome = ${input.outcome},
           receipt_ref = ${input.receiptRef ?? null},
           settled_at = ${input.nowIso}
     WHERE tick_ref = ${input.tickRef}
  `
}

// ---------------------------------------------------------------------------
// Owner-thread proactive update (same synthetic-turn shape as every Sarah turn)
// ---------------------------------------------------------------------------

/** The lane every Sarah/hosted owner-conversation turn already syncs under. */
const NOTICE_LANE = 'hosted_khala' as const

const boundedRef = (prefix: string, raw: string): string =>
  `${prefix}.${raw.replaceAll(/[^A-Za-z0-9_.:-]/g, '_')}`.slice(0, 256)

export type SarahAutonomousUpdateExecutePushFn = typeof executePushEngine

export type AppendSarahAutonomousUpdateDependencies = Readonly<{
  sql: SyncSql
  registry?: MutatorRegistry | undefined
  nowIso?: (() => string) | undefined
  uuid?: (() => string) | undefined
  executePush?: SarahAutonomousUpdateExecutePushFn | undefined
}>

/**
 * Append the tick's owner-facing update as ONE hosted-runtime turn
 * (turn.start -> turn.started -> text.delta -> text.completed -> turn.finished),
 * through the SAME `runtimeMutators` / push-engine path every real Sarah turn
 * syncs through — no new sync surface. All ids derive deterministically from
 * `tickRef`, so accidental reentry hits the mutators' own `runtime_turn_exists`
 * rejection rather than duplicating the update. Returns `true` only when every
 * mutation applied.
 */
export const appendSarahAutonomousUpdateToThread = async (
  deps: AppendSarahAutonomousUpdateDependencies,
  input: Readonly<{
    tickRef: string
    ownerUserId: string
    threadRef: string
    text: string
  }>,
): Promise<boolean> => {
  const nowIso = (deps.nowIso ?? currentIsoTimestamp)()
  const uuid = deps.uuid ?? randomUuid
  const registry = deps.registry ?? makeMutatorRegistry([...runtimeMutators])
  const executePush = deps.executePush ?? executePushEngine

  const turnId = boundedRef('turn.sarah_auto', input.tickRef)
  const intentId = boundedRef('intent.sarah_auto', input.tickRef)
  const messageId = boundedRef('message.sarah_auto', input.tickRef)
  const idempotencyKey = boundedRef('idempotency.sarah_auto', input.tickRef)
  const clientGroupId = boundedRef('server.sarah.autonomous_tick', input.ownerUserId)
  const clientId = `${clientGroupId}.${turnId}`

  const intent: KhalaRuntimeControlIntent = decodeKhalaRuntimeControlIntent({
    causalityRefs: [],
    createdAt: nowIso,
    idempotencyKey,
    intentId,
    kind: 'turn.start',
    origin: { lane: NOTICE_LANE, surface: 'server' },
    redactionClass: 'private_ref',
    schema: 'openagents.khala_runtime_control_intent.v1',
    target: { lane: NOTICE_LANE },
    threadId: input.threadRef,
    turnId,
    visibility: 'private',
  })

  const source = {
    adapterKind: 'openagents_native' as const,
    lane: NOTICE_LANE,
    modelRef: 'sarah_autonomous_tick_update',
    providerRef: 'openagents_system',
    surface: 'server' as const,
  }

  const event = (sequence: number, extra: Record<string, unknown>): KhalaRuntimeEvent =>
    decodeKhalaRuntimeEvent({
      causalityRefs: [],
      eventId: uuid(),
      observedAt: nowIso,
      redactionClass: 'private_ref',
      schema: 'openagents.khala_runtime_event.v1',
      sequence,
      source,
      threadId: input.threadRef,
      turnId,
      visibility: 'private',
      ...extra,
    })

  const response = await executePush({
    registry,
    request: decodePushRequest({
      clientGroupId,
      clientId,
      mutations: [
        { argsJson: JSON.stringify(intent), mutationId: 1, name: 'runtime.startTurn' },
        {
          argsJson: JSON.stringify(event(0, { kind: 'turn.started' })),
          mutationId: 2,
          name: 'runtime.recordEvent',
        },
        {
          argsJson: JSON.stringify(
            event(1, { chunkId: uuid(), kind: 'text.delta', messageId, text: input.text }),
          ),
          mutationId: 3,
          name: 'runtime.recordEvent',
        },
        {
          argsJson: JSON.stringify(event(2, { kind: 'text.completed', messageId })),
          mutationId: 4,
          name: 'runtime.recordEvent',
        },
        {
          argsJson: JSON.stringify(event(3, { finishReason: 'stop', kind: 'turn.finished' })),
          mutationId: 5,
          name: 'runtime.recordEvent',
        },
      ],
      protocolVersion: 1,
      schemaVersion: 1,
    }),
    sql: deps.sql,
    userId: input.ownerUserId,
  })

  return response.results.every(result => result.status === 'applied')
}

// ---------------------------------------------------------------------------
// Per-owner orchestration (testable; injected turn/authorize/append/push seams)
// ---------------------------------------------------------------------------

export type SarahAutonomousTurnResult =
  | { readonly ok: true; readonly text: string; readonly toolCallCount?: number | undefined }
  | { readonly ok: false; readonly detail: string }

export type SarahAutonomousTickOutcome =
  | { readonly outcome: 'disabled' }
  | { readonly outcome: 'interval_skip'; readonly bucket: number }
  | {
      readonly outcome: 'refused'
      readonly tickRef: string
      readonly receiptRef: string
      readonly refusalReason?: string | undefined
    }
  | {
      readonly outcome: 'turn_failed'
      readonly tickRef: string
      readonly receiptRef: string
      readonly detail: string
    }
  | {
      readonly outcome: 'acted'
      readonly tickRef: string
      readonly receiptRef: string
      readonly threadUpdateApplied: boolean
      readonly toolCallCount: number
    }
  | { readonly outcome: 'failed'; readonly detail: string }

export type RunSarahAutonomousTickForOwnerDependencies = Readonly<{
  sql: SyncSql
  intervalMinutes: number
  now?: (() => Date) | undefined
  /** Runs ONE Sarah agent turn with the autonomous objective, through the exact
   * hosted turn machinery + gated tools. Never grants new authority. */
  runTurn: (
    input: Readonly<{
      ownerUserId: string
      threadRef: string
      tickRef: string
      prompt: string
    }>,
  ) => Promise<SarahAutonomousTurnResult>
  /** Resolves + durably receipts the autonomous TRIGGER through the existing
   * `authorizeSarahOperation`/`SARAH_RUNTIME_AUTHORITY_PROFILE`. A non-owner or
   * a lost owner scope refuses here and the tick stops before running a turn. */
  authorize: (
    input: Readonly<{ ownerUserId: string; threadRef: string; tickRef: string }>,
  ) => Promise<SarahOperationAuthorityOutcome>
  appendUpdate: (
    input: Readonly<{ tickRef: string; ownerUserId: string; threadRef: string; text: string }>,
  ) => Promise<boolean>
  push?:
    | ((
        input: Readonly<{ ownerUserId: string; threadRef: string; tickRef: string }>,
      ) => Promise<unknown>)
    | undefined
  log?: ((event: string, fields: Readonly<Record<string, unknown>>) => void) | undefined
}>

/**
 * ONE bounded, gated, receipted autonomous tick for one admitted owner. Never
 * throws (fail-soft contract): every failure is returned as an outcome value.
 * Order: claim interval -> receipt the trigger -> run one gated turn -> append
 * the owner update + fire the push -> settle the audit row.
 */
export const runSarahAutonomousTickForOwner = async (
  deps: RunSarahAutonomousTickForOwnerDependencies,
  owner: SarahAutonomousTickOwner,
): Promise<SarahAutonomousTickOutcome> => {
  const nowIso = () => ((deps.now?.() ?? new Date()).toISOString())
  try {
    const bucket = sarahAutonomousTickIntervalBucket(
      (deps.now?.() ?? new Date()).getTime(),
      deps.intervalMinutes,
    )
    const tickRef = await claimSarahAutonomousTickInterval(deps.sql, {
      bucket,
      nowIso: nowIso(),
      ownerUserId: owner.ownerUserId,
      threadRef: owner.threadRef,
    })
    if (tickRef === undefined) {
      return { bucket, outcome: 'interval_skip' }
    }

    // Receipt the autonomous TRIGGER through the existing broker. This is the
    // owner-scope + admitted-authority gate for the tick itself: a refused
    // decision (lost owner scope, unadmitted profile) stops before any turn.
    const decision = await deps.authorize({
      ownerUserId: owner.ownerUserId,
      threadRef: owner.threadRef,
      tickRef,
    })
    if (!decision.allowed) {
      await settleSarahAutonomousTickRun(deps.sql, {
        nowIso: nowIso(),
        outcome: 'refused',
        receiptRef: decision.receiptRef,
        tickRef,
      }).catch(() => undefined)
      return {
        outcome: 'refused',
        receiptRef: decision.receiptRef,
        ...(decision.refusalReason === undefined
          ? {}
          : { refusalReason: decision.refusalReason }),
        tickRef,
      }
    }

    const turn = await deps.runTurn({
      ownerUserId: owner.ownerUserId,
      prompt: SARAH_AUTONOMOUS_TICK_OBJECTIVE,
      threadRef: owner.threadRef,
      tickRef,
    })
    if (!turn.ok) {
      await settleSarahAutonomousTickRun(deps.sql, {
        nowIso: nowIso(),
        outcome: 'turn_failed',
        receiptRef: decision.receiptRef,
        tickRef,
      }).catch(() => undefined)
      return {
        detail: turn.detail,
        outcome: 'turn_failed',
        receiptRef: decision.receiptRef,
        tickRef,
      }
    }

    const threadUpdateApplied = await deps
      .appendUpdate({
        ownerUserId: owner.ownerUserId,
        text: turn.text,
        threadRef: owner.threadRef,
        tickRef,
      })
      .catch(error => {
        deps.log?.('sarah_autonomous_tick_update_write_failed', {
          detail: error instanceof Error ? error.message : 'unknown',
          tickRef,
        })
        return false
      })

    if (deps.push !== undefined) {
      await deps
        .push({ ownerUserId: owner.ownerUserId, threadRef: owner.threadRef, tickRef })
        .catch(error => {
          deps.log?.('sarah_autonomous_tick_push_failed', {
            detail: error instanceof Error ? error.message : 'unknown',
            tickRef,
          })
          return undefined
        })
    }

    await settleSarahAutonomousTickRun(deps.sql, {
      nowIso: nowIso(),
      outcome: 'acted',
      receiptRef: decision.receiptRef,
      tickRef,
    }).catch(() => undefined)

    return {
      outcome: 'acted',
      receiptRef: decision.receiptRef,
      threadUpdateApplied,
      tickRef,
      toolCallCount: turn.toolCallCount ?? 0,
    }
  } catch (error) {
    deps.log?.('sarah_autonomous_tick_owner_failed', {
      detail: error instanceof Error ? error.message : 'unknown',
      ownerUserId: owner.ownerUserId,
    })
    return {
      detail: error instanceof Error ? error.message : 'unknown',
      outcome: 'failed',
    }
  }
}

export type SarahAutonomousTickDispatchSummary = Readonly<{
  ownersResolved: number
  attempted: number
  acted: number
  intervalSkipped: number
  refused: number
  turnFailed: number
  failed: number
}>

export type RunSarahAutonomousTickDispatchDependencies =
  RunSarahAutonomousTickForOwnerDependencies &
    Readonly<{
      resolveOwners?:
        | (() => Promise<ReadonlyArray<SarahAutonomousTickOwner>>)
        | undefined
      maxOwners?: number | undefined
    }>

/**
 * Resolve the admitted owner set and run one bounded tick per owner. Fail-soft
 * throughout: a resolution failure yields an empty owner set, and each owner
 * tick is isolated. Returns a summary for the cron log.
 */
export const runSarahAutonomousTickDispatch = async (
  deps: RunSarahAutonomousTickDispatchDependencies,
): Promise<SarahAutonomousTickDispatchSummary> => {
  const resolveOwners =
    deps.resolveOwners ??
    (() =>
      resolveSarahAutonomousTickOwners({
        maxOwners: deps.maxOwners ?? SARAH_AUTONOMOUS_TICK_MAX_OWNERS,
        sql: deps.sql,
      }))
  const owners = await resolveOwners().catch(error => {
    deps.log?.('sarah_autonomous_tick_owner_resolution_failed', {
      detail: error instanceof Error ? error.message : 'unknown',
    })
    return [] as ReadonlyArray<SarahAutonomousTickOwner>
  })

  const summary = {
    acted: 0,
    attempted: 0,
    failed: 0,
    intervalSkipped: 0,
    ownersResolved: owners.length,
    refused: 0,
    turnFailed: 0,
  }
  for (const owner of owners) {
    summary.attempted += 1
    const result = await runSarahAutonomousTickForOwner(deps, owner)
    switch (result.outcome) {
      case 'acted':
        summary.acted += 1
        break
      case 'interval_skip':
        summary.intervalSkipped += 1
        break
      case 'refused':
        summary.refused += 1
        break
      case 'turn_failed':
        summary.turnFailed += 1
        break
      case 'failed':
        summary.failed += 1
        break
      case 'disabled':
        break
    }
  }
  return summary
}

// SARAH-PROACTIVE-1 (#9064): proactive owner notice for a Sarah-dispatched
// Codex worker's terminal outcome.
//
// THE GAP THIS CLOSES. Sarah's `codex_workers_start` tool
// (`sarah-runtime-tools.ts`) dispatches real, owner-linked Pylon Codex
// workers against a pinned OpenAgents commit. Today, once the owner closes
// the app, the outcome only ever shows up in receipts/projections the owner
// has to go check — Sarah never proactively tells them. This module closes
// that gap: when a Sarah-dispatched assignment's `worker_closeout` event
// lands (`pylon-api-routes.ts`), it appends ONE bounded, receipt-backed
// status message into the owner's own Sarah thread and fires the paired
// push notification (SARAH-PUSH-2, #9063).
//
// WHY DISPATCH-TIME CAPTURE, NOT CLOSEOUT-TIME INFERENCE. SARAH-PUSH-2's
// closeout investigation found `PylonApiAssignmentRecord` (the
// `worker_closeout` event's target) carries no `threadId`, and its
// `ownerAgentUserId` is an AGENT-TOKEN identity never proven to share the
// OpenAuth `userId` space Sarah threads and push devices are keyed on —
// wiring push/thread delivery straight off that record would risk
// misattributed or silently-dropped notices. Instead, `codex_workers_start`
// captures the trustworthy `(ownerUserId, threadRef)` binding AT DISPATCH
// TIME, inside the already-authenticated Sarah turn
// (`hasSarahThreadAuthority` already proved owner+thread there). The
// closeout hook only ever looks up that durable, Sarah-owned mapping by
// `assignmentRef` — it never derives identity from the agent-token-scoped
// assignment record.
//
// STORAGE: `sarah_worker_dispatch_mappings` (khala-sync-server migration
// 0082) — Postgres-authoritative on the same KHALA_SYNC_DB Hyperdrive
// binding every other Sarah/Khala Sync surface uses. `consumed_at` makes
// closeout notification EXACTLY-ONCE: `consumeSarahWorkerDispatchMapping`
// atomically claims the row (`UPDATE ... WHERE consumed_at IS NULL
// RETURNING`), so a retried/duplicate `worker_closeout` event finds nothing
// left to consume and is a safe, silent no-op — never a second message.
//
// THREAD WRITE PATH. Sarah's own `owner_conversation` turns are ordinary
// hosted-runtime turns (`khala-hosted-runtime-dispatch.ts`): a
// `runtime.startTurn` control intent, then `runtime.recordEvent` mutations
// (`turn.started` -> `text.delta` -> `text.completed` -> `turn.finished`)
// through the same push-engine `runtimeMutators` every mobile/hosted Khala
// client syncs against. This module synthesizes exactly that same shape for
// ONE short, fixed-template, non-model-generated notice — no inference, no
// new authority, no new sync surface. The turn/message/intent ids are
// derived deterministically from `assignmentRef`, so a bug that somehow
// re-entered this path would hit `runtime_turn_exists` rather than silently
// duplicating the notice.
//
// FAIL-SOFT CONTRACT: nothing in this module may affect the Pylon worker
// assignment's own recorded outcome or the closeout event's normal
// handling. The wiring in `pylon-api-routes.ts` calls this AFTER the D1
// business write has already committed, via the same
// `Effect.promise(async () => { try { ... } catch { /* fail-soft */ } })`
// pattern `maybeProjectFleetAssignment` (KS-6.1) uses — never
// `Effect.tryPromise`, so a failure here can never fail the route.

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

import type { MobileAccessRevocationStore } from './auth/mobile-session'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import type { PushDeviceTokenDb } from './push/push-device-tokens'
import { dispatchNotifyEventForOwner, type NotifyEventOutcome } from './push/push-notify-routes'
import type { FetchLike } from './push/push-sender'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

/** The lane every Sarah/hosted owner-conversation turn already syncs under
 * (`khala-hosted-runtime-dispatch.ts`'s `HOSTED_RUNTIME_LANE`). Reused here
 * rather than imported to avoid pulling in that module's much larger
 * inference-dispatch dependency surface for one lane literal. */
const NOTICE_LANE = 'hosted_khala' as const

export type SarahWorkerCloseoutOutcome = 'accepted' | 'refused' | 'failed'

/**
 * Maps the real Pylon worker-closeout wire vocabulary onto the three-way
 * outcome this notice reports. The wire vocabulary comes from
 * `apps/pylon/src/assignment.ts`'s `AssignmentCloseout.status`:
 * `"accepted" | "rejected" | "cancelled" | "timed-out" | "stale"`, posted as
 * `closeout.status === "accepted" ? "closeout_submitted" : closeout.status`
 * (`hostedAssignmentWorkerCloseoutBody`). This never invents a status: an
 * unrecognized value is honestly reported as `failed` rather than silently
 * defaulting to a success wording.
 */
export const sarahWorkerCloseoutOutcomeFromStatus = (
  status: string,
): SarahWorkerCloseoutOutcome => {
  if (status === 'closeout_submitted' || status === 'accepted') return 'accepted'
  if (status === 'rejected') return 'refused'
  return 'failed'
}

/** Fixed-template, non-model-generated notice text. Always names the exact
 * assignment ref (never a fabricated completion claim) and distinguishes
 * accepted/refused/failed honestly rather than claiming uniform success. */
export const sarahWorkerCloseoutNoticeText = (input: Readonly<{
  assignmentRef: string
  outcome: SarahWorkerCloseoutOutcome
  status: string
}>): string => {
  switch (input.outcome) {
    case 'accepted':
      return `A Codex worker I dispatched has finished its run and submitted its closeout for review. Assignment: ${input.assignmentRef}.`
    case 'refused':
      return `A Codex worker I dispatched declined the assignment (status: ${input.status}). Assignment: ${input.assignmentRef}.`
    case 'failed':
      return `A Codex worker I dispatched did not complete the assignment (status: ${input.status}). Assignment: ${input.assignmentRef}.`
  }
}

const boundedRef = (prefix: string, raw: string): string =>
  `${prefix}.${raw.replaceAll(/[^A-Za-z0-9_.:-]/g, '_')}`.slice(0, 256)

/**
 * Record the (ownerUserId, threadRef) binding for a real, successfully
 * dispatched Sarah Codex worker assignment. Called from
 * `codex_workers_start` for each child assignment `khala.spawn` actually
 * admitted. `ON CONFLICT DO NOTHING`: a duplicate dispatch write for the
 * same assignmentRef (should not happen — assignmentRef is server-minted
 * per dispatch) is a safe no-op, never an overwrite.
 */
export const recordSarahWorkerDispatchMapping = async (
  sql: SyncSql,
  input: Readonly<{
    assignmentRef: string
    ownerUserId: string
    threadRef: string
    nowIso: string
  }>,
): Promise<void> => {
  await sql`
    INSERT INTO sarah_worker_dispatch_mappings
      (assignment_ref, owner_user_id, thread_ref, dispatched_at)
    VALUES (${input.assignmentRef}, ${input.ownerUserId}, ${input.threadRef}, ${input.nowIso})
    ON CONFLICT (assignment_ref) DO NOTHING
  `
}

type ConsumedSarahWorkerDispatchMapping = Readonly<{
  ownerUserId: string
  threadRef: string
}>

/** Atomically claims (and thereby consumes) the dispatch mapping for one
 * assignmentRef. Returns `undefined` when there is no mapping (not a
 * Sarah-dispatched assignment — a safe no-op) OR when it was already
 * consumed by an earlier `worker_closeout` delivery (idempotency: a
 * retried/duplicate event finds nothing left to claim). */
export const consumeSarahWorkerDispatchMapping = async (
  sql: SyncSql,
  input: Readonly<{ assignmentRef: string; nowIso: string }>,
): Promise<ConsumedSarahWorkerDispatchMapping | undefined> => {
  const rows: ReadonlyArray<{ owner_user_id: string; thread_ref: string }> = await sql`
    UPDATE sarah_worker_dispatch_mappings
       SET consumed_at = ${input.nowIso}
     WHERE assignment_ref = ${input.assignmentRef}
       AND consumed_at IS NULL
    RETURNING owner_user_id, thread_ref
  `
  const row = rows[0]
  return row === undefined
    ? undefined
    : { ownerUserId: row.owner_user_id, threadRef: row.thread_ref }
}

/** Injectable push-engine seam so tests never need a real Postgres connection
 * (same pattern `khala-hosted-runtime-dispatch.ts` uses for its own
 * synthetic-turn writes). */
export type SarahWorkerCloseoutNoticeExecutePushFn = typeof executePushEngine

export type AppendSarahWorkerCloseoutNoticeDependencies = Readonly<{
  sql: SyncSql
  registry?: MutatorRegistry | undefined
  nowIso?: () => string | undefined
  uuid?: () => string | undefined
  executePush?: SarahWorkerCloseoutNoticeExecutePushFn | undefined
}>

/** Synthesizes one short hosted-runtime turn (turn.started -> text.delta ->
 * text.completed -> turn.finished) carrying the fixed notice text, through
 * the SAME `runtimeMutators` / push-engine path every real Sarah turn syncs
 * through. Returns `true` when every mutation in the turn applied. Every
 * id is derived deterministically from `assignmentRef`, so accidental reentry
 * hits the mutators' own `runtime_turn_exists` rejection instead of a
 * duplicate message. */
export const appendSarahWorkerCloseoutNoticeToThread = async (
  deps: AppendSarahWorkerCloseoutNoticeDependencies,
  input: Readonly<{
    assignmentRef: string
    ownerUserId: string
    threadRef: string
    text: string
  }>,
): Promise<boolean> => {
  const nowIso = (deps.nowIso ?? currentIsoTimestamp)()
  const uuid = deps.uuid ?? randomUuid
  const registry = deps.registry ?? makeMutatorRegistry([...runtimeMutators])
  const executePush = deps.executePush ?? executePushEngine

  const turnId = boundedRef('turn.sarah_wc', input.assignmentRef)
  const intentId = boundedRef('intent.sarah_wc', input.assignmentRef)
  const messageId = boundedRef('message.sarah_wc', input.assignmentRef)
  const idempotencyKey = boundedRef('idempotency.sarah_wc', input.assignmentRef)
  const clientGroupId = boundedRef('server.sarah.worker_closeout', input.ownerUserId)
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
    modelRef: 'sarah_worker_closeout_notice',
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

export type NotifySarahWorkerCloseoutDependencies = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  makeSqlClient: MakeKhalaSyncPushSqlClient
  /** CFG-4 Domain 4 push registry/preferences handle (`paymentsLedgerDbForEnv`). */
  pushDb: PushDeviceTokenDb
  authStorage: MobileAccessRevocationStore
  fetchImpl?: FetchLike | undefined
  registry?: MutatorRegistry | undefined
  nowIso?: (() => string) | undefined
  uuid?: (() => string) | undefined
  executePush?: SarahWorkerCloseoutNoticeExecutePushFn | undefined
  log?: ((event: string, fields: Readonly<Record<string, unknown>>) => void) | undefined
}>

export type NotifySarahWorkerCloseoutOutcome =
  | { readonly outcome: 'no_mapping' }
  | { readonly outcome: 'no_binding' }
  | {
      readonly outcome: 'notified'
      readonly threadNoticeApplied: boolean
      readonly push: NotifyEventOutcome | undefined
    }
  | { readonly outcome: 'failed'; readonly detail: string }

/**
 * The closeout-side entry point. Looks up (and atomically consumes) the
 * dispatch mapping for `assignmentRef`; when found, appends the bounded
 * thread notice and fires the paired push notification. NEVER throws — see
 * the module header's fail-soft contract. Callers that need a business
 * outcome for logging get one back as a value; nothing here should ever be
 * awaited for its rejection.
 */
export const notifySarahWorkerCloseout = async (
  deps: NotifySarahWorkerCloseoutDependencies,
  input: Readonly<{ assignmentRef: string; eventStatus: string; nowIso: string }>,
): Promise<NotifySarahWorkerCloseoutOutcome> => {
  if (
    deps.binding === undefined ||
    typeof deps.binding.connectionString !== 'string' ||
    deps.binding.connectionString.length === 0
  ) {
    return { outcome: 'no_binding' }
  }

  let client: KhalaSyncPushSqlClient | undefined
  try {
    client = await deps.makeSqlClient(deps.binding.connectionString)
    const mapping = await consumeSarahWorkerDispatchMapping(client.sql, {
      assignmentRef: input.assignmentRef,
      nowIso: input.nowIso,
    })
    if (mapping === undefined) {
      return { outcome: 'no_mapping' }
    }

    const outcome = sarahWorkerCloseoutOutcomeFromStatus(input.eventStatus)
    const text = sarahWorkerCloseoutNoticeText({
      assignmentRef: input.assignmentRef,
      outcome,
      status: input.eventStatus,
    })

    const threadNoticeApplied = await appendSarahWorkerCloseoutNoticeToThread(
      {
        sql: client.sql,
        ...(deps.executePush === undefined ? {} : { executePush: deps.executePush }),
        ...(deps.nowIso === undefined ? {} : { nowIso: deps.nowIso }),
        ...(deps.registry === undefined ? {} : { registry: deps.registry }),
        ...(deps.uuid === undefined ? {} : { uuid: deps.uuid }),
      },
      {
        assignmentRef: input.assignmentRef,
        ownerUserId: mapping.ownerUserId,
        text,
        threadRef: mapping.threadRef,
      },
    ).catch(error => {
      deps.log?.('sarah_worker_closeout_notify_thread_write_failed', {
        assignmentRef: input.assignmentRef,
        detail: error instanceof Error ? error.message : 'unknown',
      })
      return false
    })

    const push = await dispatchNotifyEventForOwner(
      deps.pushDb,
      deps.authStorage,
      {
        // Reuses the existing SARAH-PUSH-2 fixed-template kinds rather than
        // adding a new one: `turn_completed`/`turn_failed` already cover
        // "your task finished" / "your task ran into a problem and stopped",
        // which honestly describes an accepted-vs-refused/failed worker
        // closeout without widening the fuzz-tested payload-safety oracle
        // in `push-notify-events.ts` for a bounded, ref-only companion event.
        kind: outcome === 'accepted' ? 'turn_completed' : 'turn_failed',
        ownerUserId: mapping.ownerUserId,
        threadId: mapping.threadRef,
      },
      deps.fetchImpl,
    ).catch(error => {
      deps.log?.('sarah_worker_closeout_notify_push_failed', {
        assignmentRef: input.assignmentRef,
        detail: error instanceof Error ? error.message : 'unknown',
      })
      return undefined
    })

    return { outcome: 'notified', push, threadNoticeApplied }
  } catch (error) {
    deps.log?.('sarah_worker_closeout_notify_failed', {
      assignmentRef: input.assignmentRef,
      detail: error instanceof Error ? error.message : 'unknown',
    })
    return {
      detail: error instanceof Error ? error.message : 'unknown',
      outcome: 'failed',
    }
  } finally {
    if (client !== undefined) {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route / fleet
        // projection dual-write.
      }
    }
  }
}

/**
 * CFG-17 (#8533): Postgres-backed EventLedgerOwnerSequenceStore + oa-infra
 * Mutex serialization — the Cloud Run replacement for the EVENT_LEDGER_OWNER
 * Durable Object.
 *
 * The DO gave per-owner single-writer ordering: `getByName(ownerAgentUserId)`
 * routed every event-ledger-ingest job for an owner to ONE single-threaded
 * object whose per-owner SQLite `event_ledger_owner_order` table allocated a
 * gapless monotonic `ordering_sequence` (MAX+1) and deduped by ordering key.
 * On Cloud Run there is no DO — so this module reproduces the SAME guarantee
 * with two owned pieces:
 *
 *  1. a Postgres owner-sequence table (`event_ledger_owner_order`,
 *     khala-sync migration 0045), keyed and sequenced PER owner, and
 *  2. the owned oa-infra Mutex (`pg_advisory_xact_lock` keyed by owner,
 *     packages/oa-infra/src/mutex.ts), held for the WHOLE
 *     reserve→insert→mark-persisted append so two concurrent jobs for the
 *     same owner never interleave (exactly what the DO's single thread did).
 *
 * Driver discipline: the SAME transaction-mode-safe postgres.js client seam
 * every other khala-sync Worker store uses (`defaultMakeKhalaSyncSqlClient`
 * over `KHALA_SYNC_DB`). The mutex reserves its OWN dedicated connection; the
 * sequence store uses a SEPARATE client so the two never contend for one
 * connection while the lock is held.
 *
 * Idempotency: reservation is keyed by (owner, `source:externalRef`), so a
 * redelivered job re-reads its existing sequence with `duplicate: true`
 * instead of allocating a new one (matches the DO and the ledger's
 * `INSERT OR IGNORE`).
 */
import { Effect } from 'effect'

import { makePostgresMutex } from '@openagentsinc/oa-infra/mutex-postgres'
import type { MutexSqlClient } from '@openagentsinc/oa-infra/mutex-postgres'
import type { MutexShape } from '@openagentsinc/oa-infra/mutex'

import {
  makeEventLedgerStoreForEnv,
  recordEventLedgerIngestMessage,
  type EventLedgerIngestOutcome,
  type EventLedgerIngestQueueMessage,
  type EventLedgerOwnerSequenceReservation,
  type EventLedgerOwnerSequenceStore,
  type EventLedgerStore,
} from './event-ledger'
import type { AgentRuntimeRemainderStoreEnv } from './agent-runtime-remainder-store'
import {
  defaultMakeKhalaSyncSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'

/**
 * Minimal structural tagged-template client the sequence store needs — a
 * parameterized query that resolves with result rows. Satisfied by Bun's
 * `SQL` and postgres.js alike (same seam as oa-infra's `KvSql`).
 */
export type EventLedgerOwnerSequenceSql = <T = Record<string, unknown>>(
  strings: TemplateStringsArray,
  ...values: ReadonlyArray<unknown>
) => Promise<Array<T>>

type OwnerSequenceRow = Readonly<{
  ordering_sequence: string | number
  persisted_at: string | null
}>

type NextSequenceRow = Readonly<{ next_sequence: string | number }>

/** The advisory-lock name the per-owner Mutex serializes on. */
export const eventLedgerOwnerLockName = (ownerAgentUserId: string): string =>
  `event-ledger-owner:${ownerAgentUserId}`

/**
 * Postgres owner-sequence store scoped to ONE owner (the dispatch handles a
 * single message, so the owner is fixed). Correctness of the MAX+1 read and
 * the dedup read depends on the caller holding the per-owner advisory lock
 * around `reserve`; this store does not lock on its own.
 */
export const makePostgresEventLedgerOwnerSequenceStore = (
  sql: EventLedgerOwnerSequenceSql,
  ownerAgentUserId: string,
): EventLedgerOwnerSequenceStore => ({
  markPersisted: async (orderingKey, persistedAt) => {
    await sql`
      UPDATE event_ledger_owner_order
         SET persisted_at = ${persistedAt}
       WHERE owner_agent_user_id = ${ownerAgentUserId}
         AND ordering_key = ${orderingKey}
    `
  },
  reserve: async (
    message: EventLedgerIngestQueueMessage,
  ): Promise<EventLedgerOwnerSequenceReservation> => {
    const orderingKey = `${message.source}:${message.externalRef}`

    const existing = (await sql<OwnerSequenceRow>`
      SELECT ordering_sequence, persisted_at
        FROM event_ledger_owner_order
       WHERE owner_agent_user_id = ${ownerAgentUserId}
         AND ordering_key = ${orderingKey}
       LIMIT 1
    `)[0]

    if (existing !== undefined) {
      return {
        duplicate: true,
        orderingKey,
        orderingSequence: Number(existing.ordering_sequence),
        persisted: existing.persisted_at !== null,
      }
    }

    const nextRow = (await sql<NextSequenceRow>`
      SELECT COALESCE(MAX(ordering_sequence), 0) + 1 AS next_sequence
        FROM event_ledger_owner_order
       WHERE owner_agent_user_id = ${ownerAgentUserId}
    `)[0]
    const orderingSequence = Number(nextRow?.next_sequence ?? 1)

    await sql`
      INSERT INTO event_ledger_owner_order
        (owner_agent_user_id, ordering_key, ordering_sequence,
         first_seen_at, persisted_at)
      VALUES
        (${ownerAgentUserId}, ${orderingKey}, ${orderingSequence},
         ${message.receivedAt}, NULL)
    `

    return {
      duplicate: false,
      orderingKey,
      orderingSequence,
      persisted: false,
    }
  },
})

export type EventLedgerOwnerMutexEnv = AgentRuntimeRemainderStoreEnv &
  Readonly<{ OPENAGENTS_DB: D1Database }>

/** Typed wrapper around a failed append (never a bare `throw new Error`). */
export class EventLedgerOwnerAppendError extends Error {
  override readonly name = 'EventLedgerOwnerAppendError'

  constructor(readonly reason: unknown) {
    super('event-ledger owner-serialized append failed')
  }
}

export type RecordEventLedgerMessageWithOwnerMutexOptions = Readonly<{
  /** Injectable postgres.js client factory (tests). Default: KHALA_SYNC_DB. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Injectable Mutex backend (tests). Default: oa-infra Postgres advisory. */
  makeMutex?: ((sql: MutexSqlClient) => MutexShape) | undefined
  /** Injectable sequence store (tests). Default: Postgres. */
  makeSequenceStore?:
    | ((
        sql: EventLedgerOwnerSequenceSql,
        ownerAgentUserId: string,
      ) => EventLedgerOwnerSequenceStore)
    | undefined
  /** Injectable ledger store (tests). Default: D1 over the env. */
  makeStore?: ((env: EventLedgerOwnerMutexEnv) => EventLedgerStore) | undefined
  nowIso?: (() => string) | undefined
}>

/**
 * The Cloud Run event-ledger-ingest append: reserve a per-owner sequence,
 * write the D1 ledger row, and mark the reservation persisted — all inside
 * the owned oa-infra Mutex keyed by the ledger owner, so concurrent jobs for
 * the same owner serialize (advisory lock held for the whole append).
 *
 * Throws on failure so the caller (`dispatchOaQueueMessage`) rethrows and the
 * oa-queue-worker pump nacks → retries → dead-letters, same as the DO path.
 */
export const recordEventLedgerMessageWithOwnerMutex = async (
  env: EventLedgerOwnerMutexEnv,
  message: EventLedgerIngestQueueMessage,
  options: RecordEventLedgerMessageWithOwnerMutexOptions = {},
): Promise<EventLedgerIngestOutcome> => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    throw new EventLedgerOwnerAppendError({
      error: 'event_ledger_owner_khala_sync_db_missing',
    })
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const makeMutex = options.makeMutex ?? makePostgresMutex
  const makeSequenceStore =
    options.makeSequenceStore ?? makePostgresEventLedgerOwnerSequenceStore
  const makeStore = options.makeStore ?? makeEventLedgerStoreForEnv

  // The mutex holds a dedicated reserved connection for the lock's lifetime;
  // the sequence store uses a SEPARATE connection so its own queries never
  // wait on the lock connection. D1 (the ledger rows) is a separate HTTP path.
  const lockClient = await makeSqlClient(connectionString)
  const seqClient = await makeSqlClient(connectionString)

  try {
    const mutex = makeMutex(lockClient.sql as unknown as MutexSqlClient)
    const sequenceStore = makeSequenceStore(
      seqClient.sql as unknown as EventLedgerOwnerSequenceSql,
      message.ownerAgentUserId,
    )
    const store = makeStore(env)

    return await Effect.runPromise(
      mutex.withLock(
        eventLedgerOwnerLockName(message.ownerAgentUserId),
        Effect.tryPromise({
          try: () =>
            recordEventLedgerIngestMessage(
              { nowIso: options.nowIso, sequenceStore, store },
              message,
            ),
          catch: cause => new EventLedgerOwnerAppendError(cause),
        }),
      ),
    )
  } finally {
    await Promise.all([
      lockClient.end().catch(() => undefined),
      seqClient.end().catch(() => undefined),
    ])
  }
}

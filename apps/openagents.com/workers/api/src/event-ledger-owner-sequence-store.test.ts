import { describe, expect, test } from 'vitest'

import {
  EventLedgerIngestQueueMessage,
  EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION,
  type EventLedgerEntry,
  type EventLedgerIngestQueueMessage as EventLedgerIngestQueueMessageType,
  type EventLedgerOwnerSequenceReservation,
  type EventLedgerOwnerSequenceStore,
  type EventLedgerStore,
} from './event-ledger'
import {
  eventLedgerOwnerLockName,
  recordEventLedgerMessageWithOwnerMutex,
  type EventLedgerOwnerMutexEnv,
} from './event-ledger-owner-sequence-store'
import { makeMemoryMutex } from '@openagentsinc/oa-infra/mutex-memory'

const message = (
  overrides: Partial<{
    externalRef: string
    ownerAgentUserId: string
    source: 'github' | 'slack'
  }> = {},
): EventLedgerIngestQueueMessageType =>
  new EventLedgerIngestQueueMessage({
    schemaVersion: EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION,
    actorRef: 'github.user.octocat',
    contentRef: 'github.issue.1',
    eventType: 'issues.opened',
    externalRef: overrides.externalRef ?? 'github.delivery.abc',
    occurredAt: '2026-07-06T00:00:00.000Z',
    ownerAgentUserId: overrides.ownerAgentUserId ?? 'owner-a',
    ownerRef: 'agent.owner-a',
    payloadSummary: { action: 'opened' },
    receivedAt: '2026-07-06T00:00:01.000Z',
    source: overrides.source ?? 'github',
    sourceRefs: [overrides.externalRef ?? 'github.delivery.abc'],
    subjectRef: 'github.issue.1',
    trainingConsent: false,
  })

/** Per-owner in-memory sequence store — the shared state the Postgres table
 * gives all concurrent dispatches for one owner. */
class MemoryOwnerSequenceStore implements EventLedgerOwnerSequenceStore {
  private readonly reservations = new Map<
    string,
    EventLedgerOwnerSequenceReservation
  >()

  async markPersisted(orderingKey: string): Promise<void> {
    const existing = this.reservations.get(orderingKey)
    if (existing !== undefined) {
      this.reservations.set(orderingKey, { ...existing, persisted: true })
    }
  }

  async reserve(
    m: EventLedgerIngestQueueMessageType,
  ): Promise<EventLedgerOwnerSequenceReservation> {
    const orderingKey = `${m.source}:${m.externalRef}`
    const existing = this.reservations.get(orderingKey)
    if (existing !== undefined) {
      const duplicate = { ...existing, duplicate: true }
      this.reservations.set(orderingKey, duplicate)
      return duplicate
    }
    const reservation: EventLedgerOwnerSequenceReservation = {
      duplicate: false,
      orderingKey,
      orderingSequence: this.reservations.size + 1,
      persisted: false,
    }
    this.reservations.set(orderingKey, reservation)
    return reservation
  }
}

const dummySqlClient = async () => ({
  sql: (() => Promise.resolve([])) as never,
  end: async () => undefined,
})

const envWithDb: EventLedgerOwnerMutexEnv = {
  KHALA_SYNC_DB: { connectionString: 'postgres://test/db' },
} as unknown as EventLedgerOwnerMutexEnv

describe('event ledger owner mutex serialization', () => {
  test('serializes concurrent same-owner appends (no interleave) with distinct monotonic sequences', async () => {
    const sharedMutex = makeMemoryMutex()
    const storesByOwner = new Map<string, MemoryOwnerSequenceStore>()
    const inserted: Array<EventLedgerEntry> = []
    let active = 0
    let maxActive = 0

    const store: EventLedgerStore = {
      insertEntry: async ({ entryId, message: m, orderingSequence }) => {
        active += 1
        maxActive = Math.max(maxActive, active)
        // Yield so an unserialized competitor would be observed here.
        await new Promise(resolve => setTimeout(resolve, 5))
        active -= 1
        const entry = {
          entryId,
          orderingSequence,
          ownerAgentUserId: m.ownerAgentUserId,
        } as unknown as EventLedgerEntry
        inserted.push(entry)
        return entry
      },
      listOwnerEntries: async () => [],
      readOwnerEntry: async () => undefined,
      updateHandledState: async () => undefined,
    }

    const options = {
      makeSqlClient: dummySqlClient,
      makeMutex: () => sharedMutex,
      makeSequenceStore: (_sql: unknown, owner: string) => {
        const existing = storesByOwner.get(owner)
        if (existing !== undefined) {
          return existing
        }
        const created = new MemoryOwnerSequenceStore()
        storesByOwner.set(owner, created)
        return created
      },
      makeStore: () => store,
    }

    const outcomes = await Promise.all([
      recordEventLedgerMessageWithOwnerMutex(
        envWithDb,
        message({ externalRef: 'github.delivery.1' }),
        options,
      ),
      recordEventLedgerMessageWithOwnerMutex(
        envWithDb,
        message({ externalRef: 'github.delivery.2' }),
        options,
      ),
      recordEventLedgerMessageWithOwnerMutex(
        envWithDb,
        message({ externalRef: 'github.delivery.3' }),
        options,
      ),
    ])

    // Same-owner critical sections never overlapped.
    expect(maxActive).toBe(1)
    // Distinct, gapless per-owner sequences 1..3.
    const sequences = outcomes.map(o => o.orderingSequence).sort((a, b) => a - b)
    expect(sequences).toEqual([1, 2, 3])
    expect(inserted).toHaveLength(3)
    expect(outcomes.every(o => o.persisted)).toBe(true)
  })

  test('different owners get independent per-owner sequences', async () => {
    const sharedMutex = makeMemoryMutex()
    const storesByOwner = new Map<string, MemoryOwnerSequenceStore>()

    const store: EventLedgerStore = {
      insertEntry: async ({ entryId, message: m, orderingSequence }) =>
        ({
          entryId,
          orderingSequence,
          ownerAgentUserId: m.ownerAgentUserId,
        }) as unknown as EventLedgerEntry,
      listOwnerEntries: async () => [],
      readOwnerEntry: async () => undefined,
      updateHandledState: async () => undefined,
    }

    const options = {
      makeSqlClient: dummySqlClient,
      makeMutex: () => sharedMutex,
      makeSequenceStore: (_sql: unknown, owner: string) => {
        const existing = storesByOwner.get(owner)
        if (existing !== undefined) {
          return existing
        }
        const created = new MemoryOwnerSequenceStore()
        storesByOwner.set(owner, created)
        return created
      },
      makeStore: () => store,
    }

    const [a, b] = await Promise.all([
      recordEventLedgerMessageWithOwnerMutex(
        envWithDb,
        message({ ownerAgentUserId: 'owner-a', externalRef: 'd.a' }),
        options,
      ),
      recordEventLedgerMessageWithOwnerMutex(
        envWithDb,
        message({ ownerAgentUserId: 'owner-b', externalRef: 'd.b' }),
        options,
      ),
    ])

    expect(a.orderingSequence).toBe(1)
    expect(b.orderingSequence).toBe(1)
    expect(a.ownerAgentUserId).toBe('owner-a')
    expect(b.ownerAgentUserId).toBe('owner-b')
  })

  test('redelivered message dedups to the same sequence (idempotent)', async () => {
    const sharedMutex = makeMemoryMutex()
    const sequenceStore = new MemoryOwnerSequenceStore()

    const store: EventLedgerStore = {
      insertEntry: async ({ entryId, message: m, orderingSequence }) =>
        ({
          entryId,
          orderingSequence,
          ownerAgentUserId: m.ownerAgentUserId,
        }) as unknown as EventLedgerEntry,
      listOwnerEntries: async () => [],
      readOwnerEntry: async () => undefined,
      updateHandledState: async () => undefined,
    }

    const options = {
      makeSqlClient: dummySqlClient,
      makeMutex: () => sharedMutex,
      makeSequenceStore: () => sequenceStore,
      makeStore: () => store,
    }

    const first = await recordEventLedgerMessageWithOwnerMutex(
      envWithDb,
      message({ externalRef: 'github.delivery.dup' }),
      options,
    )
    const again = await recordEventLedgerMessageWithOwnerMutex(
      envWithDb,
      message({ externalRef: 'github.delivery.dup' }),
      options,
    )

    expect(first.duplicate).toBe(false)
    expect(again.duplicate).toBe(true)
    expect(again.orderingSequence).toBe(first.orderingSequence)
  })

  test('throws when KHALA_SYNC_DB is not configured', async () => {
    await expect(
      recordEventLedgerMessageWithOwnerMutex(
        {} as unknown as EventLedgerOwnerMutexEnv,
        message(),
        { makeSqlClient: dummySqlClient },
      ),
    ).rejects.toThrow(/append failed|khala_sync_db/i)
  })

  test('lock name is namespaced by owner', () => {
    expect(eventLedgerOwnerLockName('owner-x')).toBe(
      'event-ledger-owner:owner-x',
    )
  })
})

// ---------------------------------------------------------------------------
// CFG-17 (#8533): the append writes the ledger row to Postgres, not D1.
//
// This exercises the REAL defaults — the Postgres owner-sequence store AND the
// Postgres event_ledger_entries store — against one in-memory fake Postgres
// client shared by both (the same KHALA_SYNC_DB connection the code shares),
// with the memory mutex standing in for the advisory lock. It proves the
// append completes on Postgres end-to-end: a row lands in event_ledger_entries,
// persisted_at flips on the reservation, `persisted: true` is returned, and a
// redelivery dedups idempotently with no second ledger row.
// ---------------------------------------------------------------------------

type FakeRow = Record<string, unknown>

/** One in-memory Postgres double backing BOTH event_ledger_owner_order and
 * event_ledger_entries via the same tagged-template `sql` seam postgres.js
 * exposes. Dispatches on the reconstructed query text + positional params. */
const makeFakeKhalaSyncSql = () => {
  const orders: Array<{
    owner: string
    key: string
    seq: number
    persisted_at: string | null
  }> = []
  const entries: Array<FakeRow> = []

  const sql = (async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  ): Promise<Array<FakeRow>> => {
    const q = strings.join(' ')
    const v = values as ReadonlyArray<unknown>

    if (q.includes('event_ledger_owner_order')) {
      if (q.includes('INSERT INTO event_ledger_owner_order')) {
        const [owner, key, seq] = v as [string, string, number]
        orders.push({ owner, key, seq: Number(seq), persisted_at: null })
        return []
      }
      if (q.includes('UPDATE event_ledger_owner_order')) {
        const [persistedAt, owner, key] = v as [string, string, string]
        const row = orders.find(o => o.owner === owner && o.key === key)
        if (row !== undefined) {
          row.persisted_at = persistedAt
        }
        return []
      }
      if (q.includes('MAX(ordering_sequence)')) {
        const [owner] = v as [string]
        const max = orders
          .filter(o => o.owner === owner)
          .reduce((m, o) => Math.max(m, o.seq), 0)
        return [{ next_sequence: max + 1 }]
      }
      const [owner, key] = v as [string, string]
      const row = orders.find(o => o.owner === owner && o.key === key)
      return row === undefined
        ? []
        : [{ ordering_sequence: row.seq, persisted_at: row.persisted_at }]
    }

    if (q.includes('INSERT INTO event_ledger_entries')) {
      const [
        entry_id,
        owner_agent_user_id,
        owner_ref,
        source,
        external_ref,
        actor_ref,
        content_ref,
        subject_ref,
        event_type,
        source_refs_json,
        payload_summary_json,
        occurred_at,
        received_at,
        ordering_key,
        ordering_sequence,
        created_at,
        updated_at,
      ] = v as Array<unknown>
      const duplicate = entries.some(
        r =>
          r.owner_agent_user_id === owner_agent_user_id &&
          r.source === source &&
          r.external_ref === external_ref,
      )
      if (!duplicate) {
        entries.push({
          entry_id,
          owner_agent_user_id,
          owner_ref,
          source,
          external_ref,
          actor_ref,
          content_ref,
          subject_ref,
          event_type,
          source_refs_json,
          payload_summary_json,
          occurred_at,
          received_at,
          ordering_key,
          // bigint comes back from postgres.js as a string; keep it that way so
          // the store's Number() coercion is exercised.
          ordering_sequence: String(ordering_sequence),
          handled_state: 'open',
          handled_by_run_id: null,
          handled_by_definition_id: null,
          handled_at: null,
          handled_reason_ref: null,
          training_consent: 0,
          created_at,
          updated_at,
        })
      }
      return []
    }

    if (q.includes('SELECT * FROM event_ledger_entries')) {
      const [owner, source, external] = v as [string, string, string]
      return entries
        .filter(
          r =>
            r.owner_agent_user_id === owner &&
            r.source === source &&
            r.external_ref === external,
        )
        .slice(0, 1)
    }

    return []
  }) as unknown as (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  ) => Promise<Array<FakeRow>>

  return { sql, orders, entries }
}

describe('event ledger owner append writes to Postgres (CFG-17 #8533)', () => {
  test('append persists the ledger row in Postgres and flips persisted_at', async () => {
    const fake = makeFakeKhalaSyncSql()
    const options = {
      makeSqlClient: async () => ({
        sql: fake.sql as never,
        end: async () => undefined,
      }),
      makeMutex: () => makeMemoryMutex(),
    }

    const outcome = await recordEventLedgerMessageWithOwnerMutex(
      envWithDb,
      message({ externalRef: 'github.delivery.pg-1' }),
      options,
    )

    expect(outcome.persisted).toBe(true)
    expect(outcome.duplicate).toBe(false)
    expect(outcome.orderingSequence).toBe(1)

    // The ledger row landed in Postgres event_ledger_entries.
    expect(fake.entries).toHaveLength(1)
    expect(fake.entries[0]?.entry_id).toBe(outcome.entryId)
    expect(fake.entries[0]?.source).toBe('github')
    expect(fake.entries[0]?.external_ref).toBe('github.delivery.pg-1')

    // The reservation's persisted_at flipped (append proof), on Postgres.
    expect(fake.orders).toHaveLength(1)
    expect(fake.orders[0]?.persisted_at).not.toBeNull()
  })

  test('redelivery dedups against Postgres with no second ledger row', async () => {
    const fake = makeFakeKhalaSyncSql()
    const options = {
      makeSqlClient: async () => ({
        sql: fake.sql as never,
        end: async () => undefined,
      }),
      makeMutex: () => makeMemoryMutex(),
    }

    const first = await recordEventLedgerMessageWithOwnerMutex(
      envWithDb,
      message({ externalRef: 'github.delivery.pg-dup' }),
      options,
    )
    const again = await recordEventLedgerMessageWithOwnerMutex(
      envWithDb,
      message({ externalRef: 'github.delivery.pg-dup' }),
      options,
    )

    expect(first.duplicate).toBe(false)
    expect(again.duplicate).toBe(true)
    expect(again.orderingSequence).toBe(first.orderingSequence)
    expect(again.persisted).toBe(true)
    // No duplicate ledger row was written.
    expect(fake.entries).toHaveLength(1)
  })
})

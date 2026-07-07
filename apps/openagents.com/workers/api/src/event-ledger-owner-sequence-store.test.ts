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

// KS-8.10 remainder (#8338): forum remainder repository CONTRACT suite.
//
// Two layers, one behavioral spec:
//
//  1. `ForumRemainderWriteStore` contract — the row seam's converge
//     semantics run identically against BOTH implementations:
//     - D1: `makeD1ForumRemainderWriteStore` over real SQLite (node:sqlite).
//     - Postgres: `makePostgresForumRemainderStore` over a throwaway local
//       Postgres, schema from khala-sync-server migration 0024. Skipped when
//       no local Postgres binaries exist.
//
//  2. END-TO-END mirror fidelity — the REAL private-message + notification
//     repository writers run UNCHANGED through the composed
//     `wrapForumRemainderMirroring` database (SQLite as D1 authority, the
//     real Postgres store as the mirror); afterwards the private-message and
//     notification tables are row-for-row IDENTICAL across both stores, with
//     ZERO unclassified-write diagnostics — and no message content ever
//     appears in a diagnostic.

import {
  FORUM_REMAINDER_TABLE_COLUMNS,
  FORUM_REMAINDER_TABLE_PK,
  normalizeForumContentValue,
  type ForumRemainderTable,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  makeD1ForumRemainderWriteStore,
  wrapForumRemainderMirroring,
  makePostgresForumRemainderStore,
  type ForumRemainderRow,
  type ForumRemainderWriteStore,
} from './forum-remainder-store'
import {
  addForumPrivateMessage,
  createForumPrivateMessageThread,
  recordForumNotificationRead,
} from './repository'
import { FORUM_REMAINDER_D1_SCHEMA, makeSqliteD1 } from '../test/sqlite-d1'

const T0 = '2026-07-04T00:00:00.000Z'

const projection = {
  classificationCaveatRef: 'classification.public_forum_projection',
  customerSafe: true,
  dataClassification: 'public',
  excludedPrivateRefs: [],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.public.v1',
  safeArtifactRefs: ['artifact.forum.contract'],
  safeReceiptRefs: [],
  trustTier: 'reviewed',
} as never

// ---------------------------------------------------------------------------
// Layer 1: write-store contract (both implementations)
// ---------------------------------------------------------------------------

type ContractHarness = Readonly<{
  store: ForumRemainderWriteStore
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

const workRequestRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): ForumRemainderRow => ({
  archived_at: null,
  budget_msats: 1000000,
  budget_sats: 1000,
  created_at: T0,
  deadline_ref: 'deadline.forum.wr',
  first_post_id: `post_wr_${n}`,
  id: `store_wr_${n}`,
  idempotency_key: `store-wr-key-${n}`,
  job_event_id: `job_${n}`,
  job_event_kind: 5934,
  job_result_kind: 6934,
  objective_ref: 'objective.forum.wr',
  public_projection_json: '{}',
  quote_count: 0,
  relay_url: 'wss://relay.example',
  repository_refs_json: '[]',
  required_capability_refs_json: '[]',
  requester_actor_ref: 'agent_raynor',
  state: 'open',
  title: `Work request ${n}`,
  topic_id: `topic_wr_${n}`,
  updated_at: T0,
  verification_command_ref: 'verify.forum.wr',
  ...overrides,
})

const specContractSuite = (harness: () => ContractHarness) => {
  test('upsertRows converges on the PK and is idempotent', async () => {
    const { query, store } = harness()
    expect(
      await store.upsertRows('forum_work_requests', [
        workRequestRow(1),
        workRequestRow(2),
      ]),
    ).toBe(2)
    expect(
      await store.upsertRows('forum_work_requests', [
        workRequestRow(1),
        workRequestRow(2),
      ]),
    ).toBe(2)
    const counted = await query(
      `SELECT COUNT(*) AS total FROM forum_work_requests WHERE id LIKE 'store_wr_%'`,
    )
    expect(Number(counted[0]?.['total'])).toBe(2)

    await store.upsertRows('forum_work_requests', [
      workRequestRow(1, { quote_count: 4, state: 'settled' }),
    ])
    const rows = await query(
      `SELECT state, quote_count FROM forum_work_requests WHERE id = 'store_wr_1'`,
    )
    expect(rows[0]?.['state']).toBe('settled')
    expect(Number(rows[0]?.['quote_count'])).toBe(4)
  })

  test('idempotency dedupe key ports exactly: same key on a new id rejects', async () => {
    const { store } = harness()
    await store.upsertRows('forum_work_requests', [workRequestRow(3)])
    await expect(
      store.upsertRows('forum_work_requests', [
        workRequestRow(4, { idempotency_key: 'store-wr-key-3' }),
      ]),
    ).rejects.toThrow()
  })
}

describe('forum remainder write-store contract — D1 (real SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1> | undefined
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(FORUM_REMAINDER_D1_SCHEMA)
    harness = {
      query: async sql =>
        (await sqlite!.db.prepare(sql).all<Record<string, unknown>>())
          .results ?? [],
      store: makeD1ForumRemainderWriteStore(sqlite.db),
    }
  })

  afterAll(() => {
    sqlite?.close()
  })

  specContractSuite(() => harness)
})

const MIGRATION_0024 = path.resolve(
  __dirname,
  '../../../../../../packages/khala-sync-server/migrations/0026_forum_remainder.sql',
)

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'forum remainder write-store contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE forum_remainder_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('forum_remainder_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0024, 'utf8'))
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresForumRemainderStore({
          acquireSql: () =>
            Promise.resolve({
              end: () => Promise.resolve(),
              sql: raw as never,
            }),
        }),
      }
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    specContractSuite(() => harness)
  },
)

// ---------------------------------------------------------------------------
// Layer 2: end-to-end mirror fidelity through the REAL private writers
// ---------------------------------------------------------------------------

const projectRow = (
  table: ForumRemainderTable,
  row: Record<string, unknown>,
): Record<string, string | null> =>
  Object.fromEntries(
    FORUM_REMAINDER_TABLE_COLUMNS[table].map(column => {
      const value = normalizeForumContentValue(row[column])
      return [column, value === null ? null : String(value)]
    }),
  )

describe.skipIf(!hasLocalPostgres())(
  'forum remainder private writers mirror byte-faithfully into Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined
    let db: D1Database
    const diagnostics: Array<string> = []

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE forum_remainder_mirror')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('forum_remainder_mirror'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0024, 'utf8'))

      sqlite = makeSqliteD1()
      sqlite.exec(FORUM_REMAINDER_D1_SCHEMA)

      db = wrapForumRemainderMirroring(
        sqlite.db,
        {
          KHALA_SYNC_DB: { connectionString: 'postgres://mirror' },
          OPENAGENTS_DB: sqlite.db,
        },
        {
          log: (event, fields) => diagnostics.push(`${event}:${fields.op}`),
          makeSqlClient: () =>
            Promise.resolve({
              end: () => Promise.resolve(),
              sql: raw as never,
            }),
        },
      )
    }, 120_000)

    afterAll(async () => {
      sqlite?.close()
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    const expectConverged = async (
      table: ForumRemainderTable,
    ): Promise<void> => {
      const pk = FORUM_REMAINDER_TABLE_PK[table]
      const d1Rows = (
        (await sqlite!.db
          .prepare(`SELECT * FROM ${table}`)
          .all<Record<string, unknown>>()).results ?? []
      )
        .map(row => projectRow(table, row))
        .sort((a, b) => String(a[pk]).localeCompare(String(b[pk])))
      const pgRows = (await (client as PgClient).unsafe(`SELECT * FROM ${table}`))
        .map(row => projectRow(table, row))
        .sort((a, b) => String(a[pk]).localeCompare(String(b[pk])))
      expect(pgRows, `table ${table}`).toEqual(d1Rows)
    }

    test('private-message + notification writes converge both stores', async () => {
      await Effect.runPromise(
        createForumPrivateMessageThread(db, {
          createdByActorRef: 'agent_a',
          id: 'thread_e2e_1',
          participantRefs: ['agent_a', 'agent_b'],
          slug: 'e2e-thread-1',
          subject: 'Contract thread',
        }),
      )
      await Effect.runPromise(
        addForumPrivateMessage(db, {
          contentRef: 'content.forum.pm.e2e.1',
          id: 'msg_e2e_1',
          publicProjection: projection,
          recipientActorRef: 'agent_b',
          senderActorRef: 'agent_a',
          threadId: 'thread_e2e_1',
        }),
      )
      await Effect.runPromise(
        addForumPrivateMessage(db, {
          contentRef: 'content.forum.pm.e2e.2',
          id: 'msg_e2e_2',
          publicProjection: projection,
          recipientActorRef: 'agent_a',
          senderActorRef: 'agent_b',
          threadId: 'thread_e2e_1',
        }),
      )
      await Effect.runPromise(
        recordForumNotificationRead(db, {
          actorRef: 'agent_a',
          id: 'read_e2e_1',
          idempotencyKey: 'e2e-read-1',
          notificationId: 'notif_e2e_1',
          readAt: T0,
        }),
      )
      // Duplicate (actor, notification): D1 discards it, no phantom mirror.
      await Effect.runPromise(
        recordForumNotificationRead(db, {
          actorRef: 'agent_a',
          id: 'read_e2e_dupe',
          idempotencyKey: 'e2e-read-dupe',
          notificationId: 'notif_e2e_1',
          readAt: T0,
        }),
      )

      expect(diagnostics).toEqual([])

      await expectConverged('forum_private_message_threads')
      await expectConverged('forum_private_messages')
      await expectConverged('forum_notification_reads')

      const thread = await (client as PgClient).unsafe(
        `SELECT message_count, latest_message_id FROM forum_private_message_threads WHERE id = 'thread_e2e_1'`,
      )
      expect(Number(thread[0]?.['message_count'])).toBe(2)
      expect(thread[0]?.['latest_message_id']).toBe('msg_e2e_2')
      const reads = await (client as PgClient).unsafe(
        `SELECT COUNT(*) AS total FROM forum_notification_reads`,
      )
      expect(Number(reads[0]?.['total'])).toBe(1)
    }, 120_000)
  },
)

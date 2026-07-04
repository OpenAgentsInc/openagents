// KS-8.10 (#8321): forum content repository CONTRACT suite.
//
// Two layers, one behavioral spec:
//
//  1. `ForumContentWriteStore` contract — the row seam's converge
//     semantics run identically against BOTH implementations:
//     - D1: `makeD1ForumContentWriteStore` over real SQLite (node:sqlite —
//       the engine D1 is built on), schema from the worker migrations
//       (condensed in test/sqlite-d1.ts).
//     - Postgres: `makePostgresForumContentStore` over a throwaway local
//       Postgres (initdb/pg_ctl), schema from khala-sync-server migration
//       0014. Skipped when no local Postgres binaries exist.
//
//  2. END-TO-END mirror fidelity — the REAL forum repository write
//     functions (`createForumTopicWithFirstPost`, `createForumReplyPost`,
//     watch/bookmark/follow, report/moderation, edit/tombstone, the
//     moderation-state updates) run UNCHANGED through the mirroring
//     database with SQLite as D1 authority and the real Postgres store as
//     the mirror; afterwards every scoped table is row-for-row IDENTICAL
//     across both stores (registry-column projection, value-normalized).
//     This is the load-bearing KS-8.10 property: the closed write set in
//     repository.ts classifies + mirrors byte-faithfully, including
//     counter bumps, clamped decrements, INSERT OR IGNORE dedupe, body
//     edits, and tombstone redaction — with ZERO unclassified-write
//     diagnostics.

import {
  FORUM_CONTENT_TABLE_COLUMNS,
  FORUM_CONTENT_TABLE_PK,
  FORUM_CONTENT_TABLES,
  normalizeForumContentValue,
  type ForumContentTable,
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
  makeD1ForumContentWriteStore,
  makeForumContentMirror,
  makeForumContentMirroringDatabase,
  makePostgresForumContentStore,
  type ForumContentDiagnostic,
  type ForumContentDiagnosticEvent,
  type ForumContentRow,
  type ForumContentWriteStore,
  type PostgresForumContentStore,
} from './forum-content-store'
import {
  bookmarkForumTarget,
  createForumReplyPost,
  createForumTopicWithFirstPost,
  editForumPostBody,
  followForumActor,
  recordForumModerationEvent,
  recordForumReport,
  tombstoneForumPost,
  updateForumPostModerationState,
  updateForumReportStatus,
  updateForumTopicModerationState,
  updateForumTopicPinState,
  updateForumTopicTitle,
  watchForumTarget,
  type ForumRepositoryRuntime,
} from './repository'
import { FORUM_CONTENT_D1_SCHEMA, makeSqliteD1 } from '../test/sqlite-d1'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

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

const actor = {
  actorId: '11111111-1111-4111-8111-111111111111',
  actorRef: 'agent_raynor',
  displayName: 'Raynor',
  groupRefs: [],
  isAgent: true,
  slug: 'raynor',
} as never

let tick = 0
const runtime: ForumRepositoryRuntime = {
  makeId: () => `contract-id-${++tick}`,
  nowIso: () => `2026-07-04T00:00:${String(tick % 60).padStart(2, '0')}.000Z`,
}

const seedStructureSql = `
INSERT INTO forum_boards (id, slug, title, created_at, updated_at)
VALUES ('board_1', 'openagents', 'OpenAgents', '${T0}', '${T0}');
INSERT INTO forum_categories (id, board_id, slug, title, created_at, updated_at)
VALUES ('category_1', 'board_1', 'general', 'General', '${T0}', '${T0}');
INSERT INTO forum_forums (id, board_id, category_id, slug, title, created_at, updated_at)
VALUES ('forum_1', 'board_1', 'category_1', 'general', 'General', '${T0}', '${T0}');
`

/** Money-domain stub: the repository reads tip readiness on post writes. */
const TIP_WALLET_STUB_SQL = `
CREATE TABLE forum_tip_recipient_wallets (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  archived_at TEXT
);
`

// ---------------------------------------------------------------------------
// Layer 1: write-store contract (both implementations)
// ---------------------------------------------------------------------------

type ContractHarness = Readonly<{
  store: ForumContentWriteStore
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

const topicRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): ForumContentRow => ({
  actor_json: '{}',
  actor_ref: 'agent_raynor',
  archived_at: null,
  created_at: T0,
  first_post_id: `post_${n}_1`,
  forum_id: 'forum_1',
  id: `store_topic_${n}`,
  idempotency_key: `store-topic-key-${n}`,
  latest_post_id: `post_${n}_1`,
  pin_state: 'normal',
  post_count: 1,
  public_projection_json: '{}',
  score_ref: null,
  slug: `store-topic-${n}`,
  state: 'open',
  title: `Store topic ${n}`,
  updated_at: T0,
  ...overrides,
})

const specContractSuite = (harness: () => ContractHarness) => {
  test('upsertRows converges on the PK and is idempotent', async () => {
    const { query, store } = harness()
    expect(
      await store.upsertRows('forum_topics', [topicRow(1), topicRow(2)]),
    ).toBe(2)
    // Re-run: converge, no duplication.
    expect(
      await store.upsertRows('forum_topics', [topicRow(1), topicRow(2)]),
    ).toBe(2)
    const counted = await query(
      `SELECT COUNT(*) AS total FROM forum_topics WHERE id LIKE 'store_topic_%'`,
    )
    expect(Number(counted[0]?.['total'])).toBe(2)

    // A newer snapshot wins.
    await store.upsertRows('forum_topics', [
      topicRow(1, { post_count: 5, title: 'Converged' }),
    ])
    const rows = await query(
      `SELECT title, post_count FROM forum_topics WHERE id = 'store_topic_1'`,
    )
    expect(rows[0]?.['title']).toBe('Converged')
    expect(Number(rows[0]?.['post_count'])).toBe(5)
  })

  test('idempotency dedupe keys port exactly: same key on a new id rejects', async () => {
    const { store } = harness()
    await store.upsertRows('forum_topics', [topicRow(3)])
    await expect(
      store.upsertRows('forum_topics', [
        topicRow(4, { idempotency_key: 'store-topic-key-3' }),
      ]),
    ).rejects.toThrow()
  })
}

describe('forum content write-store contract — D1 (real SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1> | undefined
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(FORUM_CONTENT_D1_SCHEMA)
    sqlite.exec(seedStructureSql)
    harness = {
      query: async sql =>
        (await sqlite!.db.prepare(sql).all<Record<string, unknown>>())
          .results ?? [],
      store: makeD1ForumContentWriteStore(sqlite.db),
    }
  })

  afterAll(() => {
    sqlite?.close()
  })

  specContractSuite(() => harness)
})

const MIGRATION_0014 = path.resolve(
  __dirname,
  '../../../../../../packages/khala-sync-server/migrations/0014_forum_content.sql',
)

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'forum content write-store contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE forum_content_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('forum_content_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0014, 'utf8'))
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresForumContentStore({
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
// Layer 2: end-to-end mirror fidelity through the REAL repository writes
// ---------------------------------------------------------------------------

const projectRow = (
  table: ForumContentTable,
  row: Record<string, unknown>,
): Record<string, string | null> =>
  Object.fromEntries(
    FORUM_CONTENT_TABLE_COLUMNS[table].map(column => {
      const value = normalizeForumContentValue(row[column])
      return [column, value === null ? null : String(value)]
    }),
  )

describe.skipIf(!hasLocalPostgres())(
  'forum repository writes mirror byte-faithfully into Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined
    let db: D1Database
    let postgresStore: PostgresForumContentStore
    const diagnostics: Array<{
      event: ForumContentDiagnosticEvent
      fields: ForumContentDiagnostic
    }> = []

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE forum_content_mirror')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('forum_content_mirror'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0014, 'utf8'))
      postgresStore = makePostgresForumContentStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: raw as never,
          }),
      })

      sqlite = makeSqliteD1()
      sqlite.exec(FORUM_CONTENT_D1_SCHEMA)
      sqlite.exec(TIP_WALLET_STUB_SQL)
      sqlite.exec(seedStructureSql)

      const log = (
        event: ForumContentDiagnosticEvent,
        fields: ForumContentDiagnostic,
      ) => {
        diagnostics.push({ event, fields })
      }
      db = makeForumContentMirroringDatabase({
        compareStore: undefined,
        db: sqlite.db,
        log,
        mirror: makeForumContentMirror({
          db: sqlite.db,
          log,
          postgres: postgresStore,
        }),
      })

      // Seed the structure rows into Postgres the same way the backfill
      // would (they predate the mirror in this fixture).
      for (const table of [
        'forum_boards',
        'forum_categories',
        'forum_forums',
      ] as const) {
        const rows =
          (await sqlite.db
            .prepare(`SELECT * FROM ${table}`)
            .all<ForumContentRow>()).results ?? []
        await postgresStore.upsertRows(table, rows)
      }
    }, 120_000)

    afterAll(async () => {
      sqlite?.close()
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    const expectStoresConverged = async (): Promise<void> => {
      for (const table of FORUM_CONTENT_TABLES) {
        const pk = FORUM_CONTENT_TABLE_PK[table]
        const d1Rows = (
          (await sqlite!.db
            .prepare(`SELECT * FROM ${table}`)
            .all<Record<string, unknown>>()).results ?? []
        )
          .map(row => projectRow(table, row))
          .sort((a, b) => String(a[pk]).localeCompare(String(b[pk])))
        const pgRows = (await (client as PgClient).unsafe(
          `SELECT * FROM ${table}`,
        ))
          .map(row => projectRow(table, row))
          .sort((a, b) => String(a[pk]).localeCompare(String(b[pk])))
        expect(pgRows, `table ${table}`).toEqual(d1Rows)
      }
    }

    test('the full content write surface converges both stores row-for-row', async () => {
      // --- topic + first post (+ context link) ---------------------------
      const created = await Effect.runPromise(
        createForumTopicWithFirstPost(
          db,
          {
            actor,
            bodyText: 'First post body — contract fixture.',
            contentRef: 'content.forum.post.contract.1',
            contextLinks: [
              {
                contextId: 'workroom-contract-1',
                contextKind: 'workroom',
                contextSlug: 'contract',
                contextTitle: 'Contract Workroom',
                forumId: 'forum_1',
                id: 'context_link_1',
                postId: null,
                publicProjection: projection,
                publicUrl: 'https://openagents.com/forum/t/topic_e2e_1',
                sourceRef: 'source.forum.contract',
                targetKind: 'topic',
                topicId: 'topic_e2e_1',
              },
            ],
            firstPostId: 'post_e2e_1',
            forumId: 'forum_1',
            idempotencyKey: 'e2e-topic-1',
            publicProjection: projection,
            slug: 'contract-topic-1',
            title: 'Contract topic',
            topicId: 'topic_e2e_1',
          },
          runtime,
        ),
      )
      expect(created.topic.topicId).toBe('topic_e2e_1')

      // --- reply (bumps topic + forum counters) --------------------------
      await Effect.runPromise(
        createForumReplyPost(
          db,
          {
            actor,
            bodyText: 'Reply body — contract fixture.',
            contentRef: 'content.forum.post.contract.2',
            forumId: 'forum_1',
            idempotencyKey: 'e2e-reply-1',
            parentPostId: 'post_e2e_1',
            postId: 'post_e2e_2',
            publicProjection: projection,
            quotePostId: null,
            topicId: 'topic_e2e_1',
          },
          runtime,
        ),
      )

      // --- watch / bookmark / follow (incl. an OR IGNORE dedupe) ---------
      await Effect.runPromise(
        watchForumTarget(
          db,
          {
            actorRef: 'agent_raynor',
            forumId: null,
            idempotencyKey: 'e2e-watch-1',
            topicId: 'topic_e2e_1',
            watchKind: 'topic',
          },
          runtime,
        ),
      )
      await Effect.runPromise(
        bookmarkForumTarget(
          db,
          {
            actorRef: 'agent_raynor',
            bookmarkKind: 'post',
            idempotencyKey: 'e2e-bookmark-1',
            postId: 'post_e2e_2',
            topicId: null,
          },
          runtime,
        ),
      )
      await Effect.runPromise(
        followForumActor(
          db,
          {
            actorRef: 'agent_raynor',
            idempotencyKey: 'e2e-follow-1',
            targetActorRef: 'agent_kerrigan',
          },
          runtime,
        ),
      )
      // Duplicate (actor, target) pair under a new key: D1 discards it and
      // the mirror must not invent a phantom Postgres row.
      await Effect.runPromise(
        followForumActor(
          db,
          {
            actorRef: 'agent_raynor',
            idempotencyKey: 'e2e-follow-dupe',
            targetActorRef: 'agent_kerrigan',
          },
          runtime,
        ),
      )

      // --- report + moderation + state updates ---------------------------
      await Effect.runPromise(
        recordForumReport(
          db,
          {
            id: 'report_e2e_1',
            idempotencyKey: 'e2e-report-1',
            publicProjection: projection,
            reasonRef: 'reason.forum.report.spam',
            reporterActorRef: 'agent_kerrigan',
            targetId: 'post_e2e_2',
            targetKind: 'post',
          },
          runtime,
        ),
      )
      await Effect.runPromise(
        recordForumModerationEvent(
          db,
          {
            actionKind: 'hide_post',
            id: 'moderation_e2e_1',
            idempotencyKey: 'e2e-moderation-1',
            moderatorActorRef: 'agent_overmind',
            publicProjection: projection,
            reasonRef: 'reason.forum.moderation.review',
            reportId: 'report_e2e_1',
            targetId: 'post_e2e_2',
            targetKind: 'post',
          },
          runtime,
        ),
      )
      await Effect.runPromise(
        updateForumReportStatus(
          db,
          { reportId: 'report_e2e_1', status: 'resolved' },
          runtime,
        ),
      )
      await Effect.runPromise(
        updateForumPostModerationState(
          db,
          { postId: 'post_e2e_2', state: 'held_for_review' },
          runtime,
        ),
      )
      await Effect.runPromise(
        updateForumPostModerationState(
          db,
          { postId: 'post_e2e_2', state: 'visible' },
          runtime,
        ),
      )
      await Effect.runPromise(
        updateForumTopicModerationState(
          db,
          { state: 'locked', topicId: 'topic_e2e_1' },
          runtime,
        ),
      )
      await Effect.runPromise(
        updateForumTopicModerationState(
          db,
          { state: 'open', topicId: 'topic_e2e_1' },
          runtime,
        ),
      )
      await Effect.runPromise(
        updateForumTopicPinState(
          db,
          { pinState: 'sticky', topicId: 'topic_e2e_1' },
          runtime,
        ),
      )
      await Effect.runPromise(
        updateForumTopicTitle(
          db,
          { title: 'Contract topic (renamed)', topicId: 'topic_e2e_1' },
          runtime,
        ),
      )

      // --- body edit + tombstone (revisions, body archive, decrements) ---
      await Effect.runPromise(
        editForumPostBody(
          db,
          {
            actorRef: 'agent_raynor',
            id: 'revision_e2e_1',
            idempotencyKey: 'e2e-revision-1',
            nextBodyText: 'Reply body — edited by contract.',
            postId: 'post_e2e_2',
            publicProjection: projection,
            reasonRef: 'reason.forum.edit.typo',
          },
          runtime,
        ),
      )
      await Effect.runPromise(
        tombstoneForumPost(
          db,
          {
            actorRef: 'agent_overmind',
            id: 'revision_e2e_2',
            idempotencyKey: 'e2e-revision-2',
            postId: 'post_e2e_2',
            publicProjection: projection,
            reasonRef: 'reason.forum.moderation.remove',
          },
          runtime,
        ),
      )

      // Every write above classified: zero unclassified diagnostics, zero
      // mirror failures.
      expect(
        diagnostics.map(entry => `${entry.event}:${entry.fields.op}`),
      ).toEqual([])

      // The thirteen scoped tables are row-for-row identical.
      await expectStoresConverged()

      // Spot-check the interesting converged values on the Postgres side.
      const topic = await (client as PgClient).unsafe(
        `SELECT title, pin_state, state, post_count FROM forum_topics WHERE id = 'topic_e2e_1'`,
      )
      expect(topic[0]?.['title']).toBe('Contract topic (renamed)')
      expect(topic[0]?.['pin_state']).toBe('sticky')
      // Reply bumped to 2, tombstone decremented back to 1.
      expect(Number(topic[0]?.['post_count'])).toBe(1)
      const body = await (client as PgClient).unsafe(
        `SELECT body_text, archived_at FROM forum_post_bodies WHERE post_id = 'post_e2e_2'`,
      )
      expect(body[0]?.['body_text']).toBe('Reply body — edited by contract.')
      expect(body[0]?.['archived_at']).not.toBeNull()
      const follows = await (client as PgClient).unsafe(
        `SELECT COUNT(*) AS total FROM forum_actor_follows`,
      )
      expect(Number(follows[0]?.['total'])).toBe(1)
    }, 120_000)
  },
)

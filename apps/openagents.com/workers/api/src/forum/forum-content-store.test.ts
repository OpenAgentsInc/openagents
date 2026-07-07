// KS-8.10 (#8321): forum content store — flags, statement classifier, and
// mirroring-database behavior.
//
// Load-bearing properties:
//   * flags parse per convention (dual-write default ON, reads default
//     'd1', typos never fail open into an unproven read path);
//   * the statement classifier resolves the PRIMARY KEY of every write
//     shape the forum repository actually issues (INSERT with literal
//     columns interleaved, INSERT OR IGNORE, UPDATE with binds in SET and
//     extra WHERE conditions, MAX() clamp decrements) and refuses loudly
//     (`unclassified-write`) rather than guessing;
//   * the mirroring database read-back-mirrors the EXACT post-write D1
//     row, skips dedupe-discarded INSERT OR IGNORE ids, never fails a
//     request on a Postgres error (fail-soft + typed diagnostic), and
//     passes non-scoped statements through untouched;
//   * compare mode shadow-reads scoped SELECTs, SERVES D1 always, and
//     logs mismatch/failed diagnostics with statement heads only;
//   * `forumContentDatabaseForEnv` degrades to the RAW database when no
//     binding / flags off, and `reads=postgres` defers (compare + one
//     diagnostic) instead of serving an unproven path.

import { describe, expect, test } from 'vitest'

import {
  classifyForumContentStatement,
  forumContentDatabaseForEnv,
  forumContentFlagsFromEnv,
  makeForumContentMirror,
  makeForumContentMirroringDatabase,
  makeDualWriteForumContentWriteStore,
  resolveForumContentPk,
  toPostgresPlaceholders,
  type ForumContentDiagnostic,
  type ForumContentDiagnosticEvent,
  type ForumContentRow,
  type ForumContentTable,
  type PostgresForumContentStore,
} from './forum-content-store'
import { FORUM_CONTENT_D1_SCHEMA, makeSqliteD1 } from '../test/sqlite-d1'

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

type LoggedDiagnostic = Readonly<{
  event: ForumContentDiagnosticEvent
  fields: ForumContentDiagnostic
}>

const makeLogCapture = () => {
  const events: Array<LoggedDiagnostic> = []
  return {
    events,
    log: (event: ForumContentDiagnosticEvent, fields: ForumContentDiagnostic) => {
      events.push({ event, fields })
    },
  }
}

type CapturedUpsert = Readonly<{
  table: ForumContentTable
  rows: ReadonlyArray<ForumContentRow>
}>

const makeFakePostgresStore = (
  options: Readonly<{
    failUpserts?: boolean
    queryRows?: (
      text: string,
      params: ReadonlyArray<unknown>,
    ) => Promise<ReadonlyArray<Record<string, unknown>>>
  }> = {},
) => {
  const upserts: Array<CapturedUpsert> = []
  const store: PostgresForumContentStore = {
    queryRows:
      options.queryRows ?? (() => Promise.reject(new Error('no query fake'))),
    upsertRows: (table, rows) => {
      if (options.failUpserts === true) {
        return Promise.reject(new Error('simulated postgres outage'))
      }
      upserts.push({ rows, table })
      return Promise.resolve(rows.length)
    },
  }
  return { store, upserts }
}

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

describe('forumContentFlagsFromEnv', () => {
  test('defaults: dual-write ON, reads d1', () => {
    expect(forumContentFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
  })

  test('off values disable dual-write', () => {
    for (const value of ['off', '0', 'false', 'disabled', 'NO']) {
      expect(
        forumContentFlagsFromEnv({ KHALA_SYNC_FORUM_DUAL_WRITE: value })
          .dualWrite,
      ).toBe(false)
    }
    expect(
      forumContentFlagsFromEnv({ KHALA_SYNC_FORUM_DUAL_WRITE: 'on' })
        .dualWrite,
    ).toBe(true)
  })

  test('read routing values parse; typos fall back to d1', () => {
    expect(
      forumContentFlagsFromEnv({ KHALA_SYNC_FORUM_READS: 'compare' }).reads,
    ).toBe('compare')
    expect(
      forumContentFlagsFromEnv({ KHALA_SYNC_FORUM_READS: 'POSTGRES' }).reads,
    ).toBe('postgres')
    expect(
      forumContentFlagsFromEnv({ KHALA_SYNC_FORUM_READS: 'postgress' }).reads,
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Statement classifier — pinned against the repository's real write shapes
// ---------------------------------------------------------------------------

describe('classifyForumContentStatement', () => {
  test('topic INSERT with interleaved literals resolves the id bind', () => {
    const classified = classifyForumContentStatement(
      `INSERT INTO forum_topics (
         id, idempotency_key, forum_id, actor_ref, actor_json, slug, title,
         first_post_id, latest_post_id, post_count, pin_state, state,
         score_ref, public_projection_json, created_at, updated_at
       )
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 'normal', 'open', NULL, ?, ?, ?)`,
    )
    expect(classified).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 0, kind: 'bind' },
      table: 'forum_topics',
    })
  })

  test('post-body INSERT keys post_id even with a literal content_kind', () => {
    const classified = classifyForumContentStatement(
      `INSERT INTO forum_post_bodies (
         post_id, content_kind, body_text, created_at, updated_at
       )
       VALUES (?, 'plain_text', ?, ?, ?)`,
    )
    expect(classified).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 0, kind: 'bind' },
      table: 'forum_post_bodies',
    })
  })

  test('INSERT OR IGNORE (watch/bookmark/follow/context-link shape) classifies', () => {
    const classified = classifyForumContentStatement(
      `INSERT OR IGNORE INTO forum_actor_follows (
         id, actor_ref, target_actor_ref, idempotency_key, created_at
       )
       VALUES (?, ?, ?, ?, ?)`,
    )
    expect(classified).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 0, kind: 'bind' },
      table: 'forum_actor_follows',
    })
  })

  test('UPDATE with SET binds and trailing WHERE conditions resolves the id bind', () => {
    const classified = classifyForumContentStatement(
      `UPDATE forum_forums
          SET topic_count = topic_count + 1,
              post_count = post_count + 1,
              latest_topic_id = ?,
              latest_post_id = ?,
              updated_at = ?
        WHERE id = ?
          AND archived_at IS NULL`,
    )
    expect(classified).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 3, kind: 'bind' },
      table: 'forum_forums',
    })
  })

  test('MAX() clamp decrement with zero SET binds resolves the id bind', () => {
    const classified = classifyForumContentStatement(
      `UPDATE forum_forums
          SET post_count = MAX(0, post_count - 1)
        WHERE id = ?
          AND archived_at IS NULL`,
    )
    expect(classified).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 0, kind: 'bind' },
      table: 'forum_forums',
    })
  })

  test('post-body UPDATE keys post_id, never a column merely ending in id', () => {
    const classified = classifyForumContentStatement(
      `UPDATE forum_post_bodies
          SET body_text = ?, updated_at = ?, archived_at = NULL
        WHERE post_id = ?`,
    )
    expect(classified).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 2, kind: 'bind' },
      table: 'forum_post_bodies',
    })
    // `target_id = ?` must NOT satisfy a pk of `id`.
    expect(
      classifyForumContentStatement(
        `UPDATE forum_reports SET status = ? WHERE target_id = ?`,
      ),
    ).toEqual({ kind: 'unclassified-write', table: 'forum_reports' })
  })

  test('report INSERT with literal status classifies on the id bind', () => {
    const classified = classifyForumContentStatement(
      `INSERT INTO forum_reports (
         id, idempotency_key, reporter_actor_ref, target_kind, target_id,
         reason_ref, status, public_projection_json, created_at, updated_at
       )
       VALUES (?, ?, ?, ?, ?, ?, 'open', ?, ?, ?)`,
    )
    expect(classified).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 0, kind: 'bind' },
      table: 'forum_reports',
    })
  })

  test('scoped DELETE and non-PK writes refuse loudly', () => {
    expect(
      classifyForumContentStatement(`DELETE FROM forum_posts WHERE id = ?`),
    ).toEqual({ kind: 'unclassified-write', table: 'forum_posts' })
    expect(
      classifyForumContentStatement(
        `UPDATE forum_posts SET state = ? WHERE topic_id = ?`,
      ),
    ).toEqual({ kind: 'unclassified-write', table: 'forum_posts' })
  })

  test('non-scoped writes and mixed reads pass through', () => {
    expect(
      classifyForumContentStatement(
        `INSERT INTO forum_private_message_threads (id, subject) VALUES (?, ?)`,
      ),
    ).toEqual({ kind: 'passthrough' })
    expect(
      classifyForumContentStatement(
        `INSERT INTO forum_receipts (id, receipt_ref) VALUES (?, ?)`,
      ),
    ).toEqual({ kind: 'passthrough' })
    expect(
      classifyForumContentStatement(
        `SELECT * FROM forum_posts p JOIN agent_registrations a ON a.ref = p.actor_ref`,
      ),
    ).toEqual({ kind: 'passthrough' })
  })

  test('scoped-only SELECTs are comparable', () => {
    expect(
      classifyForumContentStatement(
        `SELECT p.id, b.body_text
           FROM forum_posts p
           LEFT JOIN forum_post_bodies b ON b.post_id = p.id
          WHERE p.topic_id = ?
          ORDER BY p.post_number ASC`,
      ),
    ).toEqual({ kind: 'comparable-select' })
  })

  test('resolveForumContentPk reads binds and literals', () => {
    expect(
      resolveForumContentPk({ index: 2, kind: 'bind' }, ['a', 'b', 'c']),
    ).toBe('c')
    expect(resolveForumContentPk({ kind: 'literal', value: 'x' }, [])).toBe(
      'x',
    )
    expect(
      resolveForumContentPk({ index: 5, kind: 'bind' }, ['a']),
    ).toBeUndefined()
  })

  test('toPostgresPlaceholders converts outside string literals only', () => {
    expect(
      toPostgresPlaceholders(`SELECT * FROM forum_posts WHERE id = ? AND state = 'a?b' AND topic_id = ?`),
    ).toBe(
      `SELECT * FROM forum_posts WHERE id = $1 AND state = 'a?b' AND topic_id = $2`,
    )
  })
})

// ---------------------------------------------------------------------------
// Mirroring database
// ---------------------------------------------------------------------------

const T0 = '2026-07-04T00:00:00.000Z'

const seedForumStructure = (exec: (sql: string) => void): void => {
  exec(
    `INSERT INTO forum_boards (id, slug, title, created_at, updated_at)
     VALUES ('board_1', 'openagents', 'OpenAgents', '${T0}', '${T0}')`,
  )
  exec(
    `INSERT INTO forum_categories (id, board_id, slug, title, created_at, updated_at)
     VALUES ('category_1', 'board_1', 'general', 'General', '${T0}', '${T0}')`,
  )
  exec(
    `INSERT INTO forum_forums (id, board_id, category_id, slug, title, created_at, updated_at)
     VALUES ('forum_1', 'board_1', 'category_1', 'general', 'General', '${T0}', '${T0}')`,
  )
}

describe('makeForumContentMirroringDatabase', () => {
  test('scoped INSERT + UPDATE read-back-mirror the exact post-write D1 rows', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_CONTENT_D1_SCHEMA)
    seedForumStructure(sqlite.exec)
    const { log } = makeLogCapture()
    const fake = makeFakePostgresStore()
    const db = makeForumContentMirroringDatabase({
      compareStore: undefined,
      db: sqlite.db,
      log,
      mirror: makeForumContentMirror({
        db: sqlite.db,
        log,
        postgres: fake.store,
      }),
    })

    await db
      .prepare(
        `INSERT INTO forum_topics (
           id, idempotency_key, forum_id, actor_ref, actor_json, slug, title,
           first_post_id, latest_post_id, post_count, pin_state, state,
           score_ref, public_projection_json, created_at, updated_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 'normal', 'open', NULL, ?, ?, ?)`,
      )
      .bind(
        'topic_1',
        'topic-key-1',
        'forum_1',
        'agent_raynor',
        '{}',
        'topic-1',
        'Hello',
        'post_1',
        'post_1',
        '{}',
        T0,
        T0,
      )
      .run()

    expect(fake.upserts).toHaveLength(1)
    expect(fake.upserts[0]?.table).toBe('forum_topics')
    expect(fake.upserts[0]?.rows[0]?.['id']).toBe('topic_1')
    expect(fake.upserts[0]?.rows[0]?.['post_count']).toBe(1)
    expect(fake.upserts[0]?.rows[0]?.['pin_state']).toBe('normal')

    await db
      .prepare(
        `UPDATE forum_topics
            SET title = ?, updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind('Renamed', '2026-07-04T01:00:00.000Z', 'topic_1')
      .run()

    expect(fake.upserts).toHaveLength(2)
    expect(fake.upserts[1]?.rows[0]?.['title']).toBe('Renamed')

    sqlite.close()
  })

  test('INSERT OR IGNORE dedupe discard mirrors zero rows (no phantom id)', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_CONTENT_D1_SCHEMA)
    const { log } = makeLogCapture()
    const fake = makeFakePostgresStore()
    const db = makeForumContentMirroringDatabase({
      compareStore: undefined,
      db: sqlite.db,
      log,
      mirror: makeForumContentMirror({
        db: sqlite.db,
        log,
        postgres: fake.store,
      }),
    })

    const follow = (id: string, key: string) =>
      db
        .prepare(
          `INSERT OR IGNORE INTO forum_actor_follows (
             id, actor_ref, target_actor_ref, idempotency_key, created_at
           )
           VALUES (?, ?, ?, ?, ?)`,
        )
        .bind(id, 'agent_a', 'agent_b', key, T0)
        .run()

    await follow('follow_1', 'follow-key-1')
    // Same (actor, target) pair, new id: D1 discards the insert.
    await follow('follow_2', 'follow-key-2')

    expect(fake.upserts).toHaveLength(2)
    expect(fake.upserts[0]?.rows).toHaveLength(1)
    expect(fake.upserts[0]?.rows[0]?.['id']).toBe('follow_1')
    // The discarded id read back zero rows — nothing phantom mirrored.
    expect(fake.upserts[1]?.rows).toHaveLength(0)

    sqlite.close()
  })

  test('a postgres outage NEVER fails the D1 write; it logs the drift metric', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_CONTENT_D1_SCHEMA)
    const capture = makeLogCapture()
    const fake = makeFakePostgresStore({ failUpserts: true })
    const db = makeForumContentMirroringDatabase({
      compareStore: undefined,
      db: sqlite.db,
      log: capture.log,
      mirror: makeForumContentMirror({
        db: sqlite.db,
        log: capture.log,
        postgres: fake.store,
      }),
    })

    const result = await db
      .prepare(
        `INSERT INTO forum_reports (
           id, idempotency_key, reporter_actor_ref, target_kind, target_id,
           reason_ref, status, public_projection_json, created_at, updated_at
         )
         VALUES (?, ?, ?, ?, ?, ?, 'open', ?, ?, ?)`,
      )
      .bind(
        'report_1',
        'report-key-1',
        'agent_a',
        'post',
        'post_1',
        'reason.spam',
        '{}',
        T0,
        T0,
      )
      .run()

    expect(result.success).toBe(true)
    const d1Row = await sqlite.db
      .prepare(`SELECT id FROM forum_reports WHERE id = ?`)
      .bind('report_1')
      .first<{ id: string }>()
    expect(d1Row?.id).toBe('report_1')

    const driftEvents = capture.events.filter(
      entry => entry.event === 'khala_sync_forum_dual_write_failed',
    )
    expect(driftEvents).toHaveLength(1)
    expect(driftEvents[0]?.fields.op).toBe('mirror:forum_reports')
    expect(driftEvents[0]?.fields.refs).toEqual(['report_1'])
    // Diagnostics carry keys only — never body/title content.
    expect(JSON.stringify(driftEvents[0])).not.toContain('reason.spam')

    sqlite.close()
  })

  test('non-scoped writes pass through without mirroring or diagnostics', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_CONTENT_D1_SCHEMA)
    sqlite.exec(
      `CREATE TABLE forum_private_message_threads (id TEXT PRIMARY KEY, subject TEXT)`,
    )
    const capture = makeLogCapture()
    const fake = makeFakePostgresStore()
    const db = makeForumContentMirroringDatabase({
      compareStore: undefined,
      db: sqlite.db,
      log: capture.log,
      mirror: makeForumContentMirror({
        db: sqlite.db,
        log: capture.log,
        postgres: fake.store,
      }),
    })

    await db
      .prepare(
        `INSERT INTO forum_private_message_threads (id, subject) VALUES (?, ?)`,
      )
      .bind('thread_1', 'hi')
      .run()

    expect(fake.upserts).toHaveLength(0)
    expect(capture.events).toHaveLength(0)
    sqlite.close()
  })

  test('scoped DELETE logs unclassified-write instead of guessing', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_CONTENT_D1_SCHEMA)
    const capture = makeLogCapture()
    const fake = makeFakePostgresStore()
    const db = makeForumContentMirroringDatabase({
      compareStore: undefined,
      db: sqlite.db,
      log: capture.log,
      mirror: makeForumContentMirror({
        db: sqlite.db,
        log: capture.log,
        postgres: fake.store,
      }),
    })

    await db
      .prepare(`DELETE FROM forum_watches WHERE id = ?`)
      .bind('watch_1')
      .run()

    expect(fake.upserts).toHaveLength(0)
    expect(
      capture.events.filter(
        entry => entry.event === 'khala_sync_forum_write_unclassified',
      ),
    ).toHaveLength(1)
    sqlite.close()
  })

  test('compare mode serves D1, logs mismatches, and survives postgres read failures', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_CONTENT_D1_SCHEMA)
    seedForumStructure(sqlite.exec)
    sqlite.exec(
      `INSERT INTO forum_topics (
         id, idempotency_key, forum_id, actor_ref, slug, title,
         first_post_id, latest_post_id, created_at, updated_at
       )
       VALUES ('topic_1', 'k1', 'forum_1', 'agent_a', 't-1', 'T', 'p1', 'p1', '${T0}', '${T0}')`,
    )
    const capture = makeLogCapture()
    let mode: 'match' | 'mismatch' | 'fail' = 'match'
    const fake = makeFakePostgresStore({
      queryRows: async (text) => {
        if (mode === 'fail') throw new Error('postgres read down')
        expect(text).toContain('$1')
        const row = await sqlite.db
          .prepare(`SELECT * FROM forum_topics WHERE id = ?`)
          .bind('topic_1')
          .first<Record<string, unknown>>()
        if (row === null) return []
        return mode === 'mismatch' ? [{ ...row, title: 'DRIFTED' }] : [row]
      },
    })
    const db = makeForumContentMirroringDatabase({
      compareStore: fake.store,
      db: sqlite.db,
      log: capture.log,
      mirror: undefined,
    })

    const read = () =>
      db
        .prepare(`SELECT * FROM forum_topics WHERE id = ?`)
        .bind('topic_1')
        .first<Record<string, unknown>>()

    expect((await read())?.['title']).toBe('T')
    expect(capture.events).toHaveLength(0)

    mode = 'mismatch'
    expect((await read())?.['title']).toBe('T') // D1 is always served
    expect(
      capture.events.filter(
        entry => entry.event === 'khala_sync_forum_read_compare_mismatch',
      ),
    ).toHaveLength(1)

    mode = 'fail'
    expect((await read())?.['title']).toBe('T')
    expect(
      capture.events.filter(
        entry => entry.event === 'khala_sync_forum_read_compare_failed',
      ),
    ).toHaveLength(1)

    sqlite.close()
  })
})

// ---------------------------------------------------------------------------
// Dual-write row store wrapper
// ---------------------------------------------------------------------------

describe('makeDualWriteForumContentWriteStore', () => {
  const row: ForumContentRow = {
    actor_ref: 'agent_a',
    archived_at: null,
    created_at: T0,
    id: 'follow_1',
    idempotency_key: 'k1',
    target_actor_ref: 'agent_b',
  }

  test('mirrors after the d1 write; postgres failure is fail-soft + logged', async () => {
    const capture = makeLogCapture()
    const calls: Array<string> = []
    const d1 = {
      upsertRows: (table: ForumContentTable) => {
        calls.push(`d1:${table}`)
        return Promise.resolve(1)
      },
    }
    const failing = {
      upsertRows: () => Promise.reject(new Error('down')),
    }
    const store = makeDualWriteForumContentWriteStore({
      d1,
      flags: { dualWrite: true, reads: 'd1' },
      log: capture.log,
      postgres: failing,
    })
    expect(await store.upsertRows('forum_actor_follows', [row])).toBe(1)
    expect(calls).toEqual(['d1:forum_actor_follows'])
    expect(capture.events[0]?.event).toBe('khala_sync_forum_dual_write_failed')
    expect(capture.events[0]?.fields.refs).toEqual(['follow_1'])
  })

  test('dual-write off or missing postgres returns the bare d1 store', () => {
    const d1 = { upsertRows: () => Promise.resolve(0) }
    expect(
      makeDualWriteForumContentWriteStore({
        d1,
        flags: { dualWrite: false, reads: 'd1' },
        postgres: { upsertRows: () => Promise.resolve(0) },
      }),
    ).toBe(d1)
    expect(
      makeDualWriteForumContentWriteStore({
        d1,
        flags: { dualWrite: true, reads: 'd1' },
        postgres: undefined,
      }),
    ).toBe(d1)
  })
})

// ---------------------------------------------------------------------------
// Env factory degradation
// ---------------------------------------------------------------------------

describe('forumContentDatabaseForEnv', () => {
  test('no KHALA_SYNC_DB binding returns the raw database (zero overhead)', () => {
    const sqlite = makeSqliteD1()
    const db = forumContentDatabaseForEnv({ OPENAGENTS_DB: sqlite.db })
    expect(db).toBe(sqlite.db)
    sqlite.close()
  })

  test('dual-write off with d1 reads returns the raw database', () => {
    const sqlite = makeSqliteD1()
    const db = forumContentDatabaseForEnv({
      KHALA_SYNC_DB: { connectionString: 'postgres://unused' },
      KHALA_SYNC_FORUM_DUAL_WRITE: 'off',
      OPENAGENTS_DB: sqlite.db,
    })
    expect(db).toBe(sqlite.db)
    sqlite.close()
  })

  test('binding + defaults returns a wrapped database', () => {
    const sqlite = makeSqliteD1()
    const db = forumContentDatabaseForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://unused' },
        OPENAGENTS_DB: sqlite.db,
      },
      {
        makeSqlClient: () => Promise.reject(new Error('never connected')),
      },
    )
    expect(db).not.toBe(sqlite.db)
    sqlite.close()
  })

  test('reads=postgres returns the serving wrapper (CFG #8515), no deferral', () => {
    // CFG D1 evacuation: `postgres` mode is no longer inert — it returns the
    // forum Postgres serving wrapper (reads served / writes executed on
    // Postgres). The old `khala_sync_forum_postgres_reads_deferred` diagnostic
    // is gone; construction alone never opens a Postgres connection.
    const sqlite = makeSqliteD1()
    const capture = makeLogCapture()
    const db = forumContentDatabaseForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://unused' },
        KHALA_SYNC_FORUM_READS: 'postgres',
        OPENAGENTS_DB: sqlite.db,
      },
      {
        log: capture.log,
        makeSqlClient: () => Promise.reject(new Error('never connected')),
      },
    )
    expect(db).not.toBe(sqlite.db)
    expect(
      capture.events.filter(
        entry => entry.event === 'khala_sync_forum_postgres_reads_deferred',
      ),
    ).toHaveLength(0)
    sqlite.close()
  })
})

// KS-8.10 remainder (#8338): forum remainder store — statement classifier,
// mirroring database, dual-write wrapper, and composition degradation.
//
// Load-bearing properties (mirrors the content lane, scoped to the
// remainder tables):
//   * the classifier resolves the PRIMARY KEY of every remainder write
//     shape the forum repository / work-request modules actually issue
//     (private-message INSERT, the thread-bump UPDATE whose id bind trails
//     two SET binds, INSERT OR IGNORE notification reads, the work-request
//     INSERT with trailing literals, the work-request state UPDATE with a
//     CASE bind in SET), and refuses loudly rather than guessing;
//   * content tables and forum MONEY tables are PASSTHROUGH here (the
//     content wrapper owns content; money is KS-8.8) so the two nested
//     wrappers never double-mirror;
//   * the mirror read-back-mirrors the exact post-write D1 row, skips
//     dedupe-discarded OR IGNORE ids, never fails a request on a Postgres
//     error, and — PRIVACY — logs row KEYS only, never message content;
//   * `wrapForumRemainderMirroring` degrades to the base database (identity)
//     with no binding / flags off.

import { describe, expect, test } from 'vitest'

import {
  classifyForumRemainderStatement,
  makeDualWriteForumRemainderWriteStore,
  makeForumRemainderMirror,
  makeForumRemainderMirroringDatabase,
  resolveForumRemainderPk,
  wrapForumRemainderMirroring,
  type ForumRemainderDiagnostic,
  type ForumRemainderDiagnosticEvent,
  type ForumRemainderRow,
  type ForumRemainderTable,
  type PostgresForumRemainderStore,
} from './forum-remainder-store'
import { FORUM_REMAINDER_D1_SCHEMA, makeSqliteD1 } from '../test/sqlite-d1'

const T0 = '2026-07-04T00:00:00.000Z'

type LoggedDiagnostic = Readonly<{
  event: ForumRemainderDiagnosticEvent
  fields: ForumRemainderDiagnostic
}>

const makeLogCapture = () => {
  const events: Array<LoggedDiagnostic> = []
  return {
    events,
    log: (
      event: ForumRemainderDiagnosticEvent,
      fields: ForumRemainderDiagnostic,
    ) => {
      events.push({ event, fields })
    },
  }
}

type CapturedUpsert = Readonly<{
  table: ForumRemainderTable
  rows: ReadonlyArray<ForumRemainderRow>
}>

const makeFakePostgresStore = (
  options: Readonly<{ failUpserts?: boolean }> = {},
) => {
  const upserts: Array<CapturedUpsert> = []
  const store: PostgresForumRemainderStore = {
    queryRows: () => Promise.reject(new Error('no query fake')),
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
// Classifier
// ---------------------------------------------------------------------------

describe('classifyForumRemainderStatement', () => {
  test('private-message thread INSERT keys id at bind 0', () => {
    expect(
      classifyForumRemainderStatement(
        `INSERT INTO forum_private_message_threads (
           id, subject, slug, created_by_actor_ref, participant_refs_json,
           latest_message_id, message_count, created_at, updated_at
         )
         VALUES (?, ?, ?, ?, ?, NULL, 0, ?, ?)`,
      ),
    ).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 0, kind: 'bind' },
      table: 'forum_private_message_threads',
    })
  })

  test('thread-bump UPDATE keys id at bind 2 (after two SET binds)', () => {
    expect(
      classifyForumRemainderStatement(
        `UPDATE forum_private_message_threads
            SET latest_message_id = ?,
                message_count = message_count + 1,
                updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      ),
    ).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 2, kind: 'bind' },
      table: 'forum_private_message_threads',
    })
  })

  test('notification-read INSERT OR IGNORE classifies on the id bind', () => {
    expect(
      classifyForumRemainderStatement(
        `INSERT OR IGNORE INTO forum_notification_reads (
           id, actor_ref, notification_id, idempotency_key, read_at,
           created_at, updated_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
      ),
    ).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 0, kind: 'bind' },
      table: 'forum_notification_reads',
    })
  })

  test('work-request INSERT with trailing literals keys id at bind 0', () => {
    expect(
      classifyForumRemainderStatement(
        `INSERT INTO forum_work_requests (
           id, idempotency_key, topic_id, first_post_id, requester_actor_ref,
           title, objective_ref, verification_command_ref, repository_refs_json,
           required_capability_refs_json, budget_sats, budget_msats,
           deadline_ref, relay_url, job_event_id, job_event_kind,
           job_result_kind, state, quote_count, public_projection_json,
           created_at, updated_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'open', 0, ?, ?, ?)`,
      ),
    ).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 0, kind: 'bind' },
      table: 'forum_work_requests',
    })
  })

  test('work-request state UPDATE with a CASE bind in SET keys id at bind 3', () => {
    expect(
      classifyForumRemainderStatement(
        `UPDATE forum_work_requests
            SET state = ?,
                quote_count = quote_count + CASE WHEN ? = 'quote_received' THEN 1 ELSE 0 END,
                updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      ),
    ).toEqual({
      kind: 'mirrored-write',
      pkSource: { index: 3, kind: 'bind' },
      table: 'forum_work_requests',
    })
  })

  test('content tables and money tables pass through (owned elsewhere)', () => {
    expect(
      classifyForumRemainderStatement(
        `INSERT INTO forum_posts (id, topic_id) VALUES (?, ?)`,
      ),
    ).toEqual({ kind: 'passthrough' })
    expect(
      classifyForumRemainderStatement(
        `INSERT INTO forum_receipts (id, receipt_ref) VALUES (?, ?)`,
      ),
    ).toEqual({ kind: 'passthrough' })
  })

  test('scoped DELETE and non-PK writes refuse loudly', () => {
    expect(
      classifyForumRemainderStatement(
        `DELETE FROM forum_private_messages WHERE id = ?`,
      ),
    ).toEqual({ kind: 'unclassified-write', table: 'forum_private_messages' })
    expect(
      classifyForumRemainderStatement(
        `UPDATE forum_work_request_offers SET state = ? WHERE work_request_id = ?`,
      ),
    ).toEqual({
      kind: 'unclassified-write',
      table: 'forum_work_request_offers',
    })
  })

  test('scoped-only private-message SELECT is comparable', () => {
    expect(
      classifyForumRemainderStatement(
        `SELECT * FROM forum_private_messages WHERE thread_id = ? ORDER BY created_at ASC`,
      ),
    ).toEqual({ kind: 'comparable-select' })
  })

  test('resolveForumRemainderPk reads binds and literals', () => {
    expect(
      resolveForumRemainderPk({ index: 2, kind: 'bind' }, ['a', 'b', 'c']),
    ).toBe('c')
    expect(
      resolveForumRemainderPk({ kind: 'literal', value: 'x' }, []),
    ).toBe('x')
  })
})

// ---------------------------------------------------------------------------
// Mirroring database
// ---------------------------------------------------------------------------

describe('makeForumRemainderMirroringDatabase', () => {
  const seedThread = (exec: (sql: string) => void): void => {
    exec(
      `INSERT INTO forum_private_message_threads (
         id, subject, slug, created_by_actor_ref, participant_refs_json,
         message_count, created_at, updated_at
       )
       VALUES ('thread_1', 'Secret subject', 'secret-thread',
               'agent_a', '["agent_a","agent_b"]', 0, '${T0}', '${T0}')`,
    )
  }

  test('private-message INSERT + thread bump read-back-mirror the exact rows', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_REMAINDER_D1_SCHEMA)
    seedThread(sqlite.exec)
    const { log } = makeLogCapture()
    const fake = makeFakePostgresStore()
    const db = makeForumRemainderMirroringDatabase({
      compareStore: undefined,
      db: sqlite.db,
      log,
      mirror: makeForumRemainderMirror({
        db: sqlite.db,
        log,
        postgres: fake.store,
      }),
    })

    await db
      .prepare(
        `INSERT INTO forum_private_messages (
           id, thread_id, sender_actor_ref, recipient_actor_ref, content_ref,
           public_projection_json, created_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        'msg_1',
        'thread_1',
        'agent_a',
        'agent_b',
        'content.forum.pm.secret.1',
        '{}',
        T0,
      )
      .run()

    expect(fake.upserts).toHaveLength(1)
    expect(fake.upserts[0]?.table).toBe('forum_private_messages')
    expect(fake.upserts[0]?.rows[0]?.['id']).toBe('msg_1')

    await db
      .prepare(
        `UPDATE forum_private_message_threads
            SET latest_message_id = ?, message_count = message_count + 1, updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind('msg_1', '2026-07-04T01:00:00.000Z', 'thread_1')
      .run()

    expect(fake.upserts).toHaveLength(2)
    expect(fake.upserts[1]?.table).toBe('forum_private_message_threads')
    expect(fake.upserts[1]?.rows[0]?.['id']).toBe('thread_1')
    expect(fake.upserts[1]?.rows[0]?.['message_count']).toBe(1)
    expect(fake.upserts[1]?.rows[0]?.['latest_message_id']).toBe('msg_1')

    sqlite.close()
  })

  test('notification-read OR IGNORE dedupe mirrors zero rows on discard', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_REMAINDER_D1_SCHEMA)
    const { log } = makeLogCapture()
    const fake = makeFakePostgresStore()
    const db = makeForumRemainderMirroringDatabase({
      compareStore: undefined,
      db: sqlite.db,
      log,
      mirror: makeForumRemainderMirror({
        db: sqlite.db,
        log,
        postgres: fake.store,
      }),
    })

    const read = (id: string, key: string) =>
      db
        .prepare(
          `INSERT OR IGNORE INTO forum_notification_reads (
             id, actor_ref, notification_id, idempotency_key, read_at,
             created_at, updated_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(id, 'agent_a', 'notif_1', key, T0, T0, T0)
        .run()

    await read('read_1', 'read-key-1')
    // Same (actor, notification): D1 unique index discards the new id.
    await read('read_2', 'read-key-2')

    expect(fake.upserts).toHaveLength(2)
    expect(fake.upserts[0]?.rows).toHaveLength(1)
    expect(fake.upserts[0]?.rows[0]?.['id']).toBe('read_1')
    expect(fake.upserts[1]?.rows).toHaveLength(0)

    sqlite.close()
  })

  test('a postgres outage never fails the write; privacy: keys only', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_REMAINDER_D1_SCHEMA)
    seedThread(sqlite.exec)
    const capture = makeLogCapture()
    const fake = makeFakePostgresStore({ failUpserts: true })
    const db = makeForumRemainderMirroringDatabase({
      compareStore: undefined,
      db: sqlite.db,
      log: capture.log,
      mirror: makeForumRemainderMirror({
        db: sqlite.db,
        log: capture.log,
        postgres: fake.store,
      }),
    })

    const result = await db
      .prepare(
        `INSERT INTO forum_private_messages (
           id, thread_id, sender_actor_ref, recipient_actor_ref, content_ref,
           public_projection_json, created_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        'msg_secret',
        'thread_1',
        'agent_a',
        'agent_b',
        'content.forum.pm.TOPSECRET',
        '{}',
        T0,
      )
      .run()

    expect(result.success).toBe(true)
    const drift = capture.events.filter(
      entry => entry.event === 'khala_sync_forum_dual_write_failed',
    )
    expect(drift).toHaveLength(1)
    expect(drift[0]?.fields.op).toBe('mirror:forum_private_messages')
    expect(drift[0]?.fields.refs).toEqual(['msg_secret'])
    // Diagnostics carry keys only — never the message content ref.
    expect(JSON.stringify(drift[0])).not.toContain('TOPSECRET')

    sqlite.close()
  })

  test('scoped DELETE logs unclassified-write instead of guessing', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_REMAINDER_D1_SCHEMA)
    const capture = makeLogCapture()
    const fake = makeFakePostgresStore()
    const db = makeForumRemainderMirroringDatabase({
      compareStore: undefined,
      db: sqlite.db,
      log: capture.log,
      mirror: makeForumRemainderMirror({
        db: sqlite.db,
        log: capture.log,
        postgres: fake.store,
      }),
    })

    await db
      .prepare(`DELETE FROM forum_notification_reads WHERE id = ?`)
      .bind('read_1')
      .run()

    expect(fake.upserts).toHaveLength(0)
    expect(
      capture.events.filter(
        entry => entry.event === 'khala_sync_forum_write_unclassified',
      ),
    ).toHaveLength(1)
    sqlite.close()
  })

  test('compare mode serves D1, logs mismatch, survives read failure', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(FORUM_REMAINDER_D1_SCHEMA)
    seedThread(sqlite.exec)
    sqlite.exec(
      `INSERT INTO forum_private_messages (
         id, thread_id, sender_actor_ref, recipient_actor_ref, content_ref,
         public_projection_json, created_at
       )
       VALUES ('msg_1', 'thread_1', 'agent_a', 'agent_b', 'c', '{}', '${T0}')`,
    )
    const capture = makeLogCapture()
    let mode: 'match' | 'mismatch' | 'fail' = 'match'
    const store: PostgresForumRemainderStore = {
      queryRows: async text => {
        if (mode === 'fail') throw new Error('postgres read down')
        expect(text).toContain('$1')
        const row = await sqlite.db
          .prepare(`SELECT * FROM forum_private_messages WHERE id = ?`)
          .bind('msg_1')
          .first<Record<string, unknown>>()
        if (row === null) return []
        return mode === 'mismatch' ? [{ ...row, content_ref: 'DRIFT' }] : [row]
      },
      upsertRows: () => Promise.resolve(0),
    }
    const db = makeForumRemainderMirroringDatabase({
      compareStore: store,
      db: sqlite.db,
      log: capture.log,
      mirror: undefined,
    })

    const read = () =>
      db
        .prepare(`SELECT * FROM forum_private_messages WHERE thread_id = ?`)
        .bind('thread_1')
        .all<Record<string, unknown>>()

    expect((await read()).results?.[0]?.['content_ref']).toBe('c')
    expect(capture.events).toHaveLength(0)

    mode = 'mismatch'
    expect((await read()).results?.[0]?.['content_ref']).toBe('c')
    expect(
      capture.events.filter(
        e => e.event === 'khala_sync_forum_read_compare_mismatch',
      ),
    ).toHaveLength(1)

    mode = 'fail'
    expect((await read()).results?.[0]?.['content_ref']).toBe('c')
    expect(
      capture.events.filter(
        e => e.event === 'khala_sync_forum_read_compare_failed',
      ),
    ).toHaveLength(1)

    sqlite.close()
  })
})

// ---------------------------------------------------------------------------
// Dual-write row store wrapper
// ---------------------------------------------------------------------------

describe('makeDualWriteForumRemainderWriteStore', () => {
  const row: ForumRemainderRow = {
    archived_at: null,
    created_at: T0,
    id: 'read_1',
  }

  test('mirrors after the d1 write; postgres failure is fail-soft + logged', async () => {
    const capture = makeLogCapture()
    const calls: Array<string> = []
    const d1 = {
      upsertRows: (table: ForumRemainderTable) => {
        calls.push(`d1:${table}`)
        return Promise.resolve(1)
      },
    }
    const store = makeDualWriteForumRemainderWriteStore({
      d1,
      flags: { dualWrite: true, reads: 'd1' },
      log: capture.log,
      postgres: { upsertRows: () => Promise.reject(new Error('down')) },
    })
    expect(await store.upsertRows('forum_notification_reads', [row])).toBe(1)
    expect(calls).toEqual(['d1:forum_notification_reads'])
    expect(capture.events[0]?.event).toBe('khala_sync_forum_dual_write_failed')
    expect(capture.events[0]?.fields.refs).toEqual(['read_1'])
  })

  test('dual-write off or missing postgres returns the bare d1 store', () => {
    const d1 = { upsertRows: () => Promise.resolve(0) }
    expect(
      makeDualWriteForumRemainderWriteStore({
        d1,
        flags: { dualWrite: false, reads: 'd1' },
        postgres: { upsertRows: () => Promise.resolve(0) },
      }),
    ).toBe(d1)
    expect(
      makeDualWriteForumRemainderWriteStore({
        d1,
        flags: { dualWrite: true, reads: 'd1' },
        postgres: undefined,
      }),
    ).toBe(d1)
  })
})

// ---------------------------------------------------------------------------
// Composition degradation
// ---------------------------------------------------------------------------

describe('wrapForumRemainderMirroring', () => {
  test('no KHALA_SYNC_DB binding returns the base database (identity)', () => {
    const sqlite = makeSqliteD1()
    const wrapped = wrapForumRemainderMirroring(sqlite.db, {
      OPENAGENTS_DB: sqlite.db,
    })
    expect(wrapped).toBe(sqlite.db)
    sqlite.close()
  })

  test('dual-write off with d1 reads returns the base database', () => {
    const sqlite = makeSqliteD1()
    const wrapped = wrapForumRemainderMirroring(sqlite.db, {
      KHALA_SYNC_DB: { connectionString: 'postgres://unused' },
      KHALA_SYNC_FORUM_DUAL_WRITE: 'off',
      OPENAGENTS_DB: sqlite.db,
    })
    expect(wrapped).toBe(sqlite.db)
    sqlite.close()
  })

  test('binding + defaults returns a wrapped database', () => {
    const sqlite = makeSqliteD1()
    const wrapped = wrapForumRemainderMirroring(
      sqlite.db,
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://unused' },
        OPENAGENTS_DB: sqlite.db,
      },
      { makeSqlClient: () => Promise.reject(new Error('never connected')) },
    )
    expect(wrapped).not.toBe(sqlite.db)
    sqlite.close()
  })
})

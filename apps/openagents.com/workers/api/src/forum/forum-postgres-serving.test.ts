// CFG D1 evacuation (#8515): forum Postgres serving wrapper unit tests.
// Covers the classifier (which SELECTs/writes route to Postgres), the
// SQLite→Postgres write translation, and the D1Database proxy's serve/
// fail-soft/passthrough behavior. Uses a recording fake D1 + a fake
// `queryRows` so no real database is needed.

import { describe, expect, test } from 'vitest'

import {
  classifyForumServeStatement,
  isForumServedTable,
  makeForumPostgresServingDatabase,
  translateForumWriteSql,
  type ForumServeQueryRows,
} from './forum-postgres-serving'

// ---------------------------------------------------------------------------
// Recording fake D1 (base handle)
// ---------------------------------------------------------------------------

type BaseCall = Readonly<{ method: string; sql: string; params: ReadonlyArray<unknown> }>

const makeRecordingBaseDb = (
  behavior?: Readonly<{
    all?: unknown
    first?: unknown
    run?: unknown
    throwOn?: 'all' | 'first' | 'run'
  }>,
) => {
  const calls: Array<BaseCall> = []
  const makeStatement = (sql: string, params: ReadonlyArray<unknown>) => ({
    all: async () => {
      calls.push({ method: 'all', params, sql })
      if (behavior?.throwOn === 'all') throw new Error('D1 dead (all)')
      return (behavior?.all ?? { meta: {}, results: [], success: true }) as never
    },
    bind: (...values: ReadonlyArray<unknown>) => makeStatement(sql, values),
    first: async () => {
      calls.push({ method: 'first', params, sql })
      if (behavior?.throwOn === 'first') throw new Error('D1 dead (first)')
      return (behavior?.first ?? null) as never
    },
    raw: async () => {
      calls.push({ method: 'raw', params, sql })
      return [] as never
    },
    run: async () => {
      calls.push({ method: 'run', params, sql })
      if (behavior?.throwOn === 'run') throw new Error('D1 dead (run)')
      return (behavior?.run ?? { meta: {}, results: [], success: true }) as never
    },
  })
  const db = {
    batch: async () => [],
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (sql: string) => makeStatement(sql, []),
    withSession: () => undefined,
  } as unknown as D1Database
  return { calls, db }
}

const makeQueryRows = (
  impl: (text: string, params: ReadonlyArray<unknown>) => Promise<
    ReadonlyArray<Record<string, unknown>>
  >,
): { calls: Array<{ text: string; params: ReadonlyArray<unknown> }>; queryRows: ForumServeQueryRows } => {
  const calls: Array<{ text: string; params: ReadonlyArray<unknown> }> = []
  return {
    calls,
    queryRows: (text, params) => {
      calls.push({ params, text })
      return impl(text, params)
    },
  }
}

const noopLog = () => {}

// ---------------------------------------------------------------------------
// Table set
// ---------------------------------------------------------------------------

describe('isForumServedTable', () => {
  test('content, remainder, and treasury-money forum tables are served', () => {
    for (const table of [
      'forum_topics',
      'forum_posts',
      'forum_post_bodies',
      'forum_forums',
      'forum_notification_reads',
      'forum_work_requests',
      'forum_money_actions',
      'forum_receipts',
      'forum_tip_recipient_wallets',
    ]) {
      expect(isForumServedTable(table)).toBe(true)
    }
  })

  test('non-forum tables are not served', () => {
    for (const table of ['users', 'agent_runs', 'token_usage_events', 'pay_ins']) {
      expect(isForumServedTable(table)).toBe(false)
    }
  })
})

// ---------------------------------------------------------------------------
// Classifier
// ---------------------------------------------------------------------------

describe('classifyForumServeStatement', () => {
  test('a pure forum-content SELECT is serve-select', () => {
    expect(
      classifyForumServeStatement(
        'SELECT * FROM forum_forums WHERE (id = ? OR slug = ?) LIMIT 1',
      ).kind,
    ).toBe('serve-select')
  })

  test('a content+content JOIN SELECT is serve-select', () => {
    expect(
      classifyForumServeStatement(
        `SELECT forum_topics.* FROM forum_topics
           LEFT JOIN forum_posts AS latest ON latest.id = forum_topics.latest_post_id
          WHERE forum_topics.forum_id = ?`,
      ).kind,
    ).toBe('serve-select')
  })

  test('a treasury-money forum SELECT is serve-select', () => {
    expect(
      classifyForumServeStatement(
        `SELECT * FROM forum_tip_recipient_wallets WHERE actor_ref = ? LIMIT 1`,
      ).kind,
    ).toBe('serve-select')
  })

  test('a recursive CTE (forum_post_ancestors) stays serve-select', () => {
    const sql = `WITH RECURSIVE forum_post_ancestors (id, parent_post_id, depth) AS (
        SELECT id, parent_post_id, 0 FROM forum_posts WHERE id = ?
        UNION ALL
        SELECT forum_posts.id, forum_posts.parent_post_id, forum_post_ancestors.depth + 1
          FROM forum_posts
          JOIN forum_post_ancestors ON forum_posts.id = forum_post_ancestors.parent_post_id
         WHERE forum_post_ancestors.depth < 64
      )
      SELECT id FROM forum_post_ancestors WHERE parent_post_id = ?`
    expect(classifyForumServeStatement(sql).kind).toBe('serve-select')
  })

  test('a SELECT joining a non-forum table is passthrough', () => {
    expect(
      classifyForumServeStatement(
        'SELECT * FROM forum_posts JOIN users ON users.id = forum_posts.actor_ref',
      ).kind,
    ).toBe('passthrough')
  })

  test('an INSERT/UPDATE/DELETE on a forum table is serve-write', () => {
    expect(
      classifyForumServeStatement(
        'INSERT INTO forum_topics (id, title) VALUES (?, ?)',
      ).kind,
    ).toBe('serve-write')
    expect(
      classifyForumServeStatement(
        'INSERT OR IGNORE INTO forum_watches (id, actor_ref) VALUES (?, ?)',
      ).kind,
    ).toBe('serve-write')
    expect(
      classifyForumServeStatement(
        'UPDATE forum_forums SET post_count = post_count + 1 WHERE id = ?',
      ).kind,
    ).toBe('serve-write')
    expect(
      classifyForumServeStatement('DELETE FROM forum_bookmarks WHERE id = ?').kind,
    ).toBe('serve-write')
  })

  test('a write to a non-forum table is passthrough', () => {
    expect(
      classifyForumServeStatement('UPDATE users SET name = ? WHERE id = ?').kind,
    ).toBe('passthrough')
  })
})

// ---------------------------------------------------------------------------
// Write translation
// ---------------------------------------------------------------------------

describe('translateForumWriteSql', () => {
  test('INSERT OR IGNORE becomes INSERT … ON CONFLICT DO NOTHING with $n', () => {
    const out = translateForumWriteSql(
      'INSERT OR IGNORE INTO forum_watches (id, actor_ref) VALUES (?, ?)',
    )
    expect(out).toContain('INSERT INTO forum_watches')
    expect(out).not.toMatch(/insert\s+or\s+ignore/i)
    expect(out).toContain('ON CONFLICT DO NOTHING')
    expect(out).toContain('($1, $2)')
  })

  test('a plain UPDATE only gets placeholder rewriting', () => {
    const out = translateForumWriteSql(
      'UPDATE forum_forums SET post_count = post_count + 1 WHERE id = ?',
    )
    expect(out).toBe(
      'UPDATE forum_forums SET post_count = post_count + 1 WHERE id = $1',
    )
  })

  test('a plain INSERT only gets placeholder rewriting', () => {
    const out = translateForumWriteSql(
      "INSERT INTO forum_topics (id, title, state) VALUES (?, ?, 'open')",
    )
    expect(out).toBe(
      "INSERT INTO forum_topics (id, title, state) VALUES ($1, $2, 'open')",
    )
  })
})

// ---------------------------------------------------------------------------
// The serving proxy
// ---------------------------------------------------------------------------

describe('makeForumPostgresServingDatabase', () => {
  test('serve-select .all() returns Postgres rows (never touches D1)', async () => {
    const base = makeRecordingBaseDb()
    const pg = makeQueryRows(async () => [
      { id: 't1', post_count: 5, title: 'hello' },
    ])
    const db = makeForumPostgresServingDatabase({
      db: base.db,
      log: noopLog,
      queryRows: pg.queryRows,
    })
    const result = await db
      .prepare('SELECT * FROM forum_topics WHERE forum_id = ?')
      .bind('f1')
      .all()
    expect(result.results).toEqual([{ id: 't1', post_count: 5, title: 'hello' }])
    expect(pg.calls).toHaveLength(1)
    expect(pg.calls[0]!.text).toContain('$1')
    expect(pg.calls[0]!.params).toEqual(['f1'])
    // D1 base never ran.
    expect(base.calls.filter(c => c.method === 'all')).toHaveLength(0)
  })

  test('serve-select .first() projects the first Postgres row', async () => {
    const base = makeRecordingBaseDb()
    const pg = makeQueryRows(async () => [{ id: 'f1', slug: 'general' }])
    const db = makeForumPostgresServingDatabase({
      db: base.db,
      log: noopLog,
      queryRows: pg.queryRows,
    })
    const row = await db
      .prepare('SELECT * FROM forum_forums WHERE id = ? LIMIT 1')
      .bind('f1')
      .first()
    expect(row).toEqual({ id: 'f1', slug: 'general' })
  })

  test('serve-select falls back to D1 on a Postgres read error', async () => {
    const base = makeRecordingBaseDb({
      all: { meta: {}, results: [{ id: 'fromD1' }], success: true },
    })
    const pg = makeQueryRows(async () => {
      throw new Error('json_extract does not exist')
    })
    const db = makeForumPostgresServingDatabase({
      db: base.db,
      log: noopLog,
      queryRows: pg.queryRows,
    })
    const result = await db
      .prepare('SELECT * FROM forum_receipts WHERE id = ?')
      .bind('r1')
      .all()
    expect(result.results).toEqual([{ id: 'fromD1' }])
    expect(base.calls.some(c => c.method === 'all')).toBe(true)
  })

  test('serve-write executes on Postgres and never touches D1', async () => {
    const base = makeRecordingBaseDb()
    const pg = makeQueryRows(async () => [])
    const db = makeForumPostgresServingDatabase({
      db: base.db,
      log: noopLog,
      queryRows: pg.queryRows,
    })
    const result = await db
      .prepare('INSERT INTO forum_topics (id, title) VALUES (?, ?)')
      .bind('t1', 'hi')
      .run()
    expect(result.success).toBe(true)
    expect(pg.calls).toHaveLength(1)
    expect(pg.calls[0]!.text).toContain('$1')
    expect(base.calls.filter(c => c.method === 'run')).toHaveLength(0)
  })

  test('serve-write translates INSERT OR IGNORE and throws on Postgres failure', async () => {
    const base = makeRecordingBaseDb()
    const pg = makeQueryRows(async text => {
      expect(text).toContain('ON CONFLICT DO NOTHING')
      throw new Error('unique violation')
    })
    const db = makeForumPostgresServingDatabase({
      db: base.db,
      log: noopLog,
      queryRows: pg.queryRows,
    })
    await expect(
      db
        .prepare('INSERT OR IGNORE INTO forum_watches (id, actor_ref) VALUES (?, ?)')
        .bind('w1', 'a1')
        .run(),
    ).rejects.toThrow('unique violation')
    // A failed write must NOT silently look like success on D1 either.
    expect(base.calls.filter(c => c.method === 'run')).toHaveLength(0)
  })

  test('a passthrough statement runs on D1', async () => {
    const base = makeRecordingBaseDb({
      first: { id: 'u1' },
    })
    const pg = makeQueryRows(async () => [])
    const db = makeForumPostgresServingDatabase({
      db: base.db,
      log: noopLog,
      queryRows: pg.queryRows,
    })
    const row = await db
      .prepare('SELECT * FROM users WHERE id = ?')
      .bind('u1')
      .first()
    expect(row).toEqual({ id: 'u1' })
    expect(pg.calls).toHaveLength(0)
    expect(base.calls.some(c => c.method === 'first')).toBe(true)
  })
})

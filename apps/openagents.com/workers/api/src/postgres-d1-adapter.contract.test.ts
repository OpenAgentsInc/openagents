// CFG-4 (#8519) Domain 3: proves the Postgres-backed D1 adapter serves the
// REAL Khala Code product-state consumer SQL against real local Postgres
// (khala-sync migration 0017; skipped when no local Postgres binaries exist).
//
// Covers the load-bearing shapes: the thread/turn CONTENT path
// (`insertTeamChatMessage` + read-back), int8 -> JS number parsing, `col IS ?`
// null-safe equality, the `INSERT OR IGNORE … SELECT` -> `ON CONFLICT DO
// NOTHING` rewrite inside a batch transaction (idempotent), and an
// `ON CONFLICT … DO UPDATE` upsert.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import type { IdentityDb } from './identity-db'
import {
  makePostgresD1Database,
  translateProductStateSql,
  type PostgresD1Client,
} from './postgres-d1-adapter'
import { insertTeamChatMessage, readTeamChatMessageById } from './team-chat'

const MIGRATION_0017 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0017_khala_code_product_state.sql',
)

const NOW = '2026-07-06T00:00:00.000Z'
let seq = 0
const nextId = (prefix: string) => `${prefix}_cfg4_${++seq}`

// Identity handle stub: returns a single author profile so team-chat's
// author-enrichment (which requires the author's users row to exist, as it
// always does in production) keeps the message on read-back.
const stubIdentityDb: IdentityDb = {
  batch: async () => {},
  query: async () => [
    {
      avatar_url: null,
      created_at: NOW,
      deleted_at: null,
      display_name: 'Author',
      github_id: null,
      github_username: null,
      id: 'user_x',
      kind: 'human',
      primary_email: null,
      status: 'active',
    },
  ],
}

describe('translateProductStateSql', () => {
  test('rewrites col IS ? to IS NOT DISTINCT FROM and ? to $n', () => {
    expect(
      translateProductStateSql('SELECT * FROM t WHERE a = ? AND b IS ?'),
    ).toBe('SELECT * FROM t WHERE a = $1 AND b IS NOT DISTINCT FROM $2')
  })

  test('rewrites INSERT OR IGNORE to ON CONFLICT DO NOTHING', () => {
    expect(
      translateProductStateSql('INSERT OR IGNORE INTO t (id) VALUES (?)'),
    ).toBe('INSERT INTO t (id) VALUES ($1) ON CONFLICT DO NOTHING')
  })

  test('leaves an explicit ON CONFLICT untouched', () => {
    expect(
      translateProductStateSql(
        'INSERT INTO t (id, v) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET v = excluded.v',
      ),
    ).toBe(
      'INSERT INTO t (id, v) VALUES ($1, $2) ON CONFLICT(id) DO UPDATE SET v = excluded.v',
    )
  })
})

describe.skipIf(!hasLocalPostgres())(
  'postgres D1 adapter — product-state contract',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let raw: {
      unsafe: (t: string, p?: Array<unknown>) => Promise<Array<Record<string, unknown>>>
      end: (o?: { timeout?: number }) => Promise<void>
    }
    let db: D1Database

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE khala_code_contract')
      await admin.end({ timeout: 5 })

      const url = pg.urlFor('khala_code_contract')
      raw = postgres(url, { max: 4, prepare: false }) as never
      await raw.unsafe(readFileSync(MIGRATION_0017, 'utf8'))

      // The adapter shares this one client (int8 -> number parser as the
      // production factory sets).
      const numericClient = postgres(url, {
        max: 4,
        prepare: false,
        types: {
          bigint: {
            from: [20],
            parse: (v: string) => Number(v),
            serialize: (v: number | bigint) => v.toString(),
            to: 20,
          },
        },
      })
      db = makePostgresD1Database({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: numericClient as unknown as PostgresD1Client['sql'],
          }),
      })
    }, 120_000)

    afterAll(async () => {
      await raw?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    const seedTeam = async (credits: number, archived: boolean) => {
      const id = nextId('team')
      await db
        .prepare(
          `INSERT INTO teams (id, name, slug, kind, credits, status, created_at, updated_at, archived_at)
           VALUES (?, ?, ?, 'organization', ?, 'active', ?, ?, ?)`,
        )
        .bind(id, 'Team', nextId('slug'), credits, NOW, NOW, archived ? NOW : null)
        .run()
      return id
    }

    test('int8 columns read back as JS numbers', async () => {
      const id = await seedTeam(42_000, false)
      const row = await db
        .prepare(`SELECT credits FROM teams WHERE id = ?`)
        .bind(id)
        .first<{ credits: unknown }>()
      expect(row?.credits).toBe(42_000)
      expect(typeof row?.credits).toBe('number')
    })

    test('col IS ? matches NULL null-safely', async () => {
      const live = await seedTeam(1, false)
      await seedTeam(1, true)
      const rows = await db
        .prepare(`SELECT id FROM teams WHERE archived_at IS ? AND id = ?`)
        .bind(null, live)
        .all<{ id: string }>()
      expect(rows.results.map(r => r.id)).toEqual([live])
    })

    // #8515 D1 evacuation: the training domain (`attachRunEvidence`,
    // `beginRunSealBarrier`, `transitionWindow`) treats an UPDATE that affects
    // zero rows as `not_found` via `(result.meta?.changes ?? 0) === 0`. That
    // signal is load-bearing, so prove the adapter surfaces the REAL affected
    // row count from postgres.js — not a constant, not the returned-row count
    // (an UPDATE without RETURNING returns no rows).
    test('UPDATE meta.changes reports the affected-row count (the not_found signal)', async () => {
      const id = await seedTeam(5, false)

      // Matches exactly one row -> changes === 1.
      const hit = await db
        .prepare(
          `UPDATE teams SET credits = ? WHERE id = ? AND archived_at IS NULL`,
        )
        .bind(9, id)
        .run()
      expect(hit.meta.changes).toBe(1)
      const after = await db
        .prepare(`SELECT credits FROM teams WHERE id = ?`)
        .bind(id)
        .first<{ credits: number }>()
      expect(after?.credits).toBe(9)

      // Matches no rows -> changes === 0 (this is what surfaces as not_found).
      const miss = await db
        .prepare(
          `UPDATE teams SET credits = ? WHERE id = ? AND archived_at IS NULL`,
        )
        .bind(1, `${id}_absent`)
        .run()
      expect(miss.meta.changes).toBe(0)

      // Matches multiple rows -> changes === N.
      const sharedName = nextId('shared')
      for (let i = 0; i < 2; i += 1) {
        await db
          .prepare(
            `INSERT INTO teams (id, name, slug, kind, credits, status, created_at, updated_at, archived_at)
             VALUES (?, ?, ?, 'organization', 0, 'active', ?, ?, NULL)`,
          )
          .bind(nextId('team'), sharedName, nextId('slug'), NOW, NOW)
          .run()
      }
      const multi = await db
        .prepare(`UPDATE teams SET credits = ? WHERE name = ?`)
        .bind(3, sharedName)
        .run()
      expect(multi.meta.changes).toBe(2)
    })

    test('the thread/turn content path: insert + read-back through real SQL', async () => {
      const teamId = await seedTeam(0, false)
      const message = await insertTeamChatMessage(
        db,
        stubIdentityDb,
        {
          authorUserId: 'user_x',
          body: 'hello from postgres',
          kind: 'message',
          teamId,
        },
        { now: () => new Date(NOW) },
      )
      expect(message.body).toBe('hello from postgres')

      const readBack = await readTeamChatMessageById(db, stubIdentityDb, message.id)
      expect(readBack?.body).toBe('hello from postgres')
      expect(readBack?.teamId).toBe(teamId)
    })

    test('INSERT OR IGNORE … SELECT rewrites + is idempotent in a batch', async () => {
      const teamId = await seedTeam(0, false)
      const fileId = nextId('file')
      const threadId = nextId('thread')
      const messageId = nextId('msg')
      await db
        .prepare(
          `INSERT INTO thread_files
             (id, scope, thread_id, team_id, owner_user_id, filename, content_type, size_bytes, object_key, created_at, updated_at)
           VALUES (?, 'team', ?, ?, 'user_x', 'f.txt', 'text/plain', 10, ?, ?, ?)`,
        )
        .bind(fileId, threadId, teamId, nextId('obj'), NOW, NOW)
        .run()

      const refInsert = () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO thread_file_message_refs
               (id, file_id, team_id, thread_id, message_id, reference_kind, created_at, updated_at)
             SELECT ?, thread_files.id, thread_files.team_id, ?, ?, 'attachment', ?, ?
             FROM thread_files
             WHERE thread_files.id = ?
               AND thread_files.scope = 'team'
               AND thread_files.team_id = ?
               AND thread_files.deleted_at IS NULL`,
          )
          .bind(nextId('ref'), threadId, messageId, NOW, NOW, fileId, teamId)

      await db.batch([refInsert()])
      await db.batch([refInsert()]) // idempotent: same (file, message, kind)

      const rows = await db
        .prepare(
          `SELECT id FROM thread_file_message_refs WHERE file_id = ? AND message_id = ?`,
        )
        .bind(fileId, messageId)
        .all<{ id: string }>()
      expect(rows.results).toHaveLength(1)
    })

    test('ON CONFLICT … DO UPDATE upsert works', async () => {
      const teamId = await seedTeam(0, false)
      const userId = nextId('user')
      const upsert = (id: string, role: string) =>
        db
          .prepare(
            `INSERT INTO team_memberships
               (id, team_id, user_id, role, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'active', ?, ?)
             ON CONFLICT(team_id, user_id) DO UPDATE SET
               role = excluded.role, updated_at = excluded.updated_at`,
          )
          .bind(id, teamId, userId, role, NOW, NOW)
      await upsert(nextId('mem'), 'member').run()
      // Same (team_id, user_id): must UPDATE the role, not error.
      await upsert(nextId('mem'), 'admin').run()

      const row = await db
        .prepare(
          `SELECT role FROM team_memberships WHERE team_id = ? AND user_id = ?`,
        )
        .bind(teamId, userId)
        .first<{ role: string }>()
      expect(row?.role).toBe('admin')
    })
  },
)

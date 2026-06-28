import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

const migrationSql = readFileSync(
  join(__dirname, '..', 'migrations', '0248_artanis_operator_threads.sql'),
  'utf8',
)

const rows = <T extends Record<string, unknown>>(
  db: DatabaseSync,
  query: string,
): ReadonlyArray<T> =>
  db
    .prepare(query)
    .all()
    .map(row => row as T)

describe('0248_artanis_operator_threads migration', () => {
  test('creates dedicated Artanis thread/message tables and caller timestamp indexes', () => {
    const db = new DatabaseSync(':memory:')
    db.exec('PRAGMA foreign_keys = ON')
    db.exec(migrationSql)

    const tables = rows<{ name: string }>(
      db,
      "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'artanis_%' ORDER BY name",
    ).map(row => row.name)
    expect(tables).toContain('artanis_threads')
    expect(tables).toContain('artanis_messages')

    const indexes = rows<{ name: string }>(
      db,
      "SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_artanis_%' ORDER BY name",
    ).map(row => row.name)
    expect(indexes).toEqual(
      expect.arrayContaining([
        'idx_artanis_threads_caller_last_message',
        'idx_artanis_threads_caller_created',
        'idx_artanis_messages_thread_created',
        'idx_artanis_messages_caller_created',
      ]),
    )

    db.prepare(
      `INSERT INTO artanis_threads (
        thread_ref,
        caller_id,
        caller_kind,
        subject_agent_ref,
        subject_agent_kind,
        title,
        last_message_at,
        created_at,
        updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    ).run(
      'artanis_thread_owner_codex_1',
      'github:14167547',
      'owner',
      'codex-4',
      'codex',
      'Codex burn review',
      '2026-06-27T16:10:00.000Z',
      '2026-06-27T16:00:00.000Z',
      '2026-06-27T16:10:00.000Z',
    )

    db.prepare(
      `INSERT INTO artanis_messages (
        message_ref,
        thread_ref,
        caller_id,
        author_id,
        author_kind,
        body,
        created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
    ).run(
      'artanis_message_owner_codex_1',
      'artanis_thread_owner_codex_1',
      'github:14167547',
      'codex-4',
      'agent',
      'Public-safe status update.',
      '2026-06-27T16:10:00.000Z',
    )

    const messages = rows<{ message_ref: string }>(
      db,
      "SELECT message_ref FROM artanis_messages WHERE caller_id = 'github:14167547' ORDER BY created_at DESC",
    )
    expect(messages).toEqual([
      { message_ref: 'artanis_message_owner_codex_1' },
    ])

    db.prepare(
      "DELETE FROM artanis_threads WHERE thread_ref = 'artanis_thread_owner_codex_1'",
    ).run()
    expect(
      rows<{ count: number }>(
        db,
        'SELECT COUNT(*) AS count FROM artanis_messages',
      )[0]?.count,
    ).toBe(0)
  })
})

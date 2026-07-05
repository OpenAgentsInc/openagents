import { describe, expect, test } from 'vitest'

import {
  classifyKhalaCodeProductStateStatement,
  makeKhalaCodeProductStateMirroringDatabase,
  type KhalaCodeProductStateDiagnostic,
  type KhalaCodeProductStateDiagnosticEvent,
  type KhalaCodeProductStateMirror,
} from './khala-code-product-state-store'
import { makeSqliteD1 } from './test/sqlite-d1'

type LoggedDiagnostic = Readonly<{
  event: KhalaCodeProductStateDiagnosticEvent
  fields: KhalaCodeProductStateDiagnostic
}>

const makeLogCapture = () => {
  const events: Array<LoggedDiagnostic> = []
  return {
    events,
    log: (
      event: KhalaCodeProductStateDiagnosticEvent,
      fields: KhalaCodeProductStateDiagnostic,
    ) => {
      events.push({ event, fields })
    },
  }
}

const productStateSchema = `
CREATE TABLE team_chat_messages (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  project_id TEXT,
  author_user_id TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'message',
  body TEXT NOT NULL,
  autopilot_thread_id TEXT,
  agent_run_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  archived_at TEXT
);

CREATE TABLE team_memberships (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL,
  status TEXT NOT NULL,
  invited_by_user_id TEXT,
  joined_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  removed_at TEXT,
  UNIQUE(team_id, user_id)
);

CREATE TABLE teams (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT UNIQUE,
  kind TEXT NOT NULL DEFAULT 'organization',
  plan TEXT,
  logo_url TEXT,
  credits INTEGER,
  owner_user_id TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE team_projects (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE(team_id, slug)
);

CREATE TABLE share_projection_recipients (
  share_id TEXT NOT NULL,
  subject_kind TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(share_id, subject_kind, subject_id)
);
`

describe('classifyKhalaCodeProductStateStatement', () => {
  test('classifies inserted team chat messages by id', () => {
    expect(
      classifyKhalaCodeProductStateStatement(
        `INSERT INTO team_chat_messages
          (id, team_id, project_id, author_user_id, kind, body,
           autopilot_thread_id, agent_run_id, metadata_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      ),
    ).toEqual({
      kind: 'mirrored-upsert',
      table: 'team_chat_messages',
      where: { columns: ['id'], sources: [{ index: 0, kind: 'bind' }] },
    })
  })

  test('classifies update statements after SET binds', () => {
    expect(
      classifyKhalaCodeProductStateStatement(
        `UPDATE team_chat_messages
         SET metadata_json = ?, updated_at = ?
         WHERE id = ?
           AND deleted_at IS NULL
           AND archived_at IS NULL`,
      ),
    ).toEqual({
      kind: 'mirrored-upsert',
      table: 'team_chat_messages',
      where: { columns: ['id'], sources: [{ index: 2, kind: 'bind' }] },
    })
  })

  test('classifies natural-key membership upserts', () => {
    expect(
      classifyKhalaCodeProductStateStatement(
        `INSERT INTO team_memberships
          (id, team_id, user_id, role, status, joined_at, created_at,
           updated_at, removed_at)
         VALUES (?, ?, ?, ?, 'active', ?, ?, ?, NULL)
         ON CONFLICT(team_id, user_id) DO UPDATE SET
           role = excluded.role,
           status = 'active',
           updated_at = excluded.updated_at`,
      ),
    ).toEqual({
      kind: 'mirrored-upsert',
      table: 'team_memberships',
      where: {
        columns: ['team_id', 'user_id'],
        sources: [
          { index: 1, kind: 'bind' },
          { index: 2, kind: 'bind' },
        ],
      },
    })
  })

  test('classifies slug-conflict team upserts by the accepted natural key', () => {
    expect(
      classifyKhalaCodeProductStateStatement(
        `INSERT INTO teams
          (id, name, slug, kind, plan, owner_user_id, status, created_at,
           updated_at, archived_at)
         VALUES (?, ?, ?, 'organization', 'team', NULL, 'active', ?, ?, NULL)
         ON CONFLICT(slug) DO UPDATE SET
           name = excluded.name,
           kind = excluded.kind,
           plan = excluded.plan,
           status = 'active',
           archived_at = NULL,
           updated_at = excluded.updated_at`,
      ),
    ).toEqual({
      kind: 'mirrored-upsert',
      table: 'teams',
      where: { columns: ['slug'], sources: [{ index: 2, kind: 'bind' }] },
    })
  })

  test('classifies team project slug-conflict upserts by the accepted natural key', () => {
    expect(
      classifyKhalaCodeProductStateStatement(
        `INSERT INTO team_projects
          (id, team_id, slug, name, description, status, metadata_json,
           created_at, updated_at, archived_at)
         VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, NULL)
         ON CONFLICT(team_id, slug) DO UPDATE SET
           name = excluded.name,
           description = excluded.description,
           status = 'active',
           metadata_json = excluded.metadata_json,
           archived_at = NULL,
           updated_at = excluded.updated_at`,
      ),
    ).toEqual({
      kind: 'mirrored-upsert',
      table: 'team_projects',
      where: {
        columns: ['team_id', 'slug'],
        sources: [
          { index: 1, kind: 'bind' },
          { index: 2, kind: 'bind' },
        ],
      },
    })
  })

  test('classifies share-recipient bulk replacement deletes by share id', () => {
    expect(
      classifyKhalaCodeProductStateStatement(
        'DELETE FROM share_projection_recipients WHERE share_id = ?',
      ),
    ).toEqual({
      kind: 'mirrored-delete',
      table: 'share_projection_recipients',
      where: { columns: ['share_id'], sources: [{ index: 0, kind: 'bind' }] },
    })
  })
})

describe('makeKhalaCodeProductStateMirroringDatabase', () => {
  test('read-back mirrors the accepted D1 row after a write', async () => {
    const sqlite = makeSqliteD1()
    try {
      sqlite.exec(productStateSchema)
      const upserts: Array<Readonly<{ table: string; rows: ReadonlyArray<Record<string, unknown>> }>> = []
      const mirror: KhalaCodeProductStateMirror = {
        deleteRows: () => Promise.resolve(),
        upsertRows: (table, rows) => {
          upserts.push({ rows, table })
          return Promise.resolve()
        },
      }
      const { log } = makeLogCapture()
      const db = makeKhalaCodeProductStateMirroringDatabase({
        db: sqlite.db,
        log,
        mirror,
      })

      await db
        .prepare(
          `INSERT INTO team_chat_messages
            (id, team_id, project_id, author_user_id, kind, body,
             autopilot_thread_id, agent_run_id, metadata_json, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          'msg_1',
          'team_1',
          null,
          'user_1',
          'message',
          'hello',
          'thread_1',
          null,
          '{}',
          '2026-07-04T00:00:00.000Z',
          '2026-07-04T00:00:00.000Z',
        )
        .run()

      expect(upserts).toHaveLength(1)
      expect(upserts[0]?.table).toBe('team_chat_messages')
      expect(upserts[0]?.rows[0]).toMatchObject({
        id: 'msg_1',
        team_id: 'team_1',
        body: 'hello',
        autopilot_thread_id: 'thread_1',
      })
    } finally {
      sqlite.close()
    }
  })

  test('mirrors share-recipient replacement deletes without failing D1', async () => {
    const sqlite = makeSqliteD1()
    try {
      sqlite.exec(productStateSchema)
      await sqlite.db
        .prepare(
          `INSERT INTO share_projection_recipients
            (share_id, subject_kind, subject_id, display_name, created_at)
           VALUES (?, ?, ?, ?, ?)`,
        )
        .bind('share_1', 'team', 'team_1', 'Team One', '2026-07-04T00:00:00.000Z')
        .run()
      const deletes: Array<Readonly<{ table: string; columns: ReadonlyArray<string>; values: ReadonlyArray<unknown> }>> = []
      const mirror: KhalaCodeProductStateMirror = {
        deleteRows: (table, columns, values) => {
          deletes.push({ columns, table, values })
          return Promise.resolve()
        },
        upsertRows: () => Promise.resolve(),
      }
      const { log } = makeLogCapture()
      const db = makeKhalaCodeProductStateMirroringDatabase({
        db: sqlite.db,
        log,
        mirror,
      })

      await db
        .prepare('DELETE FROM share_projection_recipients WHERE share_id = ?')
        .bind('share_1')
        .run()

      expect(deletes).toEqual([
        {
          columns: ['share_id'],
          table: 'share_projection_recipients',
          values: ['share_1'],
        },
      ])
      const remaining = await sqlite.db
        .prepare('SELECT COUNT(*) AS count FROM share_projection_recipients')
        .first<{ count: number }>()
      expect(remaining?.count).toBe(0)
    } finally {
      sqlite.close()
    }
  })

  test('reads hard-deleted recipient rows BEFORE the delete and hands them to the mirror for tombstones', async () => {
    const sqlite = makeSqliteD1()
    try {
      sqlite.exec(productStateSchema)
      await sqlite.db.batch([
        sqlite.db
          .prepare(
            `INSERT INTO share_projection_recipients
              (share_id, subject_kind, subject_id, display_name, created_at)
             VALUES (?, ?, ?, ?, ?)`,
          )
          .bind('share_1', 'team', 'team_1', 'Team One', '2026-07-04T00:00:00.000Z'),
        sqlite.db
          .prepare(
            `INSERT INTO share_projection_recipients
              (share_id, subject_kind, subject_id, display_name, created_at)
             VALUES (?, ?, ?, ?, ?)`,
          )
          .bind('share_1', 'user', 'user_9', 'Jane', '2026-07-04T00:00:00.000Z'),
      ])
      const deletes: Array<
        Readonly<{
          table: string
          columns: ReadonlyArray<string>
          values: ReadonlyArray<unknown>
          deletedRows: ReadonlyArray<Record<string, unknown>> | undefined
        }>
      > = []
      const mirror: KhalaCodeProductStateMirror = {
        deleteRows: (table, columns, values, deletedRows) => {
          deletes.push({ columns, deletedRows, table, values })
          return Promise.resolve()
        },
        upsertRows: () => Promise.resolve(),
      }
      const { log } = makeLogCapture()
      const db = makeKhalaCodeProductStateMirroringDatabase({
        db: sqlite.db,
        log,
        mirror,
      })

      await db
        .prepare('DELETE FROM share_projection_recipients WHERE share_id = ?')
        .bind('share_1')
        .run()

      // The mirror received the FULL set of rows the delete removed (read
      // before the delete committed) so it can resolve one tombstone per
      // subject scope.
      expect(deletes).toHaveLength(1)
      expect(deletes[0]?.columns).toEqual(['share_id'])
      const subjects = (deletes[0]?.deletedRows ?? [])
        .map(row => `${String(row['subject_kind'])}:${String(row['subject_id'])}`)
        .sort()
      expect(subjects).toEqual(['team:team_1', 'user:user_9'])
    } finally {
      sqlite.close()
    }
  })

  test('mirrors the accepted team row when a slug upsert keeps the existing id', async () => {
    const sqlite = makeSqliteD1()
    try {
      sqlite.exec(productStateSchema)
      await sqlite.db
        .prepare(
          `INSERT INTO teams
            (id, name, slug, kind, plan, owner_user_id, status, created_at,
             updated_at, archived_at)
           VALUES (?, ?, ?, 'organization', 'team', NULL, 'active', ?, ?, NULL)`,
        )
        .bind(
          'team_existing',
          'Old Name',
          'alpha',
          '2026-07-04T00:00:00.000Z',
          '2026-07-04T00:00:00.000Z',
        )
        .run()

      const upserts: Array<
        Readonly<{ table: string; rows: ReadonlyArray<Record<string, unknown>> }>
      > = []
      const mirror: KhalaCodeProductStateMirror = {
        deleteRows: () => Promise.resolve(),
        upsertRows: (table, rows) => {
          upserts.push({ rows, table })
          return Promise.resolve()
        },
      }
      const { log } = makeLogCapture()
      const db = makeKhalaCodeProductStateMirroringDatabase({
        db: sqlite.db,
        log,
        mirror,
      })

      await db
        .prepare(
          `INSERT INTO teams
            (id, name, slug, kind, plan, owner_user_id, status, created_at,
             updated_at, archived_at)
           VALUES (?, ?, ?, 'organization', 'team', NULL, 'active', ?, ?, NULL)
           ON CONFLICT(slug) DO UPDATE SET
             name = excluded.name,
             kind = excluded.kind,
             plan = excluded.plan,
             status = 'active',
             archived_at = NULL,
             updated_at = excluded.updated_at`,
        )
        .bind(
          'team_generated',
          'New Name',
          'alpha',
          '2026-07-04T01:00:00.000Z',
          '2026-07-04T01:00:00.000Z',
        )
        .run()

      expect(upserts).toHaveLength(1)
      expect(upserts[0]?.table).toBe('teams')
      expect(upserts[0]?.rows[0]).toMatchObject({
        id: 'team_existing',
        name: 'New Name',
        slug: 'alpha',
      })
    } finally {
      sqlite.close()
    }
  })
})

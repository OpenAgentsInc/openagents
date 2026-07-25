import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import { makeD1TeamWorkspaceInviteStore } from './team-workspace-invites'

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Record<string, unknown>>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }

  async run(): Promise<{
    meta: { changes: number }
    results: []
    success: true
  }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return {
      meta: { changes: Number(result.changes) },
      results: [],
      success: true,
    }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<ReadonlyArray<unknown>> {
    const results: Array<unknown> = []
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        results.push(await statement.run())
      }
      this.db.exec('COMMIT')
      return results
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
  }
}

const makeDatabase = (): DatabaseSync => {
  const db = new DatabaseSync(':memory:')
  db.exec(`
    PRAGMA foreign_keys = ON;
    CREATE TABLE users (
      id TEXT PRIMARY KEY
    );
    CREATE TABLE teams (
      id TEXT PRIMARY KEY,
      status TEXT NOT NULL,
      archived_at TEXT
    );
    CREATE TABLE team_projects (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      status TEXT NOT NULL,
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
    CREATE TABLE team_workspace_invites (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      project_id TEXT,
      invitee_email TEXT NOT NULL,
      invitee_email_normalized TEXT NOT NULL,
      role TEXT NOT NULL,
      status TEXT NOT NULL,
      token_hash TEXT NOT NULL UNIQUE,
      invited_by_actor_ref TEXT NOT NULL,
      accepted_by_user_id TEXT,
      email_message_id TEXT,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      expires_at TEXT NOT NULL,
      accepted_at TEXT,
      revoked_at TEXT,
      last_sent_at TEXT,
      send_count INTEGER NOT NULL DEFAULT 0,
      metadata_json TEXT NOT NULL DEFAULT '{}'
    );
  `)
  db.prepare('INSERT INTO users (id) VALUES (?), (?)').run(
    'github:first',
    'email:same@example.com',
  )
  db.prepare(
    "INSERT INTO teams (id, status, archived_at) VALUES (?, 'active', NULL)",
  ).run('team.forge')
  return db
}

describe('team workspace invite account binding', () => {
  test('the accepted account can repair a missing membership on retry', async () => {
    const db = makeDatabase()
    const store = makeD1TeamWorkspaceInviteStore(
      new SqliteD1(db) as unknown as D1Database,
      {
        makeId: prefix => `${prefix}.recovery`,
        makeToken: () => 'oa_team_invite_recovery',
        nowIso: () => '2026-07-25T20:00:00.000Z',
      },
    )
    const created = await store.createOrRefreshInvite({
      email: 'same@example.com',
      expiresAt: '2026-07-26T20:00:00.000Z',
      invitedByActorRef: 'forge_actor.inviter',
      teamId: 'team.forge',
    })
    if (created._tag !== 'Created') {
      throw new Error(`unexpected invite result: ${created._tag}`)
    }
    const accepted = await store.acceptInvite({
      sessionEmail: 'same@example.com',
      token: created.token,
      userId: 'github:first',
    })
    expect(accepted._tag).toBe('Accepted')

    db.prepare(
      'DELETE FROM team_memberships WHERE team_id = ? AND user_id = ?',
    ).run('team.forge', 'github:first')

    const retried = await store.acceptInvite({
      sessionEmail: 'same@example.com',
      token: created.token,
      userId: 'github:first',
    })

    expect(retried._tag).toBe('AlreadyAccepted')
    expect(
      db
        .prepare(
          'SELECT role, status FROM team_memberships WHERE team_id = ? AND user_id = ?',
        )
        .get('team.forge', 'github:first'),
    ).toEqual({ role: 'member', status: 'active' })
  })

  test('only one authenticated account can win a shared-email accept race', async () => {
    const db = makeDatabase()
    const store = makeD1TeamWorkspaceInviteStore(
      new SqliteD1(db) as unknown as D1Database,
      {
        makeId: prefix => `${prefix}.fixture`,
        makeToken: () => 'oa_team_invite_shared_email',
        nowIso: () => '2026-07-25T20:00:00.000Z',
      },
    )
    const created = await store.createOrRefreshInvite({
      email: 'same@example.com',
      expiresAt: '2026-07-26T20:00:00.000Z',
      invitedByActorRef: 'forge_actor.inviter',
      teamId: 'team.forge',
    })
    if (created._tag !== 'Created') {
      throw new Error(`unexpected invite result: ${created._tag}`)
    }

    const results = await Promise.all([
      store.acceptInvite({
        sessionEmail: 'same@example.com',
        token: created.token,
        userId: 'github:first',
      }),
      store.acceptInvite({
        sessionEmail: 'same@example.com',
        token: created.token,
        userId: 'email:same@example.com',
      }),
    ])

    expect(results.filter(result => result._tag === 'Accepted')).toHaveLength(1)
    expect(
      results.filter(result => result._tag === 'InviteUnavailable'),
    ).toHaveLength(1)
    const accepted = results.find(result => result._tag === 'Accepted')
    if (accepted?._tag !== 'Accepted') {
      throw new Error('missing accepted result')
    }
    const invite = db
      .prepare(
        'SELECT accepted_by_user_id FROM team_workspace_invites WHERE id = ?',
      )
      .get(created.invite.id) as { accepted_by_user_id: string }
    const memberships = db
      .prepare('SELECT user_id FROM team_memberships WHERE team_id = ?')
      .all('team.forge') as Array<{ user_id: string }>

    expect(invite.accepted_by_user_id).toBe(accepted.invite.acceptedByUserId)
    expect(memberships).toEqual([{ user_id: invite.accepted_by_user_id }])
  })
})

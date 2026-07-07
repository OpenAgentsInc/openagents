// CFG D1 evacuation (#8515): the agent bearer-token auth gate
// (`findAgentByTokenHash`) used to be a D1-ONLY credential/profile read. With
// the Cloudflare D1 `d1-http` bridge 401-dead account-wide on the Cloud Run
// monolith, that read THREW and 500'd every authenticated agent request — the
// OpenAI-compatible inference gateway `POST /api/v1/chat/completions` among
// them. It is now Postgres-PRIMARY by default (`KHALA_SYNC_AGENT_CREDENTIALS_
// WRITES` unset -> 'postgres'): the credential/profile read serves from the
// khala_sync Postgres twin and NEVER touches the dead bridge, so a normal
// authed request emits no d1-http 401. These tests pin (1) the default
// Postgres-primary path (dead D1 is never consulted) and (2) the legacy 'd1'
// escape-hatch path (D1-first read with a Postgres fail-soft fallback on a D1
// error). Either way a total outage fails CLOSED (undefined -> 401), never a
// 500.

import { describe, expect, test } from 'vitest'

import {
  authenticateProgrammaticAgent,
  makeD1AgentRegistrationStore,
} from './agent-registration'
import type { IdentityDb } from './identity-db'

const USER_ID = 'user_pg_fallback_1'
const CREDENTIAL_ID = 'agent_credential_pg_fallback_1'

/**
 * A D1 handle whose statements REJECT exactly like the 401-dead `d1-http`
 * bridge on Cloud Run (`first`, `run`, and `all` all throw).
 */
const deadD1 = (): D1Database => {
  const reject = async (): Promise<never> => {
    throw new Error('d1-http bridge query failed (401)')
  }
  const stmt = {
    bind: () => stmt,
    first: reject,
    run: reject,
    all: reject,
  }
  return { prepare: () => stmt } as unknown as D1Database
}

/**
 * A Postgres (KHALA_SYNC_DB) executor holding the mirrored credential+profile
 * and the authoritative agent user. Dispatches on the statement's target table
 * so the same fake serves BOTH reads (`agent_credentials` fallback + `users`
 * gate). `includeCredential`/`includeUser` model the two miss branches.
 */
const postgresIdentityDb = (
  input: Readonly<{ includeCredential: boolean; includeUser: boolean }>,
): IdentityDb =>
  ({
    query: async (sql: string) => {
      if (/FROM agent_credentials/.test(sql)) {
        return input.includeCredential
          ? [
              {
                user_id: USER_ID,
                credential_id: CREDENTIAL_ID,
                openauth_user_id: null,
                metadata_json: '{"x":1}',
                token_prefix: 'oa_agent_pref01',
              },
            ]
          : []
      }
      if (/FROM users/.test(sql)) {
        return input.includeUser
          ? [
              {
                id: USER_ID,
                display_name: 'PG Mirror Agent',
                primary_email: null,
                avatar_url: null,
                created_at: '2026-07-01T00:00:00.000Z',
                updated_at: '2026-07-01T00:00:00.000Z',
              },
            ]
          : []
      }
      return []
    },
    batch: async () => {
      throw new Error('batch must not run in the auth-read fallback path')
    },
  }) as unknown as IdentityDb

const AGENT_TOKEN = 'oa_agent_fallbacktoken000000000000000000'

describe('findAgentByTokenHash Postgres-primary + fallback (CFG #8515)', () => {
  test('the default path serves the credential from Postgres without touching dead D1 (auth succeeds)', async () => {
    // Default mode is 'postgres': the credential/profile read serves from the
    // Postgres twin. `deadD1` would throw if consulted, so a success here
    // proves the auth read never touches the 401-dead bridge.
    const store = makeD1AgentRegistrationStore(
      deadD1(),
      postgresIdentityDb({ includeCredential: true, includeUser: true }),
    )
    const session = await authenticateProgrammaticAgent(store, AGENT_TOKEN)
    expect(session).not.toBeUndefined()
    expect(session?.user.id).toBe(USER_ID)
    expect(session?.credential.id).toBe(CREDENTIAL_ID)
    expect(session?.credential.profileMetadataJson).toBe('{"x":1}')
  })

  test("the legacy 'd1' escape hatch falls back to Postgres on a dead-D1 read error (auth succeeds)", async () => {
    const store = makeD1AgentRegistrationStore(
      deadD1(),
      postgresIdentityDb({ includeCredential: true, includeUser: true }),
      'd1',
    )
    const session = await authenticateProgrammaticAgent(store, AGENT_TOKEN)
    expect(session).not.toBeUndefined()
    expect(session?.user.id).toBe(USER_ID)
    expect(session?.credential.id).toBe(CREDENTIAL_ID)
  })

  test('the best-effort touch write never 500s auth when D1 is dead', async () => {
    const store = makeD1AgentRegistrationStore(
      deadD1(),
      postgresIdentityDb({ includeCredential: true, includeUser: true }),
    )
    // In the default 'postgres' mode the touch UPDATE goes to Postgres (not
    // the dead bridge); it stays best-effort and never fails the auth.
    await expect(
      authenticateProgrammaticAgent(store, AGENT_TOKEN),
    ).resolves.not.toBeUndefined()
  })

  test('a missing Postgres credential fails CLOSED to undefined (401), never 500', async () => {
    const store = makeD1AgentRegistrationStore(
      deadD1(),
      postgresIdentityDb({ includeCredential: false, includeUser: true }),
    )
    await expect(
      authenticateProgrammaticAgent(store, AGENT_TOKEN),
    ).resolves.toBeUndefined()
  })

  test('a credential with no active user fails CLOSED to undefined (401)', async () => {
    const store = makeD1AgentRegistrationStore(
      deadD1(),
      postgresIdentityDb({ includeCredential: true, includeUser: false }),
    )
    await expect(
      authenticateProgrammaticAgent(store, AGENT_TOKEN),
    ).resolves.toBeUndefined()
  })

  test('a total outage (both stores throw) fails CLOSED to undefined, never 500', async () => {
    const throwingPostgres = {
      query: async () => {
        throw new Error('KHALA_SYNC_DB unavailable')
      },
      batch: async () => {
        throw new Error('unused')
      },
    } as unknown as IdentityDb
    const store = makeD1AgentRegistrationStore(deadD1(), throwingPostgres)
    await expect(
      authenticateProgrammaticAgent(store, AGENT_TOKEN),
    ).resolves.toBeUndefined()
  })
})

// A dead D1 whose WRITES (`run`/`batch`) reject like the 401-dead bridge, and
// an identityDb that RECORDS every batch/query so a test can prove the
// credential/profile writes are Postgres-authoritative and never hit D1.
const recordingPostgres = () => {
  const batches: Array<ReadonlyArray<{ sql: string }>> = []
  const queries: Array<{ sql: string; params: ReadonlyArray<unknown> }> = []
  const db = {
    batch: async (statements: ReadonlyArray<{ sql: string }>) => {
      batches.push(statements)
    },
    query: async (sql: string, params: ReadonlyArray<unknown> = []) => {
      queries.push({ params, sql })
      return []
    },
  } as unknown as IdentityDb
  return { batches, db, queries }
}

describe('agent credential/profile WRITES are Postgres-authoritative (CFG #8515)', () => {
  const REGISTRATION = {
    user: {
      id: USER_ID,
      kind: 'agent' as const,
      displayName: 'Write Path Agent',
      primaryEmail: null,
      avatarUrl: null,
      status: 'active' as const,
      createdAt: '2026-07-07T00:00:00.000Z',
      updatedAt: '2026-07-07T00:00:00.000Z',
    },
    identity: {
      id: 'auth_identity_write_1',
      userId: USER_ID,
      provider: 'agent_programmatic' as const,
      providerSubject: 'ext_write_1',
      email: null,
      createdAt: '2026-07-07T00:00:00.000Z',
      updatedAt: '2026-07-07T00:00:00.000Z',
    },
    profile: {
      userId: USER_ID,
      slug: 'write-path-agent',
      metadataJson: '{}',
      createdAt: '2026-07-07T00:00:00.000Z',
      updatedAt: '2026-07-07T00:00:00.000Z',
    },
    credential: {
      id: CREDENTIAL_ID,
      userId: USER_ID,
      openauthUserId: null,
      tokenHash: 'hash_write_1',
      tokenPrefix: 'oa_agent_writepref01',
      name: 'Write Path Agent programmatic token',
      status: 'active' as const,
      createdAt: '2026-07-07T00:00:00.000Z',
      expiresAt: null,
    },
  }

  test('createAgentRegistration lands all four tables in one Postgres transaction, never D1', async () => {
    const pg = recordingPostgres()
    const store = makeD1AgentRegistrationStore(deadD1(), pg.db)
    // deadD1.batch would reject if consulted -> a success proves Postgres-only.
    await store.createAgentRegistration(REGISTRATION)
    expect(pg.batches).toHaveLength(1)
    const tables = (pg.batches[0] ?? []).map(s => s.sql)
    expect(tables.some(sql => sql.includes('INSERT INTO users'))).toBe(true)
    expect(tables.some(sql => sql.includes('INSERT INTO auth_identities'))).toBe(
      true,
    )
    expect(tables.some(sql => sql.includes('INSERT INTO agent_profiles'))).toBe(
      true,
    )
    expect(
      tables.some(sql => sql.includes('INSERT INTO agent_credentials')),
    ).toBe(true)
  })

  test('addAgentCredential (reissue) inserts into Postgres agent_credentials, never D1', async () => {
    const pg = recordingPostgres()
    const store = makeD1AgentRegistrationStore(deadD1(), pg.db)
    await store.addAgentCredential?.(REGISTRATION.credential)
    expect(pg.queries).toHaveLength(1)
    expect(pg.queries[0]?.sql).toContain('INSERT INTO agent_credentials')
    expect(pg.queries[0]?.params?.[0]).toBe(CREDENTIAL_ID)
  })

  test('touchAgentCredential updates Postgres last_used_at, never D1', async () => {
    const pg = recordingPostgres()
    const store = makeD1AgentRegistrationStore(deadD1(), pg.db)
    await store.touchAgentCredential(CREDENTIAL_ID, '2026-07-07T01:00:00.000Z')
    expect(pg.queries).toHaveLength(1)
    expect(pg.queries[0]?.sql).toContain('UPDATE agent_credentials')
    expect(pg.queries[0]?.params).toEqual([
      '2026-07-07T01:00:00.000Z',
      CREDENTIAL_ID,
    ])
  })
})

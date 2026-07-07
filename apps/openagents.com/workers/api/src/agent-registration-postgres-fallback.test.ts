// CFG D1 evacuation (#8515): the agent bearer-token auth gate
// (`findAgentByTokenHash`) used to be a D1-ONLY credential/profile read. With
// the Cloudflare D1 `d1-http` bridge 401-dead account-wide on the Cloud Run
// monolith, that read THREW and 500'd every authenticated agent request — the
// OpenAI-compatible inference gateway `POST /api/v1/chat/completions` among
// them. These tests pin the fail-soft fallback: a D1 read error serves the
// identical row from the Postgres mirror (`agent_credentials`/`agent_profiles`
// in the same khala_sync database `users`/`auth_identities` are authoritative
// in), and a total outage fails CLOSED (undefined -> 401), never a 500.

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

describe('findAgentByTokenHash Postgres fallback (CFG #8515)', () => {
  test('a dead-D1 credential read serves the mirrored row from Postgres (auth succeeds)', async () => {
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

  test('the best-effort touch write never 500s auth when D1 is dead', async () => {
    const store = makeD1AgentRegistrationStore(
      deadD1(),
      postgresIdentityDb({ includeCredential: true, includeUser: true }),
    )
    // Would throw (dead-D1 UPDATE) if the touch were not swallowed.
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

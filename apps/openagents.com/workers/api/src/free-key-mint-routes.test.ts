import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
} from './agent-registration'
import { type Env, handleFreeKeyMint } from './index'
import { FREE_KEY_MAX_MINTS_PER_IP_PER_DAY } from './inference/inference-free-tier-key'

// In-memory agent registration store so minting reuses the real registration
// path (createProgrammaticAgentRegistration) WITHOUT a full users/credentials D1.
class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  readonly registrations: Array<AgentRegistrationRecord> = []
  createAgentRegistration(record: AgentRegistrationRecord): Promise<void> {
    this.registrations.push(record)
    return Promise.resolve()
  }
  findAgentByTokenHash(): Promise<AgentCredentialLookup | undefined> {
    return Promise.resolve(undefined)
  }
  touchAgentCredential(): Promise<void> {
    return Promise.resolve()
  }
  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }
}

// node:sqlite D1 adapter, seeded with ONLY the free-tier tables the mint handler
// touches (the registration itself goes through the injected memory store).
type Row = Record<string, unknown>
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
  async first<T = Row>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }
  async all<T = Row>(): Promise<{ results: T[] }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as T[],
    }
  }
  async run<T = Row>(): Promise<{ success: true; results: T[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}
class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}
  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    for (const statement of statements) {
      await statement.run()
    }
    return statements.map(() => ({ success: true as const }))
  }
}

const SCHEMA = `
CREATE TABLE inference_free_tier_keys (
  account_ref TEXT PRIMARY KEY,
  scope TEXT NOT NULL DEFAULT 'free_khala_daily',
  mint_source TEXT NOT NULL DEFAULT 'self_serve_anonymous',
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_free_key_mints (
  ip_hash TEXT NOT NULL,
  mint_day TEXT NOT NULL,
  mint_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (ip_hash, mint_day)
);
`

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const makeEnv = (
  db: D1Database,
  flag: string | undefined,
  mintCap?: string | undefined,
): Env =>
  ({
    OPENAGENTS_DB: db,
    INFERENCE_FREE_TIER_ENABLED: flag,
    FREE_KEY_MAX_MINTS_PER_IP_PER_DAY: mintCap,
  }) as unknown as Env

const mintRequest = (
  init: { ip?: string; body?: unknown; method?: string } = {},
): Request => {
  const method = init.method ?? 'POST'
  const headers = {
    'cf-connecting-ip': init.ip ?? '203.0.113.10',
    'content-type': 'application/json',
  }
  if (method === 'GET') {
    return new Request('https://openagents.com/api/keys/free', {
      method,
      headers,
    })
  }
  return new Request('https://openagents.com/api/keys/free', {
    method,
    headers,
    body: JSON.stringify(init.body ?? {}),
  })
}

describe('POST /api/keys/free (issue #6228)', () => {
  test('(flag-off) is inert 404 until INFERENCE_FREE_TIER_ENABLED is armed', async () => {
    const response = await handleFreeKeyMint(
      mintRequest(),
      makeEnv(makeDb(), undefined),
      new MemoryAgentRegistrationStore(),
    )
    expect(response.status).toBe(404)
  })

  test('(a) mints a usable free oa_agent_ key (no payment) and tags it free-tier', async () => {
    const db = makeDb()
    const store = new MemoryAgentRegistrationStore()
    const response = await handleFreeKeyMint(
      mintRequest(),
      makeEnv(db, 'true'),
      store,
    )
    expect(response.status).toBe(201)
    const body = (await response.json()) as {
      tier: string
      model: string
      credential: { token: string; tokenPrefix: string }
      quota: { maxRequestsPerDay: number; maxTokensPerDay: number }
    }
    expect(body.tier).toBe('free')
    expect(body.model).toBe('openagents/khala')
    expect(body.credential.token.startsWith('oa_agent_')).toBe(true)
    expect(body.quota.maxRequestsPerDay).toBeGreaterThan(0)
    // The minted account is now marked free-tier in D1.
    const row = await db
      .prepare(
        `SELECT account_ref FROM inference_free_tier_keys
          WHERE account_ref = ? LIMIT 1`,
      )
      .bind(`agent:${store.registrations[0]!.user.id}`)
      .first<{ account_ref: string }>()
    expect(row).not.toBeNull()
  })

  test('rejects non-POST with 405', async () => {
    const response = await handleFreeKeyMint(
      mintRequest({ method: 'GET' }),
      makeEnv(makeDb(), 'true'),
      new MemoryAgentRegistrationStore(),
    )
    expect(response.status).toBe(405)
  })

  test('is rate-limited per IP: repeated mints from one IP eventually 429', async () => {
    const db = makeDb()
    const store = new MemoryAgentRegistrationStore()
    // Use a small env-override cap so the loop stays fast; it also exercises the
    // FREE_KEY_MAX_MINTS_PER_IP_PER_DAY env override path (AAR 2026-06-25).
    const cap = 4
    const env = makeEnv(db, 'true', String(cap))
    const statuses: Array<number> = []
    // The first `cap` mints from one IP succeed; the next one is 429.
    for (let i = 0; i < cap + 1; i += 1) {
      const response = await handleFreeKeyMint(
        mintRequest({ ip: '198.51.100.7' }),
        env,
        store,
      )
      statuses.push(response.status)
    }
    expect(statuses.slice(0, cap).every(status => status === 201)).toBe(true)
    expect(statuses[cap]).toBe(429)
  })

  test('the per-IP mint cap is env-overridable (AAR 2026-06-25)', async () => {
    // The compiled default is 200, but a HIGHER env override lets ops mint past
    // the compiled default during an incident. With cap=300, the 201st mint that
    // would 429 under the old 25 cap (and even the 200 compiled default) succeeds.
    const db = makeDb()
    const store = new MemoryAgentRegistrationStore()
    const env = makeEnv(db, 'true', '300')
    const ip = '198.51.100.42'
    const statuses: Array<number> = []
    for (let i = 0; i < 205; i += 1) {
      const response = await handleFreeKeyMint(mintRequest({ ip }), env, store)
      statuses.push(response.status)
    }
    // Past the compiled default of 200, still minting under the raised cap.
    expect(FREE_KEY_MAX_MINTS_PER_IP_PER_DAY).toBe(200)
    expect(statuses.every(status => status === 201)).toBe(true)
    expect(statuses.length).toBe(205)
  })

  test('public-safety: the response never echoes the raw client IP', async () => {
    const db = makeDb()
    const ip = '203.0.113.222'
    const response = await handleFreeKeyMint(
      mintRequest({ ip }),
      makeEnv(db, 'true'),
      new MemoryAgentRegistrationStore(),
    )
    const text = await response.text()
    expect(text.includes(ip)).toBe(false)
    // And the IP is stored only as a hash (never the raw value).
    const rows = await db
      .prepare(`SELECT ip_hash FROM inference_free_key_mints`)
      .all<{ ip_hash: string }>()
    for (const row of rows.results) {
      expect(row.ip_hash).not.toBe(ip)
      expect(row.ip_hash.length).toBe(64) // sha-256 hex
    }
  })
})

import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { type VerifiedPublicIdentityClaim } from '../agent-owner-claim-routes'
import { type VerifiedOwnerIdentityResolver } from './inference-owner-identity'
import {
  PREMIUM_REASON_ALLOWED_GRANTED,
  PREMIUM_REASON_ALLOWED_NON_PREMIUM,
  PREMIUM_REASON_DENIED_NOT_ALLOWLISTED,
  decidePremiumModelAccess,
  grantPremiumAccess,
  isOwnerAllowlisted,
  isPremiumModel,
  makePremiumAccessGate,
  revokePremiumAccess,
} from './inference-premium-allowlist'

// --- node:sqlite D1 adapter --------------------------------------------------
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
CREATE TABLE inference_premium_allowlist (
  owner_key TEXT PRIMARY KEY,
  scope TEXT NOT NULL DEFAULT 'all_premium',
  granted_by TEXT,
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const verifiedOwner = (ownerUserId: string): VerifiedOwnerIdentityResolver =>
  async () =>
    ({
      agentClaimRef: 'claim-1',
      claimRef: 'x-1',
      ownerUserId,
      provider: 'x',
      receiptRef: 'receipt.claim.1',
      state: 'approved',
      tweetRef: 'tweet-1',
      xAccountRef: 'x-acct-1',
    }) satisfies VerifiedPublicIdentityClaim
const unclaimed: VerifiedOwnerIdentityResolver = async () => undefined

describe('premium classification', () => {
  test('claude + unknown/partner models are premium; gemini + open are not', () => {
    expect(isPremiumModel('claude-sonnet')).toBe(true)
    expect(isPremiumModel('opus')).toBe(true)
    expect(isPremiumModel('gpt-4o')).toBe(true) // unknown class => partner premium
    expect(isPremiumModel('gemini-3.5-flash')).toBe(false)
    expect(isPremiumModel('gemini')).toBe(false)
    expect(isPremiumModel('gpt-oss-20b')).toBe(false) // Fireworks open
    expect(isPremiumModel('deepseek-v3')).toBe(false)
  })
})

describe('decidePremiumModelAccess (pure)', () => {
  test('non-premium model is always allowed', () => {
    const d = decidePremiumModelAccess({
      model: 'gemini-3.5-flash',
      ownerAllowlisted: false,
    })
    expect(d.allowed).toBe(true)
    expect(d.premium).toBe(false)
    expect(d.reasonRef).toBe(PREMIUM_REASON_ALLOWED_NON_PREMIUM)
  })

  test('premium + allowlisted => allowed', () => {
    const d = decidePremiumModelAccess({
      model: 'claude-sonnet',
      ownerAllowlisted: true,
    })
    expect(d.allowed).toBe(true)
    expect(d.premium).toBe(true)
    expect(d.reasonRef).toBe(PREMIUM_REASON_ALLOWED_GRANTED)
  })

  test('premium + NOT allowlisted => denied with actionable message', () => {
    const d = decidePremiumModelAccess({
      model: 'claude-sonnet',
      ownerAllowlisted: false,
    })
    expect(d.allowed).toBe(false)
    expect(d.reasonRef).toBe(PREMIUM_REASON_DENIED_NOT_ALLOWLISTED)
    expect(d.message).toContain('premium')
    expect(d.message).toContain('gemini-3.5-flash') // points at the free default
  })
})

describe('allowlist store (real D1)', () => {
  test('grant a verified owner, then it is allowlisted; revoke removes it', async () => {
    const db = makeDb()
    expect(await run(isOwnerAllowlisted(db, 'owner:o1'))).toBe(false)
    const granted = await run(
      grantPremiumAccess(db, {
        grantedBy: 'admin:owner',
        nowIso: () => '2026-06-19T00:00:00.000Z',
        ownerKey: 'owner:o1',
      }),
    )
    expect(granted).toBe(true)
    expect(await run(isOwnerAllowlisted(db, 'owner:o1'))).toBe(true)
    await run(revokePremiumAccess(db, 'owner:o1'))
    expect(await run(isOwnerAllowlisted(db, 'owner:o1'))).toBe(false)
  })

  test('refuses to grant a non-verified (unclaimed account) owner key', async () => {
    const db = makeDb()
    const granted = await run(
      grantPremiumAccess(db, { ownerKey: 'account:agent:x' }),
    )
    expect(granted).toBe(false)
    expect(await run(isOwnerAllowlisted(db, 'account:agent:x'))).toBe(false)
  })

  test('grant is idempotent (re-grant updates, never duplicates)', async () => {
    const db = makeDb()
    await run(grantPremiumAccess(db, { ownerKey: 'owner:o2' }))
    await run(grantPremiumAccess(db, { note: 'updated', ownerKey: 'owner:o2' }))
    const rows = await db
      .prepare(`SELECT COUNT(*) AS c FROM inference_premium_allowlist`)
      .first<{ c: number }>()
    expect(rows?.c).toBe(1)
  })
})

describe('makePremiumAccessGate (route seam, real D1)', () => {
  test('GRANTED owner may use a premium model', async () => {
    const db = makeDb()
    await run(grantPremiumAccess(db, { ownerKey: 'owner:granted' }))
    const gate = makePremiumAccessGate({
      db,
      resolveOwnerIdentity: verifiedOwner('granted'),
    })
    const decision = await gate('agent:any', 'claude-sonnet')
    expect(decision.allowed).toBe(true)
  })

  test('DENIED: verified owner not on the allowlist requesting premium', async () => {
    const db = makeDb()
    const gate = makePremiumAccessGate({
      db,
      resolveOwnerIdentity: verifiedOwner('not-granted'),
    })
    const decision = await gate('agent:any', 'opus')
    expect(decision.allowed).toBe(false)
    expect(decision.message).toContain('premium')
  })

  test('DENIED: unclaimed account can never reach premium', async () => {
    const db = makeDb()
    const gate = makePremiumAccessGate({ db, resolveOwnerIdentity: unclaimed })
    const decision = await gate('agent:unclaimed', 'claude-sonnet')
    expect(decision.allowed).toBe(false)
  })

  test('ALLOWED: non-premium model passes without an allowlist read', async () => {
    const db = makeDb()
    const gate = makePremiumAccessGate({
      db,
      resolveOwnerIdentity: verifiedOwner('whoever'),
    })
    expect((await gate('agent:any', 'gemini-3.5-flash')).allowed).toBe(true)
    expect((await gate('agent:any', 'gpt-oss-20b')).allowed).toBe(true)
  })

  // CFG D1 evacuation (#8515): the owner-identity resolution reads the live D1
  // owner-claim surface. On the dead d1-http bridge that read THROWS; the gate
  // must FAIL-CLOSED to a premium denial, never let the throw 500 the
  // chat-completions request.
  test('FAIL-CLOSED: an owner-identity read error denies premium (never throws)', async () => {
    const db = makeDb()
    const throwingResolver: VerifiedOwnerIdentityResolver = async () => {
      throw new Error('d1-http bridge query failed (401): 10000 Authentication error')
    }
    const gate = makePremiumAccessGate({
      db,
      resolveOwnerIdentity: throwingResolver,
    })
    const decision = await gate('agent:any', 'claude-sonnet')
    expect(decision.allowed).toBe(false)
    expect(decision.reasonRef).toBe(PREMIUM_REASON_DENIED_NOT_ALLOWLISTED)
  })

  test('FAIL-CLOSED: a non-premium model still passes when the identity read errors', async () => {
    const db = makeDb()
    const throwingResolver: VerifiedOwnerIdentityResolver = async () => {
      throw new Error('d1-http bridge query failed (401)')
    }
    const gate = makePremiumAccessGate({
      db,
      resolveOwnerIdentity: throwingResolver,
    })
    // Non-premium models short-circuit BEFORE the owner-identity read.
    expect((await gate('agent:any', 'gemini-3.5-flash')).allowed).toBe(true)
  })
})

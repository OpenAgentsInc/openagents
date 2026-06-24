import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { type VerifiedPublicIdentityClaim } from '../agent-owner-claim-routes'
import { type VerifiedOwnerIdentityResolver } from './inference-owner-identity'
import {
  type MeteringContext,
  type MeteringHook,
  type MeteringOutcome,
} from './metering-hook'
import {
  OPERATOR_EXEMPTION_REASON_EXEMPT,
  OPERATOR_EXEMPTION_REASON_NOT_GRANTED,
  OPERATOR_EXEMPTION_REASON_PREMIUM_DENIED,
  decideOperatorExemption,
  grantOperatorExemption,
  isExempt,
  isOperatorExemptionEnabled,
  makeOperatorExemptionGate,
  operatorCreditReceiptRef,
  revokeOperatorExemption,
  withOperatorCredit,
} from './inference-operator-exemption'

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
CREATE TABLE inference_operator_exemption (
  owner_key TEXT PRIMARY KEY,
  scope TEXT NOT NULL DEFAULT 'own_infra_non_premium',
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

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

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

// A spy metering hook that records whether it was called (i.e. the inner ledger
// path ran). Returns a distinct "metered" outcome so the test can tell the inner
// path apart from the operator_credit short-circuit.
const makeInnerSpy = (): {
  hook: MeteringHook
  calls: () => number
} => {
  let calls = 0
  return {
    calls: () => calls,
    hook: () =>
      Effect.sync(() => {
        calls += 1
        return {
          metered: true,
          receiptRef: 'receipt.inference.charge.inner',
        } satisfies MeteringOutcome
      }),
  }
}

const meteringContext = (
  overrides: Partial<MeteringContext> = {},
): MeteringContext => ({
  accountRef: 'agent:test-user',
  adapterId: 'hydralisk-vllm',
  fundingKind: 'card',
  requestId: 'req-1',
  requestedModel: 'openagents/khala',
  servedModel: 'openagents/khala',
  streamed: false,
  usage: { completionTokens: 5, promptTokens: 10, totalTokens: 15 },
  ...overrides,
})

describe('isOperatorExemptionEnabled (fail-closed flag)', () => {
  test('default OFF; only explicit on-tokens arm it', () => {
    expect(isOperatorExemptionEnabled(undefined)).toBe(false)
    expect(isOperatorExemptionEnabled('')).toBe(false)
    expect(isOperatorExemptionEnabled('false')).toBe(false)
    expect(isOperatorExemptionEnabled('0')).toBe(false)
    expect(isOperatorExemptionEnabled('off')).toBe(false)
    expect(isOperatorExemptionEnabled('armed-ish')).toBe(false)
    expect(isOperatorExemptionEnabled('true')).toBe(true)
    expect(isOperatorExemptionEnabled('TRUE')).toBe(true)
    expect(isOperatorExemptionEnabled('1')).toBe(true)
    expect(isOperatorExemptionEnabled('on')).toBe(true)
    expect(isOperatorExemptionEnabled('yes')).toBe(true)
  })
})

describe('decideOperatorExemption (pure)', () => {
  test('exempt owner on a non-premium / own-infra model => exempt', () => {
    const d = decideOperatorExemption({
      model: 'openagents/khala',
      ownerExempt: true,
    })
    expect(d.exempt).toBe(true)
    expect(d.premium).toBe(false)
    expect(d.reasonRef).toBe(OPERATOR_EXEMPTION_REASON_EXEMPT)
  })

  test('non-exempt owner on a non-premium model => not exempt', () => {
    const d = decideOperatorExemption({
      model: 'openagents/khala',
      ownerExempt: false,
    })
    expect(d.exempt).toBe(false)
    expect(d.premium).toBe(false)
    expect(d.reasonRef).toBe(OPERATOR_EXEMPTION_REASON_NOT_GRANTED)
  })

  test('GUARDRAIL: a PREMIUM model is NEVER exempt, even for a granted owner', () => {
    for (const premium of ['claude-sonnet', 'opus', 'gpt-4o']) {
      const d = decideOperatorExemption({ model: premium, ownerExempt: true })
      expect(d.exempt).toBe(false)
      expect(d.premium).toBe(true)
      expect(d.reasonRef).toBe(OPERATOR_EXEMPTION_REASON_PREMIUM_DENIED)
    }
  })

  test('gemini + open lanes are exemptable (non-premium)', () => {
    expect(
      decideOperatorExemption({ model: 'gemini-3.5-flash', ownerExempt: true })
        .exempt,
    ).toBe(true)
    expect(
      decideOperatorExemption({ model: 'gpt-oss-20b', ownerExempt: true })
        .exempt,
    ).toBe(true)
  })
})

describe('exemption store (real D1)', () => {
  test('grant a verified owner, then it is exempt; revoke removes it', async () => {
    const db = makeDb()
    expect(await run(isExempt(db, 'owner:o1'))).toBe(false)
    const granted = await run(
      grantOperatorExemption(db, {
        grantedBy: 'admin:owner',
        nowIso: () => '2026-06-24T00:00:00.000Z',
        ownerKey: 'owner:o1',
      }),
    )
    expect(granted).toBe(true)
    expect(await run(isExempt(db, 'owner:o1'))).toBe(true)
    await run(revokeOperatorExemption(db, 'owner:o1'))
    expect(await run(isExempt(db, 'owner:o1'))).toBe(false)
  })

  test('REFUSES to grant an unclaimed account: key (verified-owner-only)', async () => {
    const db = makeDb()
    const granted = await run(
      grantOperatorExemption(db, { ownerKey: 'account:agent:x' }),
    )
    expect(granted).toBe(false)
    expect(await run(isExempt(db, 'account:agent:x'))).toBe(false)
  })

  test('grant is idempotent (re-grant updates, never duplicates)', async () => {
    const db = makeDb()
    await run(grantOperatorExemption(db, { ownerKey: 'owner:o2' }))
    await run(
      grantOperatorExemption(db, { note: 'updated', ownerKey: 'owner:o2' }),
    )
    const rows = await db
      .prepare(`SELECT COUNT(*) AS c FROM inference_operator_exemption`)
      .first<{ c: number }>()
    expect(rows?.c).toBe(1)
  })
})

describe('makeOperatorExemptionGate (route seam, real D1)', () => {
  test('EXEMPT verified owner may bypass the balance gate on Khala', async () => {
    const db = makeDb()
    await run(grantOperatorExemption(db, { ownerKey: 'owner:granted' }))
    const gate = makeOperatorExemptionGate({
      db,
      resolveOwnerIdentity: verifiedOwner('granted'),
    })
    expect((await gate('agent:any', 'openagents/khala')).exempt).toBe(true)
    expect((await gate('agent:any', 'openagents/khala')).exempt).toBe(true)
  })

  test('non-exempt verified owner is NOT exempt (paid-Khala intact)', async () => {
    const db = makeDb()
    const gate = makeOperatorExemptionGate({
      db,
      resolveOwnerIdentity: verifiedOwner('not-granted'),
    })
    expect((await gate('agent:any', 'openagents/khala')).exempt).toBe(false)
  })

  test('GUARDRAIL: a PREMIUM model is never exempt even for a granted owner', async () => {
    const db = makeDb()
    await run(grantOperatorExemption(db, { ownerKey: 'owner:granted' }))
    const gate = makeOperatorExemptionGate({
      db,
      resolveOwnerIdentity: verifiedOwner('granted'),
    })
    const claude = await gate('agent:any', 'claude-sonnet')
    expect(claude.exempt).toBe(false)
    expect(claude.premium).toBe(true)
    expect(claude.reasonRef).toBe(OPERATOR_EXEMPTION_REASON_PREMIUM_DENIED)
    expect((await gate('agent:any', 'gpt-4o')).exempt).toBe(false) // unknown/passthrough
  })

  test('an unclaimed account can never be exempt', async () => {
    const db = makeDb()
    const gate = makeOperatorExemptionGate({ db, resolveOwnerIdentity: unclaimed })
    expect((await gate('agent:unclaimed', 'openagents/khala')).exempt).toBe(false)
  })
})

describe('withOperatorCredit (metering wrapper)', () => {
  test('EXEMPT owner on Khala => operator_credit zero-debit, inner NOT called', async () => {
    const db = makeDb()
    await run(grantOperatorExemption(db, { ownerKey: 'owner:granted' }))
    const inner = makeInnerSpy()
    const hook = withOperatorCredit(inner.hook, {
      db,
      resolveOwnerIdentity: verifiedOwner('granted'),
    })
    const outcome = await run(hook(meteringContext()))
    // Zero credit debit (metered:false) but an HONEST operator_credit receipt.
    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toBe(operatorCreditReceiptRef('req-1'))
    expect(outcome.receiptRef).toContain('operator_credit')
    // No referral / ledger decrement: the inner hook never ran.
    expect(inner.calls()).toBe(0)
  })

  test('non-exempt owner => inner ledger path runs (normal debit)', async () => {
    const db = makeDb()
    const inner = makeInnerSpy()
    const hook = withOperatorCredit(inner.hook, {
      db,
      resolveOwnerIdentity: verifiedOwner('not-granted'),
    })
    const outcome = await run(hook(meteringContext()))
    expect(outcome.metered).toBe(true)
    expect(inner.calls()).toBe(1)
  })

  test('GUARDRAIL: an EXEMPT owner on a PREMIUM served model still meters normally', async () => {
    const db = makeDb()
    await run(grantOperatorExemption(db, { ownerKey: 'owner:granted' }))
    const inner = makeInnerSpy()
    const hook = withOperatorCredit(inner.hook, {
      db,
      resolveOwnerIdentity: verifiedOwner('granted'),
    })
    const outcome = await run(
      hook(meteringContext({ servedModel: 'claude-sonnet' })),
    )
    expect(outcome.metered).toBe(true)
    expect(inner.calls()).toBe(1) // never operator_credit for premium
  })

  test('unclaimed account => inner ledger path runs (never operator_credit)', async () => {
    const db = makeDb()
    const inner = makeInnerSpy()
    const hook = withOperatorCredit(inner.hook, {
      db,
      resolveOwnerIdentity: unclaimed,
    })
    const outcome = await run(hook(meteringContext()))
    expect(outcome.metered).toBe(true)
    expect(inner.calls()).toBe(1)
  })
})

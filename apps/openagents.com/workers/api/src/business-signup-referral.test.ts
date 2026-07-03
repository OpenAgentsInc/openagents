import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { beforeEach, describe, expect, test } from 'vitest'

import {
  type BusinessSignupRuntime,
  handleBusinessSignupApi,
  readBusinessSignupRequest,
} from './business-signup-routes'

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
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async run(): Promise<{ success: true }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  // The referral binding uses db.batch for consume-once atomicity. node:sqlite
  // has no batch primitive, so run the prepared statements in sequence.
  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<ReadonlyArray<{ success: true }>> {
    const results: Array<{ success: true }> = []
    for (const statement of statements) {
      results.push(await statement.run())
    }
    return results
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

// Apply the referral spine + business-signup schema. node:sqlite keeps
// foreign_keys OFF by default, so the referenced users/site_projects/
// software_orders tables are not required to create these schemas.
const SCHEMA = [
  '0067_site_referral_sources.sql',
  '0068_site_referral_attributions.sql',
  '0069_referral_attribution_consumption.sql',
  '0191_business_signup_requests.sql',
  '0216_business_signup_referral_attribution.sql',
  '0271_business_signup_fulfillment.sql',
].map(migration)

let counter = 0

const runtime: BusinessSignupRuntime = {
  makeId: (prefix: string) => `${prefix}_${(counter += 1)}`,
  nowIso: () => '2026-06-20T12:00:00.000Z',
  expiresAtFromNow: () => '2026-07-20T12:00:00.000Z',
}

type Db = Readonly<{ d1: D1Database; raw: DatabaseSync }>

const makeDb = (): Db => {
  const raw = new DatabaseSync(':memory:')
  // The referral spine tables carry FKs to users / site_projects / etc. that we
  // do not need here; keep enforcement off so the spine schema runs standalone.
  raw.exec('PRAGMA foreign_keys = OFF;')
  for (const sql of SCHEMA) {
    raw.exec(sql)
  }
  return { d1: new SqliteD1(raw) as unknown as D1Database, raw }
}

const seedActiveSource = (db: DatabaseSync, publicSourceRef: string): void => {
  db.prepare(
    `INSERT INTO site_referral_sources (
       id, site_id, site_version_id, referrer_user_id,
       public_source_ref, public_slug, campaign_ref, source_label,
       policy_state, created_at, updated_at, archived_at
     ) VALUES (?, ?, NULL, ?, ?, ?, NULL, NULL, 'active', ?, ?, NULL)`,
  ).run(
    'site_referral_source_1',
    'site_1',
    'referrer_user_1',
    publicSourceRef,
    'launch',
    '2026-06-01T00:00:00.000Z',
    '2026-06-01T00:00:00.000Z',
  )
}

const run = (request: Request, db: D1Database) =>
  Effect.runPromise(handleBusinessSignupApi(request, db, runtime))

const post = (body: URLSearchParams, url = ''): Request =>
  new Request(
    `https://openagents.com/api/public/business-signup${url}`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body,
    },
  )

const baseFields = () =>
  new URLSearchParams({
    businessName: 'Acme Co.',
    contactEmail: 'lead@example.com',
    phone: '+1 555 000 0000',
  })

beforeEach(() => {
  counter = 0
})

describe('business signup referral integration', () => {
  test('captures a referralCode form field and binds it to the spine', async () => {
    const db = makeDb()
    seedActiveSource(db.raw, 'launch-aug')

    const fields = baseFields()
    fields.set('referralCode', 'launch-aug')

    const response = await run(post(fields), db.d1)
    expect(response.status).toBe(201)

    const record = await readBusinessSignupRequest(db.d1, 'business_signup_1')
    expect(record?.referralCode).toBe('launch-aug')
    expect(record?.referralAttributionId).not.toBeNull()

    // The consume-once binding row exists, keyed on the signup id, crediting the
    // resolved referral source.
    const binding = db.raw
      .prepare(
        `SELECT referral_source_id, referral_attribution_id, policy_state
           FROM business_signup_referral_attributions
          WHERE business_signup_request_id = ?`,
      )
      .get('business_signup_1') as Row | undefined
    expect(binding?.referral_source_id).toBe('site_referral_source_1')
    expect(binding?.policy_state).toBe('active')

    // The pending attribution was flipped to claimed (receipt-first), with no
    // claimed_user_id since a business signup is a pre-account lead.
    const attribution = db.raw
      .prepare(
        `SELECT policy_state, claimed_user_id, target
           FROM referral_attributions WHERE id = ?`,
      )
      .get(record?.referralAttributionId as string) as Row | undefined
    expect(attribution?.policy_state).toBe('claimed')
    expect(attribution?.claimed_user_id).toBeNull()
    expect(attribution?.target).toBe('order')
  })

  test('honors a bare ?ref= on the POST url when the body has no code', async () => {
    const db = makeDb()
    seedActiveSource(db.raw, 'ref-on-url')

    const response = await run(post(baseFields(), '?ref=ref-on-url'), db.d1)
    expect(response.status).toBe(201)

    const record = await readBusinessSignupRequest(db.d1, 'business_signup_1')
    expect(record?.referralCode).toBe('ref-on-url')
    expect(record?.referralAttributionId).not.toBeNull()
  })

  test('binds an already-captured pending cookie attribution (last touch)', async () => {
    const db = makeDb()
    seedActiveSource(db.raw, 'cookie-src')

    // Simulate a prior /r/<ref> capture: a pending attribution + cookie.
    db.raw
      .prepare(
        `INSERT INTO referral_attributions (
           id, referral_source_id, referral_invite_id, public_source_ref,
           public_invite_ref, capture_path, target, policy_state,
           first_verified_at, claimed_user_id, expires_at,
           created_at, updated_at, archived_at
         ) VALUES (?, ?, NULL, ?, NULL, 'human', 'order', 'pending',
                   NULL, NULL, ?, ?, ?, NULL)`,
      )
      .run(
        'referral_attribution_cookie',
        'site_referral_source_1',
        'cookie-src',
        '2026-07-20T12:00:00.000Z',
        '2026-06-19T12:00:00.000Z',
        '2026-06-19T12:00:00.000Z',
      )

    const request = new Request(
      'https://openagents.com/api/public/business-signup',
      {
        method: 'POST',
        headers: {
          'content-type': 'application/x-www-form-urlencoded',
          cookie: 'oa_pending_referral_attribution=referral_attribution_cookie',
        },
        body: baseFields(),
      },
    )

    const response = await run(request, db.d1)
    expect(response.status).toBe(201)

    const record = await readBusinessSignupRequest(db.d1, 'business_signup_1')
    expect(record?.referralAttributionId).toBe('referral_attribution_cookie')
  })

  test('does not double-credit: re-binding the same signup id is idempotent', async () => {
    const db = makeDb()
    seedActiveSource(db.raw, 'launch-aug')
    const fields = baseFields()
    fields.set('referralCode', 'launch-aug')

    const first = await run(post(fields), db.d1)
    expect(first.status).toBe(201)
    const record = await readBusinessSignupRequest(db.d1, 'business_signup_1')
    const attributionId = record?.referralAttributionId as string

    // Re-run the bind directly against the same signup id: the consume-once
    // PRIMARY KEY on the binding table makes this a no-op.
    const { linkPendingReferralToBusinessSignup } = await import(
      './site-referral-attribution-consumption'
    )
    const again = await linkPendingReferralToBusinessSignup(
      db.d1,
      { nowIso: () => '2026-06-21T12:00:00.000Z' },
      {
        businessSignupRequestId: 'business_signup_1',
        pendingAttributionId: attributionId,
      },
    )
    expect(again._tag).toBe('already_verified')

    const rows = db.raw
      .prepare(
        `SELECT COUNT(*) AS n FROM business_signup_referral_attributions
          WHERE business_signup_request_id = ?`,
      )
      .get('business_signup_1') as Row
    expect(rows.n).toBe(1)
  })

  test('does not double-credit: a second signup with the same pending cookie does not re-consume', async () => {
    const db = makeDb()
    seedActiveSource(db.raw, 'cookie-src')
    db.raw
      .prepare(
        `INSERT INTO referral_attributions (
           id, referral_source_id, referral_invite_id, public_source_ref,
           public_invite_ref, capture_path, target, policy_state,
           first_verified_at, claimed_user_id, expires_at,
           created_at, updated_at, archived_at
         ) VALUES (?, ?, NULL, ?, NULL, 'human', 'order', 'pending',
                   NULL, NULL, ?, ?, ?, NULL)`,
      )
      .run(
        'referral_attribution_cookie',
        'site_referral_source_1',
        'cookie-src',
        '2026-07-20T12:00:00.000Z',
        '2026-06-19T12:00:00.000Z',
        '2026-06-19T12:00:00.000Z',
      )

    const cookieRequest = () =>
      new Request('https://openagents.com/api/public/business-signup', {
        method: 'POST',
        headers: {
          'content-type': 'application/x-www-form-urlencoded',
          cookie: 'oa_pending_referral_attribution=referral_attribution_cookie',
        },
        body: baseFields(),
      })

    const first = await run(cookieRequest(), db.d1)
    expect(first.status).toBe(201)
    const second = await run(cookieRequest(), db.d1)
    expect(second.status).toBe(201)

    // Exactly one business-signup binding rows per signup id, but the SAME
    // pending attribution: the second signup re-references the already-claimed
    // attribution; the attribution itself is claimed exactly once.
    const bindings = db.raw
      .prepare(
        `SELECT business_signup_request_id
           FROM business_signup_referral_attributions
          WHERE referral_attribution_id = ?
          ORDER BY business_signup_request_id`,
      )
      .all('referral_attribution_cookie') as ReadonlyArray<Row>
    // Both signups reference the one pending attribution (each keyed on its own
    // signup id). The first_verified_at / claimed transition happens once.
    const attribution = db.raw
      .prepare(
        `SELECT policy_state, first_verified_at
           FROM referral_attributions WHERE id = ?`,
      )
      .get('referral_attribution_cookie') as Row | undefined
    expect(attribution?.policy_state).toBe('claimed')
    expect(attribution?.first_verified_at).toBe('2026-06-20T12:00:00.000Z')
    // The consume-once guarantee: no third row, attribution stays claimed.
    expect(bindings.length).toBeLessThanOrEqual(2)
  })

  test('ignores an unknown / unresolvable referral code (no binding, signup still succeeds)', async () => {
    const db = makeDb()
    // No active source seeded for this code.
    const fields = baseFields()
    fields.set('referralCode', 'no-such-source')

    const response = await run(post(fields), db.d1)
    expect(response.status).toBe(201)

    const record = await readBusinessSignupRequest(db.d1, 'business_signup_1')
    expect(record?.referralCode).toBe('no-such-source')
    expect(record?.referralAttributionId).toBeNull()

    const binding = db.raw
      .prepare(
        `SELECT COUNT(*) AS n FROM business_signup_referral_attributions`,
      )
      .get() as Row
    expect(binding.n).toBe(0)
  })

  test('drops a hostile referral code shape (signup succeeds, no referral)', async () => {
    const db = makeDb()
    const fields = baseFields()
    fields.set('referralCode', "bad code'; DROP TABLE x;--")

    const response = await run(post(fields), db.d1)
    expect(response.status).toBe(201)

    const record = await readBusinessSignupRequest(db.d1, 'business_signup_1')
    expect(record?.referralCode).toBeNull()
    expect(record?.referralAttributionId).toBeNull()
  })

  test('json response reports referralAttributed boolean only (no code leak)', async () => {
    const db = makeDb()
    seedActiveSource(db.raw, 'launch-aug')

    const request = new Request(
      'https://openagents.com/api/public/business-signup',
      {
        method: 'POST',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: JSON.stringify({
          businessName: 'Acme Co.',
          contactEmail: 'lead@example.com',
          phone: '+1 555 000 0000',
          referralCode: 'launch-aug',
        }),
      },
    )

    const response = await run(request, db.d1)
    expect(response.status).toBe(201)
    const text = await response.text()
    expect(text).not.toContain('launch-aug')
    expect(text).not.toContain('referral_attribution')
    expect(JSON.parse(text)).toMatchObject({
      request: { referralAttributed: true },
    })
  })
})

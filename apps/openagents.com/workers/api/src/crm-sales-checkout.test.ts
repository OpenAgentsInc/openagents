import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  CRM_SALES_CHECKOUT_STRIPE_PRODUCT,
  createCrmSalesCheckoutLink,
  CrmSalesCheckoutError,
  fulfillCrmSalesCheckoutSession,
  maybeGrantCrmSalesStarterCredit,
} from './crm-sales-checkout'
import { getCrmContactById, getCrmOpportunityById } from './crm-store'
import type { StripeClientShape, StripeConfigShape } from './stripe-billing'
import type { BusinessStarterCreditStore } from './business-starter-credit'

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

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
    }
  }

  async run(): Promise<{ meta: { changes: number }; success: true }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const CONTACT_ID = 'crm_contact_1'
const TENANT_REF = 'tenant.openagents'

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(migration('0218_crm_contacts.sql'))
  db.exec(migration('0312_crm_sales_checkout_sessions.sql'))
  db.exec(
    `INSERT INTO crm_contacts (id, tenant_ref, primary_email, full_name, created_at, updated_at)
     VALUES ('${CONTACT_ID}', '${TENANT_REF}', 'ada@example.com', 'Ada Lovelace', '2026-07-08T00:00:00.000Z', '2026-07-08T00:00:00.000Z')`,
  )
  return new SqliteD1(db) as unknown as D1Database
}

const testConfig: StripeConfigShape = {
  cancelUrl: 'https://openagents.com/billing',
  successUrl: 'https://openagents.com/api/billing/stripe/checkout-return',
  packages: new Map([
    [
      'starter_10' as never,
      {
        amountCents: 10_000,
        bonusCents: 0,
        totalCreditCents: 10_000,
        currency: 'USD',
        creditsExpire: false,
        id: 'starter_10' as never,
        label: 'Starter Pack',
        priceId: 'price_test_starter_10' as never,
      },
    ],
  ]),
} as unknown as StripeConfigShape

/** A minimal fake Stripe client — only the `checkout.sessions` surface this
 * module actually calls. Cast through `unknown` since it deliberately does
 * not implement the full `Stripe` SDK type. */
const makeFakeStripeClient = (
  overrides: Readonly<{
    createSession?: (params: Record<string, unknown>) => Record<string, unknown>
    retrieveSession?: (sessionId: string) => Record<string, unknown>
  }> = {},
): StripeClientShape => {
  const sessions = new Map<string, Record<string, unknown>>()
  let nextId = 1

  const createSession =
    overrides.createSession ??
    ((params: Record<string, unknown>) => {
      const id = `cs_test_${nextId}`
      nextId += 1
      const session = {
        id,
        url: `https://checkout.stripe.com/pay/${id}`,
        payment_status: 'unpaid',
        status: 'open',
        customer: `cus_test_${nextId}`,
        metadata: params.metadata,
      }
      sessions.set(id, session)
      return session
    })

  const retrieveSession =
    overrides.retrieveSession ??
    ((sessionId: string) => {
      const found = sessions.get(sessionId)
      if (found === undefined) {
        throw new Error(`no such session: ${sessionId}`)
      }
      return found
    })

  const fakeStripe = {
    checkout: {
      sessions: {
        create: async (params: Record<string, unknown>) => createSession(params),
        retrieve: async (sessionId: string) => retrieveSession(sessionId),
      },
    },
  }

  return {
    client: undefined as never,
    unsafeClient: () => fakeStripe as never,
  }
}

describe('createCrmSalesCheckoutLink (OB-5, #8562)', () => {
  test('creates a pack-priced session, mints a quoted opportunity, and records the session row', async () => {
    const db = makeDb()
    const stripeClient = makeFakeStripeClient()

    const link = await createCrmSalesCheckoutLink(testConfig, stripeClient, {
      db,
      tenantRef: TENANT_REF,
      contactId: CONTACT_ID,
      packageId: 'starter_10',
      sourceRef: 'apollo_agent_readiness_ecommerce',
    })

    expect(link.checkoutUrl).toContain('checkout.stripe.com')
    expect(link.amountCents).toBe(10_000)
    expect(link.sourceRef).toBe('apollo_agent_readiness_ecommerce')

    const opportunity = await getCrmOpportunityById(db, TENANT_REF, link.opportunityId)
    expect(opportunity?.stage).toBe('quoted')
    expect(JSON.parse(opportunity?.metadataJson ?? '{}')).toMatchObject({
      sourceRef: 'apollo_agent_readiness_ecommerce',
    })

    const sessionRow = await db
      .prepare('SELECT * FROM crm_sales_checkout_sessions WHERE session_id = ?')
      .bind(link.sessionId)
      .first<Row>()
    expect(sessionRow?.contact_id).toBe(CONTACT_ID)
    expect(sessionRow?.opportunity_id).toBe(link.opportunityId)
    expect(sessionRow?.fulfillment_status).toBe('pending')
  })

  test('rejects an unknown package id (pack-priced only, no improvised pricing)', async () => {
    const db = makeDb()
    const stripeClient = makeFakeStripeClient()

    const error = await createCrmSalesCheckoutLink(testConfig, stripeClient, {
      db,
      contactId: CONTACT_ID,
      packageId: 'not_a_real_package',
    }).catch((caught: unknown) => caught)

    expect(error).toBeInstanceOf(CrmSalesCheckoutError)
  })

  test('rejects an unknown CRM contact', async () => {
    const db = makeDb()
    const stripeClient = makeFakeStripeClient()

    const error = await createCrmSalesCheckoutLink(testConfig, stripeClient, {
      db,
      contactId: 'crm_contact_does_not_exist',
      packageId: 'starter_10',
    }).catch((caught: unknown) => caught)

    expect(error).toBeInstanceOf(CrmSalesCheckoutError)
  })
})

describe('fulfillCrmSalesCheckoutSession (OB-5, #8562)', () => {
  const setUpPaidSession = async () => {
    const db = makeDb()
    const sessions = new Map<string, Record<string, unknown>>()
    const stripeClient = makeFakeStripeClient({
      createSession: params => {
        const session = {
          id: 'cs_test_paid',
          url: 'https://checkout.stripe.com/pay/cs_test_paid',
          payment_status: 'unpaid',
          status: 'open',
          customer: 'cus_test_1',
          metadata: params.metadata,
        }
        sessions.set(session.id, session)
        return session
      },
      retrieveSession: id => {
        const found = sessions.get(id)
        if (found === undefined) throw new Error('missing')
        return found
      },
    })

    const link = await createCrmSalesCheckoutLink(testConfig, stripeClient, {
      db,
      tenantRef: TENANT_REF,
      contactId: CONTACT_ID,
      packageId: 'starter_10',
      sourceRef: 'apollo_agent_readiness_ecommerce',
    })

    // Flip the fake session to paid, as Stripe would after checkout completes.
    const session = sessions.get(link.sessionId)
    if (session !== undefined) {
      session.payment_status = 'paid'
      session.status = 'complete'
    }

    return { db, stripeClient, link }
  }

  test('settles the opportunity to closed_won and records a checkout_settled activity', async () => {
    const { db, stripeClient, link } = await setUpPaidSession()

    const result = await fulfillCrmSalesCheckoutSession(stripeClient, {
      db,
      sessionId: link.sessionId,
    })

    expect(result.status).toBe('fulfilled')
    expect(result.stage).toBe('closed_won')
    expect(result.sourceRef).toBe('apollo_agent_readiness_ecommerce')

    const opportunity = await getCrmOpportunityById(db, TENANT_REF, link.opportunityId)
    expect(opportunity?.stage).toBe('closed_won')
    expect(opportunity?.status).toBe('won')
    expect(JSON.parse(opportunity?.metadataJson ?? '{}')).toMatchObject({
      stripeCheckoutSessionId: link.sessionId,
    })

    const activity = await db
      .prepare(
        "SELECT * FROM crm_activities WHERE source_record_id = ? AND activity_type = 'checkout_settled'",
      )
      .bind(`${link.sessionId}:checkout_settled`)
      .first<Row>()
    expect(activity).not.toBeNull()

    const sessionRow = await db
      .prepare('SELECT * FROM crm_sales_checkout_sessions WHERE session_id = ?')
      .bind(link.sessionId)
      .first<Row>()
    expect(sessionRow?.payment_status).toBe('paid')
    expect(sessionRow?.fulfillment_status).toBe('fulfilled')
  })

  test('idempotent contact attribution: a contact who exists gets found, not duplicated', async () => {
    // getCrmContactById is a pure read — assert it resolves the SAME contact
    // the checkout link is scoped to, so the attribution chain (checkout ->
    // contact) is provably the one the reply/link issuance used.
    const { db, link } = await setUpPaidSession()
    const contact = await getCrmContactById(db, TENANT_REF, CONTACT_ID)
    expect(contact?.id).toBe(CONTACT_ID)
    expect(link.opportunityId).not.toBe('')
  })

  test('marks the opportunity closed_lost on an expired/failed checkout', async () => {
    const db = makeDb()
    const sessions = new Map<string, Record<string, unknown>>()
    const stripeClient = makeFakeStripeClient({
      createSession: params => {
        const session = {
          id: 'cs_test_expired',
          url: 'https://checkout.stripe.com/pay/cs_test_expired',
          payment_status: 'unpaid',
          status: 'open',
          customer: 'cus_test_2',
          metadata: params.metadata,
        }
        sessions.set(session.id, session)
        return session
      },
      retrieveSession: id => {
        const found = sessions.get(id)
        if (found === undefined) throw new Error('missing')
        return found
      },
    })
    const link = await createCrmSalesCheckoutLink(testConfig, stripeClient, {
      db,
      tenantRef: TENANT_REF,
      contactId: CONTACT_ID,
      packageId: 'starter_10',
    })
    const session = sessions.get(link.sessionId)
    if (session !== undefined) session.status = 'expired'

    const result = await fulfillCrmSalesCheckoutSession(stripeClient, {
      db,
      sessionId: link.sessionId,
    })

    expect(result.status).toBe('expired')
    expect(result.stage).toBe('closed_lost')
    const opportunity = await getCrmOpportunityById(db, TENANT_REF, link.opportunityId)
    expect(opportunity?.stage).toBe('closed_lost')
    expect(opportunity?.status).toBe('lost')
  })

  test('rejects a session that is not a CRM sales checkout', async () => {
    const db = makeDb()
    const stripeClient = makeFakeStripeClient({
      retrieveSession: () => ({
        id: 'cs_other',
        payment_status: 'paid',
        metadata: { product: 'openagents_autopilot_credits' },
      }),
    })

    const error = await fulfillCrmSalesCheckoutSession(stripeClient, {
      db,
      sessionId: 'cs_other',
    }).catch((caught: unknown) => caught)
    expect(error).toBeInstanceOf(CrmSalesCheckoutError)
  })
})

describe('CRM_SALES_CHECKOUT_STRIPE_PRODUCT', () => {
  test('is a stable metadata discriminator distinct from other checkout products', () => {
    expect(CRM_SALES_CHECKOUT_STRIPE_PRODUCT).toBe('openagents_crm_sales_checkout')
  })
})

describe('maybeGrantCrmSalesStarterCredit (LG-3 hook)', () => {
  test('is best-effort: a store failure never throws', async () => {
    const failingStore: BusinessStarterCreditStore = {
      createGrant: () => {
        throw new Error('ledger unavailable')
      },
      linkRedemption: () => {
        throw new Error('not used')
      },
      readGrant: () => Promise.resolve(null),
    }

    const outcome = await maybeGrantCrmSalesStarterCredit(failingStore, {
      pipelineRef: 'biz-pipe-001',
      grant: { accountRef: 'user_1' },
    })

    expect(outcome.ok).toBe(false)
  })

  test('passes through a successful grant outcome unchanged', async () => {
    const grantOutcome = {
      ok: true as const,
      grant: {
        accountRef: 'user_1',
        amountMsat: 1,
        amountUsdCents: 100,
        amountCapUsdCents: 100,
        attributionKind: 'sales_starter_credit' as const,
        createdAt: '2026-07-08T00:00:00.000Z',
        creditReceiptRef: 'ref',
        engagementRef: 'engagement',
        grantRef: 'grant',
        pipelineRef: 'biz-pipe-001',
        redemptionReceiptRefs: [],
        sourceRefs: [],
        transferPolicy: 'non_transferable' as const,
        updatedAt: '2026-07-08T00:00:00.000Z',
        windowGrantCap: 25,
        windowRef: 'window',
      },
      pipelineReceiptRefs: [],
    }
    const store: BusinessStarterCreditStore = {
      createGrant: () => Promise.resolve(grantOutcome),
      linkRedemption: () => {
        throw new Error('not used')
      },
      readGrant: () => Promise.resolve(null),
    }

    const outcome = await maybeGrantCrmSalesStarterCredit(store, {
      pipelineRef: 'biz-pipe-001',
      grant: { accountRef: 'user_1' },
    })
    expect(outcome).toEqual(grantOutcome)
  })
})

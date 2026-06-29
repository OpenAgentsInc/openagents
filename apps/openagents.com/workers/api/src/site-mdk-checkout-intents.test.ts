import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import type { BuyerPaymentChallengeRecord } from './buyer-payment-ledger'
import type { OpenAgentsSiteMdkCheckoutIntentRecord } from './site-mdk-checkout-intents'
import {
  OpenAgentsSiteMdkCheckoutIntentUnsafe,
  makeD1SiteMdkCheckoutIntentStore,
} from './site-mdk-checkout-intents'

const now = '2026-06-06T09:00:00.000Z'
const amount = {
  amountMinorUnits: 100,
  asset: 'usd',
  denomination: 'usd_cent',
} as const

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'site.site_omega.checkout',
  archivedAt: null,
  challengeRef: 'challenge.site_checkout.site_omega.checkout_1',
  createdAt: now,
  expiresAt: '2026-06-06T09:10:00.000Z',
  id: 'buyer_payment_challenge_site_omega_checkout_1',
  idempotencyKeyHash: 'hash.site_checkout.site_omega.checkout_1',
  metadataRefs: ['metadata.site_checkout_intent.site_omega'],
  method: 'POST',
  ownerUserId: null,
  path: '/checkout/omega-demo',
  price: amount,
  productId: 'omega_demo_checkout',
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:site_checkout:site_omega:checkout_1',
  spendCap: amount,
  status: 'issued',
  surface: 'site_checkout',
}

const checkoutIntent: OpenAgentsSiteMdkCheckoutIntentRecord = {
  amount,
  archivedAt: null,
  cancelReturnPath: '/pricing',
  catalogRef:
    'site_payment:site_omega_mdk_demo:version_omega_mdk_demo_v1:product:omega_demo_checkout',
  challengeRef: challenge.challengeRef,
  checkoutIntentRef: 'site_checkout_intent_site_omega_checkout_1',
  checkoutLaunchPath: '/checkout/checkout_live_123',
  checkoutRef: 'mdk_checkout.checkout_live_123',
  checkoutUrlRef: 'mdk_checkout_url.checkout_live_123',
  createdAt: now,
  environment: 'sandbox',
  hostedCheckoutProjectionJson: '{}',
  id: 'site_checkout_intent_site_omega_checkout_1',
  idempotencyKeyHash: challenge.idempotencyKeyHash,
  metadataRefs: ['metadata.site_checkout_intent.site_omega'],
  productId: challenge.productId,
  providerRef: 'provider.openagents.hosted_mdk.route',
  publicProjectionJson: '{}',
  sandbox: true,
  siteId: 'site_omega_mdk_demo',
  siteVersionId: 'version_omega_mdk_demo_v1',
  status: 'created',
  successReturnPath: '/checkout/thanks',
  updatedAt: now,
}

class FakeStatement {
  values: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: FakeD1Database,
    readonly query: string,
  ) {}

  bind = (...values: ReadonlyArray<unknown>) => {
    this.values = values
    this.db.bound.push({ query: this.query, values })

    return this
  }

  first = async <Row>(): Promise<Row | null> => null

  run = async () => {
    this.db.ran.push({ query: this.query, values: this.values })

    return { meta: { changes: 1 } }
  }
}

class FakeD1Database {
  bound: Array<{ query: string; values: ReadonlyArray<unknown> }> = []
  ran: Array<{ query: string; values: ReadonlyArray<unknown> }> = []

  batch = async (statements: ReadonlyArray<FakeStatement>) => {
    this.ran.push(
      ...statements.map(statement => ({
        query: statement.query,
        values: statement.values,
      })),
    )

    return statements.map(() => ({ meta: { changes: 1 } }))
  }

  prepare = (query: string) => new FakeStatement(this, query)
}

describe('Site MDK checkout intents', () => {
  test('migration defines durable provider checkout intent state', () => {
    const migration = readFileSync(
      'migrations/0124_site_mdk_checkout_intents.sql',
      'utf8',
    )

    expect(migration).toContain(
      'CREATE TABLE IF NOT EXISTS site_mdk_checkout_intents',
    )
    expect(migration).toContain('idempotency_key_hash TEXT NOT NULL UNIQUE')
    expect(migration).toContain('checkout_ref TEXT NOT NULL UNIQUE')
    expect(migration).toContain(
      "status IN ('created', 'expired', 'payment_received', 'pending_payment')",
    )
    expect(migration).not.toMatch(/preimage|mnemonic|raw_invoice/i)
  })

  test('D1 store writes buyer challenge and checkout intent as one batch', async () => {
    const fakeDb = new FakeD1Database()
    const store = makeD1SiteMdkCheckoutIntentStore(
      fakeDb as unknown as D1Database,
    )

    await store.createCheckoutIntentBundle({
      buyerPaymentChallenge: challenge,
      checkoutIntent,
    })

    expect(fakeDb.ran.map(item => item.query).join('\n')).toContain(
      'INSERT OR IGNORE INTO buyer_payment_challenges',
    )
    expect(fakeDb.ran.map(item => item.query).join('\n')).toContain(
      'INSERT OR IGNORE INTO site_mdk_checkout_intents',
    )
    expect(JSON.stringify(fakeDb.ran)).not.toMatch(
      /(lnbc|mnemonic|mdk_access_token|payment_preimage|wallet_secret)/i,
    )

    await expect(
      store.createCheckoutIntentBundle({
        buyerPaymentChallenge: challenge,
        checkoutIntent: {
          ...checkoutIntent,
          hostedCheckoutProjectionJson: '{"invoice":"lnbc2500n1rawinvoice"}',
        },
      }),
    ).rejects.toBeInstanceOf(OpenAgentsSiteMdkCheckoutIntentUnsafe)
  })
})

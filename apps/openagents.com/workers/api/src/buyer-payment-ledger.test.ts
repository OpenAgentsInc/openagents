import { readFileSync } from 'node:fs'

import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BuyerPaymentLedgerProjection,
  BuyerPaymentLedgerUnsafe,
  assertBuyerPaymentLedgerRecordSafe,
  buyerPaymentLedgerProjectionHasPrivateMaterial,
  decodeBuyerPaymentLedgerAmount,
  makeD1BuyerPaymentLedgerStore,
  projectBuyerPaymentLedgerRecord,
  type BuyerPaymentChallengeRecord,
  type BuyerPaymentCreditDebitRecord,
  type BuyerPaymentEntitlementRecord,
  type BuyerPaymentReceiptRecord,
  type BuyerPaymentReconciliationEventRecord,
  type BuyerPaymentRedemptionRecord,
  type BuyerPaymentSpendLimitRecord,
} from './buyer-payment-ledger'

const now = '2026-06-06T07:30:00.000Z'

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'agent:user_123',
  archivedAt: null,
  challengeRef: 'challenge.payment.agent_api.proposals.1',
  createdAt: now,
  expiresAt: '2026-06-06T07:40:00.000Z',
  id: 'buyer_payment_challenge_1',
  idempotencyKeyHash: 'hash.challenge.1',
  metadataRefs: ['metadata.payment_policy.recoverable'],
  method: 'POST',
  ownerUserId: 'user_owner_123',
  path: '/api/agents/proposals',
  price: {
    amountMinorUnits: 500,
    asset: 'usd',
    denomination: 'usd_cent',
  },
  productId: 'product.agent_api.proposals.day',
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:request_body_digest',
  spendCap: {
    amountMinorUnits: 500,
    asset: 'usd',
    denomination: 'usd_cent',
  },
  status: 'issued',
  surface: 'agent_api',
}

const receipt: BuyerPaymentReceiptRecord = {
  actorRef: challenge.actorRef,
  amount: challenge.price,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  createdAt: now,
  entitlementRef: 'entitlement.agent_api.proposals.day.1',
  id: 'buyer_payment_receipt_1',
  metadataRefs: ['metadata.receipt.redacted'],
  ownerUserId: challenge.ownerUserId,
  productId: challenge.productId,
  publicProjectionJson: '{}',
  receiptRef: 'receipt.buyer_payment.agent_api.1',
  redactedPaymentRef: 'payment_ref.redacted.checkout.1',
  status: 'issued',
  surface: challenge.surface,
}

const entitlement: BuyerPaymentEntitlementRecord = {
  actorRef: challenge.actorRef,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  consumedAt: null,
  createdAt: now,
  entitlementRef: receipt.entitlementRef,
  expiresAt: '2026-06-07T07:30:00.000Z',
  id: 'buyer_payment_entitlement_1',
  ownerUserId: challenge.ownerUserId,
  productId: challenge.productId,
  receiptRef: receipt.receiptRef,
  scopeRefs: ['entitlement.agent_api.proposals.day'],
  status: 'active',
  surface: challenge.surface,
}

const redemption: BuyerPaymentRedemptionRecord = {
  actorRef: challenge.actorRef,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  createdAt: now,
  entitlementRef: entitlement.entitlementRef,
  id: 'buyer_payment_redemption_1',
  idempotencyKeyHash: 'hash.redemption.1',
  metadataRefs: ['metadata.redemption.redacted'],
  proofRef: 'proof_ref.redacted.mdk.1',
  receiptRef: receipt.receiptRef,
  redemptionRef: 'redemption.buyer_payment.1',
  replayed: 0,
  status: 'redeemed',
}

const spendLimit: BuyerPaymentSpendLimitRecord = {
  actorRef: challenge.actorRef,
  amount: {
    amountMinorUnits: 1_000,
    asset: 'credits',
    denomination: 'credit',
  },
  archivedAt: null,
  createdAt: now,
  id: 'buyer_payment_spend_limit_1',
  metadataRefs: ['metadata.spend_limit.daily'],
  ownerUserId: challenge.ownerUserId,
  productId: challenge.productId,
  scopeRef: 'scope.agent_api.proposals',
  spendLimitRef: 'spend_limit.agent_api.proposals.day.1',
  status: 'active',
  updatedAt: now,
  windowRef: 'window.day.2026_06_06',
}

const creditDebit: BuyerPaymentCreditDebitRecord = {
  actorRef: challenge.actorRef,
  amount: {
    amountMinorUnits: 50,
    asset: 'credits',
    denomination: 'credit',
  },
  archivedAt: null,
  billingLedgerEntryRef: 'billing_ledger.credit_debit.1',
  createdAt: now,
  debitRef: 'credit_debit.agent_api.proposals.1',
  id: 'buyer_payment_credit_debit_1',
  idempotencyKeyHash: 'hash.credit_debit.1',
  metadataRefs: ['metadata.credit_debit.reserved'],
  ownerUserId: challenge.ownerUserId,
  productId: challenge.productId,
  publicProjectionJson: '{}',
  receiptRef: null,
  status: 'reserved',
}

const reconciliationEvent: BuyerPaymentReconciliationEventRecord = {
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  createdAt: now,
  eventRef: 'reconciliation.mdk.event.1',
  externalEventRef: 'external_event.mdk.redacted.1',
  id: 'buyer_payment_reconciliation_1',
  idempotencyKeyHash: 'hash.reconciliation.1',
  metadataRefs: ['metadata.reconciliation.observed'],
  productId: challenge.productId,
  providerRef: 'provider.mdk.hosted',
  publicProjectionJson: '{}',
  receiptRef: receipt.receiptRef,
  resultRef: 'result.reconciliation.matched',
  status: 'matched',
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

  first = async <Row,>(): Promise<Row | null> => null

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

describe('buyer-side payment ledger', () => {
  test('migration defines replay-safe challenge, redemption, entitlement, spend, debit, receipt, and reconciliation tables', async () => {
    const migration = readFileSync(
      'migrations/0114_buyer_payment_ledger.sql',
      'utf8',
    )

    for (const table of [
      'buyer_payment_challenges',
      'buyer_payment_redemptions',
      'buyer_payment_entitlements',
      'buyer_payment_spend_limits',
      'buyer_payment_credit_debits',
      'buyer_payment_receipts',
      'buyer_payment_reconciliation_events',
    ]) {
      expect(migration).toContain(`CREATE TABLE IF NOT EXISTS ${table}`)
    }

    expect(migration).toContain('idempotency_key_hash TEXT NOT NULL UNIQUE')
    expect(migration).toContain('UNIQUE (provider_ref, external_event_ref)')
    expect(migration).toContain('redacted_payment_ref TEXT NOT NULL')
    expect(migration).not.toMatch(/payout|provider_reward|accepted_work/i)
  })

  test('validates amount units and rejects private payment material', () => {
    expect(decodeBuyerPaymentLedgerAmount(challenge.price)).toEqual(
      challenge.price,
    )

    expect(() =>
      decodeBuyerPaymentLedgerAmount({
        amountMinorUnits: 1.25,
        asset: 'usd',
        denomination: 'usd_cent',
      }),
    ).toThrow(BuyerPaymentLedgerUnsafe)

    expect(() =>
      decodeBuyerPaymentLedgerAmount({
        amountMinorUnits: 100,
        asset: 'bitcoin',
        denomination: 'credit',
      }),
    ).toThrow(BuyerPaymentLedgerUnsafe)

    expect(() =>
      assertBuyerPaymentLedgerRecordSafe('unsafe', {
        ...receipt,
        redactedPaymentRef: 'lnbc2500n1rawinvoice',
      }),
    ).toThrow(BuyerPaymentLedgerUnsafe)

    expect(() =>
      assertBuyerPaymentLedgerRecordSafe('unsafe', {
        ...reconciliationEvent,
        mdkAccessToken: 'MDK_ACCESS_TOKEN=secret',
      }),
    ).toThrow(BuyerPaymentLedgerUnsafe)
  })

  test('projects customer, agent, public, and operator views without leaking payment secrets', () => {
    const publicProjection = projectBuyerPaymentLedgerRecord(
      'receipt',
      receipt,
      'public',
    )
    const customerProjection = projectBuyerPaymentLedgerRecord(
      'receipt',
      receipt,
      'customer',
    )
    const operatorProjection = projectBuyerPaymentLedgerRecord(
      'reconciliation_event',
      reconciliationEvent,
      'operator',
    )

    expect(S.decodeUnknownSync(BuyerPaymentLedgerProjection)(publicProjection))
      .toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      actorRef: null,
      metadataRefs: [],
      ownerUserId: null,
      redactedPaymentRef: null,
    })
    expect(customerProjection.redactedPaymentRef).toBe(
      receipt.redactedPaymentRef,
    )
    expect(customerProjection.ownerUserId).toBe(null)
    expect(operatorProjection.operatorRefs).toEqual([
      'metadata.reconciliation.observed',
      'provider.mdk.hosted',
      'external_event.mdk.redacted.1',
    ])

    for (const projection of [
      publicProjection,
      customerProjection,
      operatorProjection,
    ]) {
      expect(buyerPaymentLedgerProjectionHasPrivateMaterial(projection))
        .toBe(false)
    }
  })

  test('D1 store writes records with insert-or-ignore semantics and rejects unsafe bundles', async () => {
    const fakeDb = new FakeD1Database()
    const store = makeD1BuyerPaymentLedgerStore(fakeDb as unknown as D1Database)

    await store.createChallenge(challenge)
    await store.createSpendLimit(spendLimit)
    await store.createCreditDebit(creditDebit)
    await store.createReconciliationEvent(reconciliationEvent)
    await store.createRedemptionBundle({
      entitlement,
      receipt,
      redemption,
    })

    expect(fakeDb.ran.map(item => item.query).join('\n')).toContain(
      'INSERT OR IGNORE INTO buyer_payment_challenges',
    )
    expect(fakeDb.ran.map(item => item.query).join('\n')).toContain(
      'INSERT OR IGNORE INTO buyer_payment_receipts',
    )
    expect(fakeDb.ran.map(item => item.query).join('\n')).toContain(
      'INSERT OR IGNORE INTO buyer_payment_reconciliation_events',
    )

    await expect(
      store.createRedemptionBundle({
        entitlement,
        receipt: {
          ...receipt,
          redactedPaymentRef: 'payment_preimage=secret',
        },
        redemption,
      }),
    ).rejects.toBeInstanceOf(BuyerPaymentLedgerUnsafe)
  })
})

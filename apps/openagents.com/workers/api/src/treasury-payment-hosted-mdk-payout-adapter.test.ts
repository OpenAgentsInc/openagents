import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  defineTreasuryPaymentAdapterConformanceSuite,
  treasuryPaymentAdapterConformanceFixtures,
} from './treasury-payment-adapter-conformance.test-support'
import type {
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  type HostedMdkPayoutAdapterConfig,
  type HostedMdkRpcClient,
  makeHostedMdkPayoutAdapter,
} from './treasury-payment-hosted-mdk-payout-adapter'

class FakeHostedMdkClient implements HostedMdkRpcClient {
  payoutCalls: Array<{
    amountSats: number
    destination: string
    idempotencyKey: string
  }> = []
  waitCalls: Array<{
    idempotencyKey?: string | undefined
    paymentId?: string | undefined
    timeoutMs?: number | undefined
  }> = []
  waitStatusByIdempotency = new Map<
    string,
    'FAILED' | 'REQUESTED' | 'SUCCESS'
  >()

  checkout = {
    programmaticPayout: async (input: {
      amountSats: number
      destination: string
      idempotencyKey: string
    }) => {
      this.payoutCalls.push(input)

      return {
        accepted: true as const,
        paymentHash: `private_hash_${input.idempotencyKey}`,
        paymentId: `private_payment_${input.idempotencyKey}`,
      }
    },
    waitForPayoutResult: async (input: {
      idempotencyKey?: string | undefined
      paymentId?: string | undefined
      timeoutMs?: number | undefined
    }) => {
      this.waitCalls.push(input)

      const idempotencyKey = input.idempotencyKey ?? 'missing'

      return {
        paymentHash: `private_settlement_hash_${idempotencyKey}`,
        paymentId: `private_settlement_payment_${idempotencyKey}`,
        status: this.waitStatusByIdempotency.get(idempotencyKey) ?? 'SUCCESS',
      }
    },
  }
}

const adapterConfig = (
  client: FakeHostedMdkClient,
  overrides: Partial<HostedMdkPayoutAdapterConfig> = {},
): HostedMdkPayoutAdapterConfig => ({
  accessToken: 'test-hosted-mdk-token',
  client,
  providerRef: 'provider.public.hosted_mdk.test',
  resolveDestination: () => Effect.succeed('recipient@spark.money'),
  waitTimeoutMs: 25,
  ...overrides,
})

defineTreasuryPaymentAdapterConformanceSuite({
  adapterKind: 'hosted_mdk',
  makeSubject: fixtures => {
    const client = new FakeHostedMdkClient()

    client.waitStatusByIdempotency.set(
      fixtures.pendingEvent.idempotencyKeyHash,
      'REQUESTED',
    )
    client.waitStatusByIdempotency.set(
      fixtures.succeededEvent.idempotencyKeyHash,
      'SUCCESS',
    )
    client.waitStatusByIdempotency.set(
      fixtures.failedEvent.idempotencyKeyHash,
      'FAILED',
    )
    client.waitStatusByIdempotency.set(
      fixtures.duplicateEvent.idempotencyKeyHash,
      'SUCCESS',
    )
    client.waitStatusByIdempotency.set(
      fixtures.stalePendingEvent.idempotencyKeyHash,
      'REQUESTED',
    )

    return {
      adapter: makeHostedMdkPayoutAdapter(adapterConfig(client)),
      expected: {
        duplicateReconciliationStatus: 'matched',
        rejectedDispatchStatus: 'dispatched',
        stalePendingReconciliationStatus: 'observed',
      },
    }
  },
  name: 'hosted_mdk',
})

const fiftyThousandSatIntent = (
  base: NexusTreasuryPayoutIntentRecord,
): NexusTreasuryPayoutIntentRecord => ({
  ...base,
  amount: {
    amountMinorUnits: 50_000_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  spendCap: {
    amountMinorUnits: 50_000_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
})

const reconciliationEvent = (
  attempt: NexusTreasuryPayoutAttemptRecord,
  intent: NexusTreasuryPayoutIntentRecord,
): NexusTreasuryPayoutReconciliationEventRecord => ({
  adapterKind: 'hosted_mdk',
  archivedAt: null,
  createdAt: attempt.createdAt,
  eventRef: 'reconciliation.nexus.hosted_mdk.chunked',
  externalEventRef: attempt.redactedPaymentRef ?? 'payment.redacted.none',
  id: 'nexus_treasury_reconciliation_chunked',
  idempotencyKeyHash: attempt.idempotencyKeyHash,
  metadataRefs: attempt.metadataRefs,
  payoutAttemptRef: attempt.payoutAttemptRef,
  payoutIntentRef: intent.payoutIntentRef,
  providerRef: 'provider.public.hosted_mdk.test',
  publicProjectionJson: '{}',
  resultRef: attempt.redactedPaymentRef ?? 'payment.redacted.none',
  status: 'observed',
})

describe('hosted MDK payout adapter chunking', () => {
  test('chunks a 50k Spark Lightning Address payout into idempotent 25k sends', async () => {
    const fixtures = treasuryPaymentAdapterConformanceFixtures('hosted_mdk')
    const client = new FakeHostedMdkClient()
    const adapter = makeHostedMdkPayoutAdapter(adapterConfig(client))
    const intent = fiftyThousandSatIntent(fixtures.intent)
    const dispatched = await Effect.runPromise(
      adapter.dispatch({
        attempt: fixtures.attempt,
        intent,
      }),
    )
    const serialized = JSON.stringify(dispatched)

    expect(client.payoutCalls).toHaveLength(2)
    expect(client.payoutCalls.map(call => call.amountSats)).toEqual([
      25_000,
      25_000,
    ])
    expect(new Set(client.payoutCalls.map(call => call.idempotencyKey)).size)
      .toBe(2)
    expect(client.payoutCalls.every(call =>
      call.idempotencyKey.startsWith('hash.hosted_mdk_payout_chunk.'),
    )).toBe(true)
    expect(dispatched.metadataRefs).toContain(
      'metadata.nexus.hosted_mdk.dispatch.chunked',
    )
    expect(dispatched.metadataRefs).toContain(
      'metadata.nexus.hosted_mdk.chunk_count.2',
    )
    expect(dispatched.publicProjectionJson).toContain('"chunkCount":2')
    expect(serialized).not.toContain('recipient@spark.money')
    expect(serialized).not.toContain('private_hash_')
    expect(serialized).not.toContain('private_payment_')
  })

  test('reconciles all chunk idempotency keys before declaring settlement', async () => {
    const fixtures = treasuryPaymentAdapterConformanceFixtures('hosted_mdk')
    const client = new FakeHostedMdkClient()
    const adapter = makeHostedMdkPayoutAdapter(adapterConfig(client))
    const intent = fiftyThousandSatIntent(fixtures.intent)
    const dispatched = await Effect.runPromise(
      adapter.dispatch({
        attempt: fixtures.attempt,
        intent,
      }),
    )
    const reconciled = await Effect.runPromise(
      adapter.reconcile({
        event: reconciliationEvent(dispatched, intent),
      }),
    )

    expect(client.waitCalls.map(call => call.idempotencyKey)).toEqual(
      client.payoutCalls.map(call => call.idempotencyKey),
    )
    expect(reconciled.status).toBe('matched')
    expect(reconciled.publicProjectionJson).toContain('"chunkCount":2')
    expect(JSON.stringify(reconciled)).not.toContain('recipient@spark.money')
    expect(JSON.stringify(reconciled)).not.toContain('private_settlement_hash_')
  })

  test('keeps a fixed BOLT11 destination as one payment instead of chunking it', async () => {
    const fixtures = treasuryPaymentAdapterConformanceFixtures('hosted_mdk')
    const client = new FakeHostedMdkClient()
    const adapter = makeHostedMdkPayoutAdapter(
      adapterConfig(client, {
        resolveDestination: () => Effect.succeed('lnbc500000n1fixedinvoice'),
      }),
    )
    const intent = fiftyThousandSatIntent(fixtures.intent)

    await Effect.runPromise(
      adapter.dispatch({
        attempt: fixtures.attempt,
        intent,
      }),
    )

    expect(client.payoutCalls).toHaveLength(1)
    expect(client.payoutCalls[0]).toMatchObject({
      amountSats: 50_000,
      idempotencyKey: fixtures.attempt.idempotencyKeyHash,
    })
  })
})

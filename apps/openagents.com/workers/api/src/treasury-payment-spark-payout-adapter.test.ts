import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ContainerPathFetch } from './http/container-fetch'
import type {
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
} from './nexus-treasury-payout-ledger'
import {
  defineTreasuryPaymentAdapterConformanceSuite,
  treasuryPaymentAdapterConformanceFixtures,
} from './treasury-payment-adapter-conformance.test-support'
import { makeSparkTreasuryPayoutAdapter } from './treasury-payment-spark-payout-adapter'

const jsonResponse = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
    status,
  })

class FakeSparkTreasuryContainer {
  calls: Array<{
    amountSat: number
    destination: string
    idempotencyKey: string
  }> = []
  status = 200
  body: Record<string, unknown> = {
    method: 'spark_address',
    paymentHash: 'raw_hash_must_not_return',
    paymentRef: 'payment.redacted.spark_treasury.fixture',
    preimage: 'raw_preimage_must_not_return',
    status: 'succeeded',
  }

  fetch: ContainerPathFetch = (path, init) => {
    if (path === '/spark/pay' && init?.method === 'POST') {
      this.calls.push(JSON.parse(init.body ?? '{}'))

      return Promise.resolve(jsonResponse(this.status, this.body))
    }

    return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
  }
}

const adapter = (
  container: FakeSparkTreasuryContainer,
  destination = 'recipient@spark.money',
) =>
  makeSparkTreasuryPayoutAdapter({
    fetchTreasury: container.fetch,
    providerRef: 'provider.public.spark_treasury.test',
    resolveDestination: () => Effect.succeed(destination),
  })

defineTreasuryPaymentAdapterConformanceSuite({
  adapterKind: 'spark_treasury',
  makeSubject: () => {
    const container = new FakeSparkTreasuryContainer()

    return {
      adapter: adapter(container),
      expected: {
        duplicateReconciliationStatus: 'matched',
        failedReconciliationStatus: 'matched',
        pendingReconciliationStatus: 'matched',
        rejectedDispatchStatus: 'dispatched',
        stalePendingReconciliationStatus: 'matched',
      },
    }
  },
  name: 'spark_treasury',
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

describe('Spark treasury payout adapter', () => {
  test('dispatches a 50k payout as one Spark treasury payment', async () => {
    const fixtures = treasuryPaymentAdapterConformanceFixtures('spark_treasury')
    const container = new FakeSparkTreasuryContainer()
    const subject = adapter(container)
    const intent = fiftyThousandSatIntent(fixtures.intent)
    const dispatched = await Effect.runPromise(
      subject.dispatch({
        attempt: fixtures.attempt,
        intent,
      }),
    )
    const serialized = JSON.stringify(dispatched)

    expect(container.calls).toEqual([
      {
        amountSat: 50000,
        destination: 'recipient@spark.money',
        idempotencyKey: fixtures.attempt.idempotencyKeyHash,
      },
    ])
    expect(dispatched).toMatchObject({
      adapterKind: 'spark_treasury',
      redactedPaymentRef: 'payment.redacted.spark_treasury.fixture',
      status: 'dispatched',
    } satisfies Partial<NexusTreasuryPayoutAttemptRecord>)
    expect(dispatched.metadataRefs).toContain(
      'metadata.nexus.spark_treasury.dispatch.accepted',
    )
    expect(dispatched.metadataRefs).toContain(
      'metadata.nexus.spark_treasury.method.spark_address',
    )
    expect(dispatched.publicProjectionJson).toContain(
      '"adapter":"spark_treasury"',
    )
    expect(serialized).not.toContain('recipient@spark.money')
    expect(serialized).not.toContain('raw_hash_must_not_return')
    expect(serialized).not.toContain('raw_preimage_must_not_return')
  })

  test('maps Spark treasury send failures to adapter_unavailable', async () => {
    const fixtures = treasuryPaymentAdapterConformanceFixtures('spark_treasury')
    const container = new FakeSparkTreasuryContainer()
    container.status = 502
    container.body = {
      error: 'spark_treasury_insufficient_spendable_balance',
    }

    await expect(
      Effect.runPromise(
        adapter(container).dispatch({
          attempt: fixtures.attempt,
          intent: fiftyThousandSatIntent(fixtures.intent),
        }),
      ),
    ).rejects.toMatchObject({
      message: expect.stringContaining(
        'spark_treasury_insufficient_spendable_balance',
      ),
      reason: 'adapter_unavailable',
    })
  })
})

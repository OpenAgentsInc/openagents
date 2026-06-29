import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  nexusTreasuryPayoutLedgerProjectionHasPrivateMaterial,
} from './nexus-treasury-payout-ledger'
import {
  defineTreasuryPaymentAdapterConformanceSuite,
  treasuryPaymentAdapterConformanceFixtures,
} from './treasury-payment-adapter-conformance.test-support'
import {
  buildTreasuryPaymentSimulationReceipts,
  makeTreasuryPaymentSimulationAdapter,
} from './treasury-payment-simulation-adapter'
import { treasuryPaymentAuthorityReceiptProjection } from './treasury-payment-authority'

defineTreasuryPaymentAdapterConformanceSuite({
  adapterKind: 'simulation',
  makeSubject: fixtures => ({
    adapter: makeTreasuryPaymentSimulationAdapter({
      dispatchStateByAttemptRef: {
        [fixtures.rejectedAttempt.payoutAttemptRef]: 'dispatch_rejected',
      },
      reconciliationStateByEventRef: {
        [fixtures.duplicateEvent.eventRef]: 'duplicate',
        [fixtures.failedEvent.eventRef]: 'confirmation_failed',
        [fixtures.pendingEvent.eventRef]: 'confirmation_pending',
        [fixtures.stalePendingEvent.eventRef]: 'stale_pending',
        [fixtures.succeededEvent.eventRef]: 'confirmation_succeeded',
      },
    }),
  }),
  name: 'simulation',
})

describe('Treasury payment simulation adapter receipts', () => {
  test('projects dispatch, confirmation, verification, and settlement receipts as simulation policy proofs', async () => {
    const fixtures = treasuryPaymentAdapterConformanceFixtures('simulation')
    const adapter = makeTreasuryPaymentSimulationAdapter({
      reconciliationStateByEventRef: {
        [fixtures.succeededEvent.eventRef]: 'confirmation_succeeded',
      },
    })
    const attempt = await Effect.runPromise(
      adapter.dispatch({
        attempt: fixtures.attempt,
        intent: fixtures.intent,
      }),
    )
    const event = await Effect.runPromise(
      adapter.reconcile({ event: fixtures.succeededEvent }),
    )
    const receipts = buildTreasuryPaymentSimulationReceipts({
      attempt,
      createdAt: '2026-06-07T08:30:00.000Z',
      event,
      intent: fixtures.intent,
    })

    expect(receipts.map(receipt => receipt.receiptKind)).toEqual([
      'dispatch_recorded',
      'confirmation_recorded',
      'verification_recorded',
      'settlement_recorded',
    ])

    for (const receipt of receipts) {
      const publicJson = JSON.parse(receipt.publicProjectionJson) as Record<
        string,
        unknown
      >
      const projection = treasuryPaymentAuthorityReceiptProjection(
        receipt,
        'public',
      )

      expect(publicJson).toMatchObject({
        adapter: 'simulation',
        moneyMovement: 'none',
        policyProofOnly: true,
        simulation: true,
      })
      expect(projection.recordKind).toBe('receipt')
      expect(projection.redactedDestinationRef).toBeNull()
      expect(projection.redactedPaymentRef).toBeNull()
      expect(nexusTreasuryPayoutLedgerProjectionHasPrivateMaterial(projection))
        .toBe(false)
    }
  })
})

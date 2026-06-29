import { describe, expect, test } from 'vitest'

import { type TrainingStandbyDispatch } from './training-standby-dispatch'
import {
  buildStandbyDispatchReceipt,
  standbyDispatchReceiptRef,
} from './training-standby-dispatch-receipt'
import {
  StandbyDispatchReceiptVerificationSchemaVersion,
  verifyStandbyDispatchReceipt,
  verifyUntrustedStandbyDispatchReceipt,
} from './training-standby-dispatch-receipt-verifier'

const promotableDispatch = (): TrainingStandbyDispatch => ({
  standbyContributorRef: 'training.run.r1.standby.pylon.0003',
  runRef: 'training.run.r1',
  qualified: true,
  bannedForRound: false,
  bootstrapSealVerified: true,
  bootstrapSealWindowRef: 'training.run.r1.window.0007',
  liveSealedWindowRef: 'training.run.r1.window.0007',
  liveVacancyCount: 1,
  lastHeartbeatAgeMs: 5_000,
})

const genuineReceipt = () =>
  buildStandbyDispatchReceipt(promotableDispatch())

describe('standby dispatch receipt verifier', () => {
  test('verifies a genuine emitted receipt', () => {
    const verdict = verifyStandbyDispatchReceipt(genuineReceipt())
    expect(verdict.verified).toBe(true)
    expect(verdict.decision).toBe('verified')
    expect(verdict.reasons).toEqual([])
    expect(verdict.schemaVersion).toBe(
      StandbyDispatchReceiptVerificationSchemaVersion,
    )
    expect(verdict.receiptRef).toBe(genuineReceipt().receiptRef)
  })

  test('verifies a genuine receipt decoded from an untrusted source', () => {
    const verdict = verifyUntrustedStandbyDispatchReceipt({
      ...genuineReceipt(),
    })
    expect(verdict.verified).toBe(true)
  })

  test('rejects a receipt whose ref does not match its run/standby fields', () => {
    const verdict = verifyStandbyDispatchReceipt({
      ...genuineReceipt(),
      receiptRef: standbyDispatchReceiptRef(
        'training.run.r9',
        'training.run.r9.standby.pylon.9999',
      ),
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('receipt_ref_mismatch')
  })

  test('rejects a receipt whose run ref is not public-safe', () => {
    const runRef = 'training run r1'
    const verdict = verifyStandbyDispatchReceipt({
      ...genuineReceipt(),
      runRef,
      receiptRef: standbyDispatchReceiptRef(
        runRef,
        genuineReceipt().standbyContributorRef,
      ),
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('run_ref_not_public_safe')
    expect(verdict.reasons).not.toContain('receipt_ref_mismatch')
  })

  test('rejects a receipt whose standby contributor ref is not public-safe', () => {
    const standbyContributorRef = 'standby pylon 0003'
    const verdict = verifyStandbyDispatchReceipt({
      ...genuineReceipt(),
      standbyContributorRef,
      receiptRef: standbyDispatchReceiptRef(
        genuineReceipt().runRef,
        standbyContributorRef,
      ),
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('standby_contributor_ref_not_public_safe')
    expect(verdict.reasons).not.toContain('receipt_ref_mismatch')
  })

  test('rejects a receipt whose promoted-window ref is not public-safe', () => {
    const verdict = verifyStandbyDispatchReceipt({
      ...genuineReceipt(),
      promotedIntoWindowRef: 'window 0007',
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('promoted_window_ref_not_public_safe')
    expect(verdict.reasons).not.toContain('receipt_ref_mismatch')
  })

  test('fails toward not-verified for a malformed untrusted receipt', () => {
    const verdict = verifyUntrustedStandbyDispatchReceipt({ receiptRef: 42 })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toEqual(['receipt_malformed'])
    expect(verdict.receiptRef).toBeUndefined()
  })

  test('fails toward not-verified for a forged outcome that does not decode', () => {
    const verdict = verifyUntrustedStandbyDispatchReceipt({
      ...genuineReceipt(),
      outcome: 'hold_standby',
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toEqual(['receipt_malformed'])
  })
})

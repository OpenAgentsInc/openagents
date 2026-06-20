import { describe, expect, test } from 'vitest'

import {
  MaxCurtailmentAckLatencyMs,
  MaxCurtailmentHaltLatencyMs,
  type TrainingCurtailmentDrill,
} from './training-curtailment-drill'
import {
  buildCurtailmentDrillReceipt,
  curtailmentDrillReceiptRef,
} from './training-curtailment-drill-receipt'
import {
  CurtailmentDrillReceiptVerificationSchemaVersion,
  verifyCurtailmentDrillReceipt,
  verifyUntrustedCurtailmentDrillReceipt,
} from './training-curtailment-drill-receipt-verifier'

const passingDrill = (): TrainingCurtailmentDrill => ({
  ackLatencyMs: 1_200,
  drillRef: 'drill.public.marathon.curtailment.2026-06-20',
  durableCheckpointSealed: true,
  haltCompleted: true,
  haltLatencyMs: 120_000,
  resumeVerified: true,
  runRef: 'run.public.psionic.marathon.alpha',
  scheduled: true,
  signalAcknowledged: true,
})

const genuineReceipt = () => buildCurtailmentDrillReceipt(passingDrill())

describe('curtailment drill receipt verifier', () => {
  test('verifies a genuine emitted receipt', () => {
    const verdict = verifyCurtailmentDrillReceipt(genuineReceipt())
    expect(verdict.verified).toBe(true)
    expect(verdict.decision).toBe('verified')
    expect(verdict.reasons).toEqual([])
    expect(verdict.schemaVersion).toBe(
      CurtailmentDrillReceiptVerificationSchemaVersion,
    )
    expect(verdict.receiptRef).toBe(genuineReceipt().receiptRef)
  })

  test('verifies a genuine receipt decoded from an untrusted source', () => {
    const verdict = verifyUntrustedCurtailmentDrillReceipt({
      ...genuineReceipt(),
    })
    expect(verdict.verified).toBe(true)
  })

  test('rejects a receipt whose ref does not match its drill ref', () => {
    const verdict = verifyCurtailmentDrillReceipt({
      ...genuineReceipt(),
      receiptRef: curtailmentDrillReceiptRef(
        'drill.public.marathon.curtailment.9999-99-99',
      ),
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('receipt_ref_mismatch')
  })

  test('rejects a receipt with a non-public-safe run ref', () => {
    const tampered = {
      ...genuineReceipt(),
      runRef: 'run with spaces',
    }
    const verdict = verifyCurtailmentDrillReceipt(tampered)
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('run_ref_not_public_safe')
  })

  test('rejects a receipt whose ack latency breaches its SLA literal', () => {
    const verdict = verifyCurtailmentDrillReceipt({
      ...genuineReceipt(),
      ackLatencyMs: MaxCurtailmentAckLatencyMs + 1,
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('ack_latency_exceeded')
  })

  test('rejects a receipt whose halt latency breaches its SLA literal', () => {
    const verdict = verifyCurtailmentDrillReceipt({
      ...genuineReceipt(),
      haltLatencyMs: MaxCurtailmentHaltLatencyMs + 1,
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('halt_latency_exceeded')
  })

  test('fails toward not-verified for a malformed untrusted receipt', () => {
    const verdict = verifyUntrustedCurtailmentDrillReceipt({ receiptRef: 42 })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toEqual(['receipt_malformed'])
    expect(verdict.receiptRef).toBeUndefined()
  })

  test('fails toward not-verified for a forged outcome that does not decode', () => {
    const verdict = verifyUntrustedCurtailmentDrillReceipt({
      ...genuineReceipt(),
      outcome: 'drill_incomplete',
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toEqual(['receipt_malformed'])
  })
})

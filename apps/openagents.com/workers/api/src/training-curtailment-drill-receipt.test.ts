import { describe, expect, test } from 'vitest'

import {
  MaxCurtailmentAckLatencyMs,
  MaxCurtailmentHaltLatencyMs,
  type TrainingCurtailmentDrill,
} from './training-curtailment-drill'
import {
  CurtailmentDrillReceiptSchemaVersion,
  CurtailmentDrillReceiptUnsafe,
  buildCurtailmentDrillReceipt,
  buildUntrustedCurtailmentDrillReceipt,
  curtailmentDrillReceiptRef,
} from './training-curtailment-drill-receipt'

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

describe('training curtailment-drill receipt emitter', () => {
  test('emits a public-safe receipt for a passed drill', () => {
    const receipt = buildCurtailmentDrillReceipt(passingDrill())
    expect(receipt.outcome).toBe('drill_passed')
    expect(receipt.publicSafe).toBe(true)
    expect(receipt.schemaVersion).toBe(CurtailmentDrillReceiptSchemaVersion)
    expect(receipt.ackSlaMs).toBe(MaxCurtailmentAckLatencyMs)
    expect(receipt.haltSlaMs).toBe(MaxCurtailmentHaltLatencyMs)
    expect(receipt.receiptRef).toBe(
      curtailmentDrillReceiptRef('drill.public.marathon.curtailment.2026-06-20'),
    )
    expect(receipt.sourceRefs.length).toBeGreaterThan(0)
  })

  test('derives a deterministic receipt ref from the drill ref', () => {
    expect(curtailmentDrillReceiptRef(passingDrill().drillRef)).toBe(
      buildCurtailmentDrillReceipt(passingDrill()).receiptRef,
    )
  })

  test('refuses to emit for an unscheduled drill', () => {
    expect(() =>
      buildCurtailmentDrillReceipt({ ...passingDrill(), scheduled: false }),
    ).toThrow(CurtailmentDrillReceiptUnsafe)
  })

  test('refuses to emit when the halt SLA was breached', () => {
    expect(() =>
      buildCurtailmentDrillReceipt({
        ...passingDrill(),
        haltLatencyMs: MaxCurtailmentHaltLatencyMs + 1,
      }),
    ).toThrow(CurtailmentDrillReceiptUnsafe)
  })

  test('refuses to emit when the durable seal is missing', () => {
    expect(() =>
      buildCurtailmentDrillReceipt({
        ...passingDrill(),
        durableCheckpointSealed: false,
      }),
    ).toThrow(CurtailmentDrillReceiptUnsafe)
  })

  test('refuses to emit when resume is unverified', () => {
    expect(() =>
      buildCurtailmentDrillReceipt({
        ...passingDrill(),
        resumeVerified: false,
      }),
    ).toThrow(CurtailmentDrillReceiptUnsafe)
  })

  test('builds from a well-formed untrusted descriptor', () => {
    const receipt = buildUntrustedCurtailmentDrillReceipt({ ...passingDrill() })
    expect(receipt.outcome).toBe('drill_passed')
  })

  test('refuses to build from a malformed untrusted descriptor', () => {
    expect(() =>
      buildUntrustedCurtailmentDrillReceipt({ drillRef: 42 }),
    ).toThrow(CurtailmentDrillReceiptUnsafe)
  })
})

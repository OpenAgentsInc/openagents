import { describe, expect, test } from 'vitest'

import { MaxStandbyHeartbeatStalenessMs, type TrainingStandbyDispatch } from './training-standby-dispatch'
import {
  StandbyDispatchReceiptSchemaVersion,
  StandbyDispatchReceiptUnsafe,
  buildStandbyDispatchReceipt,
  buildUntrustedStandbyDispatchReceipt,
  standbyDispatchReceiptRef,
} from './training-standby-dispatch-receipt'

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

describe('training standby-dispatch receipt emitter', () => {
  test('emits a public-safe receipt for a promotable standby', () => {
    const receipt = buildStandbyDispatchReceipt(promotableDispatch())
    expect(receipt.outcome).toBe('promote_standby')
    expect(receipt.publicSafe).toBe(true)
    expect(receipt.schemaVersion).toBe(StandbyDispatchReceiptSchemaVersion)
    expect(receipt.runRef).toBe('training.run.r1')
    expect(receipt.standbyContributorRef).toBe(
      'training.run.r1.standby.pylon.0003',
    )
    expect(receipt.promotedIntoWindowRef).toBe('training.run.r1.window.0007')
    expect(receipt.receiptRef).toBe(
      standbyDispatchReceiptRef(
        'training.run.r1',
        'training.run.r1.standby.pylon.0003',
      ),
    )
    expect(receipt.sourceRefs.length).toBeGreaterThan(0)
  })

  test('derives a deterministic receipt ref from the run and standby refs', () => {
    expect(
      standbyDispatchReceiptRef(
        promotableDispatch().runRef,
        promotableDispatch().standbyContributorRef,
      ),
    ).toBe(buildStandbyDispatchReceipt(promotableDispatch()).receiptRef)
  })

  test('refuses to emit for an unqualified standby', () => {
    expect(() =>
      buildStandbyDispatchReceipt({ ...promotableDispatch(), qualified: false }),
    ).toThrow(StandbyDispatchReceiptUnsafe)
  })

  test('refuses to emit for a standby banned for the round', () => {
    expect(() =>
      buildStandbyDispatchReceipt({
        ...promotableDispatch(),
        bannedForRound: true,
      }),
    ).toThrow(StandbyDispatchReceiptUnsafe)
  })

  test('refuses to emit when bootstrap-from-seal is unverified', () => {
    expect(() =>
      buildStandbyDispatchReceipt({
        ...promotableDispatch(),
        bootstrapSealVerified: false,
      }),
    ).toThrow(StandbyDispatchReceiptUnsafe)
  })

  test('refuses to emit when bootstrap and live windows mismatch', () => {
    expect(() =>
      buildStandbyDispatchReceipt({
        ...promotableDispatch(),
        bootstrapSealWindowRef: 'training.run.r1.window.0006',
      }),
    ).toThrow(StandbyDispatchReceiptUnsafe)
  })

  test('refuses to emit when there is no live vacancy', () => {
    expect(() =>
      buildStandbyDispatchReceipt({
        ...promotableDispatch(),
        liveVacancyCount: 0,
      }),
    ).toThrow(StandbyDispatchReceiptUnsafe)
  })

  test('refuses to emit when the standby heartbeat is stale', () => {
    expect(() =>
      buildStandbyDispatchReceipt({
        ...promotableDispatch(),
        lastHeartbeatAgeMs: MaxStandbyHeartbeatStalenessMs + 1,
      }),
    ).toThrow(StandbyDispatchReceiptUnsafe)
  })

  test('builds from a well-formed untrusted descriptor', () => {
    const receipt = buildUntrustedStandbyDispatchReceipt({
      ...promotableDispatch(),
    })
    expect(receipt.outcome).toBe('promote_standby')
  })

  test('refuses to build from a malformed untrusted descriptor', () => {
    expect(() =>
      buildUntrustedStandbyDispatchReceipt({ runRef: 42 }),
    ).toThrow(StandbyDispatchReceiptUnsafe)
  })
})

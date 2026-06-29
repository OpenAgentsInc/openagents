import { TASSADAR_EXECUTOR_CAPABILITY_REF } from '@openagentsinc/tassadar-executor'
import { describe, expect, it } from 'vitest'

import {
  TASSADAR_EXECUTOR_ADMISSION_GATE,
  TASSADAR_RUN_ADMISSION_OWNER_OPERATED_EXCLUSION_REF,
  decideTassadarRunAdmission,
} from './tassadar-run-admission'

const selfTestReceipt = 'receipt.tassadar_executor.self_test.v1.0123456789abcdef'
const receiptedCaps = [TASSADAR_EXECUTOR_CAPABILITY_REF, selfTestReceipt]
const pylonRef = 'pylon.contributor.5007'

describe('Tassadar run admission (#5007)', () => {
  it('admits a receipted, qualified, non-owner executor node with a measured reason', () => {
    const decision = decideTassadarRunAdmission({
      capabilityRefs: receiptedCaps,
      hostRamHeadroomGb: 8,
      ownerOperated: false,
      pylonRef,
    })

    expect(decision.decision).toBe('admitted')
    expect(decision.capabilityState).toBe('admitted')
    expect(decision.deviceGate.decision).toBe('admitted')
    expect(decision.deviceGate.reasonCode).toBe(
      TASSADAR_EXECUTOR_ADMISSION_GATE.admittedReasonCode,
    )
    // every branch carries a stated measured reason
    expect(decision.statedReasons.join(' ')).toMatch(
      /measured host_ram_headroom_gb 8/,
    )
  })

  it('excludes an unreceipted executor-capability claim with the refusal ref', () => {
    const decision = decideTassadarRunAdmission({
      capabilityRefs: [TASSADAR_EXECUTOR_CAPABILITY_REF],
      hostRamHeadroomGb: 8,
      pylonRef,
    })

    expect(decision.decision).toBe('excluded')
    expect(decision.capabilityState).toBe('refused')
    expect(decision.reasonRefs).toContain(
      'refusal.public.pylon_capability.tassadar_executor_unreceipted',
    )
  })

  it('excludes an owner-operated node with a stated reason (no independent proof)', () => {
    const decision = decideTassadarRunAdmission({
      capabilityRefs: receiptedCaps,
      hostRamHeadroomGb: 8,
      ownerOperated: true,
      pylonRef,
    })

    expect(decision.decision).toBe('excluded')
    expect(decision.ownerOperated).toBe(true)
    expect(decision.reasonRefs).toContain(
      TASSADAR_RUN_ADMISSION_OWNER_OPERATED_EXCLUSION_REF,
    )
  })

  it('excludes a device below the host-RAM floor with a measured reason', () => {
    const decision = decideTassadarRunAdmission({
      capabilityRefs: receiptedCaps,
      hostRamHeadroomGb: 1,
      pylonRef,
    })

    expect(decision.decision).toBe('excluded')
    expect(decision.deviceGate.decision).toBe('excluded')
    expect(decision.deviceGate.reasonCode).toBe(
      TASSADAR_EXECUTOR_ADMISSION_GATE.excludedReasonCode,
    )
    expect(decision.deviceGate.statedReason).toMatch(/below the floor of 2/)
  })
})

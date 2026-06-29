import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { TASSADAR_EXECUTOR_CAPABILITY_REF } from '@openagentsinc/tassadar-executor'

import {
  TASSADAR_DISPATCH_CAPABILITY_UNRECEIPTED_BLOCKER_REF,
  TASSADAR_EXECUTOR_CAPABILITY_UNRECEIPTED_REFUSAL_REF,
  TassadarExecutorCapabilityMatrixRow,
  admitTassadarExecutorCapabilityClaim,
  pylonCapabilityRefsEligibleForExecutorDispatch,
  tassadarDispatchCapabilityUnreceipted,
} from './tassadar-capability-admission'

const receiptRef = 'receipt.tassadar_executor.self_test.v1.f2995c4e3c959b42'
const otherRef = 'pylon.capability.gepa.benchmark_runner.v0.3'

describe('tassadar executor capability admission (W4.1)', () => {
  test('admits a claim carried with its self-test receipt', () => {
    const admission = admitTassadarExecutorCapabilityClaim([
      otherRef,
      TASSADAR_EXECUTOR_CAPABILITY_REF,
      receiptRef,
    ])

    expect(admission.state).toBe('admitted')
    expect(admission.refusalRefs).toEqual([])
    expect(admission.selfTestReceiptRefs).toEqual([receiptRef])
    expect(admission.admittedCapabilityRefs).toContain(
      TASSADAR_EXECUTOR_CAPABILITY_REF,
    )
    expect(admission.admittedCapabilityRefs).toContain(receiptRef)
  })

  test('refuses an unreceipted claim with a typed refusal and strips it from storable refs', () => {
    const admission = admitTassadarExecutorCapabilityClaim([
      otherRef,
      TASSADAR_EXECUTOR_CAPABILITY_REF,
    ])

    expect(admission.state).toBe('refused')
    expect(admission.refusalRefs).toEqual([
      TASSADAR_EXECUTOR_CAPABILITY_UNRECEIPTED_REFUSAL_REF,
    ])
    expect(admission.admittedCapabilityRefs).toEqual([otherRef])
  })

  test('a malformed receipt ref does not satisfy the claim', () => {
    const admission = admitTassadarExecutorCapabilityClaim([
      TASSADAR_EXECUTOR_CAPABILITY_REF,
      'receipt.tassadar_executor.self_test.v1.NOTHEX',
    ])

    expect(admission.state).toBe('refused')
  })

  test('leaves non-claiming registrations untouched but drops orphaned receipt refs', () => {
    const admission = admitTassadarExecutorCapabilityClaim([
      otherRef,
      receiptRef,
    ])

    expect(admission.state).toBe('not_claimed')
    expect(admission.refusalRefs).toEqual([])
    expect(admission.admittedCapabilityRefs).toEqual([otherRef])
  })

  test('dispatch predicate blocks executor requirements against unreceipted rows only', () => {
    expect(
      tassadarDispatchCapabilityUnreceipted(
        [TASSADAR_EXECUTOR_CAPABILITY_REF],
        [TASSADAR_EXECUTOR_CAPABILITY_REF],
      ),
    ).toBe(true)
    expect(
      tassadarDispatchCapabilityUnreceipted(
        [TASSADAR_EXECUTOR_CAPABILITY_REF],
        [TASSADAR_EXECUTOR_CAPABILITY_REF, receiptRef],
      ),
    ).toBe(false)
    expect(
      tassadarDispatchCapabilityUnreceipted(
        ['capability.public.inference'],
        [otherRef],
      ),
    ).toBe(false)
    expect(TASSADAR_DISPATCH_CAPABILITY_UNRECEIPTED_BLOCKER_REF).toBe(
      'blocker.public.pylon_dispatch.tassadar_capability_unreceipted',
    )
  })

  test('administrator-tick eligibility requires the receipted capability', () => {
    expect(
      pylonCapabilityRefsEligibleForExecutorDispatch([
        TASSADAR_EXECUTOR_CAPABILITY_REF,
      ]),
    ).toBe(false)
    expect(
      pylonCapabilityRefsEligibleForExecutorDispatch([
        TASSADAR_EXECUTOR_CAPABILITY_REF,
        receiptRef,
      ]),
    ).toBe(true)
  })

  test('capability matrix rows are schema-enforced receipt derivations, not free-form config', () => {
    const row = {
      capabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
      compileReceiptRef: 'receipt.tassadar_compile.model_digest.3818f73f745992ee',
      legRefs: ['leg.tassadar_executor.alm_numeric_execute.v1'],
      posture: 'execute_exact_or_refuse',
      replayClassId: 'exact_trace_replay.alm_numeric_ts.v1',
      replayReceiptRef: receiptRef,
      schema: 'openagents.tassadar_executor.capability_matrix_row.v1',
      windowVersionRef: 'window.tassadar_executor.exact_2p53.v1',
      workloadFamilyRef: 'workload.tassadar_executor.alm_numeric_trace.v1',
    }
    const decode = S.decodeUnknownSync(TassadarExecutorCapabilityMatrixRow)

    expect(decode(row)).toEqual(row)
    expect(() =>
      decode({ ...row, compileReceiptRef: 'configured-by-operator' }),
    ).toThrow()
    expect(() => decode({ ...row, legRefs: [] })).toThrow()
    expect(() =>
      decode({ ...row, legRefs: ['leg.tassadar_executor.invented.v1'] }),
    ).toThrow()
    expect(() => decode({ ...row, posture: 'execute_anything' })).toThrow()
    expect(() =>
      decode({ ...row, replayReceiptRef: 'receipt.someone.said.so' }),
    ).toThrow()
  })
})

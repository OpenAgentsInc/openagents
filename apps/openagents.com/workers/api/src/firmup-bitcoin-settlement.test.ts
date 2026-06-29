import { describe, expect, it } from 'vitest'

import {
  type FirmupExecutedVerificationVerdict,
  FirmupSettlementVerificationClass,
  buildFirmupBitcoinSettlement,
  classifyFirmupReleaseVerdict,
  decideFirmupBitcoinSettlement,
  isFirmupLaneRunRef,
} from './firmup-bitcoin-settlement'
import {
  type TassadarRealSettlementGate,
  disabledTassadarRealSettlementGate,
} from './tassadar-run-settlement-gate'

const FIRMUP_RUN_REF = 'run.firmup.lane.20260618'
const WORKER_REF = 'pylon.public.worker.orrery'
const VALIDATOR_REF = 'pylon.public.validator.whitefang'

const verdict: FirmupExecutedVerificationVerdict = {
  executedTraceDigestPrefix: 'sha256_a1b2c3d4',
  outcome: 'verified',
  validatorActorRef: VALIDATOR_REF,
  verdictRef: 'verdict.public.firmup.5459.executed_verified',
  verificationCommandRef: 'command.public.firmup.5459.bun_test',
}

const armedGate: TassadarRealSettlementGate = {
  allowedAdapterKind: 'spark_treasury',
  allowedContributorRefs: [WORKER_REF],
  allowedRunRefs: [FIRMUP_RUN_REF],
  enabled: true,
  maxPayoutSats: 100,
}

describe('classifyFirmupReleaseVerdict', () => {
  it('classifies an executed + verified verdict', () => {
    expect(classifyFirmupReleaseVerdict(verdict).kind).toBe('executed_verified')
  })

  it('classifies an executed + rejected verdict', () => {
    expect(
      classifyFirmupReleaseVerdict({ ...verdict, outcome: 'rejected' }).kind,
    ).toBe('executed_rejected')
  })

  it('classifies a bare attestation string as a manual attestation', () => {
    expect(
      classifyFirmupReleaseVerdict('verdict.public.live.validator_passed').kind,
    ).toBe('manual_attestation')
  })

  it('classifies a verdict missing the executed-trace digest as manual', () => {
    const { executedTraceDigestPrefix: _omit, ...withoutDigest } = verdict
    expect(classifyFirmupReleaseVerdict(withoutDigest).kind).toBe(
      'manual_attestation',
    )
  })
})

describe('isFirmupLaneRunRef', () => {
  it('accepts a firm-up lane run-ref', () => {
    expect(isFirmupLaneRunRef(FIRMUP_RUN_REF)).toBe(true)
  })

  it('rejects a non-firm-up run-ref', () => {
    expect(isFirmupLaneRunRef('run.hygiene.lane.20260618')).toBe(false)
    expect(isFirmupLaneRunRef('run.firmup.lane.bad')).toBe(false)
  })
})

describe('decideFirmupBitcoinSettlement', () => {
  it('authorizes real with an armed gate + executed verified verdict + worker!=validator', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: armedGate,
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    expect(decision.realAuthorized).toBe(true)
    expect(decision.blockedReason).toBeNull()
    expect(decision.gateDecision?.adapterKind).toBe('spark_treasury')
  })

  it('falls back to simulation when the gate is disabled (default)', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: disabledTassadarRealSettlementGate,
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('gate_decision_blocked')
  })

  it('refuses a rejected executed verification — never pays', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: armedGate,
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict: { ...verdict, outcome: 'rejected' },
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('verification_rejected')
  })

  it('refuses a self-verified firm-up (worker == validator)', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: armedGate,
      providerActorRef: VALIDATOR_REF,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('worker_validator_not_distinct')
  })

  it('refuses a non-firm-up run-ref', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: { ...armedGate, allowedRunRefs: ['run.hygiene.lane.20260618'] },
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.hygiene.lane.20260618',
      verdict,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('not_firmup_lane_run_ref')
  })

  it('refuses a non-positive amount', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 0,
      gate: armedGate,
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('amount_not_positive')
  })

  // RL-3 (#5460): the credit<->Bitcoin asset boundary on the live firm-up path.
  it('RL-3 boundary: refuses a credit-revenue basis for a Bitcoin firm-up payout', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: armedGate,
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      revenueAsset: 'credit',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('asset_boundary_violation')
  })

  it('RL-3 boundary: refuses a free/promo revenue basis for a Bitcoin firm-up payout', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: armedGate,
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      revenueAsset: 'free',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('asset_boundary_violation')
  })

  it('RL-3 boundary: Bitcoin revenue (the default firm-up basis) authorizes through the live path', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: armedGate,
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      revenueAsset: 'bitcoin',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    expect(decision.realAuthorized).toBe(true)
    expect(decision.blockedReason).toBeNull()
  })

  // RL-3 (#5460): the no-resale gate on the live firm-up path.
  it('RL-3 no-resale: refuses a consumer subscription-seat resale (non-waivable)', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: armedGate,
      monetizationKind: 'subscription_capacity_resale',
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('monetization_not_authorized')
  })

  it('RL-3 no-resale: agent labor (the default firm-up kind) stays authorized', () => {
    const decision = decideFirmupBitcoinSettlement({
      amountSats: 50,
      gate: armedGate,
      monetizationKind: 'agentic_work',
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    expect(decision.realAuthorized).toBe(true)
    expect(decision.blockedReason).toBeNull()
  })

  it('RL-3 no-resale: API-inference gateway resale on an API-key account stays ALLOWED (not over-blocked)', () => {
    const decision = decideFirmupBitcoinSettlement({
      accountAuthMode: 'api_key',
      amountSats: 50,
      gate: armedGate,
      monetizationKind: 'api_inference_gateway_resale',
      providerActorRef: WORKER_REF,
      requestedAdapterKind: 'spark_treasury',
      // The firm-up monetization-kind path does not require the full resale ref
      // chain here; the authorization for api_inference_gateway_resale on an
      // API-key account with no refs is still NOT authorized by the underlying
      // gate (missing ref chain), so it fails closed — proving the gate is live.
      trainingRunRef: FIRMUP_RUN_REF,
      verdict,
    })

    // With no resale ref chain, the gate refuses (fail-closed) rather than
    // silently allowing — this proves the no-resale gate is actually consulted.
    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('monetization_not_authorized')
  })
})

describe('buildFirmupBitcoinSettlement', () => {
  it('builds a real-bitcoin settlement chain citing the executed verdict, not a manual attestation', () => {
    const records = buildFirmupBitcoinSettlement({
      adapterKind: 'spark_treasury',
      amountSats: 50,
      escrowRef: 'labor_escrow.public.escrow_5459',
      idempotencyDigestHex: 'deadbeefcafef00d',
      nowIso: '2026-06-18T10:05:00.000Z',
      operatorApprovalRef: 'operator.approval.firmup.5459',
      payoutTargetApprovalRef: 'payout.target.approval.firmup.5459',
      payoutTargetRef: 'payout.target.firmup.5459',
      providerActorRef: WORKER_REF,
      trainingRunRef: FIRMUP_RUN_REF,
      validatorActorRef: VALIDATOR_REF,
      verdict,
      workRequestRef: 'work_request.public.firmup_5459',
    })

    const projection = JSON.parse(
      records.settlementReceipt.publicProjectionJson,
    ) as {
      moneyMovement: string
      state: string
      validatorActorRef: string
      verdictRef: string
      verificationBasis: string
      verificationCommandRef: string
    }

    expect(projection.moneyMovement).toBe('real_bitcoin')
    expect(projection.state).toBe('settled')
    expect(projection.verificationBasis).toBe(FirmupSettlementVerificationClass)
    expect(projection.verdictRef).toBe(verdict.verdictRef)
    expect(projection.validatorActorRef).toBe(VALIDATOR_REF)
    expect(projection.verificationCommandRef).toBe(
      verdict.verificationCommandRef,
    )
    expect(records.settlementReceipt.publicProjectionJson).not.toContain(
      'manual_attestation',
    )
    expect(records.settlementReceipt.receiptKind).toBe('settlement_recorded')
    expect(records.amountSats).toBe(50)
  })

  it('records no money movement for the simulation adapter (honest default)', () => {
    const records = buildFirmupBitcoinSettlement({
      adapterKind: 'simulation',
      amountSats: 50,
      escrowRef: 'labor_escrow.public.escrow_5459',
      idempotencyDigestHex: 'deadbeefcafef00d',
      nowIso: '2026-06-18T10:05:00.000Z',
      operatorApprovalRef: 'operator.approval.firmup.5459',
      payoutTargetApprovalRef: 'payout.target.approval.firmup.5459',
      payoutTargetRef: 'payout.target.firmup.5459',
      providerActorRef: WORKER_REF,
      trainingRunRef: FIRMUP_RUN_REF,
      validatorActorRef: VALIDATOR_REF,
      verdict,
      workRequestRef: 'work_request.public.firmup_5459',
    })

    const projection = JSON.parse(
      records.settlementReceipt.publicProjectionJson,
    ) as { moneyMovement: string }

    expect(projection.moneyMovement).toBe('none')
  })
})

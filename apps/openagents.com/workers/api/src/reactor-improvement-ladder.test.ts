import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  REACTOR_DOGFOOD_DISTILL_TO_FIT_RECEIPT,
  REACTOR_DOGFOOD_HARNESS_EVOLUTION_RECEIPT,
  REACTOR_IMPROVEMENT_LADDER_DOGFOOD_RECEIPT,
  REACTOR_IMPROVEMENT_LADDER_PLAN_RECEIPT,
  ReactorDistillToFitDogfoodReceipt,
  ReactorHarnessEvolutionDogfoodReceipt,
  ReactorImprovementLadderDogfoodReceipt,
  ReactorImprovementLadderPlanReceipt,
} from '@openagentsinc/reactor-contracts'

describe('Reactor improvement ladder dogfood receipts', () => {
  test('design receipt keeps customer consent, boundary, and weight ownership explicit', () => {
    const plan = S.decodeUnknownSync(ReactorImprovementLadderPlanReceipt)(
      REACTOR_IMPROVEMENT_LADDER_PLAN_RECEIPT,
    )

    expect(plan.stages).toEqual([
      'harness_evolution',
      'distill_to_fit',
      'flywheel_training',
    ])
    expect(plan.consentRequired).toBe(true)
    expect(plan.boundaryRequirement).toBe('customer_premises_or_regulated_private')
    expect(plan.customerWeightsOwnerRequired).toBe(true)
    expect(plan.customerDataUsed).toBe(false)
    expect(plan.capabilityClaimsAuthorized).toBe(false)
  })

  test('harness evolution is rung zero and changes no weights', () => {
    const receipt = S.decodeUnknownSync(ReactorHarnessEvolutionDogfoodReceipt)(
      REACTOR_DOGFOOD_HARNESS_EVOLUTION_RECEIPT,
    )

    expect(receipt.stage).toBe('harness_evolution')
    expect(receipt.runnerRef).toBe('psionic')
    expect(receipt.optimizerRef).toBe('mutalisk')
    expect(receipt.oneMechanismOnly).toBe(true)
    expect(receipt.weightChangesAllowed).toBe(false)
    expect(receipt.customerDataUsed).toBe(false)
    expect(receipt.deltaBps).toBeGreaterThan(receipt.acceptanceThresholdBps)
    expect(receipt.accepted).toBe(true)
  })

  test('distill-to-fit records measured cost/quality deltas behind the RX-3 router gate', () => {
    const receipt = S.decodeUnknownSync(ReactorDistillToFitDogfoodReceipt)(
      REACTOR_DOGFOOD_DISTILL_TO_FIT_RECEIPT,
    )

    expect(receipt.stage).toBe('distill_to_fit')
    expect(receipt.candidateModelClass).toBe('smaller_distilled_model')
    expect(receipt.policyRevalidated).toBe(true)
    expect(receipt.routerSwapGate).toBe('eval_gated_rx3_router')
    expect(receipt.routerSwapGateStatus).toBe('passed')
    expect(receipt.routeSwapAuthorized).toBe(false)
    expect(receipt.qualityDeltaBps).toBe(
      receipt.candidateQualityScoreBps - receipt.baselineQualityScoreBps,
    )
    expect(receipt.costReductionBps).toBeGreaterThan(5000)
    expect(receipt.customerDataUsed).toBe(false)
  })

  test('aggregate receipt remains internal and claim-blocked', () => {
    const receipt = S.decodeUnknownSync(ReactorImprovementLadderDogfoodReceipt)(
      REACTOR_IMPROVEMENT_LADDER_DOGFOOD_RECEIPT,
    )

    expect(receipt.status).toBe('completed_internal')
    expect(receipt.capabilityClaimsAuthorized).toBe(false)
    expect(receipt.externalClaimFlipAllowed).toBe(false)
    expect(receipt.customerDataUsed).toBe(false)
    expect(receipt.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.reactor.improvement_ladder.no_customer_consent_receipt',
        'blocker.reactor.improvement_ladder.no_customer_boundary_run',
        'blocker.reactor.improvement_ladder.public_claims_owner_approval_missing',
      ]),
    )
  })
})

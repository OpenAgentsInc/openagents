import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_PROMOTION_DECISION_READ_ONLY_AUTHORITY,
  OmniPromotionDecisionLedgerRecord,
  OmniPromotionDecisionProjection,
  OmniPromotionDecisionUnsafe,
  exampleOmniPromotionDecisionLedger,
  omniPromotionDecisionProjectionHasPrivateMaterial,
  projectOmniPromotionDecisionLedger,
} from './omni-model-lab-promotion-decision'

const nowIso = '2026-06-07T00:30:00.000Z'

const promotionLedger = (
  overrides: Partial<OmniPromotionDecisionLedgerRecord> = {},
): OmniPromotionDecisionLedgerRecord =>
  S.decodeUnknownSync(OmniPromotionDecisionLedgerRecord)({
    ...exampleOmniPromotionDecisionLedger(),
    ...overrides,
  })

describe('Omni Model Lab promotion decision ledger', () => {
  test('projects passed promotion evidence without runtime, model, adapter, route, rollback, provider, marketplace, spend, payout, settlement, or public-claim authority', () => {
    const projection = projectOmniPromotionDecisionLedger(
      exampleOmniPromotionDecisionLedger(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniPromotionDecisionProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      adapterInstallAllowed: false,
      blockedCount: 0,
      claimState: 'passed_not_deployed',
      createdAtDisplay: '25 minutes ago',
      decisionCount: 1,
      failedCount: 0,
      marketplaceRankMutationAllowed: false,
      modelDeploymentAllowed: false,
      passedCount: 1,
      paymentSpendAllowed: false,
      payoutMutationAllowed: false,
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      rollbackExecutionAllowed: false,
      routeMutationAllowed: false,
      runtimePromotionAllowed: false,
      settlementMutationAllowed: false,
      supersededCount: 0,
      updatedAtDisplay: '16 minutes ago',
    })
    expect(projection.authority).toEqual(
      OMNI_PROMOTION_DECISION_READ_ONLY_AUTHORITY,
    )
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(omniPromotionDecisionProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('validates passed decision gates, reviewer receipts, rollback posture, marketplace memory, and attribution', () => {
    const base = exampleOmniPromotionDecisionLedger()
    const passed = base.decisions[0]!

    for (const badLedger of [
      promotionLedger({ decisions: [] }),
      promotionLedger({
        decisions: [{ ...passed, releaseGateRefs: [] }],
      }),
      promotionLedger({
        decisions: [{ ...passed, reviewerReceiptRefs: [] }],
      }),
      promotionLedger({
        decisions: [{ ...passed, rollbackPosture: 'missing' }],
      }),
      promotionLedger({
        decisions: [{ ...passed, rollbackRefs: [] }],
      }),
      promotionLedger({
        decisions: [{ ...passed, marketplaceMemoryRefs: [] }],
      }),
      promotionLedger({
        decisions: [{ ...passed, outcomeAttributionRefs: [] }],
      }),
      promotionLedger({
        decisions: [{ ...passed, riskLabels: ['critical'] }],
      }),
      promotionLedger({
        decisions: [
          {
            ...passed,
            benchmarkEvidenceRefs: ['benchmark.public.missing'],
          },
        ],
      }),
      promotionLedger({
        decisions: [
          passed,
          { ...passed, decisionRef: passed.decisionRef },
        ],
      }),
    ]) {
      expect(() =>
        projectOmniPromotionDecisionLedger(badLedger, 'operator', nowIso),
      ).toThrow(OmniPromotionDecisionUnsafe)
    }
  })

  test('projects failed, blocked, and superseded decisions with explicit blockers and supersession refs', () => {
    const base = exampleOmniPromotionDecisionLedger()
    const passed = base.decisions[0]!
    const failed = {
      ...passed,
      blockerRefs: ['blocker.public.eval_regression_failed_gate'],
      decisionRef: 'decision.public.autopilot_lora_v2_failed',
      marketplaceMemoryRefs: [],
      outcomeAttributionRefs: [],
      riskLabels: ['high'] as const,
      rollbackPosture: 'candidate' as const,
      rollbackRefs: [],
      state: 'failed' as const,
      supersedesRefs: [],
    }
    const blocked = {
      ...passed,
      blockerRefs: ['blocker.public.missing_benchmark_receipt'],
      caveatRefs: ['caveat.public.waiting_for_benchmark_cloud_receipt'],
      decisionRef: 'decision.public.autopilot_lora_v2_blocked',
      marketplaceMemoryRefs: [],
      outcomeAttributionRefs: [],
      reviewerReceiptRefs: [],
      rollbackPosture: 'missing' as const,
      rollbackRefs: [],
      state: 'blocked' as const,
      supersedesRefs: [],
    }
    const superseded = {
      ...passed,
      decisionRef: 'decision.public.autopilot_lora_v1_superseded',
      marketplaceMemoryRefs: [],
      outcomeAttributionRefs: [],
      reviewerReceiptRefs: [],
      rollbackPosture: 'missing' as const,
      rollbackRefs: [],
      state: 'superseded' as const,
      supersededByRefs: ['decision.public.autopilot_lora_v2_passed'],
      supersedesRefs: [],
    }

    const failedProjection = projectOmniPromotionDecisionLedger(
      promotionLedger({
        blockerRefs: ['blocker.public.eval_regression_failed_gate'],
        decisions: [failed],
      }),
      'operator',
      nowIso,
    )
    const blockedProjection = projectOmniPromotionDecisionLedger(
      promotionLedger({
        blockerRefs: ['blocker.public.missing_benchmark_receipt'],
        decisions: [blocked],
      }),
      'operator',
      nowIso,
    )
    const supersededProjection = projectOmniPromotionDecisionLedger(
      promotionLedger({ decisions: [superseded] }),
      'operator',
      nowIso,
    )

    expect(failedProjection.claimState).toBe('failed_reviewed')
    expect(failedProjection.failedCount).toBe(1)
    expect(blockedProjection.claimState).toBe('blocked')
    expect(blockedProjection.blockedCount).toBe(1)
    expect(supersededProjection.claimState).toBe('superseded')
    expect(supersededProjection.supersededCount).toBe(1)

    expect(() =>
      projectOmniPromotionDecisionLedger(
        promotionLedger({
          decisions: [{ ...failed, blockerRefs: [] }],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniPromotionDecisionUnsafe)
    expect(() =>
      projectOmniPromotionDecisionLedger(
        promotionLedger({
          decisions: [{ ...blocked, caveatRefs: [] }],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniPromotionDecisionUnsafe)
    expect(() =>
      projectOmniPromotionDecisionLedger(
        promotionLedger({
          decisions: [{ ...superseded, supersededByRefs: [] }],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniPromotionDecisionUnsafe)
  })

  test('redacts private promotion decisions, reviewer receipts, routes, rollback refs, marketplace memory, and attribution publicly', () => {
    const base = exampleOmniPromotionDecisionLedger()
    const passed = base.decisions[0]!
    const projection = projectOmniPromotionDecisionLedger(
      promotionLedger({
        candidateRefs: ['candidate.private.operator_candidate'],
        decisions: [
          {
            ...passed,
            candidateRefs: ['candidate.private.operator_candidate'],
            decisionRef: 'decision.private.operator_decision',
            marketplaceMemoryRefs: ['marketplace.private.operator_margin'],
            outcomeAttributionRefs: ['outcome.private.operator_outcome'],
            releaseGateRefs: ['gate.private.operator_gate'],
            reviewerReceiptRefs: ['receipt.private.operator_review'],
            rollbackRefs: ['rollback.private.operator_restore'],
            routeRefs: ['route.private.operator_route'],
          },
        ],
        id: 'promotion.private.operator_ledger',
        ledgerRef: 'ledger.private.operator_promotion',
        marketplaceMemoryRefs: ['marketplace.private.operator_margin'],
        outcomeAttributionRefs: ['outcome.private.operator_outcome'],
        releaseGateRefs: ['gate.private.operator_gate'],
        routeRefs: ['route.private.operator_route'],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.id).toBe('promotion-ledger.redacted')
    expect(projection.ledgerRef).toBe('ledger.redacted.promotion')
    expect(projection.decisions[0]!.decisionRef).toBe(
      'decision.redacted.promotion',
    )
    expect(serialized).not.toContain('private')
    expect(serialized).not.toContain('operator')
    expect(omniPromotionDecisionProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('rejects private prompts, source archives, provider payloads, datasets, model weights, secrets, payment material, raw timestamps, and mutable authority', () => {
    for (const badLedger of [
      promotionLedger({ caveatRefs: ['raw_prompt.customer'] }),
      promotionLedger({ blockerRefs: ['source_archive.raw'] }),
      promotionLedger({ caveatRefs: ['provider_payload.raw'] }),
      promotionLedger({ blockerRefs: ['dataset.raw.customer'] }),
      promotionLedger({ blockerRefs: ['weights.safetensors'] }),
      promotionLedger({ caveatRefs: ['secret.promotion_token'] }),
      promotionLedger({ caveatRefs: ['payment_preimage.raw'] }),
      promotionLedger({ caveatRefs: ['caveat.public.2026-06-07T00:00:00'] }),
      promotionLedger({
        authority: {
          ...OMNI_PROMOTION_DECISION_READ_ONLY_AUTHORITY,
          noRuntimePromotion: false,
        },
      }),
      promotionLedger({
        authority: {
          ...OMNI_PROMOTION_DECISION_READ_ONLY_AUTHORITY,
          noRouteMutation: false,
        },
      }),
    ]) {
      expect(() =>
        projectOmniPromotionDecisionLedger(badLedger, 'operator', nowIso),
      ).toThrow(OmniPromotionDecisionUnsafe)
    }
  })
})

import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_INVESTOR_OUTCOME_ECONOMICS_READ_ONLY_AUTHORITY,
  OmniInvestorOutcomeEconomicsMetricRecord,
  OmniInvestorOutcomeEconomicsProjection,
  OmniInvestorOutcomeEconomicsUnsafe,
  projectOmniInvestorOutcomeEconomicsMetrics,
} from './omni-investor-outcome-economics-metrics'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-06T12:30:00.000Z'

const metricRecord = (
  overrides: Partial<OmniInvestorOutcomeEconomicsMetricRecord> = {},
): OmniInvestorOutcomeEconomicsMetricRecord =>
  S.decodeUnknownSync(OmniInvestorOutcomeEconomicsMetricRecord)({
    acceptedOutcomeCount: 1,
    acceptedOutcomeRefs: [
      'accepted_outcome.public.otec_r3',
      'accepted_outcome.private.operator_otec',
    ],
    acceptedRevenueCents: 120000,
    artifactCostCents: 4000,
    authority: OMNI_INVESTOR_OUTCOME_ECONOMICS_READ_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.public.modeled_until_receipts'],
    createdAtIso: '2026-06-06T12:00:00.000Z',
    economicsRefs: [
      'economics.public.otec_r3',
      'economics.private.operator_otec',
    ],
    evidenceRefs: [
      'evidence.public.otec_r3.accepted',
      'evidence.private.operator_review',
    ],
    gradingCostCents: 3000,
    gradingRefs: ['grading.public.otec_r3'],
    id: 'investor_economics_metric.otec_r3',
    providerPayableCents: 22000,
    providerSettledCents: 22000,
    providerSettlementRefs: [
      'settlement.public.otec_r3.provider',
      'settlement.private.otec_r3.provider_internal',
    ],
    providerSettlementState: 'settled',
    refundExposureCents: 5000,
    refundRefs: ['refund.private.operator_hold'],
    refundState: 'exposure',
    refundedCents: 0,
    revenueRefs: ['revenue.public.otec_r3.accepted'],
    revenueState: 'accepted',
    reviewCostCents: 7000,
    reviewMinutes: 18,
    reviewRefs: ['review.public.otec_r3.operator'],
    retryCostCents: 2000,
    retryCount: 1,
    retryRefs: ['retry.public.otec_r2_to_r3'],
    runnerCostCents: 15000,
    sourceRefs: ['source.public.otec_r3'],
    updatedAtIso: '2026-06-06T12:25:00.000Z',
    workKind: 'site',
    workroomRefs: ['workroom.private.otec'],
    ...overrides,
  })

describe('Omni investor outcome economics metrics', () => {
  test('aggregates investor-grade economics by work class without mutation authority', () => {
    const projection = projectOmniInvestorOutcomeEconomicsMetrics(
      [
        metricRecord(),
        metricRecord({
          acceptedOutcomeRefs: ['accepted_outcome.public.coding_pr'],
          acceptedRevenueCents: 40000,
          artifactCostCents: 1000,
          economicsRefs: ['economics.public.coding_pr'],
          evidenceRefs: ['evidence.public.coding_pr.accepted'],
          gradingCostCents: 1000,
          gradingRefs: ['grading.public.coding_pr'],
          id: 'investor_economics_metric.coding_pr',
          providerPayableCents: 5000,
          providerSettledCents: 0,
          providerSettlementRefs: ['settlement.public.coding_pr.dispatch'],
          providerSettlementState: 'dispatched',
          refundExposureCents: 0,
          refundRefs: [],
          refundState: 'none',
          revenueRefs: ['revenue.public.coding_pr.accepted'],
          reviewCostCents: 2500,
          reviewMinutes: 6,
          reviewRefs: ['review.public.coding_pr'],
          retryCostCents: 0,
          retryCount: 0,
          retryRefs: [],
          runnerCostCents: 8000,
          sourceRefs: ['source.public.coding_pr'],
          updatedAtIso: '2026-06-06T12:20:00.000Z',
          workKind: 'coding',
          workroomRefs: ['workroom.private.coding_pr'],
        }),
      ],
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniInvestorOutcomeEconomicsProjection)(
      projection,
    )).toEqual(projection)
    expect(projection.generatedFromRecordCount).toBe(2)
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(projection.buyerChargeMutationAllowed).toBe(false)
    expect(projection.economicsLedgerMutationAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.providerSettlementMutationAllowed).toBe(false)
    expect(projection.publicClaimUpgradeAllowed).toBe(false)
    expect(projection.refundMutationAllowed).toBe(false)
    expect(projection.totals).toMatchObject({
      acceptedGrossProfitCents: 84500,
      acceptedOutcomeCount: 2,
      acceptedRevenueCents: 160000,
      artifactCostCents: 5000,
      grossMarginBps: 5281,
      providerPayableCents: 27000,
      providerSettledCents: 22000,
      refundExposureCents: 5000,
      reviewCostCents: 9500,
      reviewMinutes: 24,
      retryCostCents: 2000,
      retryCount: 1,
    })
    expect(projection.workClassMetrics.map(metric => metric.workKind)).toEqual([
      'coding',
      'site',
    ])
    expect(projection.workClassMetrics.find(metric => metric.workKind === 'site'))
      .toMatchObject({
        acceptedGrossProfitCents: 62000,
        acceptedRevenueCents: 120000,
        providerSettlementClaimAllowed: true,
        providerSettlementStateLabel: 'Settled',
        refundStateLabel: 'Exposure',
        workKindLabel: 'Site',
      })
  })

  test('redacts private refs for public and agent projections while retaining safe settlement evidence', () => {
    const projection = projectOmniInvestorOutcomeEconomicsMetrics(
      [metricRecord()],
      'public',
      nowIso,
    )

    expect(projection.totals.acceptedOutcomeRefs).toEqual([
      'accepted_outcome.public.otec_r3',
    ])
    expect(projection.totals.economicsRefs).toEqual([
      'economics.public.otec_r3',
    ])
    expect(projection.totals.evidenceRefs).toEqual([
      'evidence.public.otec_r3.accepted',
    ])
    expect(projection.totals.providerSettlementRefs).toEqual([
      'settlement.public.otec_r3.provider',
    ])
    expect(projection.totals.refundRefs).toEqual([])
    expect(projection.totals.workroomRefs).toEqual([])
    expect(projection.totals.providerSettlementClaimAllowed).toBe(true)
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('keeps accepted, payable, settlement, and refund claims separate', () => {
    const modeled = projectOmniInvestorOutcomeEconomicsMetrics(
      [
        metricRecord({
          acceptedOutcomeCount: 0,
          acceptedRevenueCents: 0,
          providerPayableCents: 0,
          providerSettledCents: 0,
          providerSettlementRefs: [],
          providerSettlementState: 'none',
          refundExposureCents: 0,
          refundRefs: [],
          refundState: 'none',
          revenueRefs: ['revenue.public.model_only'],
          revenueState: 'modeled',
        }),
      ],
      'team',
      nowIso,
    )
    const payable = projectOmniInvestorOutcomeEconomicsMetrics(
      [
        metricRecord({
          providerSettledCents: 0,
          providerSettlementRefs: ['settlement.public.payable_ref'],
          providerSettlementState: 'payable',
        }),
      ],
      'team',
      nowIso,
    )
    const refunded = projectOmniInvestorOutcomeEconomicsMetrics(
      [
        metricRecord({
          providerSettlementState: 'settled',
          refundExposureCents: 8000,
          refundedCents: 8000,
          refundRefs: ['refund.public.customer_refund'],
          refundState: 'refunded',
          revenueState: 'refunded',
        }),
      ],
      'team',
      nowIso,
    )

    expect(modeled.totals.modeledOnly).toBe(true)
    expect(modeled.totals.acceptedRevenueClaimAllowed).toBe(false)
    expect(modeled.totals.providerPayableClaimAllowed).toBe(false)
    expect(modeled.totals.providerSettlementClaimAllowed).toBe(false)
    expect(payable.totals.providerPayableClaimAllowed).toBe(true)
    expect(payable.totals.providerSettlementClaimAllowed).toBe(false)
    expect(refunded.totals.acceptedRevenueClaimAllowed).toBe(true)
    expect(refunded.totals.refundClaimAllowed).toBe(true)
  })

  test('rejects false authority, invalid settlement, refund overclaim, and unsafe refs', () => {
    expect(() =>
      projectOmniInvestorOutcomeEconomicsMetrics(
        [
          metricRecord({
            authority: {
              ...OMNI_INVESTOR_OUTCOME_ECONOMICS_READ_ONLY_AUTHORITY,
              noPublicClaimUpgrade: false,
            },
          }),
        ],
        'operator',
        nowIso,
      ),
    ).toThrow(OmniInvestorOutcomeEconomicsUnsafe)

    expect(() =>
      projectOmniInvestorOutcomeEconomicsMetrics(
        [
          metricRecord({
            providerSettlementRefs: [],
            providerSettlementState: 'settled',
          }),
        ],
        'operator',
        nowIso,
      ),
    ).toThrow(OmniInvestorOutcomeEconomicsUnsafe)

    expect(() =>
      projectOmniInvestorOutcomeEconomicsMetrics(
        [
          metricRecord({
            refundExposureCents: 1000,
            refundedCents: 2000,
            refundRefs: ['refund.public.overclaim'],
            refundState: 'refunded',
          }),
        ],
        'operator',
        nowIso,
      ),
    ).toThrow(OmniInvestorOutcomeEconomicsUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw timestamp', value: 'evidence.2026-06-06T12:00:00' },
      { label: 'payment id', value: 'payment_id.raw_internal' },
      { label: 'invoice', value: 'invoice.lnbc123' },
      { label: 'wallet material', value: 'wallet.secret.seed' },
      { label: 'provider token', value: 'provider_token.local' },
    ]) {
      expect(() =>
        projectOmniInvestorOutcomeEconomicsMetrics(
          [
            metricRecord({
              evidenceRefs: [fixture.value],
            }),
          ],
          'operator',
          nowIso,
        ),
      ).toThrow(OmniInvestorOutcomeEconomicsUnsafe)
    }
  })
})

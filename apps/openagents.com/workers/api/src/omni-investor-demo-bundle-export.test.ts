import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_INVESTOR_DEMO_BUNDLE_READ_ONLY_AUTHORITY,
  OmniInvestorDemoBundleExport,
  OmniInvestorDemoBundleUnsafe,
  OmniInvestorDemoProofBundleSummary,
  OmniInvestorDemoRouteScorecardSummary,
  projectOmniInvestorDemoBundleExport,
} from './omni-investor-demo-bundle-export'
import {
  OMNI_INVESTOR_OUTCOME_ECONOMICS_READ_ONLY_AUTHORITY,
  OmniInvestorOutcomeEconomicsMetricRecord,
  projectOmniInvestorOutcomeEconomicsMetrics,
} from './omni-investor-outcome-economics-metrics'
import {
  OMNI_OUTCOME_POWER_PRODUCTIVITY_READ_ONLY_AUTHORITY,
  OmniOutcomePowerProductivityRecord,
  projectOmniOutcomePowerProductivity,
} from './omni-outcome-power-productivity'
import {
  PylonCapacityFunnelRecord,
  accountPylonCapacityFunnel,
} from './pylon-capacity-funnel'

const nowIso = '2026-06-06T23:30:00.000Z'

const proofBundle = (
  overrides: Partial<OmniInvestorDemoProofBundleSummary> = {},
): OmniInvestorDemoProofBundleSummary =>
  S.decodeUnknownSync(OmniInvestorDemoProofBundleSummary)({
    acceptanceStateRef: 'acceptance.public.otec_site_r4',
    artifactRefs: ['artifact.public.otec_site_r4'],
    economicsCaveatRef: 'caveat.public.investor_demo_not_a_security_offer',
    legalCaveatRef: null,
    noSettlementImplication: false,
    privacyCaveatRef: 'privacy.public.no_customer_private_data',
    publicReceiptRef: 'receipt.public.otec_site_r4',
    receiptRefs: ['receipt.public.otec_site_r4.email'],
    reviewStateRef: 'review.public.operator_approved',
    sourceRefs: ['source.public.otec_site_r4'],
    status: 'ready',
    workKind: 'site',
    workroomId: 'workroom.public.otec_site',
    ...overrides,
  })

const routeScorecard = (
  overrides: Partial<OmniInvestorDemoRouteScorecardSummary> = {},
): OmniInvestorDemoRouteScorecardSummary =>
  S.decodeUnknownSync(OmniInvestorDemoRouteScorecardSummary)({
    observedResultKind: 'success',
    observedResultRef: 'route_result.public.otec_site_success',
    postCloseoutScore: 92,
    publicCaveatRef: 'caveat.public.route_cost_estimate',
    selectedModelRef: 'model.public.codex_sites',
    selectedRuntimeRef: 'runtime.public.autopilot_sites',
    trustTier: 'reviewed',
    workKind: 'site',
    workroomId: 'workroom.public.otec_site',
    ...overrides,
  })

const economicsRecord = (
  overrides: Partial<OmniInvestorOutcomeEconomicsMetricRecord> = {},
): OmniInvestorOutcomeEconomicsMetricRecord =>
  S.decodeUnknownSync(OmniInvestorOutcomeEconomicsMetricRecord)({
    acceptedOutcomeCount: 1,
    acceptedOutcomeRefs: [
      'accepted_outcome.public.otec_site_r4',
      'accepted_outcome.private.operator_otec_site',
    ],
    acceptedRevenueCents: 150000,
    artifactCostCents: 5000,
    authority: OMNI_INVESTOR_OUTCOME_ECONOMICS_READ_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.public.beta_pricing'],
    createdAtIso: '2026-06-06T23:00:00.000Z',
    economicsRefs: [
      'economics.public.otec_site_r4',
      'economics.private.operator_otec_site',
    ],
    evidenceRefs: ['evidence.public.otec_site_r4.accepted'],
    gradingCostCents: 2000,
    gradingRefs: ['grading.public.otec_site_r4'],
    id: 'investor_economics_metric.otec_site_r4',
    providerPayableCents: 25000,
    providerSettledCents: 25000,
    providerSettlementRefs: [
      'settlement.public.otec_site_r4.provider',
      'settlement.private.operator_receipt',
    ],
    providerSettlementState: 'settled',
    refundExposureCents: 0,
    refundRefs: [],
    refundState: 'none',
    refundedCents: 0,
    revenueRefs: ['revenue.public.otec_site_r4.accepted'],
    revenueState: 'accepted',
    reviewCostCents: 9000,
    reviewMinutes: 15,
    reviewRefs: ['review.public.otec_site_r4'],
    retryCostCents: 3000,
    retryCount: 1,
    retryRefs: ['retry.public.otec_site_r3_to_r4'],
    runnerCostCents: 18000,
    sourceRefs: ['source.public.otec_site_r4'],
    updatedAtIso: '2026-06-06T23:25:00.000Z',
    workKind: 'site',
    workroomRefs: ['workroom.private.otec_site'],
    ...overrides,
  })

const powerRecord = (
  overrides: Partial<OmniOutcomePowerProductivityRecord> = {},
): OmniOutcomePowerProductivityRecord =>
  S.decodeUnknownSync(OmniOutcomePowerProductivityRecord)({
    acceptedGrossProfitCents: 90000,
    acceptedOutcomeCount: 1,
    acceptedOutcomeRefs: ['accepted_outcome.public.otec_site_r4'],
    acceptedRevenueCents: 150000,
    authority: OMNI_OUTCOME_POWER_PRODUCTIVITY_READ_ONLY_AUTHORITY,
    caveatRefs: ['caveat.public.metered_at_provider_boundary'],
    darkCapacityReasonRefs: [],
    darkCapacityWattHours: 0,
    energyEvidenceRefs: ['evidence.public.energy.otec_site_r4'],
    energyModelRefs: [],
    energyWattHours: 1000000,
    id: 'outcome_power.otec_site_r4',
    measuredEnergyRefs: ['meter.public.otec_site_r4'],
    powerDataState: 'measured',
    providerPayableCents: 25000,
    providerSettledCents: 25000,
    settlementRefs: ['settlement.public.otec_site_r4.provider'],
    settlementState: 'settled',
    sourceRefs: ['source.public.energy.otec_site_r4'],
    updatedAtIso: '2026-06-06T23:20:00.000Z',
    workKind: 'site',
    workroomRefs: ['workroom.private.otec_site'],
    ...overrides,
  })

const capacityRecord = (
  overrides: Partial<PylonCapacityFunnelRecord> = {},
): PylonCapacityFunnelRecord =>
  S.decodeUnknownSync(PylonCapacityFunnelRecord)({
    acceptanceRefs: ['acceptance.public.capacity_1'],
    artifactRefs: ['artifact.public.capacity_1'],
    assignmentRefs: ['assignment.public.capacity_1'],
    benchmarkRefs: ['benchmark.public.capacity_1'],
    capacityRef: 'capacity.public.otec_provider_1',
    caveatRefs: ['caveat.public.capacity_not_settlement_claim'],
    darkCapacityReasonRefs: [],
    eligibilityRefs: ['eligibility.public.capacity_1'],
    evidenceRefs: ['evidence.public.capacity_1'],
    id: 'capacity_funnel.public_otec_1',
    nodeRef: 'node.public.otec_provider_1',
    nodeVisibility: 'public',
    providerRef: 'provider.public.otec_provider_1',
    providerVisibility: 'public',
    rewardRefs: [],
    runRefs: ['run.public.capacity_1'],
    settlementRefs: [],
    stage: 'accepted',
    updatedAtIso: '2026-06-06T23:20:00.000Z',
    workClassRefs: ['work_class.public.flexible_inference'],
    ...overrides,
  })

const readyBundle = () =>
  projectOmniInvestorDemoBundleExport({
    audience: 'public',
    capacityFunnel: accountPylonCapacityFunnel(
      [capacityRecord()],
      'public',
      nowIso,
    ),
    caveatRefs: ['caveat.public.review_before_sharing'],
    economics: projectOmniInvestorOutcomeEconomicsMetrics(
      [economicsRecord()],
      'public',
      nowIso,
    ),
    generatedAtIso: '2026-06-06T23:25:00.000Z',
    id: 'investor_demo.public.otec_r4',
    powerProductivity: projectOmniOutcomePowerProductivity(
      [powerRecord()],
      'public',
      nowIso,
    ),
    proofBundles: [proofBundle()],
    routeScorecards: [routeScorecard()],
    sourceRefs: ['source.public.investor_demo_otec'],
    title: 'OTEC Site investor demo',
  }, nowIso)

describe('Omni investor demo bundle export', () => {
  test('assembles a ready investor-safe bundle without mutation authority', () => {
    const bundle = readyBundle()

    expect(S.decodeUnknownSync(OmniInvestorDemoBundleExport)(bundle))
      .toEqual(bundle)
    expect(bundle).toMatchObject({
      acceptedOutcomeClaimAllowed: true,
      audience: 'public',
      downloadRouteMutationAllowed: false,
      generatedAtDisplay: '5 minutes ago',
      investorShareMutationAllowed: false,
      liveWalletSpendAllowed: false,
      publicClaimUpgradeAllowed: false,
      rawDataCopyAllowed: false,
      readiness: 'ready',
      reviewBeforeSharing: false,
      settlementMutationAllowed: false,
      settlementStateLabel: 'Settled',
      title: 'OTEC Site investor demo',
    })
    expect(bundle.authority).toEqual(OMNI_INVESTOR_DEMO_BUNDLE_READ_ONLY_AUTHORITY)
    expect(bundle.economics).toMatchObject({
      acceptedGrossProfitCents: 88000,
      acceptedOutcomeCount: 1,
      acceptedRevenueCents: 150000,
      acceptedRevenueClaimAllowed: true,
      grossMarginBps: 5867,
      providerSettlementClaimAllowed: true,
      providerSettlementStateLabel: 'Settled',
      providerSettledCents: 25000,
      revenueStateLabel: 'Accepted',
    })
    expect(bundle.capacityFunnel).toMatchObject({
      acceptedCount: 1,
      staleCount: 0,
      totalCount: 1,
    })
    expect(bundle.powerProductivity).toMatchObject({
      acceptedOutcomesPerMwh: 1,
      energyMwh: 1,
      measuredEnergyClaimAllowed: true,
      settlementClaimAllowed: true,
    })
    expect(bundle.sections.map(section => section.state)).toEqual([
      'complete',
      'complete',
      'complete',
      'complete',
      'complete',
    ])
    expect(bundle.missingEvidence).toEqual([])
    expect(JSON.stringify(bundle)).not.toContain('2026-06-06T')
    expect(JSON.stringify(bundle)).not.toMatch(
      /(accepted_outcome|artifact|caveat|economics|evidence|receipt|settlement|source)\.private/,
    )
  })

  test('lists missing evidence instead of overclaiming an incomplete bundle', () => {
    const bundle = projectOmniInvestorDemoBundleExport({
      audience: 'public',
      capacityFunnel: accountPylonCapacityFunnel([], 'public', nowIso),
      economics: projectOmniInvestorOutcomeEconomicsMetrics(
        [
          economicsRecord({
            acceptedOutcomeCount: 0,
            acceptedOutcomeRefs: [],
            acceptedRevenueCents: 0,
            providerPayableCents: 0,
            providerSettledCents: 0,
            providerSettlementRefs: [],
            providerSettlementState: 'none',
            revenueRefs: ['revenue.public.modeled_only'],
            revenueState: 'modeled',
          }),
        ],
        'public',
        nowIso,
      ),
      generatedAtIso: '2026-06-06T23:25:00.000Z',
      id: 'investor_demo.public.incomplete',
      powerProductivity: projectOmniOutcomePowerProductivity(
        [
          powerRecord({
            acceptedOutcomeCount: 0,
            acceptedOutcomeRefs: [],
            acceptedRevenueCents: 0,
            energyEvidenceRefs: [],
            energyModelRefs: [],
            energyWattHours: null,
            measuredEnergyRefs: [],
            powerDataState: 'unknown',
            providerPayableCents: 0,
            providerSettledCents: 0,
            settlementRefs: [],
            settlementState: 'not_settled',
          }),
        ],
        'public',
        nowIso,
      ),
      proofBundles: [],
      routeScorecards: [],
      title: 'Incomplete investor demo',
    }, nowIso)

    expect(bundle.readiness).toBe('needs_evidence')
    expect(bundle.acceptedOutcomeClaimAllowed).toBe(false)
    expect(bundle.reviewBeforeSharing).toBe(true)
    expect(bundle.missingEvidence.map(item => item.kind)).toEqual([
      'proof_bundle_ready',
      'route_scorecard_success',
      'accepted_revenue',
      'capacity_funnel',
      'measured_power',
    ])
    expect(bundle.sections).toEqual([
      { label: 'Proof bundles', state: 'missing' },
      { label: 'Route scorecards', state: 'missing' },
      { label: 'Outcome economics', state: 'missing' },
      { label: 'Capacity funnel', state: 'missing' },
      { label: 'Power productivity', state: 'missing' },
    ])
  })

  test('redacts private refs before public or investor sharing', () => {
    const bundle = projectOmniInvestorDemoBundleExport({
      audience: 'investor',
      capacityFunnel: accountPylonCapacityFunnel(
        [
          capacityRecord({
            capacityRef: 'capacity.private.operator_site',
            caveatRefs: [
              'caveat.public.capacity_not_settlement_claim',
              'caveat.private.operator_capacity_model',
            ],
            evidenceRefs: ['evidence.private.capacity_meter'],
          }),
        ],
        'operator',
        nowIso,
      ),
      caveatRefs: [
        'caveat.public.review_before_sharing',
        'caveat.private.operator_notes',
      ],
      economics: projectOmniInvestorOutcomeEconomicsMetrics(
        [economicsRecord()],
        'operator',
        nowIso,
      ),
      generatedAtIso: '2026-06-06T23:25:00.000Z',
      id: 'investor_demo.public.redacted',
      powerProductivity: projectOmniOutcomePowerProductivity(
        [powerRecord()],
        'operator',
        nowIso,
      ),
      proofBundles: [
        proofBundle({
          artifactRefs: [
            'artifact.public.otec_site_r4',
            'artifact.private.operator_notes',
          ],
          receiptRefs: [
            'receipt.public.otec_site_r4.email',
            'receipt.private.operator_email',
          ],
          sourceRefs: [
            'source.public.otec_site_r4',
            'source.private.operator_export',
          ],
        }),
      ],
      routeScorecards: [routeScorecard()],
      sourceRefs: [
        'source.public.investor_demo_otec',
        'source.private.operator_deck',
      ],
      title: 'Redacted investor demo',
    }, nowIso)

    const serialized = JSON.stringify(bundle)

    expect(serialized).not.toMatch(
      /(accepted_outcome|artifact|caveat|economics|evidence|receipt|settlement|source)\.private/,
    )
    expect(serialized).not.toContain('operator_export')
    expect(serialized).not.toContain('operator_deck')
    expect(bundle.proofBundles[0]!.artifactRefs).toEqual([
      'artifact.public.otec_site_r4',
    ])
    expect(bundle.economics.providerSettlementRefs).toEqual([
      'settlement.public.otec_site_r4.provider',
    ])
    expect(bundle.caveatRefs).toContain('caveat.public.review_before_sharing')
  })

  test('keeps blocker, claim-state, and settlement gaps visible', () => {
    const bundle = projectOmniInvestorDemoBundleExport({
      audience: 'public',
      capacityFunnel: accountPylonCapacityFunnel(
        [
          capacityRecord({
            stage: 'paid',
            rewardRefs: ['reward.public.capacity_1'],
            updatedAtIso: '2026-06-04T23:20:00.000Z',
          }),
        ],
        'public',
        nowIso,
      ),
      economics: projectOmniInvestorOutcomeEconomicsMetrics(
        [
          economicsRecord({
            blockerRefs: ['blocker.public.settlement_review'],
            providerSettledCents: 0,
            providerSettlementRefs: ['settlement.public.payable_ref'],
            providerSettlementState: 'payable',
          }),
        ],
        'public',
        nowIso,
      ),
      generatedAtIso: '2026-06-06T23:25:00.000Z',
      id: 'investor_demo.public.blocked',
      powerProductivity: projectOmniOutcomePowerProductivity(
        [
          powerRecord({
            providerSettledCents: 0,
            settlementRefs: [],
            settlementState: 'payable',
          }),
        ],
        'public',
        nowIso,
      ),
      proofBundles: [proofBundle({ status: 'blocked' })],
      routeScorecards: [routeScorecard({ observedResultKind: 'partial' })],
      title: 'Blocked investor demo',
    }, nowIso)

    expect(bundle.readiness).toBe('blocked')
    expect(bundle.economics.providerSettlementStateLabel).toBe('Payable')
    expect(bundle.powerProductivity.settlementStateLabel).toBe('Payable')
    expect(bundle.missingEvidence.map(item => item.kind)).toContain(
      'provider_settlement',
    )
    expect(bundle.missingEvidence.map(item => item.kind)).toContain(
      'power_settlement',
    )
    expect(bundle.missingEvidence.map(item => item.kind)).toContain(
      'fresh_capacity',
    )
  })

  test('rejects unsafe source, route, proof, payment, wallet, raw data, secret, timestamp, and title material', () => {
    expect(() =>
      projectOmniInvestorDemoBundleExport({
        audience: 'operator',
        capacityFunnel: accountPylonCapacityFunnel(
          [capacityRecord()],
          'operator',
          nowIso,
        ),
        economics: projectOmniInvestorOutcomeEconomicsMetrics(
          [economicsRecord()],
          'operator',
          nowIso,
        ),
        generatedAtIso: '2026-06-06T23:25:00.000Z',
        id: 'investor_demo.public.unsafe',
        powerProductivity: projectOmniOutcomePowerProductivity(
          [powerRecord()],
          'operator',
          nowIso,
        ),
        proofBundles: [proofBundle()],
        routeScorecards: [
          routeScorecard({ observedResultRef: 'raw_run_log.operator' }),
        ],
        title: 'Unsafe investor demo',
      }, nowIso),
    ).toThrow(OmniInvestorDemoBundleUnsafe)

    expect(() =>
      projectOmniInvestorDemoBundleExport({
        audience: 'public',
        capacityFunnel: accountPylonCapacityFunnel(
          [capacityRecord()],
          'public',
          nowIso,
        ),
        economics: projectOmniInvestorOutcomeEconomicsMetrics(
          [economicsRecord()],
          'public',
          nowIso,
        ),
        generatedAtIso: '2026-06-06T23:25:00.000Z',
        id: 'investor_demo.public.title',
        powerProductivity: projectOmniOutcomePowerProductivity(
          [powerRecord()],
          'public',
          nowIso,
        ),
        proofBundles: [proofBundle()],
        routeScorecards: [routeScorecard()],
        title: 'Wallet secret 2026-06-06T23:00:00Z',
      }, nowIso),
    ).toThrow(OmniInvestorDemoBundleUnsafe)
  })
})

import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ProbeGepaCampaignProjection,
  ProbeGepaCampaignProjectionSchemaVersion,
  ProbeGepaCampaignProjectionUnsafe,
  assertProbeGepaCampaignProjectionSafe,
  probeGepaCampaignEvidenceCounts,
  probeGepaCampaignPublicSummary,
} from './probe-gepa-campaign-projection'

const projection = (
  overrides: Partial<ProbeGepaCampaignProjection> = {},
): ProbeGepaCampaignProjection =>
  new ProbeGepaCampaignProjection({
    activeCandidateRefs: ['candidate.probe_gepa.stage1.mutation_08'],
    artifactManifestRefs: ['artifact_manifest.probe_gepa.stage1.001'],
    baselineCandidateRef: 'candidate.probe_gepa.stage1.baseline',
    benchmarkSuiteRefs: ['benchmark_suite.terminal_bench_2.harbor.v1'],
    blockerRefs: [],
    campaignRef: 'campaign.probe_gepa.stage1.retained_failure_sprint',
    candidateHashRefs: [
      'sha256:0000000000000000000000000000000000000000000000000000000000002008',
    ],
    claimState: 'retained_summary',
    completedMetricCalls: 210,
    costSummaryRefs: ['cost_summary.probe_gepa.stage1.zero_spend'],
    holdoutResultRefs: [],
    invalidMetricCalls: 95,
    nextActionRefs: ['next_action.probe_gepa.validation_sweep'],
    objectiveRef: 'objective.probe_gepa.retained_failure_improvement',
    plannedMetricCalls: 210,
    policyFindingRefs: [],
    probeCommitRefs: ['probe_commit.ebe108d'],
    promotionDecisionRefs: ['promotion_decision.optimizer_accepted.not_active'],
    pylonBatchRefs: ['pylon_batch.probe_gepa.stage1.unpaid_smoke'],
    receiptRefs: [],
    resourceReceiptRefs: ['resource_usage.probe_gepa.stage1.001'],
    retainedResultRefs: ['benchmark_result.probe_gepa.stage1.retained.001'],
    schemaVersion: ProbeGepaCampaignProjectionSchemaVersion,
    settlementReceiptRefs: [],
    splitManifestRefs: [
      'benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1',
    ],
    stage: 'stage_1_retained_sprint',
    validMetricCalls: 115,
    validationResultRefs: [],
    ...overrides,
  })

describe('Probe GEPA campaign projection', () => {
  test('summarizes retained Stage 1 campaign refs without implying payout', () => {
    const record = assertProbeGepaCampaignProjectionSafe(projection())
    const summary = probeGepaCampaignPublicSummary(record)

    expect(S.decodeUnknownSync(ProbeGepaCampaignProjection)(record)).toEqual(
      record,
    )
    expect(summary).toEqual({
      campaignRef: 'campaign.probe_gepa.stage1.retained_failure_sprint',
      claimState: 'retained_summary',
      completedMetricCalls: 210,
      evidenceCounts: {
        holdout: 0,
        retained: 1,
        validation: 0,
      },
      payoutClaimAllowed: false,
      pylonWorkVisibleWithoutPayoutClaim: true,
      stage: 'stage_1_retained_sprint',
    })
  })

  test('distinguishes retained validation and holdout evidence counts', () => {
    const record = assertProbeGepaCampaignProjectionSafe(
      projection({
        claimState: 'validation_measured_only',
        retainedResultRefs: ['benchmark_result.probe_gepa.stage1.retained.001'],
        validationResultRefs: [
          'benchmark_result.probe_gepa.validation.configure_git_webserver.001',
          'benchmark_result.probe_gepa.validation.db_wal_recovery.001',
        ],
      }),
    )

    expect(probeGepaCampaignEvidenceCounts(record)).toEqual({
      holdout: 0,
      retained: 1,
      validation: 2,
    })
  })

  test('requires evidence refs before claim-state upgrades', () => {
    expect(() =>
      assertProbeGepaCampaignProjectionSafe(
        projection({
          claimState: 'validation_measured_only',
          retainedResultRefs: [],
          validationResultRefs: [],
        }),
      ),
    ).toThrow(ProbeGepaCampaignProjectionUnsafe)

    expect(() =>
      assertProbeGepaCampaignProjectionSafe(
        projection({
          claimState: 'holdout_summary',
          holdoutResultRefs: [],
        }),
      ),
    ).toThrow(ProbeGepaCampaignProjectionUnsafe)
  })

  test('allows settled payout projection only with receipt and settlement refs', () => {
    const noSettlement = probeGepaCampaignPublicSummary(projection())
    const settled = probeGepaCampaignPublicSummary(
      projection({
        receiptRefs: ['receipt.public.pylon_gepa.stage1.payment.1'],
        settlementReceiptRefs: [
          'settlement.public.pylon_gepa.stage1.bitcoin.1',
        ],
      }),
    )

    expect(noSettlement.payoutClaimAllowed).toBe(false)
    expect(noSettlement.pylonWorkVisibleWithoutPayoutClaim).toBe(true)
    expect(settled.payoutClaimAllowed).toBe(true)
    expect(settled.pylonWorkVisibleWithoutPayoutClaim).toBe(false)
  })

  test('rejects raw prompts traces fixtures account refs wallet material and local paths', () => {
    for (const unsafe of [
      'raw_prompt.public',
      'raw_trace.public',
      'raw_benchmark_fixture.public',
      'provider_account.secret',
      'Bearer abc',
      'wallet_mnemonic.secret',
      'lnbc123',
      '/Users/christopherdavid/private/path',
    ]) {
      expect(() =>
        assertProbeGepaCampaignProjectionSafe(
          projection({
            artifactManifestRefs: [unsafe],
          }),
        ),
      ).toThrow(ProbeGepaCampaignProjectionUnsafe)
    }
  })

  test('requires valid and invalid metric counts to reconcile with completed calls', () => {
    expect(() =>
      assertProbeGepaCampaignProjectionSafe(
        projection({
          completedMetricCalls: 210,
          invalidMetricCalls: 1,
          validMetricCalls: 115,
        }),
      ),
    ).toThrow(ProbeGepaCampaignProjectionUnsafe)
  })
})

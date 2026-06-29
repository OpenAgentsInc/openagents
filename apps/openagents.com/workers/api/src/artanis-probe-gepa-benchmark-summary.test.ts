import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ArtanisProbeGepaBenchmarkSummaryInput,
  ArtanisProbeGepaBenchmarkSummaryProjection,
  ArtanisProbeGepaBenchmarkSummaryUnsafe,
  assertArtanisProbeGepaBenchmarkSummaryProjectionSafe,
  buildArtanisProbeGepaBenchmarkSummary,
} from './artanis-probe-gepa-benchmark-summary'
import {
  ProbeGepaCampaignProjection,
  ProbeGepaCampaignProjectionSchemaVersion,
} from './probe-gepa-campaign-projection'
import {
  ProbeGepaStage1PolicyFinding,
  ProbeGepaStage1ShadowPromotionInput,
  evaluateProbeGepaStage1ShadowPromotion,
} from './probe-gepa-stage1-shadow-promotion-gate'

const campaignProjection = (
  overrides: Partial<ProbeGepaCampaignProjection> = {},
): ProbeGepaCampaignProjection =>
  new ProbeGepaCampaignProjection({
    activeCandidateRefs: [],
    artifactManifestRefs: [
      'artifact_manifest.probe.shc_harbor.db_wal_recovery.20260608',
    ],
    baselineCandidateRef: 'candidate.probe_gepa.stage0.baseline',
    benchmarkSuiteRefs: ['benchmark_suite.terminal_bench_2.harbor.v1'],
    blockerRefs: [],
    campaignRef: 'campaign.probe_gepa.stage0.live_receipts.2026_06_08',
    candidateHashRefs: [
      'sha256:0000000000000000000000000000000000000000000000000000000000004563',
    ],
    claimState: 'measured_retained_smoke',
    completedMetricCalls: 1,
    costSummaryRefs: ['cost_summary.probe_gepa.stage0.no_spend'],
    holdoutResultRefs: [],
    invalidMetricCalls: 1,
    nextActionRefs: ['next_action.probe_gepa.stage1.shadow_gate'],
    objectiveRef: 'objective.probe_gepa.stage0.live_smoke',
    plannedMetricCalls: 1,
    policyFindingRefs: ['policy_finding.probe_gepa.no_public_score_claim'],
    probeCommitRefs: ['probe.commit.shc_live_smoke_20260608'],
    promotionDecisionRefs: ['promotion_decision.probe_gepa.stage0.no_promotion'],
    pylonBatchRefs: ['proof.omega.probe_gepa.unpaid_pylon_leases.20260608'],
    receiptRefs: [],
    resourceReceiptRefs: [
      'resource_usage_unavailable.probe.benchmark_run_probe_shc_harbor_db_wal_recovery_20260608',
    ],
    retainedResultRefs: [
      'benchmark_result.probe.shc_harbor.db_wal_recovery.20260608',
    ],
    schemaVersion: ProbeGepaCampaignProjectionSchemaVersion,
    settlementReceiptRefs: [],
    splitManifestRefs: [
      'benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1',
    ],
    stage: 'stage_0_smoke',
    validMetricCalls: 0,
    validationResultRefs: [],
    ...overrides,
  })

const shadowPromotion = () =>
  evaluateProbeGepaStage1ShadowPromotion(
    new ProbeGepaStage1ShadowPromotionInput({
      blueprintGateRefs: [
        'blueprint_gate.probe_gepa.stage1.shadow_candidate.v1',
      ],
      candidateHash:
        'sha256:0000000000000000000000000000000000000000000000000000000000002008',
      candidateRef: 'candidate.probe_gepa.stage1.mutation_08',
      omegaGateRefs: ['omega_gate.probe_gepa.stage1.shadow_candidate.v1'],
      policyFindings: [
        new ProbeGepaStage1PolicyFinding({
          findingRef: 'policy_finding.probe_gepa.stage1.none_blocking',
          severity: 'none',
        }),
      ],
      proofBundleRefs: ['proof_bundle.probe_gepa.stage1.validation.001'],
      proofCompletenessBps: 9_000,
      psionicFrontierRefs: ['psionic_frontier.probe_gepa.stage1.candidate_08'],
      requestedState: 'shadow',
      retainedResultRefs: ['benchmark_result.probe_gepa.stage1.retained.001'],
      routeScorecardRefs: ['route_scorecard.probe_gepa.validation.001'],
      validationDeltaBps: 250,
      validationResultRefs: ['benchmark_result.probe_gepa.validation.001'],
    }),
  )

const input = (
  overrides: Partial<ArtanisProbeGepaBenchmarkSummaryInput> = {},
): ArtanisProbeGepaBenchmarkSummaryInput =>
  new ArtanisProbeGepaBenchmarkSummaryInput({
    campaignProjection: campaignProjection(),
    forumTopicRef: 'topic.public.forum.artanis.pylon_release_work_log',
    liveSmokeReceiptRefs: [
      'bundle.probe_gepa.stage0.live_shc_harbor.2026_06_08',
    ],
    operatorAuthorityRefs: [
      'operator_authority.omega.artanis.forum_publication.v1',
    ],
    projectionAuthorityRefs: [
      'projection_authority.omega.artanis.public_report.v1',
    ],
    publicReportRefs: ['report.public.artanis.status_aggregator'],
    shadowPromotion: null,
    ...overrides,
  })

describe('Artanis Probe GEPA benchmark summary projection', () => {
  test('builds a public-safe live smoke summary from Omega authority refs', () => {
    const summary = buildArtanisProbeGepaBenchmarkSummary(input())

    expect(
      S.decodeUnknownSync(ArtanisProbeGepaBenchmarkSummaryProjection)(summary),
    ).toEqual(summary)
    expect(summary).toMatchObject({
      evidenceLabel: 'live_smoke',
      noDistributedTrainingOverclaim: true,
      noPaidWorkClaim: true,
      noPublicBenchmarkScoreClaim: true,
      noSettlementClaim: true,
      postingMode: 'omega_operator_artanis_authorized',
      title: 'Artanis Probe GEPA live smoke summary',
    })
    expect(summary.bodyText).toContain(
      'Pylon-distributed GEPA rollout optimization, not distributed neural-network training',
    )
    expect(summary.bodyText).toContain('Claim boundary: live smoke measured only')
    expect(summary.bodyText).not.toMatch(/beats Terminal-Bench/i)
  })

  test('labels shadow candidate summaries from the Stage 1 shadow gate', () => {
    const summary = buildArtanisProbeGepaBenchmarkSummary(
      input({
        campaignProjection: campaignProjection({
          claimState: 'validation_measured_only',
          completedMetricCalls: 18,
          invalidMetricCalls: 2,
          plannedMetricCalls: 18,
          stage: 'validation_sweep',
          validMetricCalls: 16,
          validationResultRefs: [
            'benchmark_result.probe_gepa.validation.001',
          ],
        }),
        liveSmokeReceiptRefs: [],
        shadowPromotion: shadowPromotion(),
      }),
    )

    expect(summary.evidenceLabel).toBe('shadow_candidate')
    expect(summary.claimBoundaryLine).toBe(
      'Claim boundary: shadow candidate; validation measured only, with no active production claim.',
    )
    expect(summary.sourceEvidenceRefs).toContain(
      'promotion_decision.probe_gepa.stage1.shadow.candidate.probe_gepa.stage1.mutation_08',
    )
    expect(summary.sourceEvidenceRefs).toContain(
      'route_scorecard.probe_gepa.validation.001',
    )
  })

  test('requires operator and projection authority refs', () => {
    expect(() =>
      buildArtanisProbeGepaBenchmarkSummary(
        input({
          operatorAuthorityRefs: [],
        }),
      ),
    ).toThrow(ArtanisProbeGepaBenchmarkSummaryUnsafe)

    expect(() =>
      buildArtanisProbeGepaBenchmarkSummary(
        input({
          projectionAuthorityRefs: [],
        }),
      ),
    ).toThrow(ArtanisProbeGepaBenchmarkSummaryUnsafe)
  })

  test('rejects unsupported holdout summaries and unsafe refs', () => {
    expect(() =>
      buildArtanisProbeGepaBenchmarkSummary(
        input({
          campaignProjection: campaignProjection({
            claimState: 'holdout_summary',
            holdoutResultRefs: ['benchmark_result.probe_gepa.holdout.001'],
            retainedResultRefs: [],
          }),
          liveSmokeReceiptRefs: [],
        }),
      ),
    ).toThrow(ArtanisProbeGepaBenchmarkSummaryUnsafe)

    expect(() =>
      buildArtanisProbeGepaBenchmarkSummary(
        input({
          liveSmokeReceiptRefs: ['raw_trace.private'],
        }),
      ),
    ).toThrow(ArtanisProbeGepaBenchmarkSummaryUnsafe)
  })

  test('rejects distributed-training wording without the rollout-optimization boundary', () => {
    const summary = buildArtanisProbeGepaBenchmarkSummary(input())

    expect(() =>
      assertArtanisProbeGepaBenchmarkSummaryProjectionSafe(
        new ArtanisProbeGepaBenchmarkSummaryProjection({
          ...summary,
          bodyText: 'This is distributed training for Probe.',
        }),
      ),
    ).toThrow(ArtanisProbeGepaBenchmarkSummaryUnsafe)
  })
})

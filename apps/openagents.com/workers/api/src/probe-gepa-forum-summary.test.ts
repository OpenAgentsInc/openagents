import { describe, expect, test } from 'vitest'

import {
  ProbeGepaCampaignProjection,
  ProbeGepaCampaignProjectionSchemaVersion,
} from './probe-gepa-campaign-projection'
import {
  ProbeGepaForumSummaryInput,
  ProbeGepaForumSummaryUnsafe,
  generateProbeGepaForumSummary,
  probeGepaForumSummaryFromUnknown,
} from './probe-gepa-forum-summary'
import {
  ProbeGepaCodingOutcomeMetricSnapshot,
  ProbeGepaOutcomeMetricsProjection,
  ProbeGepaOutcomeMetricsSchemaVersion,
} from './probe-gepa-outcome-metrics'

const projection = (
  overrides: Partial<ProbeGepaCampaignProjection> = {},
): ProbeGepaCampaignProjection =>
  new ProbeGepaCampaignProjection({
    activeCandidateRefs: ['candidate.probe_gepa.stage1.mutation_08'],
    artifactManifestRefs: ['artifact_manifest.probe_gepa.stage1.001'],
    baselineCandidateRef: 'candidate.probe_gepa.stage1.baseline',
    benchmarkSuiteRefs: ['benchmark_suite.terminal_bench_2.harbor.v1'],
    blockerRefs: ['blocker.probe_gepa.none'],
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
    policyFindingRefs: ['policy_finding.probe_gepa.no_public_score_claim'],
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

const input = (
  overrides: Partial<ProbeGepaForumSummaryInput> = {},
): ProbeGepaForumSummaryInput =>
  new ProbeGepaForumSummaryInput({
    forumTopicRef: 'forum.topic.probe_gepa.benchmarks',
    outcomeMetrics: null,
    projection: projection(),
    proofBundleRefs: ['proof_bundle.probe_gepa.stage1.001'],
    scorerRefs: ['scorer.terminal_bench.binary.v1'],
    targetThreadRef: 'forum.thread.artanis.probe_gepa_benchmarks',
    verifierRefs: ['verifier.terminal_bench.harbor.v1'],
    ...overrides,
  })

const outcomeSnapshot = (
  overrides: Partial<ProbeGepaCodingOutcomeMetricSnapshot> = {},
): ProbeGepaCodingOutcomeMetricSnapshot =>
  new ProbeGepaCodingOutcomeMetricSnapshot({
    acceptanceRateBps: 5_000,
    artifactCompletenessBps: 7_000,
    closeoutQualityBps: 6_000,
    costPerAcceptedOutcomeRef: 'cost_per_accepted_outcome.coding',
    failureFamilyReductionBps: 1_000,
    humanReviewMinutes: 12,
    privateProofState: 'private_proof_available',
    proofBundleCompletenessBps: 7_000,
    publicProofState: 'public_proof_available',
    regressionCount: 1,
    retriesPerAcceptedOutcome: 1,
    retryCount: 1,
    turnsPerAcceptedOutcome: 5,
    ...overrides,
  })

const outcomeMetrics = (
  overrides: Partial<ProbeGepaOutcomeMetricsProjection> = {},
): ProbeGepaOutcomeMetricsProjection =>
  new ProbeGepaOutcomeMetricsProjection({
    acceptedOutcomeRefs: [],
    after: outcomeSnapshot({
      acceptanceRateBps: 6_000,
      artifactCompletenessBps: 8_000,
      closeoutQualityBps: 7_000,
      failureFamilyReductionBps: 2_500,
      humanReviewMinutes: 8,
      proofBundleCompletenessBps: 8_500,
      regressionCount: 0,
      retriesPerAcceptedOutcome: 0.5,
      retryCount: 0,
      turnsPerAcceptedOutcome: 4,
    }),
    before: outcomeSnapshot(),
    benchmarkCampaignRefs: ['campaign.probe_gepa.stage1.retained_failure_sprint'],
    benchmarkValidationRefs: ['benchmark_result.probe_gepa.validation.001'],
    candidateHash:
      'sha256:0000000000000000000000000000000000000000000000000000000000002008',
    candidateRef: 'candidate.probe_gepa.stage1.mutation_08',
    candidateState: 'shadow',
    claimBoundaryRef: 'claim_boundary.benchmark_validation_only',
    closeoutQualityRef: 'closeout_quality.probe_gepa.coding.after',
    failureFamilyRefs: ['failure_family.service_readiness'],
    privateProofRefs: [],
    publicProofRefs: [],
    regressionRefs: ['regression.probe_gepa.validation.none_blocking'],
    routeScorecardRefs: ['route_scorecard.probe.benchmark.validation.001'],
    schemaVersion: ProbeGepaOutcomeMetricsSchemaVersion,
    selectedSignatureRefs: [
      'program_signature.probe.benchmark.service_readiness.v1',
    ],
    toolMenuRefs: ['tool_menu.probe.terminal_bench.service_readiness.v1'],
    workroomComparisonRefs: [
      'workroom_comparison.coding_autopilot.before_after.001',
    ],
    workroomOutcomeRefs: [],
    workroomRefs: ['workroom.coding_autopilot.probe_gepa.shadow_001'],
    ...overrides,
  })

describe('Probe GEPA Forum summary generator', () => {
  test('generates idempotent retained-only Forum copy from refs', () => {
    const first = generateProbeGepaForumSummary(input())
    const second = generateProbeGepaForumSummary(input())

    expect(first.idempotencyKey).toBe(second.idempotencyKey)
    expect(first.bodyMarkdown).toContain(
      'Campaign: campaign.probe_gepa.stage1.retained_failure_sprint',
    )
    expect(first.bodyMarkdown).toContain('Completed metric calls: 210')
    expect(first.bodyMarkdown).toContain('Valid/invalid rollout count: 115/95')
    expect(first.bodyMarkdown).toContain(
      'Product outcome boundary: no accepted coding outcome evidence attached.',
    )
    expect(first.claimBoundaryLine).toBe(
      'Claim boundary: retained evidence summary only; this is not a public benchmark score.',
    )
    expect(first.postingAuthorityBoundary).toContain(
      'Posting as Artanis requires Omega/operator authority',
    )
    expect(probeGepaForumSummaryFromUnknown(first)).toEqual(first)
  })

  test('uses validation claim language without describing holdout performance', () => {
    const draft = generateProbeGepaForumSummary(
      input({
        projection: projection({
          claimState: 'validation_measured_only',
          retainedResultRefs: [],
          stage: 'validation_sweep',
          validMetricCalls: 16,
          invalidMetricCalls: 2,
          completedMetricCalls: 18,
          plannedMetricCalls: 18,
          validationResultRefs: [
            'benchmark_result.probe_gepa.validation.configure_git_webserver.001',
          ],
        }),
      }),
    )

    expect(draft.claimBoundaryLine).toBe(
      'Claim boundary: validation measured only; this is not frozen holdout performance.',
    )
    expect(draft.bodyMarkdown).not.toMatch(/frozen holdout performance\.$/i)
    expect(draft.bodyMarkdown).not.toMatch(/beats Terminal-Bench/i)
  })

  test('adds accepted outcome wording only when outcome evidence is attached', () => {
    const benchmarkOnly = generateProbeGepaForumSummary(
      input({
        outcomeMetrics: outcomeMetrics(),
      }),
    )
    const acceptedOutcome = generateProbeGepaForumSummary(
      input({
        outcomeMetrics: outcomeMetrics({
          acceptedOutcomeRefs: ['accepted_outcome.coding.workroom.001'],
          privateProofRefs: ['proof.private.coding.workroom.001'],
          publicProofRefs: ['proof.public.coding.workroom.001'],
          workroomOutcomeRefs: ['workroom_outcome.coding.accepted.001'],
        }),
      }),
    )

    expect(benchmarkOnly.bodyMarkdown).toContain(
      'Product outcome boundary: Benchmark validation only; no paid customer outcome improvement claim.',
    )
    expect(acceptedOutcome.bodyMarkdown).toContain(
      'Product outcome boundary: Accepted coding outcome comparison; paid customer outcome improvement is linked to accepted outcome refs and proof refs.',
    )
  })

  test('allows Probe registered-agent reply mode but never Artanis bridge mode', () => {
    const draft = generateProbeGepaForumSummary(
      input(),
      'probe_registered_agent_reply',
    )

    expect(draft.postingMode).toBe('probe_registered_agent_reply')
    expect(draft.postingAuthorityBoundary).toContain(
      'this draft does not invoke the Artanis bridge',
    )
    expect(() =>
      generateProbeGepaForumSummary(input(), 'artanis_bridge' as never),
    ).toThrow(ProbeGepaForumSummaryUnsafe)
  })

  test('rejects raw traces prompts fixtures secrets and local paths', () => {
    for (const unsafe of [
      'raw_trace.public',
      'raw_prompt.public',
      'raw_benchmark_fixture.public',
      'Bearer abc',
      'wallet_mnemonic.secret',
      '/Users/christopherdavid/private/path',
    ]) {
      expect(() =>
        generateProbeGepaForumSummary(
          input({
            proofBundleRefs: [unsafe],
          }),
        ),
      ).toThrow(ProbeGepaForumSummaryUnsafe)
    }
  })

  test('does not describe retained improvements as public benchmark scores', () => {
    const draft = generateProbeGepaForumSummary(input())

    expect(draft.bodyMarkdown).not.toMatch(/retained improvements/i)
    expect(draft.bodyMarkdown).not.toMatch(/public benchmark score\.$/i)
    expect(draft.bodyMarkdown).toContain(
      'Claim boundary: retained evidence summary only',
    )
  })
})

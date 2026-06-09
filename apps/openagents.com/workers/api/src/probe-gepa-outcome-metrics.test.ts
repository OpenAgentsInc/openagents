import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ProbeGepaCodingOutcomeMetricSnapshot,
  ProbeGepaOutcomeMetricsProjection,
  ProbeGepaOutcomeMetricsAudienceProjection,
  ProbeGepaOutcomeMetricsSchemaVersion,
  ProbeGepaOutcomeMetricsUnsafe,
  assertProbeGepaOutcomeMetricsSafe,
  projectProbeGepaOutcomeMetricsForAudience,
  probeGepaOutcomeMetricSummary,
} from './probe-gepa-outcome-metrics'

const before = () =>
  new ProbeGepaCodingOutcomeMetricSnapshot({
    acceptanceRateBps: 5_000,
    artifactCompletenessBps: 7_000,
    closeoutQualityBps: 6_000,
    costPerAcceptedOutcomeRef: 'cost_per_accepted_outcome.before.coding',
    failureFamilyReductionBps: 0,
    humanReviewMinutes: 18,
    privateProofState: 'private_proof_available',
    publicProofState: 'redacted',
    proofBundleCompletenessBps: 6_500,
    regressionCount: 4,
    retryCount: 3,
    retriesPerAcceptedOutcome: 2,
    turnsPerAcceptedOutcome: 9,
  })

const after = () =>
  new ProbeGepaCodingOutcomeMetricSnapshot({
    acceptanceRateBps: 6_500,
    artifactCompletenessBps: 8_500,
    closeoutQualityBps: 7_500,
    costPerAcceptedOutcomeRef: 'cost_per_accepted_outcome.after.coding',
    failureFamilyReductionBps: 2_000,
    humanReviewMinutes: 11,
    privateProofState: 'private_proof_available',
    publicProofState: 'public_proof_available',
    proofBundleCompletenessBps: 8_000,
    regressionCount: 2,
    retryCount: 1,
    retriesPerAcceptedOutcome: 0.5,
    turnsPerAcceptedOutcome: 6,
  })

const projection = (
  overrides: Partial<ProbeGepaOutcomeMetricsProjection> = {},
): ProbeGepaOutcomeMetricsProjection =>
  new ProbeGepaOutcomeMetricsProjection({
    acceptedOutcomeRefs: [],
    after: after(),
    before: before(),
    benchmarkCampaignRefs: [
      'campaign.probe_gepa.stage1.retained_failure_sprint',
    ],
    benchmarkValidationRefs: ['benchmark_result.probe_gepa.validation.001'],
    candidateHash:
      'sha256:0000000000000000000000000000000000000000000000000000000000002008',
    candidateRef: 'candidate.probe_gepa.stage1.mutation_08',
    candidateState: 'benchmark_only',
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

describe('Probe GEPA accepted outcome metrics projection', () => {
  test('keeps benchmark-only candidates from claiming product wins', () => {
    const summary = probeGepaOutcomeMetricSummary(projection())

    expect(summary).toMatchObject({
      candidateState: 'benchmark_only',
      claimText:
        'Benchmark validation only; no paid customer outcome improvement claim.',
      productOutcomeClaimAllowed: false,
    })
    expect(summary.delta).toEqual({
      acceptanceRateDeltaBps: 1_500,
      artifactCompletenessDeltaBps: 1_500,
      closeoutQualityDeltaBps: 1_500,
      failureFamilyReductionDeltaBps: 2_000,
      humanReviewMinutesDelta: -7,
      proofBundleCompletenessDeltaBps: 1_500,
      regressionCountDelta: -2,
      retriesPerAcceptedOutcomeDelta: -1.5,
      retryCountDelta: -2,
      turnsPerAcceptedOutcomeDelta: -3,
    })
  })

  test('allows accepted coding outcome comparison only with outcome and proof refs', () => {
    const safe = assertProbeGepaOutcomeMetricsSafe(
      projection({
        acceptedOutcomeRefs: ['accepted_outcome.coding.workroom.001'],
        candidateState: 'shadow',
        privateProofRefs: ['proof.private.coding.workroom.001'],
        publicProofRefs: ['proof.public.coding.workroom.001'],
        workroomOutcomeRefs: ['workroom_outcome.coding.accepted.001'],
      }),
    )
    const summary = probeGepaOutcomeMetricSummary(safe)

    expect(
      S.decodeUnknownSync(ProbeGepaOutcomeMetricsProjection)(safe),
    ).toEqual(safe)
    expect(summary.productOutcomeClaimAllowed).toBe(true)
    expect(summary.claimText).toBe(
      'Accepted coding outcome comparison; paid customer outcome improvement is linked to accepted outcome refs and proof refs.',
    )
  })

  test('displays benchmark-only shadow release candidate and active states with gates', () => {
    for (const candidateState of [
      'benchmark_only',
      'shadow',
      'release_candidate',
    ] as const) {
      expect(
        assertProbeGepaOutcomeMetricsSafe(projection({ candidateState }))
          .candidateState,
      ).toBe(candidateState)
    }

    expect(() =>
      assertProbeGepaOutcomeMetricsSafe(
        projection({
          candidateState: 'active',
        }),
      ),
    ).toThrow(ProbeGepaOutcomeMetricsUnsafe)

    expect(
      assertProbeGepaOutcomeMetricsSafe(
        projection({
          acceptedOutcomeRefs: ['accepted_outcome.coding.workroom.001'],
          candidateState: 'active',
          privateProofRefs: ['proof.private.coding.workroom.001'],
          publicProofRefs: ['proof.public.coding.workroom.001'],
          workroomOutcomeRefs: ['workroom_outcome.coding.accepted.001'],
        }),
      ).candidateState,
    ).toBe('active')
  })

  test('requires validation refs and route scorecards for product comparison', () => {
    expect(() =>
      assertProbeGepaOutcomeMetricsSafe(
        projection({
          benchmarkValidationRefs: [],
        }),
      ),
    ).toThrow(ProbeGepaOutcomeMetricsUnsafe)

    expect(() =>
      assertProbeGepaOutcomeMetricsSafe(
        projection({
          routeScorecardRefs: [],
        }),
      ),
    ).toThrow(ProbeGepaOutcomeMetricsUnsafe)
  })

  test('requires selected signatures tool menu and workroom outcome refs for product claims', () => {
    expect(() =>
      assertProbeGepaOutcomeMetricsSafe(
        projection({
          selectedSignatureRefs: [],
        }),
      ),
    ).toThrow(ProbeGepaOutcomeMetricsUnsafe)

    expect(() =>
      assertProbeGepaOutcomeMetricsSafe(
        projection({
          toolMenuRefs: [],
        }),
      ),
    ).toThrow(ProbeGepaOutcomeMetricsUnsafe)

    expect(() =>
      assertProbeGepaOutcomeMetricsSafe(
        projection({
          acceptedOutcomeRefs: ['accepted_outcome.coding.workroom.001'],
          privateProofRefs: ['proof.private.coding.workroom.001'],
          publicProofRefs: ['proof.public.coding.workroom.001'],
          workroomOutcomeRefs: [],
        }),
      ),
    ).toThrow(ProbeGepaOutcomeMetricsUnsafe)
  })

  test('projects public route scorecards without private workroom deltas until outcomes are accepted', () => {
    const publicProjection = projectProbeGepaOutcomeMetricsForAudience(
      projection(),
      'public',
    )
    const operatorProjection = projectProbeGepaOutcomeMetricsForAudience(
      projection(),
      'operator',
    )

    expect(
      S.decodeUnknownSync(ProbeGepaOutcomeMetricsAudienceProjection)(
        publicProjection,
      ),
    ).toEqual(publicProjection)
    expect(publicProjection.productOutcomeClaimAllowed).toBe(false)
    expect(publicProjection.routeScorecardRefs).toEqual([
      'route_scorecard.probe.benchmark.validation.001',
    ])
    expect(publicProjection.workroomRefs).toEqual([])
    expect(publicProjection.workroomComparisonRefs).toEqual([])
    expect(operatorProjection.workroomRefs).toEqual([
      'workroom.coding_autopilot.probe_gepa.shadow_001',
    ])
    expect(operatorProjection.workroomComparisonRefs).toEqual([
      'workroom_comparison.coding_autopilot.before_after.001',
    ])
  })

  test('public projection can expose accepted outcome and public proof refs only after the gate passes', () => {
    const publicProjection = projectProbeGepaOutcomeMetricsForAudience(
      projection({
        acceptedOutcomeRefs: ['accepted_outcome.coding.workroom.001'],
        candidateState: 'shadow',
        privateProofRefs: ['proof.private.coding.workroom.001'],
        publicProofRefs: ['proof.public.coding.workroom.001'],
        workroomOutcomeRefs: ['workroom_outcome.coding.accepted.001'],
      }),
      'public',
    )

    expect(publicProjection.productOutcomeClaimAllowed).toBe(true)
    expect(publicProjection.acceptedOutcomeRefs).toEqual([
      'accepted_outcome.coding.workroom.001',
    ])
    expect(publicProjection.publicProofRefs).toEqual([
      'proof.public.coding.workroom.001',
    ])
    expect(publicProjection.privateProofRefs).toEqual([])
    expect(publicProjection.workroomOutcomeRefs).toEqual([
      'workroom_outcome.coding.accepted.001',
    ])
  })

  test('rejects unsafe refs and invalid metric bounds', () => {
    expect(() =>
      assertProbeGepaOutcomeMetricsSafe(
        projection({
          routeScorecardRefs: ['raw_trace.private'],
        }),
      ),
    ).toThrow(ProbeGepaOutcomeMetricsUnsafe)

    expect(() =>
      assertProbeGepaOutcomeMetricsSafe(
        projection({
          after: new ProbeGepaCodingOutcomeMetricSnapshot({
            ...after(),
            acceptanceRateBps: 10_001,
          }),
        }),
      ),
    ).toThrow(ProbeGepaOutcomeMetricsUnsafe)
  })
})

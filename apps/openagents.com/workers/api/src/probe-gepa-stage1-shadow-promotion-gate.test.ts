import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ProbeGepaStage1PolicyFinding,
  ProbeGepaStage1ShadowPromotionInput,
  ProbeGepaStage1ShadowPromotionResult,
  ProbeGepaStage1ShadowPromotionUnsafe,
  evaluateProbeGepaStage1ShadowPromotion,
} from './probe-gepa-stage1-shadow-promotion-gate'

const input = (
  overrides: Partial<ProbeGepaStage1ShadowPromotionInput> = {},
): ProbeGepaStage1ShadowPromotionInput =>
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
    ...overrides,
  })

describe('Probe GEPA Stage 1 shadow promotion gate', () => {
  test('accepts sufficient retained and validation evidence to shadow only', () => {
    const result = evaluateProbeGepaStage1ShadowPromotion(input())

    expect(S.decodeUnknownSync(ProbeGepaStage1ShadowPromotionResult)(result)).toEqual(
      result,
    )
    expect(result).toMatchObject({
      activeProductionAllowed: false,
      candidateState: 'shadow',
      decision: 'shadow',
      publicClaimLabel: 'shadow candidate; validation measured only',
      publicStatusLabel: 'shadow candidate',
      releaseCandidateAllowed: false,
      requestedState: 'shadow',
    })
    expect(result.blockerRefs).toEqual([])
    expect(result.omegaGateRefs).toEqual([
      'omega_gate.probe_gepa.stage1.shadow_candidate.v1',
    ])
    expect(result.blueprintGateRefs).toEqual([
      'blueprint_gate.probe_gepa.stage1.shadow_candidate.v1',
    ])
  })

  test('blocks active production requests from the Stage 1 benchmark gate', () => {
    const result = evaluateProbeGepaStage1ShadowPromotion(
      input({ requestedState: 'active' }),
    )

    expect(result).toMatchObject({
      activeProductionAllowed: false,
      candidateState: 'benchmark_only',
      decision: 'rejected',
      publicClaimLabel: 'candidate rejected by shadow gate',
      releaseCandidateAllowed: false,
      requestedState: 'active',
    })
    expect(result.blockerRefs).toContain(
      'blocker.probe_gepa.stage1.active_not_allowed_by_shadow_gate',
    )
  })

  test('blocks release-candidate requests without a separate production gate', () => {
    const result = evaluateProbeGepaStage1ShadowPromotion(
      input({ requestedState: 'release_candidate' }),
    )

    expect(result.candidateState).toBe('benchmark_only')
    expect(result.decision).toBe('rejected')
    expect(result.blockerRefs).toContain(
      'blocker.probe_gepa.stage1.release_candidate_requires_separate_gate',
    )
  })

  test('requires proof completeness, route scorecards, and nonblocking policy findings', () => {
    const result = evaluateProbeGepaStage1ShadowPromotion(
      input({
        policyFindings: [
          new ProbeGepaStage1PolicyFinding({
            findingRef: 'policy_finding.probe_gepa.stage1.tool_scope_blocking',
            severity: 'blocking',
          }),
        ],
        proofBundleRefs: [],
        proofCompletenessBps: 7_500,
        routeScorecardRefs: [],
      }),
    )

    expect(result.decision).toBe('rejected')
    expect(result.blockerRefs).toEqual([
      'blocker.probe_gepa.stage1.missing_proof_bundles',
      'blocker.probe_gepa.stage1.missing_route_scorecards',
      'blocker.probe_gepa.stage1.policy.policy_finding.probe_gepa.stage1.tool_scope_blocking',
      'blocker.probe_gepa.stage1.proof_incomplete',
    ])
  })

  test('rejects unsafe refs before producing a promotion decision', () => {
    expect(() =>
      evaluateProbeGepaStage1ShadowPromotion(
        input({ routeScorecardRefs: ['raw_trace.private'] }),
      ),
    ).toThrow(ProbeGepaStage1ShadowPromotionUnsafe)
  })
})

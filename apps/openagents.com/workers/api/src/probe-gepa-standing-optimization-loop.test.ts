import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ProbeGepaStandingOptimizationLoopInput,
  ProbeGepaStandingOptimizationLoopProjection,
  ProbeGepaStandingOptimizationLoopSchemaVersion,
  ProbeGepaStandingOptimizationLoopUnsafe,
  projectProbeGepaStandingOptimizationLoop,
} from './probe-gepa-standing-optimization-loop'

const input = (
  overrides: Partial<ProbeGepaStandingOptimizationLoopInput> = {},
): ProbeGepaStandingOptimizationLoopInput =>
  new ProbeGepaStandingOptimizationLoopInput({
    candidateArtifactRefs: [
      'artifact_manifest.probe_gepa_standing_loop.candidate.instructions_001',
    ],
    candidateManifestRefs: [
      'candidate_manifest.probe_gepa_standing_loop.dspy_gepa_001',
    ],
    dspyRlmAuditRefs: ['github.pr.openagents.6704'],
    effectAuthorityGateRefs: [
      'effect_authority_gate.blueprint.candidate_admission.v1',
    ],
    evalResultRefs: ['eval_result.studybench.recent.low_quality_001'],
    failureFamilyRefs: ['failure_family.coding.closeout_incomplete'],
    issueRefs: ['github.issue.openagents.6707'],
    loopRef: 'loop.probe_gepa_standing_optimization.issue_6707',
    lowQualityTurnRefs: ['turn.public.khala_trace.low_quality_001'],
    metricCallCount: 32,
    mutaliskLaneRefs: ['lane.mutalisk.gepa_dspy.offline.v1'],
    optimizerRunRefs: ['optimizer_run.gepa_dspy.mutalisk.issue_6707.001'],
    releaseGateRefs: ['release_gate.blueprint.gepa_candidate.operator.v1'],
    requestedAction: 'emit_candidates',
    sourceTraceRefs: ['trace.public.khala.redacted_recent_001'],
    ...overrides,
  })

describe('Probe GEPA standing optimization loop projection (#6707)', () => {
  test('admits candidate artifacts only as offline Mutalisk output for the Effect authority gate', () => {
    const projection = projectProbeGepaStandingOptimizationLoop(input())

    expect(
      S.decodeUnknownSync(ProbeGepaStandingOptimizationLoopProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.schemaVersion).toBe(
      ProbeGepaStandingOptimizationLoopSchemaVersion,
    )
    expect(projection.decision).toBe('candidate_artifacts_ready')
    expect(projection.offlineOptimizationReady).toBe(true)
    expect(projection.candidateArtifactsAdmissibleToAuthority).toBe(true)
    expect(projection.livePromotionAllowed).toBe(false)
    expect(projection.evalResultRefs).toEqual([
      'eval_result.studybench.recent.low_quality_001',
    ])
    expect(projection.evidenceRefs).toEqual([
      'eval_result.studybench.recent.low_quality_001',
      'github.pr.openagents.6704',
      'optimizer_run.gepa_dspy.mutalisk.issue_6707.001',
      'trace.public.khala.redacted_recent_001',
    ])
  })

  test('blocks the standing loop without trace or eval evidence and low-quality selection', () => {
    const projection = projectProbeGepaStandingOptimizationLoop(
      input({
        evalResultRefs: [],
        failureFamilyRefs: [],
        lowQualityTurnRefs: [],
        sourceTraceRefs: [],
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.trace_or_eval_refs_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.low_quality_selection_missing',
    )
  })

  test('requires the DSPy/RLM audit, Mutalisk optimizer runs, candidate manifests, and authority gates before emission', () => {
    const projection = projectProbeGepaStandingOptimizationLoop(
      input({
        candidateArtifactRefs: [],
        candidateManifestRefs: [],
        dspyRlmAuditRefs: [],
        effectAuthorityGateRefs: [],
        mutaliskLaneRefs: [],
        optimizerRunRefs: [],
        releaseGateRefs: [],
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.offlineOptimizationReady).toBe(false)
    expect(projection.candidateArtifactsAdmissibleToAuthority).toBe(false)
    expect(projection.blockerRefs).toEqual([
      'blocker.probe_gepa_standing_loop.candidate_artifacts_missing',
      'blocker.probe_gepa_standing_loop.candidate_manifests_missing',
      'blocker.probe_gepa_standing_loop.dspy_rlm_audit_missing',
      'blocker.probe_gepa_standing_loop.mutalisk_lane_missing',
      'blocker.probe_gepa_standing_loop.optimizer_run_refs_missing',
    ])
  })

  test('candidate refs require separate Effect authority and release gates', () => {
    const projection = projectProbeGepaStandingOptimizationLoop(
      input({
        effectAuthorityGateRefs: [],
        releaseGateRefs: [],
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.effect_authority_gate_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.release_gate_missing',
    )
  })

  test('rejects any request to promote live behavior from the standing loop itself', () => {
    const projection = projectProbeGepaStandingOptimizationLoop(
      input({
        requestedAction: 'promote_live',
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.livePromotionAllowed).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.live_promotion_not_allowed',
    )
  })

  test('rejects raw traces private paths credentials wallet material and raw timestamps', () => {
    for (const unsafe of [
      'raw_trace.private.full',
      '/Users/christopherdavid/private/trace',
      'provider_token.secret',
      'wallet_mnemonic.private',
      'trace.public.2026-06-28T12:00:00',
    ]) {
      expect(() =>
        projectProbeGepaStandingOptimizationLoop(
          input({
            sourceTraceRefs: [unsafe],
          }),
        ),
      ).toThrow(ProbeGepaStandingOptimizationLoopUnsafe)
    }
  })

  test('dedupes and exposes eval result refs as first-class closure evidence', () => {
    const projection = projectProbeGepaStandingOptimizationLoop(
      input({
        evalResultRefs: [
          'eval_result.studybench.recent.low_quality_002',
          'eval_result.studybench.recent.low_quality_001',
          'eval_result.studybench.recent.low_quality_002',
        ],
      }),
    )

    expect(projection.evalResultRefs).toEqual([
      'eval_result.studybench.recent.low_quality_001',
      'eval_result.studybench.recent.low_quality_002',
    ])
  })
})

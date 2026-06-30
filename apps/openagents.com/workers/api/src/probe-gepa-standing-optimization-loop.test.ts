import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  KhalaFleetDelegationBlueprintSelection,
  KhalaFleetDelegationCandidateAdmissionInput,
  KhalaFleetDelegationCandidateAdmissionProjection,
  KhalaFleetDelegationCandidateAdmissionSchemaVersion,
  KhalaFleetDelegationCandidateManifestSummary,
  KhalaFleetDelegationCandidateSignature,
  KhalaFleetDelegationProgramSignatureId,
  KhalaFleetDelegationProgramTypeId,
  ProbeGepaStandingOptimizationLoopInput,
  ProbeGepaStandingOptimizationLoopProjection,
  ProbeGepaCandidateManifestSchemaVersion,
  ProbeGepaStandingOptimizationLoopSchemaVersion,
  ProbeGepaStandingOptimizationLoopUnsafe,
  projectKhalaFleetDelegationCandidateAdmission,
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

const khalaCandidate = (
  overrides: Partial<KhalaFleetDelegationCandidateManifestSummary> = {},
): KhalaFleetDelegationCandidateManifestSummary =>
  new KhalaFleetDelegationCandidateManifestSummary({
    baseModuleRef: 'module.khala_fleet_delegation.seed.v1',
    candidateManifestRef: 'candidate_manifest.khala_fleet_delegation.001',
    candidateRef: 'candidate.khala_fleet_delegation.001',
    evalEvidenceRefs: ['eval_result.khala_delegation.gd1.001'],
    metricName: KhalaFleetDelegationCandidateSignature,
    metricValueBps: 8123,
    optimizedModuleRef: 'module.khala_fleet_delegation.optimized.v1',
    schemaVersion: ProbeGepaCandidateManifestSchemaVersion,
    signature: KhalaFleetDelegationCandidateSignature,
    traceProvenanceRefs: ['trace.public.khala_delegation.001'],
    ...overrides,
  })

const blueprintSelection = (
  overrides: Partial<KhalaFleetDelegationBlueprintSelection> = {},
): KhalaFleetDelegationBlueprintSelection =>
  new KhalaFleetDelegationBlueprintSelection({
    actionSubmissionRequiredForDirectEffects: true,
    candidateEntryIds: ['blueprint.entry.khala_fleet_delegation.v1'],
    directMutationAllowed: false,
    evidenceRequirementRefs: ['evidence_requirement.khala_delegation.eval_refs'],
    lookupId: 'lookup.khala_fleet_delegation.001',
    moduleVersionIds: ['module_version.khala_fleet_delegation.policy.v1'],
    policyRef: 'policy.blueprint.action_submission_required.v1',
    programSignatureIds: [KhalaFleetDelegationProgramSignatureId],
    programTypeIds: [KhalaFleetDelegationProgramTypeId],
    receiptRequirementRefs: [
      'receipt_requirement.action_submission.operator_review',
    ],
    registryVersionRef: 'registry.blueprint.khala_fleet_delegation.v1',
    releaseGateRefs: ['release_gate.khala_fleet_delegation.operator.v1'],
    safeProjection: true,
    toolScopes: ['tool_scope.khala_delegation.policy_proposal'],
    ...overrides,
  })

const khalaAdmissionInput = (
  overrides: Partial<KhalaFleetDelegationCandidateAdmissionInput> = {},
): KhalaFleetDelegationCandidateAdmissionInput =>
  new KhalaFleetDelegationCandidateAdmissionInput({
    actorRef: 'actor.operator.openagents',
    approvalPolicyRef:
      'policy.khala_delegation.operator_approval_required.v1',
    blueprintSelection: blueprintSelection(),
    candidate: khalaCandidate(),
    contextPackRefs: ['context_pack.khala_delegation.gd3.v1'],
    observedAt: '2026-06-30T14:00:00.000Z',
    programRunRef: 'program_run.khala_delegation.candidate_admission.001',
    standingLoop: input({
      candidateArtifactRefs: [
        'artifact.khala_fleet_delegation.optimized_module.001',
      ],
      candidateManifestRefs: ['candidate_manifest.khala_fleet_delegation.001'],
      dspyRlmAuditRefs: ['audit.gepa_khala_delegation.feedback.001'],
      effectAuthorityGateRefs: [
        'effect_authority_gate.blueprint.khala_delegation.v1',
      ],
      evalResultRefs: ['eval_result.khala_delegation.gd1.001'],
      failureFamilyRefs: [
        'failure_family.khala_delegation.no_available_codex_capacity',
      ],
      issueRefs: ['github.issue.openagents.7735'],
      loopRef: 'loop.khala_fleet_delegation.issue_7735',
      lowQualityTurnRefs: [],
      mutaliskLaneRefs: ['lane.mutalisk.gepa_delegation.offline.v1'],
      optimizerRunRefs: ['optimizer_run.gepa_khala_delegation.001'],
      releaseGateRefs: ['release_gate.khala_fleet_delegation.operator.v1'],
      requestedAction: 'emit_candidates',
      sourceTraceRefs: ['trace.public.khala_delegation.001'],
    }),
    summaryRef: 'summary.khala_delegation.candidate_admission.001',
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

  test('blocks the standing loop without trace evidence, eval evidence, or low-quality selection', () => {
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
      'blocker.probe_gepa_standing_loop.source_trace_refs_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.eval_result_refs_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.low_quality_selection_missing',
    )
  })

  test('requires both source trace refs and eval result refs before candidate emission', () => {
    expect(
      projectProbeGepaStandingOptimizationLoop(
        input({
          evalResultRefs: [],
        }),
      ).blockerRefs,
    ).toContain('blocker.probe_gepa_standing_loop.eval_result_refs_missing')

    expect(
      projectProbeGepaStandingOptimizationLoop(
        input({
          sourceTraceRefs: [],
        }),
      ).blockerRefs,
    ).toContain('blocker.probe_gepa_standing_loop.source_trace_refs_missing')
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

describe('Khala fleet delegation candidate admission projection (#7735)', () => {
  test('surfaces a khala.fleet.delegation Candidate as a gated evidence-only Action Submission proposal', () => {
    const projection = projectKhalaFleetDelegationCandidateAdmission(
      khalaAdmissionInput(),
    )

    expect(
      S.decodeUnknownSync(KhalaFleetDelegationCandidateAdmissionProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.schemaVersion).toBe(
      KhalaFleetDelegationCandidateAdmissionSchemaVersion,
    )
    expect(projection.decision).toBe('gated_proposal_ready')
    expect(projection.proposalRequired).toBe(true)
    expect(projection.livePromotionAllowed).toBe(false)
    expect(projection.runtimePromotionAllowed).toBe(false)
    expect(projection.directExecutionAllowed).toBe(false)
    expect(projection.autoPromotionPathExists).toBe(false)
    expect(projection.actionSubmissionProposalRefs).toEqual([
      'action_submission.khala_fleet_delegation.candidate.khala_fleet_delegation.001',
    ])

    const proposal = projection.actionSubmissionProposal
    expect(proposal).not.toBeNull()
    expect(proposal?.approvalRequired).toBe(true)
    expect(proposal?.directExecution).toBe(false)
    expect(proposal?.directProgramRunExecutionAllowed).toBe(false)
    expect(proposal?.modelConfidenceBypassDisabled).toBe(true)
    expect(proposal?.proposalOnly).toBe(true)
    expect(proposal?.programRunAuthorityBoundary).toBe('evidence_only')
    expect(proposal?.effectKind).toBe('mutate_source_backed_business_fact')
    expect(proposal?.status).toBe('proposed')
    expect(proposal?.programSignatureId).toBe(
      KhalaFleetDelegationProgramSignatureId,
    )
    expect(proposal?.programTypeId).toBe(KhalaFleetDelegationProgramTypeId)
    expect(proposal?.typedIntent).toMatchObject({
      approvalRequired: true,
      candidateManifestRef: 'candidate_manifest.khala_fleet_delegation.001',
      candidateRef: 'candidate.khala_fleet_delegation.001',
      directExecutionAllowed: false,
      metricName: KhalaFleetDelegationCandidateSignature,
      metricValueBps: 8123,
      schemaVersion: 'openagents.khala.fleet_delegation_candidate_intent.v0',
      signature: KhalaFleetDelegationCandidateSignature,
    })
    expect(proposal?.evidenceRefs).toEqual([
      'audit.gepa_khala_delegation.feedback.001',
      'eval_result.khala_delegation.gd1.001',
      'evidence_requirement.khala_delegation.eval_refs',
      'optimizer_run.gepa_khala_delegation.001',
      'trace.public.khala_delegation.001',
    ])
  })

  test('blocks admission instead of creating an auto-promotion path when the standing loop asks to promote live', () => {
    const projection = projectKhalaFleetDelegationCandidateAdmission(
      khalaAdmissionInput({
        standingLoop: input({
          candidateArtifactRefs: [
            'artifact.khala_fleet_delegation.optimized_module.001',
          ],
          candidateManifestRefs: [
            'candidate_manifest.khala_fleet_delegation.001',
          ],
          dspyRlmAuditRefs: ['audit.gepa_khala_delegation.feedback.001'],
          effectAuthorityGateRefs: [
            'effect_authority_gate.blueprint.khala_delegation.v1',
          ],
          evalResultRefs: ['eval_result.khala_delegation.gd1.001'],
          failureFamilyRefs: [
            'failure_family.khala_delegation.no_available_codex_capacity',
          ],
          issueRefs: ['github.issue.openagents.7735'],
          loopRef: 'loop.khala_fleet_delegation.issue_7735',
          mutaliskLaneRefs: ['lane.mutalisk.gepa_delegation.offline.v1'],
          optimizerRunRefs: ['optimizer_run.gepa_khala_delegation.001'],
          releaseGateRefs: [
            'release_gate.khala_fleet_delegation.operator.v1',
          ],
          requestedAction: 'promote_live',
          sourceTraceRefs: ['trace.public.khala_delegation.001'],
        }),
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.actionSubmissionProposal).toBeNull()
    expect(projection.livePromotionAllowed).toBe(false)
    expect(projection.autoPromotionPathExists).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.live_promotion_not_allowed',
    )
  })

  test('blocks admission when Blueprint signature lookup is missing the khala.fleet.delegation signature', () => {
    const projection = projectKhalaFleetDelegationCandidateAdmission(
      khalaAdmissionInput({
        blueprintSelection: blueprintSelection({
          programSignatureIds: ['program_signature.other_policy.v1'],
        }),
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.actionSubmissionProposal).toBeNull()
    expect(projection.actionSubmissionProposalRefs).toEqual([])
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.khala_delegation_signature_lookup_missing',
    )
  })

  test('blocks admission when the Blueprint lookup would allow direct mutation', () => {
    const projection = projectKhalaFleetDelegationCandidateAdmission(
      khalaAdmissionInput({
        blueprintSelection: blueprintSelection({
          directMutationAllowed: true,
        }),
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.actionSubmissionProposal).toBeNull()
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_standing_loop.khala_delegation_direct_mutation_not_allowed',
    )
  })

  test('rejects unsafe candidate refs before building a proposal', () => {
    expect(() =>
      projectKhalaFleetDelegationCandidateAdmission(
        khalaAdmissionInput({
          candidate: khalaCandidate({
            traceProvenanceRefs: ['/Users/christopherdavid/raw_trace.json'],
          }),
        }),
      ),
    ).toThrow(ProbeGepaStandingOptimizationLoopUnsafe)
  })
})

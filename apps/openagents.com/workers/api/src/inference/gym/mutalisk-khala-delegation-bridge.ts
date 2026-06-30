import { Schema as S } from 'effect'

import {
  KhalaFleetDelegationBlueprintSelection,
  KhalaFleetDelegationCandidateAdmissionInput,
  KhalaFleetDelegationCandidateAdmissionProjection,
  KhalaFleetDelegationCandidateSignature,
  KhalaFleetDelegationProgramSignatureId,
  KhalaFleetDelegationProgramTypeId,
  KhalaFleetDelegationCandidateManifestSummary,
  ProbeGepaCandidateManifestSchemaVersion,
  ProbeGepaStandingOptimizationLoopInput,
  projectKhalaFleetDelegationCandidateAdmission,
} from '../../probe-gepa-standing-optimization-loop'

export const MutaliskKhalaDelegationJobSchemaVersion =
  'openagents.gym.mutalisk_khala_delegation_job.v0'
export const MutaliskKhalaDelegationSummarySchemaVersion =
  'openagents.gym.mutalisk_khala_delegation_summary.v0'
export const MutaliskKhalaDelegationBridgeOutputSchemaVersion =
  'openagents.gym.mutalisk_khala_delegation_bridge_output.v0'
export const KhalaCodeDelegationGepaEnvironmentId =
  'khala-code-delegation-gepa'
export const MutaliskKhalaDelegationDemandKind = 'internal'
export const MutaliskKhalaDelegationDemandSource =
  'gym_khala_code_delegation_gepa'
export const GymRunProgressSchemaVersion = 'openagents.gym.run_progress.v1'

export const MutaliskKhalaDelegationStage = S.Literals([
  'queued',
  'running',
  'summary_ingested',
  'admission_projected',
  'completed',
  'blocked',
])
export type MutaliskKhalaDelegationStage =
  typeof MutaliskKhalaDelegationStage.Type

export class MutaliskKhalaDelegationJob extends S.Class<MutaliskKhalaDelegationJob>(
  'MutaliskKhalaDelegationJob',
)({
  schemaVersion: S.Literal(MutaliskKhalaDelegationJobSchemaVersion),
  runRef: S.String,
  jobRef: S.String,
  environmentId: S.Literal(KhalaCodeDelegationGepaEnvironmentId),
  signature: S.Literal(KhalaFleetDelegationCandidateSignature),
  baseModuleRef: S.String,
  seedCandidateRef: S.String,
  datasetRef: S.String,
  trainSplitRefs: S.Array(S.String),
  validationSplitRefs: S.Array(S.String),
  feedbackSchemaRef: S.Literal(
    'openagents.khala.delegation_gepa_feedback.v0',
  ),
  candidateManifestSchemaVersion: S.Literal(
    ProbeGepaCandidateManifestSchemaVersion,
  ),
  maxMetricCalls: S.Number,
  ownerApprovalRef: S.String,
  demandKind: S.Literal(MutaliskKhalaDelegationDemandKind),
  demandSource: S.Literal(MutaliskKhalaDelegationDemandSource),
  publicSafetyPolicyRef: S.String,
}) {}

export class MutaliskKhalaDelegationSummary extends S.Class<MutaliskKhalaDelegationSummary>(
  'MutaliskKhalaDelegationSummary',
)({
  schemaVersion: S.Literal(MutaliskKhalaDelegationSummarySchemaVersion),
  runRef: S.String,
  jobRef: S.String,
  candidateManifestRef: S.String,
  candidateRef: S.String,
  signature: S.Literal(KhalaFleetDelegationCandidateSignature),
  baseModuleRef: S.String,
  optimizedModuleRef: S.String,
  metricName: S.Literal(KhalaFleetDelegationCandidateSignature),
  metricValueBps: S.Number,
  evalEvidenceRefs: S.Array(S.String),
  traceProvenanceRefs: S.Array(S.String),
  optimizerRunRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  publicSafetyChecks: S.Array(S.String),
}) {}

export class MutaliskKhalaDelegationRunProgress extends S.Class<MutaliskKhalaDelegationRunProgress>(
  'MutaliskKhalaDelegationRunProgress',
)({
  schemaVersion: S.Literal(GymRunProgressSchemaVersion),
  runRef: S.String,
  jobRef: S.String,
  environmentId: S.Literal(KhalaCodeDelegationGepaEnvironmentId),
  runner: S.Literal('mutalisk'),
  stage: MutaliskKhalaDelegationStage,
  decisionGrade: S.Literal(false),
  inProgress: S.Boolean,
  demandKind: S.Literal(MutaliskKhalaDelegationDemandKind),
  demandSource: S.Literal(MutaliskKhalaDelegationDemandSource),
  candidateManifestRef: S.optionalKey(S.String),
  candidateRef: S.optionalKey(S.String),
  metricValueBps: S.optionalKey(S.Number),
  admissionDecision: S.optionalKey(S.Literals(['blocked', 'gated_proposal_ready'])),
  actionSubmissionProposalRef: S.optionalKey(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  updatedAt: S.String,
}) {}

export class MutaliskKhalaDelegationBridgeOutput extends S.Class<MutaliskKhalaDelegationBridgeOutput>(
  'MutaliskKhalaDelegationBridgeOutput',
)({
  schemaVersion: S.Literal(MutaliskKhalaDelegationBridgeOutputSchemaVersion),
  job: MutaliskKhalaDelegationJob,
  summary: MutaliskKhalaDelegationSummary,
  progress: S.Array(MutaliskKhalaDelegationRunProgress),
  admission: KhalaFleetDelegationCandidateAdmissionProjection,
  candidateManifestRef: S.String,
  candidateRef: S.String,
  metricValueBps: S.Number,
  admissionDecision: S.Literals(['blocked', 'gated_proposal_ready']),
  actionSubmissionProposalRef: S.NullOr(S.String),
  blockerRefs: S.Array(S.String),
  decisionGrade: S.Literal(false),
}) {}

export class MutaliskKhalaDelegationGymBridgeUnsafe extends S.TaggedErrorClass<MutaliskKhalaDelegationGymBridgeUnsafe>()(
  'MutaliskKhalaDelegationGymBridgeUnsafe',
  {
    reason: S.String,
  },
) {}

export type MutaliskKhalaDelegationGymStoreSnapshot = Readonly<{
  jobs: ReadonlyArray<MutaliskKhalaDelegationJob>
  summaries: ReadonlyArray<MutaliskKhalaDelegationSummary>
  progress: ReadonlyArray<MutaliskKhalaDelegationRunProgress>
}>

export type MutaliskKhalaDelegationGymStore = Readonly<{
  saveJob: (job: MutaliskKhalaDelegationJob) => void
  saveSummary: (summary: MutaliskKhalaDelegationSummary) => void
  saveProgress: (progress: MutaliskKhalaDelegationRunProgress) => void
  snapshot: () => MutaliskKhalaDelegationGymStoreSnapshot
}>

export type RunMutaliskKhalaDelegationBridgeOptions = Readonly<{
  actorRef?: string
  artifactRefs?: ReadonlyArray<string>
  maxMetricCalls?: number
  observedAt?: string
  optimizerRunRefs?: ReadonlyArray<string>
  store?: MutaliskKhalaDelegationGymStore
}>

const unsafePublicProjectionValue =
  /\/Users\/|\/home\/|auth\.json|bearer |authorization:|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|http:\/\/|https:\/\/|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof)|preimage|private[_-]?(endpoint|repo|source)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|email|fixture|log|payload|prompt|provider|runner|source|trace|traces)|secret|sk-[a-z0-9]|token|wallet/i
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const publicRefSegment = (value: string, fallback: string): string => {
  const segment = value
    .trim()
    .replace(/[^A-Za-z0-9_.-]/g, '_')
    .replace(/_+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 80)
  return segment === '' ? fallback : segment
}

const collectStringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }
  if (Array.isArray(value)) {
    return value.flatMap(item => collectStringValues(item))
  }
  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(item => collectStringValues(item))
  }
  return []
}

const assertPublicSafeValues = (label: string, value: unknown): void => {
  const unsafe = collectStringValues(value).find(item =>
    unsafePublicProjectionValue.test(item),
  )
  if (unsafe !== undefined) {
    throw new MutaliskKhalaDelegationGymBridgeUnsafe({
      reason: `${label} contains private or raw optimizer material: ${unsafe}`,
    })
  }
}

const assertPublicSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref => !safeRefPattern.test(ref) || unsafePublicProjectionValue.test(ref),
  )
  if (unsafe !== undefined) {
    throw new MutaliskKhalaDelegationGymBridgeUnsafe({
      reason: `${label} contains an unsafe ref: ${unsafe}`,
    })
  }
}

export const createInMemoryMutaliskKhalaDelegationGymStore =
  (): MutaliskKhalaDelegationGymStore => {
    const jobs = new Map<string, MutaliskKhalaDelegationJob>()
    const summaries = new Map<string, MutaliskKhalaDelegationSummary>()
    const progress = new Map<string, MutaliskKhalaDelegationRunProgress>()
    return {
      saveJob: job => {
        jobs.set(job.jobRef, job)
      },
      saveSummary: summary => {
        summaries.set(summary.candidateManifestRef, summary)
      },
      saveProgress: snapshot => {
        progress.set(`${snapshot.runRef}:${snapshot.stage}`, snapshot)
      },
      snapshot: () => ({
        jobs: [...jobs.values()],
        summaries: [...summaries.values()],
        progress: [...progress.values()],
      }),
    }
  }

const defaultArtifactRefs = (
  candidate: KhalaFleetDelegationCandidateManifestSummary,
): ReadonlyArray<string> => [
  `artifact.mutalisk.khala_fleet_delegation.${publicRefSegment(
    candidate.candidateManifestRef,
    'candidate_manifest',
  )}`,
]

const defaultOptimizerRunRefs = (
  candidate: KhalaFleetDelegationCandidateManifestSummary,
): ReadonlyArray<string> => [
  `optimizer_run.mutalisk.khala_fleet_delegation.${publicRefSegment(
    candidate.candidateRef,
    'candidate',
  )}`,
]

const buildJob = (
  candidate: KhalaFleetDelegationCandidateManifestSummary,
  maxMetricCalls: number,
): MutaliskKhalaDelegationJob => {
  const candidateSegment = publicRefSegment(candidate.candidateRef, 'candidate')
  return new MutaliskKhalaDelegationJob({
    schemaVersion: MutaliskKhalaDelegationJobSchemaVersion,
    runRef: `gym.run.khala_code_delegation_gepa.${candidateSegment}`,
    jobRef: `gym.job.mutalisk_khala_delegation.${candidateSegment}`,
    environmentId: KhalaCodeDelegationGepaEnvironmentId,
    signature: KhalaFleetDelegationCandidateSignature,
    baseModuleRef: candidate.baseModuleRef,
    seedCandidateRef: 'candidate.khala_fleet_delegation.seed.v1',
    datasetRef: 'eval.mutalisk.fixtures.khala_fleet_delegation_demo.v1',
    trainSplitRefs: ['eval_split.khala_fleet_delegation_demo.train.v1'],
    validationSplitRefs: ['eval_split.khala_fleet_delegation_demo.val.v1'],
    feedbackSchemaRef: 'openagents.khala.delegation_gepa_feedback.v0',
    candidateManifestSchemaVersion: ProbeGepaCandidateManifestSchemaVersion,
    maxMetricCalls,
    ownerApprovalRef: 'approval.owner.khala_delegation.operator_review.v1',
    demandKind: MutaliskKhalaDelegationDemandKind,
    demandSource: MutaliskKhalaDelegationDemandSource,
    publicSafetyPolicyRef:
      'policy.public_safe.mutalisk_khala_delegation_summary.v0',
  })
}

const buildSummary = (
  candidate: KhalaFleetDelegationCandidateManifestSummary,
  job: MutaliskKhalaDelegationJob,
  artifactRefs: ReadonlyArray<string>,
  optimizerRunRefs: ReadonlyArray<string>,
): MutaliskKhalaDelegationSummary =>
  new MutaliskKhalaDelegationSummary({
    schemaVersion: MutaliskKhalaDelegationSummarySchemaVersion,
    runRef: job.runRef,
    jobRef: job.jobRef,
    candidateManifestRef: candidate.candidateManifestRef,
    candidateRef: candidate.candidateRef,
    signature: candidate.signature,
    baseModuleRef: candidate.baseModuleRef,
    optimizedModuleRef: candidate.optimizedModuleRef,
    metricName: candidate.metricName,
    metricValueBps: candidate.metricValueBps,
    evalEvidenceRefs: uniqueRefs(candidate.evalEvidenceRefs),
    traceProvenanceRefs: uniqueRefs(candidate.traceProvenanceRefs),
    optimizerRunRefs: uniqueRefs(optimizerRunRefs),
    blockerRefs: [],
    artifactRefs: uniqueRefs(artifactRefs),
    publicSafetyChecks: [
      'check.public_projection.prompt_bodies_excluded',
      'check.public_projection.trace_bodies_excluded',
      'check.public_safe.no_local_paths',
      'check.public_safe.no_bearer_material',
      'check.public_safe.no_optimizer_scratch_logs',
    ],
  })

const buildStandingLoopInput = (
  summary: MutaliskKhalaDelegationSummary,
  job: MutaliskKhalaDelegationJob,
): ProbeGepaStandingOptimizationLoopInput =>
  new ProbeGepaStandingOptimizationLoopInput({
    candidateArtifactRefs: summary.artifactRefs,
    candidateManifestRefs: [summary.candidateManifestRef],
    dspyRlmAuditRefs: ['audit.gd1.khala_delegation.feedback_dimensions.v1'],
    effectAuthorityGateRefs: [
      'effect_authority_gate.blueprint.khala_delegation.v1',
    ],
    evalResultRefs: summary.evalEvidenceRefs,
    failureFamilyRefs: [
      'failure_family.khala_delegation.no_available_codex_capacity',
    ],
    issueRefs: ['github.issue.openagents.7730', 'github.issue.openagents.7754'],
    loopRef: `loop.khala_fleet_delegation.${publicRefSegment(
      summary.candidateRef,
      'candidate',
    )}`,
    lowQualityTurnRefs: [],
    metricCallCount: job.maxMetricCalls,
    mutaliskLaneRefs: ['lane.mutalisk.gepa_delegation.offline.v1'],
    optimizerRunRefs: summary.optimizerRunRefs,
    releaseGateRefs: ['release_gate.khala_fleet_delegation.operator.v1'],
    requestedAction: 'emit_candidates',
    sourceTraceRefs: summary.traceProvenanceRefs,
  })

const defaultBlueprintSelection = (): KhalaFleetDelegationBlueprintSelection =>
  new KhalaFleetDelegationBlueprintSelection({
    actionSubmissionRequiredForDirectEffects: true,
    candidateEntryIds: ['blueprint.entry.khala_fleet_delegation.v1'],
    directMutationAllowed: false,
    evidenceRequirementRefs: ['evidence_requirement.khala_delegation.eval_refs'],
    lookupId: 'lookup.khala_fleet_delegation.gym_bridge.v1',
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
  })

const buildProgress = (
  stage: MutaliskKhalaDelegationStage,
  job: MutaliskKhalaDelegationJob,
  observedAt: string,
  summary?: MutaliskKhalaDelegationSummary,
  admission?: KhalaFleetDelegationCandidateAdmissionProjection,
): MutaliskKhalaDelegationRunProgress =>
  new MutaliskKhalaDelegationRunProgress({
    schemaVersion: GymRunProgressSchemaVersion,
    runRef: job.runRef,
    jobRef: job.jobRef,
    environmentId: KhalaCodeDelegationGepaEnvironmentId,
    runner: 'mutalisk',
    stage,
    decisionGrade: false,
    inProgress: stage !== 'completed' && stage !== 'blocked',
    demandKind: MutaliskKhalaDelegationDemandKind,
    demandSource: MutaliskKhalaDelegationDemandSource,
    ...(summary === undefined
      ? {}
      : {
          candidateManifestRef: summary.candidateManifestRef,
          candidateRef: summary.candidateRef,
          metricValueBps: summary.metricValueBps,
        }),
    ...(admission === undefined
      ? {}
      : {
          admissionDecision: admission.decision,
          ...(admission.actionSubmissionProposalRefs[0] === undefined
            ? {}
            : {
                actionSubmissionProposalRef:
                  admission.actionSubmissionProposalRefs[0],
              }),
        }),
    blockerRefs: uniqueRefs([
      ...(summary?.blockerRefs ?? []),
      ...(admission?.blockerRefs ?? []),
    ]),
    caveatRefs: [
      'caveat.gym.khala_delegation_gepa.no_live_promotion',
      'caveat.gym.khala_delegation_gepa.decision_grade_false_until_live_evidence',
    ],
    updatedAt: observedAt,
  })

export const runMutaliskKhalaDelegationNoUiBridge = (
  rawManifestSummary: unknown,
  options: RunMutaliskKhalaDelegationBridgeOptions = {},
): MutaliskKhalaDelegationBridgeOutput => {
  const candidate = S.decodeUnknownSync(KhalaFleetDelegationCandidateManifestSummary)(
    rawManifestSummary,
  )
  const maxMetricCalls = options.maxMetricCalls ?? 8
  const observedAt = options.observedAt ?? 'time.gym_bridge.operator_supplied'
  const store =
    options.store ?? createInMemoryMutaliskKhalaDelegationGymStore()
  const artifactRefs = uniqueRefs(
    options.artifactRefs ?? defaultArtifactRefs(candidate),
  )
  const optimizerRunRefs = uniqueRefs(
    options.optimizerRunRefs ?? defaultOptimizerRunRefs(candidate),
  )
  const job = buildJob(candidate, maxMetricCalls)
  const summary = buildSummary(candidate, job, artifactRefs, optimizerRunRefs)

  assertPublicSafeValues('Mutalisk Khala delegation Gym job', job)
  assertPublicSafeValues('Mutalisk Khala delegation summary', summary)
  assertPublicSafeRefs('Mutalisk Khala delegation artifact refs', artifactRefs)
  assertPublicSafeRefs(
    'Mutalisk Khala delegation optimizer run refs',
    optimizerRunRefs,
  )

  const standingLoop = buildStandingLoopInput(summary, job)
  const admission = projectKhalaFleetDelegationCandidateAdmission(
    new KhalaFleetDelegationCandidateAdmissionInput({
      actorRef: options.actorRef ?? 'actor.operator.openagents',
      approvalPolicyRef:
        'policy.khala_delegation.operator_approval_required.v1',
      blueprintSelection: defaultBlueprintSelection(),
      candidate,
      contextPackRefs: ['context_pack.khala_delegation.gd3.gym_bridge.v1'],
      observedAt,
      programRunRef: `program_run.khala_delegation.candidate_admission.${publicRefSegment(
        candidate.candidateRef,
        'candidate',
      )}`,
      standingLoop,
      summaryRef: `summary.khala_delegation.gym.${publicRefSegment(
        candidate.candidateManifestRef,
        'candidate_manifest',
      )}`,
    }),
  )
  const finalStage: MutaliskKhalaDelegationStage =
    admission.decision === 'gated_proposal_ready' ? 'completed' : 'blocked'
  const progress = [
    buildProgress('queued', job, observedAt),
    buildProgress('running', job, observedAt),
    buildProgress('summary_ingested', job, observedAt, summary),
    buildProgress('admission_projected', job, observedAt, summary, admission),
    buildProgress(finalStage, job, observedAt, summary, admission),
  ]

  store.saveJob(job)
  store.saveSummary(summary)
  progress.forEach(snapshot => {
    store.saveProgress(snapshot)
  })

  const actionSubmissionProposalRef =
    admission.actionSubmissionProposalRefs[0] ?? null
  const output = new MutaliskKhalaDelegationBridgeOutput({
    schemaVersion: MutaliskKhalaDelegationBridgeOutputSchemaVersion,
    job,
    summary,
    progress,
    admission,
    candidateManifestRef: summary.candidateManifestRef,
    candidateRef: summary.candidateRef,
    metricValueBps: summary.metricValueBps,
    admissionDecision: admission.decision,
    actionSubmissionProposalRef,
    blockerRefs: admission.blockerRefs,
    decisionGrade: false,
  })
  assertPublicSafeValues('Mutalisk Khala delegation bridge output', output)
  return output
}

import {
  buildKhalaGymGraphProjection,
  type KhalaGymBridgeProgressLike,
  type KhalaGymBridgeProofLike,
  type KhalaGymGraphDatum,
  type KhalaGymGraphNode,
  type KhalaGymGraphProjection,
} from "./gym-graph-projection"
import type {
  GymPaneActiveParameters,
  GymPaneDetail,
  GymPaneState,
} from "./gym-pane"

export type KhalaGymProofLoadRequest = Readonly<{
  proof?: KhalaGymBridgeProofLike | null
  generatedAt?: string
  sourceRef?: string
}>

export type KhalaGymDelegationOptimizationPhase =
  | "blocked"
  | "proposal_ready"
  | "queued"
  | "running"

export type KhalaGymDelegationOptimizationRun = Readonly<{
  actionSubmissionProposalRef?: string
  activeParameters: GymPaneActiveParameters
  admissionDecision?: "blocked" | "gated_proposal_ready"
  blockerRefs: ReadonlyArray<string>
  candidateManifestRef?: string
  candidateRef?: string
  datasetRefs: ReadonlyArray<string>
  metricValueBps?: number
  phase: KhalaGymDelegationOptimizationPhase
  runRef: string
  stage: string
}>

export type KhalaGymDelegationRunProjectionLike = Readonly<{
  actionSubmissionProposalRef?: string
  admissionDecision?: "blocked" | "gated_proposal_ready"
  baseModuleRef?: string
  blockerRefs?: ReadonlyArray<string>
  candidateManifestRef?: string
  candidateRef?: string
  caveatRefs?: ReadonlyArray<string>
  datasetRef?: string
  feedbackSchemaRef?: string
  jobRef?: string
  latestStage?: string
  maxMetricCalls?: number
  metricValueBps?: number
  ownerApprovalRef?: string
  progress?: ReadonlyArray<KhalaGymBridgeProgressLike>
  publicSafetyPolicyRef?: string
  runRef?: string
  seedCandidateRef?: string
  trainSplitRefs?: ReadonlyArray<string>
  validationSplitRefs?: ReadonlyArray<string>
}>

export const defaultKhalaFleetDelegationActiveParameters: GymPaneActiveParameters = {
  blockerRefs: [],
  caveatRefs: [
    "caveat.khala_fleet_delegation.active_parameters.default_until_owner_admission",
  ],
  parameterRef: "parameters.khala_fleet_delegation.default.v1",
  schemaVersion: "openagents.khala.fleet_delegation.parameters.v0",
  source: "default",
}

export const khalaCodeGymDemoBridgeProof = {
  schemaVersion: "openagents.gym.mutalisk_khala_delegation_bridge_output.v0",
  job: {
    runRef: "gym.run.khala_code_delegation_gepa.part2_fixture",
    jobRef: "gym.job.mutalisk_khala_delegation.part2_fixture",
    datasetRef: "eval.mutalisk.fixtures.khala_fleet_delegation_demo.v1",
    trainSplitRefs: ["eval_split.khala_fleet_delegation_demo.train.v1"],
    validationSplitRefs: ["eval_split.khala_fleet_delegation_demo.val.v1"],
    feedbackSchemaRef: "openagents.khala.delegation_gepa_feedback.v0",
    ownerApprovalRef: "approval.owner.khala_delegation.operator_review.v1",
    publicSafetyPolicyRef:
      "policy.public_safe.mutalisk_khala_delegation_summary.v0",
  },
  summary: {
    candidateManifestRef: "manifest.khala_fleet_delegation.part2_fixture.v1",
    candidateRef: "candidate.khala_fleet_delegation.part2_fixture.v1",
    baseModuleRef: "module.khala_fleet_delegation.base.v1",
    optimizedModuleRef:
      "module.khala_fleet_delegation.optimized.part2_fixture.v1",
    metricValueBps: 10000,
    evalEvidenceRefs: ["eval_result.khala_delegation.gd1.part2_fixture.v1"],
    traceProvenanceRefs: [
      "trace_provenance.khala_delegation.closeout.part2_fixture.v1",
    ],
    optimizerRunRefs: [
      "optimizer_run.mutalisk.khala_fleet_delegation.part2_fixture.v1",
    ],
    artifactRefs: [
      "artifact.mutalisk.khala_fleet_delegation.part2_fixture.v1",
    ],
    blockerRefs: [],
    publicSafetyChecks: [
      "check.public_projection.prompt_bodies_excluded",
      "check.public_safe.no_optimizer_scratch_logs",
    ],
  },
  progress: [
    {
      runRef: "gym.run.khala_code_delegation_gepa.part2_fixture",
      jobRef: "gym.job.mutalisk_khala_delegation.part2_fixture",
      stage: "queued",
      blockerRefs: [],
      caveatRefs: ["caveat.gym.khala_delegation_gepa.no_live_promotion"],
    },
    {
      runRef: "gym.run.khala_code_delegation_gepa.part2_fixture",
      jobRef: "gym.job.mutalisk_khala_delegation.part2_fixture",
      stage: "completed",
      candidateManifestRef: "manifest.khala_fleet_delegation.part2_fixture.v1",
      candidateRef: "candidate.khala_fleet_delegation.part2_fixture.v1",
      metricValueBps: 10000,
      admissionDecision: "gated_proposal_ready",
      actionSubmissionProposalRef:
        "action_submission.proposal.khala_delegation.part2_fixture.v1",
      blockerRefs: [],
      caveatRefs: [
        "caveat.gym.khala_delegation_gepa.decision_grade_false_until_live_evidence",
      ],
    },
  ],
  admission: {
    decision: "gated_proposal_ready",
    actionSubmissionProposalRefs: [
      "action_submission.proposal.khala_delegation.part2_fixture.v1",
    ],
    blockerRefs: [],
    candidateManifestRef: "manifest.khala_fleet_delegation.part2_fixture.v1",
    candidateRef: "candidate.khala_fleet_delegation.part2_fixture.v1",
    standingLoop: {
      issueRefs: ["github.issue.openagents.7760"],
      evalResultRefs: ["eval_result.khala_delegation.gd1.part2_fixture.v1"],
      optimizerRunRefs: [
        "optimizer_run.mutalisk.khala_fleet_delegation.part2_fixture.v1",
      ],
      releaseGateRefs: ["release_gate.khala_fleet_delegation.operator.v1"],
      effectAuthorityGateRefs: [
        "effect_authority_gate.blueprint.khala_delegation.v1",
      ],
      mutaliskLaneRefs: ["lane.mutalisk.gepa_delegation.offline.v1"],
    },
  },
  candidateManifestRef: "manifest.khala_fleet_delegation.part2_fixture.v1",
  candidateRef: "candidate.khala_fleet_delegation.part2_fixture.v1",
  metricValueBps: 10000,
  admissionDecision: "gated_proposal_ready",
  actionSubmissionProposalRef:
    "action_submission.proposal.khala_delegation.part2_fixture.v1",
  blockerRefs: [],
  decisionGrade: false,
} satisfies KhalaGymBridgeProofLike

const unique = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.filter(ref => ref.trim() !== ""))].sort()

const lastProgress = (
  proof: KhalaGymBridgeProofLike,
) => proof.progress?.at(-1)

const firstDefined = <T>(
  values: ReadonlyArray<T | null | undefined>,
): T | undefined =>
  values.find((value): value is T => value !== undefined && value !== null)

const blockerRefsForProof = (
  proof: KhalaGymBridgeProofLike,
): ReadonlyArray<string> =>
  unique([
    ...(proof.blockerRefs ?? []),
    ...(proof.summary?.blockerRefs ?? []),
    ...(proof.admission?.blockerRefs ?? []),
    ...(lastProgress(proof)?.blockerRefs ?? []),
  ])

const actionSubmissionProposalRefForProof = (
  proof: KhalaGymBridgeProofLike,
): string | undefined =>
  firstDefined([
    proof.actionSubmissionProposalRef ?? undefined,
    proof.admission?.actionSubmissionProposalRefs?.[0],
    lastProgress(proof)?.actionSubmissionProposalRef,
  ])

const candidateManifestRefForProof = (
  proof: KhalaGymBridgeProofLike,
): string | undefined =>
  firstDefined([
    proof.candidateManifestRef,
    proof.summary?.candidateManifestRef,
    proof.admission?.candidateManifestRef,
    lastProgress(proof)?.candidateManifestRef,
  ])

const candidateRefForProof = (
  proof: KhalaGymBridgeProofLike,
): string | undefined =>
  firstDefined([
    proof.candidateRef,
    proof.summary?.candidateRef,
    proof.admission?.candidateRef,
    lastProgress(proof)?.candidateRef,
  ])

export const activeParametersFromBridgeProof = (
  proof: KhalaGymBridgeProofLike | null | undefined,
): GymPaneActiveParameters => {
  if (proof === undefined || proof === null) {
    return defaultKhalaFleetDelegationActiveParameters
  }
  const blockerRefs = blockerRefsForProof(proof)
  const actionSubmissionProposalRef = actionSubmissionProposalRefForProof(proof)
  const candidateManifestRef = candidateManifestRefForProof(proof)
  const candidateRef = candidateRefForProof(proof)
  return {
    ...defaultKhalaFleetDelegationActiveParameters,
    blockerRefs,
    caveatRefs: unique([
      ...defaultKhalaFleetDelegationActiveParameters.caveatRefs,
      ...(lastProgress(proof)?.caveatRefs ?? []),
    ]),
    ...(actionSubmissionProposalRef === undefined
      ? {}
      : { actionSubmissionProposalRef }),
    ...(candidateManifestRef === undefined ? {} : { candidateManifestRef }),
    ...(candidateRef === undefined ? {} : { candidateRef }),
  }
}

const graphNode = (
  graph: KhalaGymGraphProjection,
  id: string,
): KhalaGymGraphNode | undefined => graph.nodes.find(node => node.id === id)

const datumByLabel = (
  node: KhalaGymGraphNode | undefined,
  label: string,
): KhalaGymGraphDatum | undefined =>
  node?.datum.find(item => item.label === label)

const datumValue = (
  node: KhalaGymGraphNode | undefined,
  label: string,
): string | undefined => {
  const item = datumByLabel(node, label)
  if (item === undefined) return undefined
  const value = String(item.value)
  return item.unit === undefined ? value : `${value} ${item.unit}`
}

const detail = (
  label: string,
  value: string | number | boolean | undefined,
): ReadonlyArray<GymPaneDetail> =>
  value === undefined ? [] : [{ label, value: String(value) }]

const detailRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<GymPaneDetail> =>
  refs.length === 0 ? [] : [{ label, value: refs.join(" ") }]

const proofDetails = (
  proof: KhalaGymBridgeProofLike,
  graph: KhalaGymGraphProjection,
): ReadonlyArray<GymPaneDetail> => {
  const candidate = graphNode(graph, "candidate-manifest")
  const admission = graphNode(graph, "admission")
  const proposal = graphNode(graph, "action-submission")
  return [
    ...detail("metricValueBps", datumValue(candidate, "metric")),
    ...detail("admissionDecision", datumValue(admission, "decision")),
    ...detail("decisionGrade", proof.decisionGrade),
    ...detailRefs("candidate refs", candidate?.evidenceRefs ?? []),
    ...detailRefs("blocker refs", graph.blockerRefs),
    ...detailRefs("Action Submission proposal refs", proposal?.evidenceRefs ?? []),
  ]
}

const refsForGraph = (graph: KhalaGymGraphProjection): ReadonlyArray<string> =>
  unique([
    ...graph.sourceRefs,
    ...graph.evidenceRefs,
    ...graph.blockerRefs,
    ...graph.caveatRefs,
  ]).slice(0, 16)

const runRefForProof = (proof: KhalaGymBridgeProofLike): string =>
  proof.job?.runRef ?? lastProgress(proof)?.runRef ?? "run ref unavailable"

const datasetRefsForProof = (
  proof: KhalaGymBridgeProofLike,
): ReadonlyArray<string> =>
  unique([
    ...(proof.job?.datasetRef === undefined ? [] : [proof.job.datasetRef]),
    ...(proof.job?.trainSplitRefs ?? []),
    ...(proof.job?.validationSplitRefs ?? []),
  ])

const phaseForProof = (
  proof: KhalaGymBridgeProofLike,
): KhalaGymDelegationOptimizationPhase => {
  const stage = lastProgress(proof)?.stage ?? "queued"
  if (blockerRefsForProof(proof).length > 0 || proof.admissionDecision === "blocked") {
    return "blocked"
  }
  if (actionSubmissionProposalRefForProof(proof) !== undefined) {
    return "proposal_ready"
  }
  return stage === "queued" ? "queued" : "running"
}

export const gymOptimizationRunFromBridgeProof = (
  proof: KhalaGymBridgeProofLike,
): KhalaGymDelegationOptimizationRun => {
  const latest = lastProgress(proof)
  const actionSubmissionProposalRef = actionSubmissionProposalRefForProof(proof)
  const candidateManifestRef = candidateManifestRefForProof(proof)
  const candidateRef = candidateRefForProof(proof)
  const metricValueBps = proof.metricValueBps ?? latest?.metricValueBps
  return {
    activeParameters: activeParametersFromBridgeProof(proof),
    blockerRefs: blockerRefsForProof(proof),
    datasetRefs: datasetRefsForProof(proof),
    phase: phaseForProof(proof),
    runRef: runRefForProof(proof),
    stage: latest?.stage ?? "queued",
    ...(actionSubmissionProposalRef === undefined
      ? {}
      : { actionSubmissionProposalRef }),
    ...(proof.admissionDecision === undefined
      ? latest?.admissionDecision === undefined
        ? {}
        : { admissionDecision: latest.admissionDecision }
      : { admissionDecision: proof.admissionDecision }),
    ...(candidateManifestRef === undefined ? {} : { candidateManifestRef }),
    ...(candidateRef === undefined ? {} : { candidateRef }),
    ...(metricValueBps === undefined ? {} : { metricValueBps }),
  }
}

export const bridgeProofFromOptimizationProjection = (
  run: KhalaGymDelegationRunProjectionLike,
): KhalaGymBridgeProofLike => {
  const candidateManifestRef = firstDefined([
    run.candidateManifestRef,
    run.progress?.at(-1)?.candidateManifestRef,
  ])
  const candidateRef = firstDefined([
    run.candidateRef,
    run.progress?.at(-1)?.candidateRef,
  ])
  const metricValueBps = firstDefined([
    run.metricValueBps,
    run.progress?.at(-1)?.metricValueBps,
  ])
  const actionSubmissionProposalRef = firstDefined([
    run.actionSubmissionProposalRef,
    run.progress?.at(-1)?.actionSubmissionProposalRef,
  ])
  const admissionDecision = firstDefined([
    run.admissionDecision,
    run.progress?.at(-1)?.admissionDecision,
  ])
  const job: NonNullable<KhalaGymBridgeProofLike["job"]> = {
    ...(run.datasetRef === undefined ? {} : { datasetRef: run.datasetRef }),
    ...(run.feedbackSchemaRef === undefined
      ? {}
      : { feedbackSchemaRef: run.feedbackSchemaRef }),
    ...(run.jobRef === undefined ? {} : { jobRef: run.jobRef }),
    ...(run.ownerApprovalRef === undefined
      ? {}
      : { ownerApprovalRef: run.ownerApprovalRef }),
    ...(run.publicSafetyPolicyRef === undefined
      ? {}
      : { publicSafetyPolicyRef: run.publicSafetyPolicyRef }),
    ...(run.runRef === undefined ? {} : { runRef: run.runRef }),
    ...(run.trainSplitRefs === undefined
      ? {}
      : { trainSplitRefs: run.trainSplitRefs }),
    ...(run.validationSplitRefs === undefined
      ? {}
      : { validationSplitRefs: run.validationSplitRefs }),
  }
  const summary: NonNullable<KhalaGymBridgeProofLike["summary"]> | undefined =
    candidateManifestRef === undefined && candidateRef === undefined
      ? undefined
      : {
          ...(run.baseModuleRef === undefined
            ? {}
            : { baseModuleRef: run.baseModuleRef }),
          blockerRefs: run.blockerRefs ?? [],
          ...(candidateManifestRef === undefined
            ? {}
            : { candidateManifestRef }),
          ...(candidateRef === undefined ? {} : { candidateRef }),
          ...(metricValueBps === undefined ? {} : { metricValueBps }),
        }
  const progress: KhalaGymBridgeProofLike["progress"] = run.progress ?? [
    {
      blockerRefs: run.blockerRefs ?? [],
      caveatRefs: run.caveatRefs ?? [],
      ...(run.jobRef === undefined ? {} : { jobRef: run.jobRef }),
      ...(run.runRef === undefined ? {} : { runRef: run.runRef }),
      stage: run.latestStage ?? "queued",
      ...(actionSubmissionProposalRef === undefined
        ? {}
        : { actionSubmissionProposalRef }),
      ...(admissionDecision === undefined ? {} : { admissionDecision }),
      ...(candidateManifestRef === undefined ? {} : { candidateManifestRef }),
      ...(candidateRef === undefined ? {} : { candidateRef }),
      ...(metricValueBps === undefined ? {} : { metricValueBps }),
    },
  ]
  const admission: NonNullable<KhalaGymBridgeProofLike["admission"]> = {
    actionSubmissionProposalRefs:
      actionSubmissionProposalRef === undefined ? [] : [actionSubmissionProposalRef],
    blockerRefs: run.blockerRefs ?? [],
    ...(candidateManifestRef === undefined ? {} : { candidateManifestRef }),
    ...(candidateRef === undefined ? {} : { candidateRef }),
    ...(admissionDecision === undefined ? {} : { decision: admissionDecision }),
  }
  return {
    schemaVersion: "openagents.gym.mutalisk_khala_delegation_bridge_output.v0",
    job,
    ...(summary === undefined ? {} : { summary }),
    progress,
    admission,
    blockerRefs: run.blockerRefs ?? [],
    decisionGrade: false,
    ...(actionSubmissionProposalRef === undefined
      ? { actionSubmissionProposalRef: null }
      : { actionSubmissionProposalRef }),
    ...(admissionDecision === undefined ? {} : { admissionDecision }),
    ...(candidateManifestRef === undefined ? {} : { candidateManifestRef }),
    ...(candidateRef === undefined ? {} : { candidateRef }),
    ...(metricValueBps === undefined ? {} : { metricValueBps }),
  }
}

export const gymOptimizationRunFromProjection = (
  run: KhalaGymDelegationRunProjectionLike,
): KhalaGymDelegationOptimizationRun =>
  gymOptimizationRunFromBridgeProof(bridgeProofFromOptimizationProjection(run))

export const gymPaneStateFromOptimizationRun = (
  run: KhalaGymDelegationRunProjectionLike,
): GymPaneState => gymPaneStateFromBridgeProof(bridgeProofFromOptimizationProjection(run))

const isProofLoadRequest = (
  input: KhalaGymProofLoadRequest | KhalaGymBridgeProofLike,
): input is KhalaGymProofLoadRequest =>
  "proof" in input || "generatedAt" in input || "sourceRef" in input

const requestFromInput = (
  input?: KhalaGymProofLoadRequest | KhalaGymBridgeProofLike | null,
): KhalaGymProofLoadRequest => {
  if (input === undefined || input === null) return { proof: null }
  if (isProofLoadRequest(input)) return input
  return { proof: input }
}

export const gymPaneStateFromBridgeProof = (
  input?: KhalaGymProofLoadRequest | KhalaGymBridgeProofLike | null,
): GymPaneState => {
  const request = requestFromInput(input)
  if (request.proof === undefined || request.proof === null) {
    return {
      activeParameters: defaultKhalaFleetDelegationActiveParameters,
      phase: "empty",
    }
  }

  const graph = buildKhalaGymGraphProjection({
    proof: request.proof,
    generatedAt: request.generatedAt ?? "time.khala_gym_projection.local",
  })
  const details = proofDetails(request.proof, graph)
  const refs = refsForGraph(graph)
  const title = "Mutalisk bridge proof"
  const activeParameters = activeParametersFromBridgeProof(request.proof)

  if (graph.status === "blocked") {
    return {
      phase: "blocked",
      title,
      activeParameters,
      blockerRefs: graph.blockerRefs,
      details,
      graph,
    }
  }

  return {
    phase: "loaded",
    title,
    runRef: runRefForProof(request.proof),
    status: graph.status,
    activeParameters,
    refs,
    details,
    graph,
  }
}

const paramsForLocation = (
  location: Pick<Location, "search" | "hash">,
): URLSearchParams => {
  const params = new URLSearchParams(location.search)
  const hashParams = new URLSearchParams(location.hash.replace(/^#/, ""))
  for (const [key, value] of hashParams) {
    if (!params.has(key)) params.set(key, value)
  }
  return params
}

export const gymPaneStateFromLocation = (
  location: Pick<Location, "search" | "hash">,
): GymPaneState => {
  const params = paramsForLocation(location)
  const proof = params.get("gymProof")
  if (proof === "fixture" || proof === "demo") {
    return gymPaneStateFromBridgeProof({
      proof: khalaCodeGymDemoBridgeProof,
      generatedAt: "time.khala_gym_projection.fixture",
      sourceRef: "fixture.khala_code.gym.part2_demo",
    })
  }
  return {
    activeParameters: defaultKhalaFleetDelegationActiveParameters,
    phase: "empty",
  }
}

export type KhalaCodeDesktopView =
  | "chat"
  | "fleet"
  | "forum"
  | "inbox"
  | "settings"
  | "editor"
  | "home"
  | "review"

const KHALA_CODE_VIEW_VALUES: ReadonlySet<string> = new Set<KhalaCodeDesktopView>([
  "chat",
  "fleet",
  "forum",
  "inbox",
  "settings",
  "editor",
  "home",
  "review",
])

const isKhalaCodeDesktopView = (value: string | null): value is KhalaCodeDesktopView =>
  value !== null && KHALA_CODE_VIEW_VALUES.has(value)

export const initialKhalaCodeViewFromLocation = (
  location: Pick<Location, "search" | "hash">,
): KhalaCodeDesktopView => {
  const params = paramsForLocation(location)
  const view = params.get("view")
  return isKhalaCodeDesktopView(view) ? view : "chat"
}

/**
 * Restores the last active desktop view across an app restart
 * (khala_code.project_home route-persistence gate, #8443): an explicit
 * `?view=` query param always wins (back/forward-compatible with existing
 * deep-link and visual-smoke usage), otherwise the last view persisted to
 * local storage before quit is restored, otherwise "chat".
 */
export const restoredKhalaCodeViewFromLocationAndStorage = (
  location: Pick<Location, "search" | "hash">,
  storedView: string | null,
): KhalaCodeDesktopView => {
  const params = paramsForLocation(location)
  const explicitView = params.get("view")
  if (isKhalaCodeDesktopView(explicitView)) return explicitView
  return isKhalaCodeDesktopView(storedView) ? storedView : "chat"
}

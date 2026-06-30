import {
  buildKhalaGymGraphProjection,
  type KhalaGymBridgeProofLike,
  type KhalaGymGraphDatum,
  type KhalaGymGraphNode,
  type KhalaGymGraphProjection,
} from "./gym-graph-projection"
import type { GymPaneDetail, GymPaneState } from "./gym-pane"

export type KhalaGymProofLoadRequest = Readonly<{
  proof?: KhalaGymBridgeProofLike | null
  generatedAt?: string
  sourceRef?: string
}>

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
  if (request.proof === undefined || request.proof === null) return { phase: "empty" }

  const graph = buildKhalaGymGraphProjection({
    proof: request.proof,
    generatedAt: request.generatedAt ?? "time.khala_gym_projection.local",
  })
  const details = proofDetails(request.proof, graph)
  const refs = refsForGraph(graph)
  const title = "Mutalisk bridge proof"

  if (graph.status === "blocked") {
    return {
      phase: "blocked",
      title,
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
  return { phase: "empty" }
}

export const initialKhalaCodeViewFromLocation = (
  location: Pick<Location, "search" | "hash">,
): "chat" | "fleet" | "gym" => {
  const params = paramsForLocation(location)
  const view = params.get("view")
  if (view === "fleet" || view === "gym" || view === "chat") return view
  const proof = params.get("gymProof")
  return proof === "fixture" || proof === "demo" ? "gym" : "chat"
}

export const KhalaGymGraphProjectionSchemaVersion =
  "openagents.khala_code.gym_graph_projection.v0"

export type KhalaGymGraphNodeStatus =
  | "idle"
  | "active"
  | "blocked"
  | "complete"
  | "proposal_ready"

export type KhalaGymGraphLinkStatus =
  | "inactive"
  | "active"
  | "blocked"
  | "evidence_backed"

export type KhalaGymGraphPinDirection = "input" | "output"

export type KhalaGymGraphPin = Readonly<{
  id: string
  name: string
  direction: KhalaGymGraphPinDirection
  type: string
}>

export type KhalaGymGraphDatum = Readonly<{
  label: string
  value: string | number | boolean
  unit?: string
  evidenceRefs: ReadonlyArray<string>
}>

export type KhalaGymGraphNode = Readonly<{
  id: string
  label: string
  kind: string
  status: KhalaGymGraphNodeStatus
  inputs: ReadonlyArray<KhalaGymGraphPin>
  outputs: ReadonlyArray<KhalaGymGraphPin>
  datum: ReadonlyArray<KhalaGymGraphDatum>
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  position: Readonly<{ x: number; y: number }>
}>

export type KhalaGymGraphPinRef = Readonly<{
  nodeId: string
  pinId: string
}>

export type KhalaGymGraphLink = Readonly<{
  id: string
  label: string
  status: KhalaGymGraphLinkStatus
  from: KhalaGymGraphPinRef
  to: KhalaGymGraphPinRef
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
}>

export type KhalaGymGraphProjection = Readonly<{
  schemaVersion: typeof KhalaGymGraphProjectionSchemaVersion
  title: string
  generatedAt: string
  status: KhalaGymGraphNodeStatus
  nodes: ReadonlyArray<KhalaGymGraphNode>
  links: ReadonlyArray<KhalaGymGraphLink>
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

export type KhalaGymGraphProjectionInput = Readonly<{
  proof: KhalaGymBridgeProofLike
  generatedAt?: string
}>

export type KhalaGymBridgeProgressLike = Readonly<{
  stage?: string
  runRef?: string
  jobRef?: string
  candidateManifestRef?: string
  candidateRef?: string
  metricValueBps?: number
  admissionDecision?: "blocked" | "gated_proposal_ready"
  actionSubmissionProposalRef?: string
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
}>

export type KhalaGymBridgeProofLike = Readonly<{
  schemaVersion?: string
  job?: Readonly<{
    runRef?: string
    jobRef?: string
    datasetRef?: string
    trainSplitRefs?: ReadonlyArray<string>
    validationSplitRefs?: ReadonlyArray<string>
    feedbackSchemaRef?: string
    ownerApprovalRef?: string
    publicSafetyPolicyRef?: string
  }>
  summary?: Readonly<{
    candidateManifestRef?: string
    candidateRef?: string
    baseModuleRef?: string
    optimizedModuleRef?: string
    metricValueBps?: number
    evalEvidenceRefs?: ReadonlyArray<string>
    traceProvenanceRefs?: ReadonlyArray<string>
    optimizerRunRefs?: ReadonlyArray<string>
    blockerRefs?: ReadonlyArray<string>
    artifactRefs?: ReadonlyArray<string>
    publicSafetyChecks?: ReadonlyArray<string>
  }>
  progress?: ReadonlyArray<KhalaGymBridgeProgressLike>
  admission?: Readonly<{
    decision?: "blocked" | "gated_proposal_ready"
    actionSubmissionProposalRefs?: ReadonlyArray<string>
    blockerRefs?: ReadonlyArray<string>
    candidateManifestRef?: string
    candidateRef?: string
    standingLoop?: Readonly<{
      issueRefs?: ReadonlyArray<string>
      evalResultRefs?: ReadonlyArray<string>
      optimizerRunRefs?: ReadonlyArray<string>
      releaseGateRefs?: ReadonlyArray<string>
      effectAuthorityGateRefs?: ReadonlyArray<string>
      mutaliskLaneRefs?: ReadonlyArray<string>
    }>
  }>
  candidateManifestRef?: string
  candidateRef?: string
  metricValueBps?: number
  admissionDecision?: "blocked" | "gated_proposal_ready"
  actionSubmissionProposalRef?: string | null
  blockerRefs?: ReadonlyArray<string>
  decisionGrade?: boolean
}>

export class KhalaGymGraphProjectionUnsafe extends Error {
  override readonly name = "KhalaGymGraphProjectionUnsafe"

  constructor(readonly reason: string) {
    super(reason)
  }
}

const unsafePublicProjectionValue =
  /\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer |authorization:|cookie|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|http:\/\/|https:\/\/|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof)|preimage|private[_-]?(endpoint|repo|source)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|email|fixture|log|payload|prompt|provider|runner|source|trace|traces)|secret|(?:^|[^A-Za-z0-9])sk-[a-z0-9]|scratch[_-]?log|token|wallet/i

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const counterOnlyRefPattern = /(^|[.:/-])counter([.:/-]|$)|=\d+$/
const publicSafetyCheckRefPattern =
  /^check\.(public_projection|public_safe)\.[A-Za-z0-9_.:/-]+$/

const uniqueRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  [
    ...new Set(
      refs
        .flatMap(ref => (ref === undefined || ref === null ? [] : [ref.trim()]))
        .filter(ref => ref !== ""),
    ),
  ].sort()

const collectStringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === "string") return [value]
  if (Array.isArray(value)) return value.flatMap(item => collectStringValues(item))
  if (value !== null && typeof value === "object") {
    return Object.values(value).flatMap(item => collectStringValues(item))
  }
  return []
}

const assertPublicSafeString = (label: string, value: string): void => {
  if (
    !safeRefPattern.test(value) ||
    (!publicSafetyCheckRefPattern.test(value) &&
      unsafePublicProjectionValue.test(value))
  ) {
    throw new KhalaGymGraphProjectionUnsafe(
      `${label} contains private, raw, credential, provider, local-path, endpoint, payment, or optimizer scratch material.`,
    )
  }
}

const assertProjectionPublicSafe = (
  projection: KhalaGymGraphProjection,
): void => {
  const unsafe = collectStringValues(projection).find(value =>
    !publicSafetyCheckRefPattern.test(value) &&
    unsafePublicProjectionValue.test(value),
  )
  if (unsafe !== undefined) {
    throw new KhalaGymGraphProjectionUnsafe(
      `Gym graph projection leaked private material: ${unsafe}`,
    )
  }
}

const publicRefs = (
  label: string,
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs).filter(ref => !counterOnlyRefPattern.test(ref))
  normalized.forEach(ref => assertPublicSafeString(label, ref))
  return normalized
}

export const isDereferenceableKhalaGymGraphRef = (ref: string): boolean => {
  if (counterOnlyRefPattern.test(ref)) return false
  if (
    !safeRefPattern.test(ref) ||
    (!publicSafetyCheckRefPattern.test(ref) &&
      unsafePublicProjectionValue.test(ref))
  ) {
    return false
  }
  return /^[A-Za-z][A-Za-z0-9_-]*(\.[A-Za-z0-9_:/-]+){1,}$/.test(ref)
}

const datum = (
  label: string,
  value: string | number | boolean | null | undefined,
  evidenceRefs: ReadonlyArray<string> = [],
  unit?: string,
): ReadonlyArray<KhalaGymGraphDatum> =>
  value === undefined || value === null
    ? []
    : [
        {
          label,
          value,
          evidenceRefs,
          ...(unit === undefined ? {} : { unit }),
        },
      ]

const pin = (
  direction: KhalaGymGraphPinDirection,
  id: string,
  name: string,
  type: string,
): KhalaGymGraphPin => ({
  direction,
  id,
  name,
  type,
})

const node = (input: {
  id: string
  label: string
  kind: string
  status: KhalaGymGraphNodeStatus
  inputs?: ReadonlyArray<KhalaGymGraphPin>
  outputs?: ReadonlyArray<KhalaGymGraphPin>
  datum?: ReadonlyArray<KhalaGymGraphDatum>
  evidenceRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
  x: number
  y: number
}): KhalaGymGraphNode => ({
  id: input.id,
  label: input.label,
  kind: input.kind,
  status: input.status,
  inputs: input.inputs ?? [],
  outputs: input.outputs ?? [],
  datum: input.datum ?? [],
  evidenceRefs: input.evidenceRefs ?? [],
  blockerRefs: input.blockerRefs ?? [],
  caveatRefs: input.caveatRefs ?? [],
  position: { x: input.x, y: input.y },
})

const linkStatus = (
  evidenceRefs: ReadonlyArray<string>,
  blockerRefs: ReadonlyArray<string>,
): KhalaGymGraphLinkStatus => {
  if (blockerRefs.length > 0) return "blocked"
  if (evidenceRefs.length > 0) return "evidence_backed"
  return "inactive"
}

const link = (input: {
  id: string
  label: string
  fromNodeId: string
  fromPinId: string
  toNodeId: string
  toPinId: string
  evidenceRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
}): KhalaGymGraphLink => {
  const evidenceRefs = input.evidenceRefs ?? []
  const blockerRefs = input.blockerRefs ?? []
  return {
    id: input.id,
    label: input.label,
    status: linkStatus(evidenceRefs, blockerRefs),
    from: { nodeId: input.fromNodeId, pinId: input.fromPinId },
    to: { nodeId: input.toNodeId, pinId: input.toPinId },
    evidenceRefs,
    blockerRefs,
    caveatRefs: input.caveatRefs ?? [],
  }
}

const lastProgress = (
  progress: ReadonlyArray<KhalaGymBridgeProgressLike> | undefined,
): KhalaGymBridgeProgressLike | undefined => progress?.at(-1)

const projectionStatus = (
  proof: KhalaGymBridgeProofLike,
  blockerRefs: ReadonlyArray<string>,
): KhalaGymGraphNodeStatus => {
  const decision = proof.admissionDecision ?? proof.admission?.decision
  if (blockerRefs.length > 0 || decision === "blocked") return "blocked"
  if (proof.actionSubmissionProposalRef !== null && proof.actionSubmissionProposalRef !== undefined) {
    return "proposal_ready"
  }
  if ((proof.admission?.actionSubmissionProposalRefs?.length ?? 0) > 0) {
    return "proposal_ready"
  }
  if (lastProgress(proof.progress)?.stage === "completed") return "complete"
  return "active"
}

const progressRefs = (
  progress: ReadonlyArray<KhalaGymBridgeProgressLike> | undefined,
): ReadonlyArray<string> =>
  publicRefs(
    "Gym progress refs",
    (progress ?? []).flatMap(snapshot => [
      snapshot.runRef,
      snapshot.jobRef,
      snapshot.candidateManifestRef,
      snapshot.candidateRef,
      snapshot.actionSubmissionProposalRef,
      ...(snapshot.blockerRefs ?? []),
      ...(snapshot.caveatRefs ?? []),
    ]),
  )

const activeEvidenceRefs = (
  label: string,
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  publicRefs(label, refs).filter(ref => isDereferenceableKhalaGymGraphRef(ref))

export const buildKhalaGymGraphProjection = (
  input: KhalaGymGraphProjectionInput,
): KhalaGymGraphProjection => {
  const { proof } = input
  const job = proof.job
  const summary = proof.summary
  const admission = proof.admission
  const latestProgress = lastProgress(proof.progress)

  const runRefs = activeEvidenceRefs("Gym run refs", [
    job?.runRef,
    job?.jobRef,
    latestProgress?.runRef,
    latestProgress?.jobRef,
  ])
  const datasetRefs = activeEvidenceRefs("Gym dataset refs", [
    job?.datasetRef,
    ...(job?.trainSplitRefs ?? []),
    ...(job?.validationSplitRefs ?? []),
  ])
  const feedbackRefs = activeEvidenceRefs("Gym feedback refs", [
    job?.feedbackSchemaRef,
    ...(summary?.evalEvidenceRefs ?? []),
    ...(admission?.standingLoop?.evalResultRefs ?? []),
  ])
  const traceRefs = activeEvidenceRefs("Gym trace provenance refs", [
    ...(summary?.traceProvenanceRefs ?? []),
  ])
  const candidateRefs = activeEvidenceRefs("Gym candidate refs", [
    proof.candidateManifestRef,
    proof.candidateRef,
    summary?.candidateManifestRef,
    summary?.candidateRef,
    summary?.baseModuleRef,
    summary?.optimizedModuleRef,
    admission?.candidateManifestRef,
    admission?.candidateRef,
  ])
  const optimizerRefs = activeEvidenceRefs("Gym optimizer refs", [
    ...(summary?.optimizerRunRefs ?? []),
    ...(summary?.artifactRefs ?? []),
    ...(admission?.standingLoop?.optimizerRunRefs ?? []),
    ...(admission?.standingLoop?.mutaliskLaneRefs ?? []),
  ])
  const gateRefs = activeEvidenceRefs("Gym gate refs", [
    job?.ownerApprovalRef,
    job?.publicSafetyPolicyRef,
    ...(summary?.publicSafetyChecks ?? []),
    ...(admission?.standingLoop?.releaseGateRefs ?? []),
    ...(admission?.standingLoop?.effectAuthorityGateRefs ?? []),
  ])
  const proposalRefs = activeEvidenceRefs("Gym proposal refs", [
    proof.actionSubmissionProposalRef ?? undefined,
    latestProgress?.actionSubmissionProposalRef,
    ...(admission?.actionSubmissionProposalRefs ?? []),
  ])
  const issueRefs = activeEvidenceRefs("Gym issue refs", [
    ...(admission?.standingLoop?.issueRefs ?? []),
  ])
  const blockerRefs = publicRefs("Gym blocker refs", [
    ...(proof.blockerRefs ?? []),
    ...(summary?.blockerRefs ?? []),
    ...(admission?.blockerRefs ?? []),
    ...(latestProgress?.blockerRefs ?? []),
  ])
  const caveatRefs = publicRefs("Gym caveat refs", [
    ...(latestProgress?.caveatRefs ?? []),
    ...(proof.progress ?? []).flatMap(snapshot => snapshot.caveatRefs ?? []),
  ])
  const allEvidenceRefs = activeEvidenceRefs("Gym projection evidence refs", [
    ...runRefs,
    ...datasetRefs,
    ...feedbackRefs,
    ...traceRefs,
    ...candidateRefs,
    ...optimizerRefs,
    ...gateRefs,
    ...proposalRefs,
    ...issueRefs,
    ...progressRefs(proof.progress),
  ])
  const status = projectionStatus(proof, blockerRefs)
  const metricValueBps =
    proof.metricValueBps ?? latestProgress?.metricValueBps ?? summary?.metricValueBps
  const admissionDecision =
    proof.admissionDecision ??
    latestProgress?.admissionDecision ??
    admission?.decision

  const nodes: ReadonlyArray<KhalaGymGraphNode> = [
    node({
      id: "khala-code-prompt",
      label: "Khala Code prompt",
      kind: "operator_request",
      status: traceRefs.length > 0 ? "complete" : "idle",
      outputs: [pin("output", "request", "request", "khala.fleet.delegate.request")],
      datum: datum("workflow", "khala.fleet.delegate", traceRefs),
      evidenceRefs: traceRefs,
      x: 40,
      y: 70,
    }),
    node({
      id: "khala-fleet-delegate",
      label: "khala.fleet.delegate",
      kind: "tool_program",
      status: runRefs.length > 0 ? "complete" : "active",
      inputs: [pin("input", "request", "request", "khala.fleet.delegate.request")],
      outputs: [pin("output", "capacity", "capacity", "pylon.capacity.selection")],
      datum: [
        ...datum("run", job?.runRef ?? latestProgress?.runRef, runRefs),
        ...datum("job", job?.jobRef ?? latestProgress?.jobRef, runRefs),
      ],
      evidenceRefs: runRefs,
      x: 255,
      y: 70,
    }),
    node({
      id: "pylon-capacity",
      label: "Pylon",
      kind: "capacity_gate",
      status: runRefs.length > 0 ? "complete" : "idle",
      inputs: [pin("input", "capacity", "capacity", "pylon.capacity.selection")],
      outputs: [pin("output", "assignment", "assignment", "codex.assignment")],
      evidenceRefs: runRefs,
      x: 470,
      y: 70,
    }),
    node({
      id: "codex-assignment",
      label: "Codex assignment",
      kind: "worker_assignment",
      status: traceRefs.length > 0 ? "complete" : "idle",
      inputs: [pin("input", "assignment", "assignment", "codex.assignment")],
      outputs: [pin("output", "closeout", "closeout", "khala.closeout.proof")],
      evidenceRefs: traceRefs,
      x: 685,
      y: 70,
    }),
    node({
      id: "closeout-proof",
      label: "closeout/proof refs",
      kind: "proof_bundle",
      status: traceRefs.length > 0 ? "complete" : "idle",
      inputs: [pin("input", "closeout", "closeout", "khala.closeout.proof")],
      evidenceRefs: traceRefs,
      x: 900,
      y: 70,
    }),
    node({
      id: "gd0-examples",
      label: "GD-0 examples",
      kind: "dataset",
      status: datasetRefs.length > 0 ? "complete" : "idle",
      outputs: [pin("output", "examples", "examples", "gym.dataset.split")],
      evidenceRefs: datasetRefs,
      x: 40,
      y: 280,
    }),
    node({
      id: "gd1-feedback",
      label: "GD-1 feedback",
      kind: "metric",
      status: feedbackRefs.length > 0 ? "complete" : "idle",
      inputs: [pin("input", "examples", "examples", "gym.dataset.split")],
      outputs: [pin("output", "reward", "reward", "khala.delegation.feedback")],
      evidenceRefs: feedbackRefs,
      x: 255,
      y: 280,
    }),
    node({
      id: "mutalisk-optimizer",
      label: "Mutalisk",
      kind: "optimizer",
      status: optimizerRefs.length > 0 ? "complete" : "active",
      inputs: [pin("input", "reward", "reward", "khala.delegation.feedback")],
      outputs: [pin("output", "candidate", "candidate", "khala.fleet.delegation.candidate")],
      evidenceRefs: optimizerRefs,
      x: 470,
      y: 280,
    }),
    node({
      id: "candidate-manifest",
      label: "candidate manifest",
      kind: "candidate_artifact",
      status: candidateRefs.length > 0 ? "complete" : "idle",
      inputs: [pin("input", "candidate", "candidate", "khala.fleet.delegation.candidate")],
      outputs: [pin("output", "summary", "summary", "gym.candidate.summary")],
      datum: datum("metric", metricValueBps, candidateRefs, "bps"),
      evidenceRefs: candidateRefs,
      x: 685,
      y: 280,
    }),
    node({
      id: "gym-ingest",
      label: "Gym ingest",
      kind: "gym_projection",
      status: runRefs.length > 0 ? "complete" : "idle",
      inputs: [pin("input", "summary", "summary", "gym.candidate.summary")],
      outputs: [pin("output", "admission", "admission", "gym.admission.projection")],
      evidenceRefs: [...runRefs, ...candidateRefs],
      caveatRefs,
      x: 900,
      y: 280,
    }),
    node({
      id: "admission",
      label: "admission",
      kind: "authority_gate",
      status: status === "blocked" ? "blocked" : "complete",
      inputs: [pin("input", "admission", "admission", "gym.admission.projection")],
      outputs: [pin("output", "proposal", "proposal", "blueprint.action_submission")],
      datum: datum("decision", admissionDecision, gateRefs),
      evidenceRefs: gateRefs,
      blockerRefs,
      caveatRefs,
      x: 1115,
      y: 280,
    }),
    node({
      id: "action-submission",
      label: "Action Submission",
      kind: "proposal",
      status: proposalRefs.length > 0 ? "proposal_ready" : status,
      inputs: [pin("input", "proposal", "proposal", "blueprint.action_submission")],
      evidenceRefs: proposalRefs,
      blockerRefs,
      caveatRefs,
      x: 1330,
      y: 280,
    }),
  ]

  const links: ReadonlyArray<KhalaGymGraphLink> = [
    link({
      id: "prompt-to-delegate",
      label: "request",
      fromNodeId: "khala-code-prompt",
      fromPinId: "request",
      toNodeId: "khala-fleet-delegate",
      toPinId: "request",
      evidenceRefs: traceRefs,
    }),
    link({
      id: "delegate-to-pylon",
      label: "capacity",
      fromNodeId: "khala-fleet-delegate",
      fromPinId: "capacity",
      toNodeId: "pylon-capacity",
      toPinId: "capacity",
      evidenceRefs: runRefs,
    }),
    link({
      id: "pylon-to-codex",
      label: "assignment",
      fromNodeId: "pylon-capacity",
      fromPinId: "assignment",
      toNodeId: "codex-assignment",
      toPinId: "assignment",
      evidenceRefs: runRefs,
    }),
    link({
      id: "codex-to-closeout",
      label: "closeout",
      fromNodeId: "codex-assignment",
      fromPinId: "closeout",
      toNodeId: "closeout-proof",
      toPinId: "closeout",
      evidenceRefs: traceRefs,
    }),
    link({
      id: "gd0-to-gd1",
      label: "examples",
      fromNodeId: "gd0-examples",
      fromPinId: "examples",
      toNodeId: "gd1-feedback",
      toPinId: "examples",
      evidenceRefs: datasetRefs,
    }),
    link({
      id: "gd1-to-mutalisk",
      label: "feedback",
      fromNodeId: "gd1-feedback",
      fromPinId: "reward",
      toNodeId: "mutalisk-optimizer",
      toPinId: "reward",
      evidenceRefs: feedbackRefs,
    }),
    link({
      id: "mutalisk-to-candidate",
      label: "candidate",
      fromNodeId: "mutalisk-optimizer",
      fromPinId: "candidate",
      toNodeId: "candidate-manifest",
      toPinId: "candidate",
      evidenceRefs: [...optimizerRefs, ...candidateRefs],
    }),
    link({
      id: "candidate-to-gym",
      label: "ingest",
      fromNodeId: "candidate-manifest",
      fromPinId: "summary",
      toNodeId: "gym-ingest",
      toPinId: "summary",
      evidenceRefs: [...candidateRefs, ...runRefs],
      caveatRefs,
    }),
    link({
      id: "gym-to-admission",
      label: "admission",
      fromNodeId: "gym-ingest",
      fromPinId: "admission",
      toNodeId: "admission",
      toPinId: "admission",
      evidenceRefs: gateRefs,
      blockerRefs,
      caveatRefs,
    }),
    link({
      id: "admission-to-action-submission",
      label: "proposal",
      fromNodeId: "admission",
      fromPinId: "proposal",
      toNodeId: "action-submission",
      toPinId: "proposal",
      evidenceRefs: proposalRefs,
      blockerRefs,
      caveatRefs,
    }),
  ]

  const projection: KhalaGymGraphProjection = {
    schemaVersion: KhalaGymGraphProjectionSchemaVersion,
    title: "Khala Code Gym delegation projection",
    generatedAt: input.generatedAt ?? "time.khala_gym_projection.local",
    status,
    nodes,
    links,
    evidenceRefs: allEvidenceRefs,
    blockerRefs,
    caveatRefs,
    sourceRefs: publicRefs("Gym source refs", [
      proof.schemaVersion,
      job?.publicSafetyPolicyRef,
      ...issueRefs,
    ]),
  }

  assertProjectionPublicSafe(projection)
  return projection
}

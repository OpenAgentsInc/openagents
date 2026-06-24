import { createHash } from "node:crypto"
import { assertPublicProjectionSafe } from "./state.js"

export const KHALA_M7_CONDUCTOR_COMPOSITION_SCHEMA =
  "openagents.khala.m7.conductor_composition_proof.v0.1"

export type KhalaM7ConductorCompositionBlockerRef =
  | "blocker.khala.m7.conductor_composition.run_ref_missing"
  | "blocker.khala.m7.conductor_composition.policy_backend_missing"
  | "blocker.khala.m7.conductor_composition.training_run_missing"
  | "blocker.khala.m7.conductor_composition.training_run_not_executed"
  | "blocker.khala.m7.conductor_composition.non_tmax_recipe"
  | "blocker.khala.m7.conductor_composition.worker_pool_missing"
  | "blocker.khala.m7.conductor_composition.worker_pool_incomplete"
  | "blocker.khala.m7.conductor_composition.topology_missing"
  | "blocker.khala.m7.conductor_composition.topology_incomplete"
  | "blocker.khala.m7.conductor_composition.topology_invalid"
  | "blocker.khala.m7.conductor_composition.verdict_missing"
  | "blocker.khala.m7.conductor_composition.verdict_not_accepted"
  | "blocker.khala.m7.conductor_composition.verse_fanout_missing"
  | "blocker.khala.m7.conductor_composition.cost_comparison_missing"
  | "blocker.khala.m7.conductor_composition.cost_not_lower"
  | "blocker.khala.m7.conductor_composition.quality_not_comparable"
  | "blocker.khala.m7.conductor_composition.publication_ref_missing"
  | "blocker.khala.m7.conductor_composition.unsafe_ref"

export type KhalaM7ConductorStepRole = "plan" | "implement" | "verify" | "refine"

export type KhalaM7ConductorCompositionStep = {
  stepRef: string
  role: KhalaM7ConductorStepRole
  workerId: string
  workerKind: "frontier_gateway" | "open_pylon" | "tassadar_module" | "verifier" | "khala"
  dependsOn: string[]
  accessList: string[]
  artifactRef?: string | null
  verdictRef?: string | null
}

export type KhalaM7ConductorCompositionProjection = {
  schema: typeof KHALA_M7_CONDUCTOR_COMPOSITION_SCHEMA
  observedAt: string
  canPublishCompositionProof: boolean
  compositionProofRef: string | null
  compositionRunRef: string | null
  policyBackendRef: string | null
  trainingRunRef: string | null
  trainerConfigRef: string | null
  plannerAlgorithm: "grpo_dppo" | null
  fp32LmHead: boolean
  zeroStdFiltered: boolean
  workerPoolRefs: string[]
  topologyStepRefs: string[]
  verdictRef: string | null
  rubricRef: string | null
  verseFanoutRef: string | null
  compositionCostMsats: number | null
  singleModelBaselineCostMsats: number | null
  qualityComparable: boolean
  publicationRef: string | null
  evidenceRefs: string[]
  blockerRefs: KhalaM7ConductorCompositionBlockerRef[]
  externalDependencyRefs: string[]
  authorityBoundary: string
  contentRedacted: true
}

const publicRefPattern = /^[a-z][a-z0-9._:/-]{1,220}$/i
const unsafeRefPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|checkpoint[-_]?path|invoice|lnbc|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i

const requiredRoles = ["plan", "implement", "verify", "refine"] as const
const requiredWorkerKinds = ["frontier_gateway", "open_pylon", "verifier"] as const
const externalDependencyRefs = [
  "external.psionic.m7.conductor_policy_backend",
  "external.psionic.m7.grpo_training_run",
  "external.openagents.khala.worker_pool",
  "external.openagents.verse.multi_worker_fanout",
  "external.openagents.khala.m2_crossy_road_rubric",
]

export function proveKhalaM7ConductorComposition(input: {
  observedAt: string
  compositionRunRef?: string | null
  policyBackendRef?: string | null
  trainingRunRef?: string | null
  trainingRunExecuted?: boolean
  trainerConfigRef?: string | null
  plannerAlgorithm?: "grpo_dppo" | "grpo" | "dpo" | null
  fp32LmHead?: boolean
  zeroStdFiltered?: boolean
  workerPoolRefs?: string[]
  topology?: KhalaM7ConductorCompositionStep[]
  verdictRef?: string | null
  rubricRef?: string | null
  verdictAccepted?: boolean
  verseFanoutRef?: string | null
  fanoutVisible?: boolean
  compositionCostMsats?: number | null
  singleModelBaselineCostMsats?: number | null
  qualityComparable?: boolean
  publicationRef?: string | null
}): KhalaM7ConductorCompositionProjection {
  const blockerRefs = new Set<KhalaM7ConductorCompositionBlockerRef>()
  const rawRefs = [
    normalizedRef(input.compositionRunRef),
    normalizedRef(input.policyBackendRef),
    normalizedRef(input.trainingRunRef),
    normalizedRef(input.trainerConfigRef),
    normalizedRef(input.verdictRef),
    normalizedRef(input.rubricRef),
    normalizedRef(input.verseFanoutRef),
    normalizedRef(input.publicationRef),
    ...(input.workerPoolRefs ?? []).map(normalizedRef),
    ...(input.topology ?? []).flatMap((step) => [
      normalizedRef(step.stepRef),
      normalizedRef(step.workerId),
      ...step.dependsOn.map(normalizedRef),
      ...step.accessList.map(normalizedRef),
      normalizedRef(step.artifactRef),
      normalizedRef(step.verdictRef),
    ]),
  ]
  const safeRefs = new Map<string, string | null>()
  for (const ref of rawRefs) {
    if (ref !== null && !safeRefs.has(ref)) {
      safeRefs.set(ref, isSafeRef(ref) ? ref : null)
    }
  }
  const safeRef = (value: string | null | undefined) => {
    const ref = normalizedRef(value)
    return ref === null ? null : safeRefs.get(ref) ?? null
  }

  const compositionRunRef = safeRef(input.compositionRunRef)
  const policyBackendRef = safeRef(input.policyBackendRef)
  const trainingRunRef = safeRef(input.trainingRunRef)
  const trainerConfigRef = safeRef(input.trainerConfigRef)
  const verdictRef = safeRef(input.verdictRef)
  const rubricRef = safeRef(input.rubricRef)
  const verseFanoutRef = safeRef(input.verseFanoutRef)
  const publicationRef = safeRef(input.publicationRef)
  const workerPoolRefs = uniqueRefs((input.workerPoolRefs ?? []).map(safeRef))
  const topology = input.topology ?? []
  const topologyStepRefs = uniqueRefs(topology.map((step) => safeRef(step.stepRef)))
  const topologyEvidenceRefs = topology.flatMap((step) => [
    safeRef(step.stepRef),
    safeRef(step.workerId),
    safeRef(step.artifactRef),
    safeRef(step.verdictRef),
  ])
  const compositionCostMsats = normalizedPositiveInt(input.compositionCostMsats)
  const singleModelBaselineCostMsats = normalizedPositiveInt(input.singleModelBaselineCostMsats)

  if (compositionRunRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.run_ref_missing")
  }
  if (policyBackendRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.policy_backend_missing")
  }
  if (trainingRunRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.training_run_missing")
  }
  if (input.trainingRunExecuted !== true) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.training_run_not_executed")
  }
  if (
    trainerConfigRef === null ||
    input.plannerAlgorithm !== "grpo_dppo" ||
    input.fp32LmHead !== true ||
    input.zeroStdFiltered !== true
  ) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.non_tmax_recipe")
  }
  if (workerPoolRefs.length === 0) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.worker_pool_missing")
  }
  if (!requiredWorkerKinds.every((kind) => topology.some((step) => step.workerKind === kind))) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.worker_pool_incomplete")
  }
  if (topology.length === 0) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.topology_missing")
  }
  if (!requiredRoles.every((role) => topology.some((step) => step.role === role))) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.topology_incomplete")
  }
  if (!topologyIsValid(topology, workerPoolRefs)) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.topology_invalid")
  }
  if (verdictRef === null || rubricRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.verdict_missing")
  }
  if (input.verdictAccepted !== true) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.verdict_not_accepted")
  }
  if (verseFanoutRef === null || input.fanoutVisible !== true) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.verse_fanout_missing")
  }
  if (compositionCostMsats === null || singleModelBaselineCostMsats === null) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.cost_comparison_missing")
  } else if (compositionCostMsats >= singleModelBaselineCostMsats) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.cost_not_lower")
  }
  if (input.qualityComparable !== true) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.quality_not_comparable")
  }
  if (publicationRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.publication_ref_missing")
  }
  if (rawRefs.some((ref) => ref !== null && !isSafeRef(ref))) {
    blockerRefs.add("blocker.khala.m7.conductor_composition.unsafe_ref")
  }

  const canPublishCompositionProof = blockerRefs.size === 0
  const compositionProofRef =
    canPublishCompositionProof && compositionRunRef !== null
      ? `receipt.khala.m7.conductor_composition.${stableHash(compositionRunRef)}`
      : null
  const evidenceRefs = uniqueRefs([
    compositionProofRef,
    compositionRunRef,
    policyBackendRef,
    trainingRunRef,
    trainerConfigRef,
    verdictRef,
    rubricRef,
    verseFanoutRef,
    publicationRef,
    ...workerPoolRefs,
    ...topologyStepRefs,
    ...topologyEvidenceRefs,
  ])

  const projection: KhalaM7ConductorCompositionProjection = {
    schema: KHALA_M7_CONDUCTOR_COMPOSITION_SCHEMA,
    observedAt: input.observedAt,
    canPublishCompositionProof,
    compositionProofRef,
    compositionRunRef,
    policyBackendRef,
    trainingRunRef,
    trainerConfigRef,
    plannerAlgorithm: input.plannerAlgorithm === "grpo_dppo" ? "grpo_dppo" : null,
    fp32LmHead: input.fp32LmHead === true,
    zeroStdFiltered: input.zeroStdFiltered === true,
    workerPoolRefs,
    topologyStepRefs,
    verdictRef,
    rubricRef,
    verseFanoutRef,
    compositionCostMsats,
    singleModelBaselineCostMsats,
    qualityComparable: input.qualityComparable === true,
    publicationRef,
    evidenceRefs,
    blockerRefs: [...blockerRefs].sort(),
    externalDependencyRefs,
    authorityBoundary:
      "Read-only M7 Conductor composition proof. It validates public evidence refs only; it does not run GRPO, serve a policy backend, dispatch workers, spend sats, publish benchmark claims, or move settlement authority.",
    contentRedacted: true,
  }
  assertPublicProjectionSafe(projection)
  return projection
}

function topologyIsValid(steps: KhalaM7ConductorCompositionStep[], workerPoolRefs: string[]) {
  const seenStepRefs = new Set<string>()
  const workerPool = new Set(workerPoolRefs)
  for (const step of steps) {
    const stepRef = normalizedRef(step.stepRef)
    const workerId = normalizedRef(step.workerId)
    if (stepRef === null || workerId === null || !isSafeRef(stepRef) || !isSafeRef(workerId)) {
      return false
    }
    if (seenStepRefs.has(stepRef) || !workerPool.has(workerId) || !step.accessList.includes(workerId)) {
      return false
    }
    if (!step.dependsOn.every((dependencyRef) => seenStepRefs.has(dependencyRef))) {
      return false
    }
    seenStepRefs.add(stepRef)
  }
  return true
}

function normalizedRef(value: string | null | undefined): string | null {
  if (value === null || value === undefined) return null
  const trimmed = value.trim()
  return trimmed === "" ? null : trimmed
}

function normalizedPositiveInt(value: number | null | undefined): number | null {
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? Math.trunc(value) : null
}

function isSafeRef(value: string) {
  return publicRefPattern.test(value) && !unsafeRefPattern.test(value)
}

function stableHash(input: string) {
  return createHash("sha256").update(input).digest("hex").slice(0, 20)
}

function uniqueRefs(refs: (string | null)[]) {
  return [...new Set(refs.filter((ref): ref is string => ref !== null && ref.trim().length > 0))]
    .map((ref) => ref.trim())
    .sort()
}

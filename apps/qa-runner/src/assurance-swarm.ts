import {
  type AssuranceExecutionUnit,
  type AssuranceManifest,
  type AssuranceReceipt,
  assuranceReceiptArtifact,
  canonicalArtifact,
  decodeAssuranceReceipt,
  sha256Digest,
} from "@openagentsinc/assurance-spec/execution"
import { Effect, Schema as S } from "effect"

export const QA_SWARM_ASSURANCE_RUN_FORMAT_VERSION = "0.1" as const
export const OPENAGENTS_DESKTOP_TARGET_REF = "openagents.desktop.current" as const
export const OPENAGENTS_DESKTOP_TARGET_PATH = "apps/openagents-desktop" as const

export const QaSwarmAssuranceLaneKind = S.Literals([
  "scripted_browser",
  "seeded_monkey",
  "llm_explorer",
  "performance",
  "terminal",
  "macos_native",
])
export type QaSwarmAssuranceLaneKind = typeof QaSwarmAssuranceLaneKind.Type

export const QA_SWARM_ASSURANCE_LANE_KINDS = [
  "scripted_browser",
  "seeded_monkey",
  "llm_explorer",
  "performance",
  "terminal",
  "macos_native",
] as const satisfies ReadonlyArray<QaSwarmAssuranceLaneKind>

export const QaSwarmLaneBudget = S.Struct({
  maxActions: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  maxDurationMs: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  maxModelTokens: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
})
export type QaSwarmLaneBudget = typeof QaSwarmLaneBudget.Type

export const QaSwarmLaneArming = S.Struct({
  real: S.Boolean,
  spend: S.Boolean,
  native: S.Boolean,
})
export type QaSwarmLaneArming = typeof QaSwarmLaneArming.Type

export const QaSwarmProviderUsage = S.Union([
  S.Struct({
    kind: S.Literal("no_model"),
    exact: S.Literal(true),
    inputTokens: S.Literal(0),
    outputTokens: S.Literal(0),
    totalTokens: S.Literal(0),
  }),
  S.Struct({
    kind: S.Literal("model_not_run"),
    exact: S.Literal(true),
    inputTokens: S.Literal(0),
    outputTokens: S.Literal(0),
    totalTokens: S.Literal(0),
  }),
  S.Struct({
    kind: S.Literal("model_observed"),
    exact: S.Literal(true),
    inputTokens: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    outputTokens: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    totalTokens: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  }),
])
export type QaSwarmProviderUsage = typeof QaSwarmProviderUsage.Type

export type QaSwarmLanePlan = Readonly<{
  laneRef: string
  kind: QaSwarmAssuranceLaneKind
  adapterRef: string
  executionUnitRefs: ReadonlyArray<string>
  budget: QaSwarmLaneBudget
  arming: QaSwarmLaneArming
  supported: boolean
}>

export type QaSwarmAssurancePlan = Readonly<{
  target: Readonly<{
    ref: typeof OPENAGENTS_DESKTOP_TARGET_REF
    repositoryPath: typeof OPENAGENTS_DESKTOP_TARGET_PATH
  }>
  manifest: AssuranceManifest
  manifestDigest: `sha256:${string}`
  lanes: ReadonlyArray<QaSwarmLanePlan>
  producerRef: string
  reviewerRef: string
}>

const Digest = S.String.check(S.isPattern(/^sha256:[a-f0-9]{64}$/))
const RelativeArtifactPath = S.String.check(
  S.isPattern(/^(?!\/)(?!.*(?:^|\/)\.\.(?:\/|$))[A-Za-z0-9._/-]+$/),
)

export const QaSwarmUnitObservation = S.Struct({
  executionUnitRef: S.String,
  observation: S.Literals(["CONFIRMED", "REFUTED", "INCONCLUSIVE"]),
  infrastructure: S.Literals(["ready", "unarmed", "unavailable", "failed"]),
  nativeReportRef: RelativeArtifactPath,
  nativeReportDigest: Digest,
  artifactDigest: Digest,
  sourceDigest: Digest,
  commandDigest: Digest,
})
export type QaSwarmUnitObservation = typeof QaSwarmUnitObservation.Type

export const QaSwarmLaneExecution = S.Struct({
  observations: S.Array(QaSwarmUnitObservation),
  usage: QaSwarmProviderUsage,
  actionsObserved: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  durationMsObserved: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
})
export type QaSwarmLaneExecution = typeof QaSwarmLaneExecution.Type

export type QaSwarmLaneExecutor = (
  lane: QaSwarmLanePlan,
  units: ReadonlyArray<AssuranceExecutionUnit>,
) => Effect.Effect<QaSwarmLaneExecution, unknown>

export type QaSwarmLaneReceipt = Readonly<{
  laneRef: string
  kind: QaSwarmAssuranceLaneKind
  budget: QaSwarmLaneBudget
  arming: QaSwarmLaneArming
  usage: QaSwarmProviderUsage
  actionsObserved: number
  durationMsObserved: number
  observation: "CONFIRMED" | "REFUTED" | "INCONCLUSIVE"
  blockerRefs: ReadonlyArray<string>
  commitments: ReadonlyArray<Readonly<{
    executionUnitRef: string
    role: "candidate" | "falsifier"
    expectedObservation: "CONFIRMED" | "REFUTED"
    environmentRef: string
    environmentDigest: `sha256:${string}`
    adapterRef: string
    adapterLockDigest: string
    sourceDigest: `sha256:${string}`
    commandDigest: `sha256:${string}`
    artifactDigest: `sha256:${string}`
  }>>
  assuranceReceipts: ReadonlyArray<AssuranceReceipt>
  receiptDigests: ReadonlyArray<`sha256:${string}`>
}>

export type QaSwarmAssuranceRun = Readonly<{
  formatVersion: typeof QA_SWARM_ASSURANCE_RUN_FORMAT_VERSION
  targetRef: typeof OPENAGENTS_DESKTOP_TARGET_REF
  targetRepositoryPath: typeof OPENAGENTS_DESKTOP_TARGET_PATH
  manifestDigest: `sha256:${string}`
  laneReceipts: ReadonlyArray<QaSwarmLaneReceipt>
  observation: "CONFIRMED" | "REFUTED" | "INCONCLUSIVE"
  authority: "evidence_only"
}>

export class QaSwarmAssurancePlanError extends S.TaggedErrorClass<QaSwarmAssurancePlanError>()(
  "QaSwarmAssurancePlanError",
  { code: S.String, message: S.String },
) {}

const noModelUsage = (): QaSwarmProviderUsage => ({
  kind: "no_model",
  exact: true,
  inputTokens: 0,
  outputTokens: 0,
  totalTokens: 0,
})

const modelNotRunUsage = (): QaSwarmProviderUsage => ({
  kind: "model_not_run",
  exact: true,
  inputTokens: 0,
  outputTokens: 0,
  totalTokens: 0,
})

const isModelLane = (kind: QaSwarmAssuranceLaneKind): boolean => kind === "llm_explorer"

const requiredArmingBlockers = (lane: QaSwarmLanePlan): ReadonlyArray<string> => {
  const blockers: Array<string> = []
  if (!lane.arming.real) blockers.push(`blocker.qa_swarm.${lane.kind}.real_unarmed`)
  if (isModelLane(lane.kind) && !lane.arming.spend) {
    blockers.push(`blocker.qa_swarm.${lane.kind}.spend_unarmed`)
  }
  if (lane.kind === "macos_native" && !lane.arming.native) {
    blockers.push(`blocker.qa_swarm.${lane.kind}.native_unarmed`)
  }
  return blockers
}

const aggregateObservation = (
  values: ReadonlyArray<"CONFIRMED" | "REFUTED" | "INCONCLUSIVE">,
): "CONFIRMED" | "REFUTED" | "INCONCLUSIVE" =>
  values.includes("REFUTED") ? "REFUTED"
    : values.includes("INCONCLUSIVE") ? "INCONCLUSIVE"
    : "CONFIRMED"

const validatePlan = (plan: QaSwarmAssurancePlan): Effect.Effect<void, QaSwarmAssurancePlanError> =>
  Effect.gen(function* () {
    if (canonicalArtifact(plan.manifest).digest !== plan.manifestDigest) {
      return yield* new QaSwarmAssurancePlanError({
        code: "manifest_digest_mismatch",
        message: "The supplied Manifest digest does not bind the exact Manifest document.",
      })
    }
    if (plan.target.ref !== OPENAGENTS_DESKTOP_TARGET_REF || plan.target.repositoryPath !== OPENAGENTS_DESKTOP_TARGET_PATH) {
      return yield* new QaSwarmAssurancePlanError({
        code: "unsupported_target",
        message: "QA Swarm Assurance execution targets only the current OpenAgents Desktop app.",
      })
    }
    if (plan.producerRef === plan.reviewerRef) {
      return yield* new QaSwarmAssurancePlanError({
        code: "producer_reviewer_not_independent",
        message: "Assurance receipt producer and reviewer refs must differ.",
      })
    }
    if (new Set(plan.lanes.map(lane => lane.kind)).size !== QA_SWARM_ASSURANCE_LANE_KINDS.length ||
      QA_SWARM_ASSURANCE_LANE_KINDS.some(kind => !plan.lanes.some(lane => lane.kind === kind))) {
      return yield* new QaSwarmAssurancePlanError({
        code: "incomplete_lane_set",
        message: "The Assurance swarm plan must declare each of the six typed lanes exactly once.",
      })
    }
    const unitRefs = new Set(plan.manifest.execution_units.map(unit => unit.unit_ref))
    const assigned = plan.lanes.flatMap(lane => lane.executionUnitRefs)
    if (
      plan.lanes.some(lane => lane.executionUnitRefs.length === 0) ||
      new Set(assigned).size !== assigned.length ||
      assigned.some(ref => !unitRefs.has(ref)) ||
      assigned.length !== unitRefs.size
    ) {
      return yield* new QaSwarmAssurancePlanError({
        code: "invalid_unit_assignment",
        message: "Lane unit assignments must be unique and name exact Manifest units.",
      })
    }
    for (const lane of plan.lanes) {
      const mismatched = lane.executionUnitRefs.find(ref =>
        plan.manifest.execution_units.find(unit => unit.unit_ref === ref)?.adapter_ref !== lane.adapterRef)
      if (mismatched !== undefined) {
        return yield* new QaSwarmAssurancePlanError({
          code: "lane_adapter_mismatch",
          message: `Lane ${lane.laneRef} does not bind the Manifest adapter for ${mismatched}.`,
        })
      }
    }
  })

const receiptFor = (
  plan: QaSwarmAssurancePlan,
  lane: QaSwarmLanePlan,
  unit: AssuranceExecutionUnit,
  observation: QaSwarmUnitObservation,
): AssuranceReceipt => {
  const graph = plan.manifest.obligation_graph.find(entry => entry.obligation_id === unit.obligation_id)
  if (graph === undefined) throw new Error(`Manifest graph missing obligation ${unit.obligation_id}`)
  const seed = `${plan.manifestDigest}:${lane.laneRef}:${unit.unit_ref}:${observation.artifactDigest}`
  return decodeAssuranceReceipt({
    assurance_receipt_format_version: "0.1",
    receipt_ref: `assurance.qa_swarm.${sha256Digest(seed).slice("sha256:".length)}`,
    manifest_digest: plan.manifestDigest,
    product_spec_digest: plan.manifest.product_spec.document_digest,
    assurance_spec_digest: plan.manifest.assurance_spec.document_digest,
    admission_digest: plan.manifest.admission.digest,
    obligation_id: unit.obligation_id,
    criterion_refs: graph.criterion_refs,
    environment_ref: unit.environment_ref,
    adapter_ref: unit.adapter_ref,
    execution_unit_ref: unit.unit_ref,
    producer_ref: plan.producerRef,
    reviewer_ref: plan.reviewerRef,
    native_report_ref: observation.nativeReportRef,
    native_report_digest: observation.nativeReportDigest,
    command_digest: observation.commandDigest,
    source_digest: observation.sourceDigest,
    axes: {
      admission: "admitted",
      readiness: "executable",
      observation: observation.observation,
      infrastructure: observation.infrastructure,
      stability: "unknown",
      freshness: "current",
      disposition: "pending_review",
      exception: "none",
    },
    public_safety: { classification: "reviewed_public_safe", contains_raw_output: false },
  })
}

const validateExecution = (
  lane: QaSwarmLanePlan,
  units: ReadonlyArray<AssuranceExecutionUnit>,
  execution: QaSwarmLaneExecution,
): QaSwarmLaneExecution => {
  const decoded = S.decodeUnknownSync(QaSwarmLaneExecution)(execution)
  const expected = [...units.map(unit => unit.unit_ref)].sort()
  const actual = [...decoded.observations.map(item => item.executionUnitRef)].sort()
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error("qa_swarm_observation_unit_mismatch")
  if (decoded.actionsObserved > lane.budget.maxActions || decoded.durationMsObserved > lane.budget.maxDurationMs) {
    throw new Error("qa_swarm_lane_budget_exceeded")
  }
  if (decoded.observations.some(item => item.infrastructure !== "ready" && item.observation !== "INCONCLUSIVE")) {
    throw new Error("qa_swarm_infrastructure_false_green")
  }
  if (isModelLane(lane.kind)) {
    if (decoded.usage.kind !== "model_observed" ||
      decoded.usage.totalTokens !== decoded.usage.inputTokens + decoded.usage.outputTokens ||
      decoded.usage.totalTokens > lane.budget.maxModelTokens) {
      throw new Error("qa_swarm_model_usage_not_exact")
    }
  } else if (decoded.usage.kind !== "no_model" || decoded.usage.totalTokens !== 0) {
    throw new Error("qa_swarm_no_model_usage_not_zero")
  }
  return decoded
}

const runLane = (
  plan: QaSwarmAssurancePlan,
  lane: QaSwarmLanePlan,
  executor: QaSwarmLaneExecutor | undefined,
): Effect.Effect<QaSwarmLaneReceipt> => Effect.gen(function* () {
  const units = lane.executionUnitRefs.map(ref => plan.manifest.execution_units.find(unit => unit.unit_ref === ref)!)
  const armingBlockers = requiredArmingBlockers(lane)
  const unavailable = !lane.supported || executor === undefined
  const blockers = unavailable
    ? [`blocker.qa_swarm.${lane.kind}.unsupported`]
    : armingBlockers
  const fallbackInfrastructure = unavailable ? "unavailable" as const : "unarmed" as const
  const outcome = blockers.length > 0
    ? { execution: {
        observations: [] as ReadonlyArray<QaSwarmUnitObservation>,
        usage: isModelLane(lane.kind) ? modelNotRunUsage() : noModelUsage(),
        actionsObserved: 0,
        durationMsObserved: 0,
      }, runtimeBlockers: [] as ReadonlyArray<string>, forcedObservation: "INCONCLUSIVE" as const, infrastructure: fallbackInfrastructure }
    : yield* executor!(lane, units).pipe(
        Effect.map(result => ({
          execution: validateExecution(lane, units, result),
          runtimeBlockers: [] as ReadonlyArray<string>,
        })),
        Effect.catchCause(() => Effect.succeed({
          execution: {
            observations: [] as ReadonlyArray<QaSwarmUnitObservation>,
            usage: isModelLane(lane.kind) ? modelNotRunUsage() : noModelUsage(),
            actionsObserved: 0,
            durationMsObserved: 0,
          },
          runtimeBlockers: [`blocker.qa_swarm.${lane.kind}.execution_failed`],
          forcedObservation: "INCONCLUSIVE" as const,
          infrastructure: "failed" as const,
        })),
      )
  const execution = outcome.execution
  const receipts = execution.observations.length === 0 ? [] : units.map(unit => receiptFor(
    plan,
    lane,
    unit,
    execution.observations.find(item => item.executionUnitRef === unit.unit_ref)!,
  ))
  return {
    laneRef: lane.laneRef,
    kind: lane.kind,
    budget: lane.budget,
    arming: lane.arming,
    usage: execution.usage,
    actionsObserved: execution.actionsObserved,
    durationMsObserved: execution.durationMsObserved,
    observation: "forcedObservation" in outcome ? outcome.forcedObservation : aggregateObservation(receipts.map(receipt =>
      receipt.axes.observation === "not_run" ? "INCONCLUSIVE" : receipt.axes.observation)),
    blockerRefs: [...blockers, ...outcome.runtimeBlockers],
    commitments: execution.observations.length === 0 ? [] : units.map(unit => {
      const observation = execution.observations.find(item => item.executionUnitRef === unit.unit_ref)!
      return {
        executionUnitRef: unit.unit_ref,
        role: unit.role,
        expectedObservation: unit.expected_observation,
        environmentRef: unit.environment_ref,
        environmentDigest: plan.manifest.environment.digest as `sha256:${string}`,
        adapterRef: unit.adapter_ref,
        adapterLockDigest: plan.manifest.adapter_lock_digest,
        sourceDigest: observation.sourceDigest as `sha256:${string}`,
        commandDigest: observation.commandDigest as `sha256:${string}`,
        artifactDigest: observation.artifactDigest as `sha256:${string}`,
      }
    }),
    assuranceReceipts: receipts,
    receiptDigests: receipts.map(receipt => assuranceReceiptArtifact(receipt).digest),
  }
})

export const runQaSwarmAssuranceManifest = Effect.fn("runQaSwarmAssuranceManifest")(
  function* (
    plan: QaSwarmAssurancePlan,
    executors: Readonly<Partial<Record<QaSwarmAssuranceLaneKind, QaSwarmLaneExecutor>>>,
  ) {
    yield* validatePlan(plan)
    const laneReceipts = yield* Effect.forEach(
      plan.lanes,
      lane => runLane(plan, lane, executors[lane.kind]),
      { concurrency: plan.lanes.length },
    )
    return {
      formatVersion: QA_SWARM_ASSURANCE_RUN_FORMAT_VERSION,
      targetRef: OPENAGENTS_DESKTOP_TARGET_REF,
      targetRepositoryPath: OPENAGENTS_DESKTOP_TARGET_PATH,
      manifestDigest: plan.manifestDigest,
      laneReceipts,
      observation: aggregateObservation(laneReceipts.map(receipt => receipt.observation)),
      authority: "evidence_only",
    } satisfies QaSwarmAssuranceRun
  },
)

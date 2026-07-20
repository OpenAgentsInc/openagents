import { Schema } from "effect"

const bounded = (maximum: number) => Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(maximum),
)
const sha = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u))
const sha256 = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/u))
const nonNegative = Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))
const nonNegativeInteger = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
)
const positiveInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0))
const publicRef = bounded(512)

export const IDE_PORTABLE_TARGET_CLASSES = [
  "owner_local",
  "owner_managed",
  "openagents_managed",
  "managed_provider",
] as const

export const IDE_PORTABLE_EVIDENCE_CLASSES = [
  "real_local",
  "real_owner_managed",
  "real_openagents_managed",
  "real_managed_provider",
  "simulator",
  "not_run",
] as const

export const IDE_PORTABLE_PHASES = [
  "quiesce",
  "checkpoint",
  "upload",
  "redeem",
  "attach",
  "helper_readiness",
  "failback",
  "teardown",
] as const

export const IDE_PORTABLE_ACCEPTANCE_METRICS = [
  "phase_latency",
  "checkpoint_size",
  "cpu",
  "memory",
  "network",
  "queue",
  "lease",
  "resource_cleanup",
  "teardown",
] as const

export const IdePortableEvidenceClassSchema = Schema.Literals(IDE_PORTABLE_EVIDENCE_CLASSES)
export const IdePortablePhaseSchema = Schema.Literals(IDE_PORTABLE_PHASES)

export const IdePortableEvidenceMetricSchema = Schema.Struct({
  metricRef: publicRef,
  metric: Schema.Literals(IDE_PORTABLE_ACCEPTANCE_METRICS),
  phase: Schema.NullOr(IdePortablePhaseSchema),
  unit: Schema.Literals([
    "milliseconds",
    "bytes",
    "percent",
    "count",
    "bytes_per_second",
  ]),
  repetitions: positiveInteger,
  p50: nonNegative,
  p95: nonNegative,
  p99: nonNegative,
  thresholdP95: nonNegative,
  thresholdP99: nonNegative,
  passed: Schema.Boolean,
  receiptRef: publicRef,
})

export const IdePortablePhaseReceiptSchema = Schema.Struct({
  phase: IdePortablePhaseSchema,
  evidenceClass: IdePortableEvidenceClassSchema,
  receiptRef: Schema.NullOr(publicRef),
  operationRef: Schema.NullOr(publicRef),
  attachmentGeneration: Schema.NullOr(positiveInteger),
  result: Schema.Literals(["passed", "failed", "not_run"]),
})

export const IdePortablePlacementCohortSchema = Schema.Struct({
  targetClass: Schema.Literals(IDE_PORTABLE_TARGET_CLASSES),
  evidenceClass: IdePortableEvidenceClassSchema,
  journeyScope: Schema.Literals(["full_move", "foundation_only", "not_run"]),
  operatingSystem: Schema.Literals(["darwin", "windows", "linux", "unknown"]),
  architecture: Schema.Literals(["arm64", "x64", "unknown"]),
  adapter: Schema.Struct({
    kind: Schema.Literals(["production", "deterministic_simulator", "not_run"]),
    ref: Schema.NullOr(publicRef),
    name: Schema.NullOr(bounded(160)),
    version: Schema.NullOr(bounded(80)),
  }),
  targetRef: Schema.NullOr(publicRef),
  artifact: Schema.Struct({
    ref: Schema.NullOr(publicRef),
    sha256: Schema.NullOr(sha256),
    bytes: Schema.NullOr(nonNegativeInteger),
  }),
  candidateCommitSha: sha,
  baseCommitSha: sha,
  capabilityState: Schema.Literals(["ready", "degraded", "unsupported", "unverified"]),
  custody: Schema.Literals([
    "owner_device",
    "owner_managed",
    "openagents_managed",
    "provider_managed",
    "unverified",
  ]),
  networkDestinations: Schema.Array(bounded(320)).check(Schema.isMaxLength(32)),
  dataDestinations: Schema.Array(bounded(320)).check(Schema.isMaxLength(32)),
  retentionSeconds: nonNegativeInteger,
  costFact: bounded(160),
  phaseReceipts: Schema.Array(IdePortablePhaseReceiptSchema).check(
    Schema.isMinLength(IDE_PORTABLE_PHASES.length),
    Schema.isMaxLength(IDE_PORTABLE_PHASES.length),
  ),
  metrics: Schema.Array(IdePortableEvidenceMetricSchema).check(Schema.isMaxLength(64)),
  result: bounded(1_000),
})

const expectedEvidenceClass = {
  owner_local: "real_local",
  owner_managed: "real_owner_managed",
  openagents_managed: "real_openagents_managed",
  managed_provider: "real_managed_provider",
} as const

const IdePortableEvidenceReceiptBaseSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-evidence.v2"),
  issue: Schema.Literal("IDE-13"),
  candidateCommitSha: sha,
  baseCommitSha: sha,
  generatedAt: Schema.String,
  producerRef: publicRef,
  environment: Schema.Struct({
    platform: bounded(40),
    architecture: bounded(40),
    node: bounded(40),
  }),
  model: Schema.Struct({
    maximumDepth: positiveInteger,
    exploredStates: positiveInteger,
    exploredTransitions: positiveInteger,
    staleWriteAttempts: nonNegativeInteger,
    counterexamples: Schema.Literal(0),
    passed: Schema.Literal(true),
  }),
  implementedChecks: Schema.Array(Schema.Struct({
    checkRef: publicRef,
    evidenceClass: Schema.Literals(["model", "regression", "packaged_fail_closed"]),
    result: Schema.Literal("passed"),
    receiptRef: publicRef,
  })).check(Schema.isMinLength(1), Schema.isMaxLength(128)),
  placementCohorts: Schema.Array(IdePortablePlacementCohortSchema).check(
    Schema.isMinLength(IDE_PORTABLE_TARGET_CLASSES.length),
    Schema.isMaxLength(IDE_PORTABLE_TARGET_CLASSES.length),
  ),
  omissions: Schema.Array(Schema.Struct({
    omissionRef: publicRef,
    targetClass: Schema.Literals(IDE_PORTABLE_TARGET_CLASSES),
    fact: bounded(500),
    disposition: Schema.Literals(["accepted_limit", "acceptance_gap"]),
    evidenceRef: publicRef,
  })).check(Schema.isMinLength(1), Schema.isMaxLength(128)),
  recoveryFacts: Schema.Array(Schema.Struct({
    recoveryRef: publicRef,
    targetClass: Schema.Literals(IDE_PORTABLE_TARGET_CLASSES),
    scenario: bounded(200),
    evidenceClass: IdePortableEvidenceClassSchema,
    outcome: Schema.Literals(["passed", "failed", "not_run"]),
    recoveryPointRef: Schema.NullOr(publicRef),
    receiptRef: Schema.NullOr(publicRef),
  })).check(Schema.isMinLength(1), Schema.isMaxLength(128)),
  security: Schema.Struct({
    forbiddenMaterialProjected: Schema.Literal(false),
    optimisticAuthorityProjected: Schema.Literal(false),
    staleGenerationAccepted: Schema.Literal(false),
    rawCredentialProjected: Schema.Literal(false),
  }),
  review: Schema.Struct({
    independentReviewerRef: Schema.NullOr(publicRef),
    independentDisposition: Schema.Literals(["accepted", "rejected", "not_run"]),
    independentDispositionRef: Schema.NullOr(publicRef),
    ownerRef: Schema.NullOr(publicRef),
    ownerDisposition: Schema.Literals(["accepted", "rejected", "not_run"]),
    ownerDispositionRef: Schema.NullOr(publicRef),
  }),
  implementationChecksPassed: Schema.Literal(true),
  acceptancePassed: Schema.Boolean,
  remainingGaps: Schema.Array(bounded(1_000)).check(Schema.isMaxLength(64)),
})

export const IdePortableEvidenceReceiptSchema = IdePortableEvidenceReceiptBaseSchema.check(
  Schema.makeFilter((receipt) => {
    if (receipt.placementCohorts.some(cohort =>
      cohort.candidateCommitSha !== receipt.candidateCommitSha ||
      cohort.baseCommitSha !== receipt.baseCommitSha
    )) return "IDE-13 cohort candidate or base refs are stale"
    if (new Set(receipt.placementCohorts.map(cohort => cohort.targetClass)).size !==
      IDE_PORTABLE_TARGET_CLASSES.length) return "IDE-13 target cohort matrix is incomplete"
    if (receipt.placementCohorts.some(cohort =>
      new Set(cohort.phaseReceipts.map(phase => phase.phase)).size !== IDE_PORTABLE_PHASES.length
    )) return "IDE-13 phase receipt matrix is incomplete"
    if (receipt.placementCohorts.some(cohort =>
      (cohort.evidenceClass.startsWith("real_") && cohort.adapter.kind !== "production") ||
      (!cohort.evidenceClass.startsWith("real_") && cohort.adapter.kind === "production")
    )) return "IDE-13 simulated evidence cannot be classified as real"
    if (receipt.review.independentReviewerRef === receipt.producerRef) {
      return "IDE-13 evidence cannot use producer self-review"
    }
    if (!receipt.acceptancePassed) return undefined
    if (receipt.placementCohorts.some(cohort => {
      const explicitlyUnclaimedProvider = cohort.targetClass === "managed_provider" &&
        cohort.evidenceClass === "not_run" && cohort.journeyScope === "not_run" &&
        cohort.adapter.kind === "not_run" && cohort.adapter.ref === null &&
        cohort.adapter.name === null && cohort.adapter.version === null &&
        cohort.targetRef === null && cohort.artifact.ref === null &&
        cohort.artifact.sha256 === null && cohort.artifact.bytes === null &&
        cohort.capabilityState === "unsupported" && cohort.custody === "unverified" &&
        cohort.phaseReceipts.every(phase => phase.evidenceClass === "not_run" &&
          phase.receiptRef === null && phase.operationRef === null &&
          phase.attachmentGeneration === null && phase.result === "not_run") &&
        cohort.metrics.length === 0
      if (explicitlyUnclaimedProvider) return false
      return cohort.evidenceClass !== expectedEvidenceClass[cohort.targetClass] ||
        cohort.journeyScope !== "full_move" || cohort.adapter.kind !== "production" ||
        cohort.adapter.ref === null || cohort.adapter.name === null || cohort.adapter.version === null ||
        cohort.targetRef === null || cohort.artifact.ref === null || cohort.artifact.sha256 === null ||
        cohort.artifact.bytes === null || cohort.phaseReceipts.some(phase =>
          phase.evidenceClass !== cohort.evidenceClass || phase.receiptRef === null ||
          phase.operationRef === null || phase.attachmentGeneration === null || phase.result !== "passed"
        ) || cohort.metrics.length !== IDE_PORTABLE_ACCEPTANCE_METRICS.length ||
        new Set(cohort.metrics.map(metric => metric.metric)).size !== IDE_PORTABLE_ACCEPTANCE_METRICS.length ||
        IDE_PORTABLE_ACCEPTANCE_METRICS.some(metric =>
          !cohort.metrics.some(observation => observation.metric === metric && observation.passed)
        )
    })) return "IDE-13 acceptance requires complete real claimed cohorts, receipts, and metrics"
    if (receipt.review.independentReviewerRef === null ||
      receipt.review.independentReviewerRef === receipt.producerRef ||
      receipt.review.independentDisposition !== "accepted" ||
      receipt.review.independentDispositionRef === null || receipt.review.ownerRef === null ||
      receipt.review.ownerDisposition !== "accepted" || receipt.review.ownerDispositionRef === null) {
      return "IDE-13 acceptance requires independent reviewer and owner dispositions"
    }
    if (receipt.remainingGaps.length > 0) return "IDE-13 acceptance cannot contain remaining gaps"
    return undefined
  }),
)
export type IdePortableEvidenceReceipt = typeof IdePortableEvidenceReceiptSchema.Type

const requireCompleteSet = (
  values: ReadonlyArray<string>,
  expected: ReadonlyArray<string>,
  label: string,
): void => {
  if (values.length !== expected.length || new Set(values).size !== expected.length ||
    expected.some(value => !values.includes(value))) {
    throw new Error(`IDE-13 ${label} is incomplete or contains a duplicate`)
  }
}

export const validateIdePortableEvidenceReceipt = (
  receipt: IdePortableEvidenceReceipt,
  expected: Readonly<{ candidateCommitSha: string; baseCommitSha?: string }>,
): void => {
  if (receipt.candidateCommitSha !== expected.candidateCommitSha) {
    throw new Error("IDE-13 evidence candidate is stale")
  }
  if (expected.baseCommitSha !== undefined && receipt.baseCommitSha !== expected.baseCommitSha) {
    throw new Error("IDE-13 evidence base is stale")
  }

  requireCompleteSet(
    receipt.placementCohorts.map(cohort => cohort.targetClass),
    IDE_PORTABLE_TARGET_CLASSES,
    "target cohort matrix",
  )

  for (const cohort of receipt.placementCohorts) {
    if (cohort.candidateCommitSha !== receipt.candidateCommitSha ||
      cohort.baseCommitSha !== receipt.baseCommitSha) {
      throw new Error(`IDE-13 ${cohort.targetClass} cohort uses stale candidate or base refs`)
    }
    requireCompleteSet(
      cohort.phaseReceipts.map(phase => phase.phase),
      IDE_PORTABLE_PHASES,
      `${cohort.targetClass} phase receipt matrix`,
    )
    const isReal = cohort.evidenceClass.startsWith("real_")
    if (isReal && cohort.adapter.kind !== "production") {
      throw new Error(`IDE-13 ${cohort.targetClass} simulated evidence is classified as real`)
    }
    if (!isReal && cohort.adapter.kind === "production") {
      throw new Error(`IDE-13 ${cohort.targetClass} production adapter has non-real evidence`)
    }
  }

  if (receipt.review.independentReviewerRef === receipt.producerRef) {
    throw new Error("IDE-13 evidence cannot use producer self-review")
  }

  if (!receipt.acceptancePassed) return

  for (const cohort of receipt.placementCohorts) {
    const explicitlyUnclaimedProvider = cohort.targetClass === "managed_provider" &&
      cohort.evidenceClass === "not_run" && cohort.journeyScope === "not_run" &&
      cohort.adapter.kind === "not_run" && cohort.adapter.ref === null &&
      cohort.adapter.name === null && cohort.adapter.version === null &&
      cohort.targetRef === null && cohort.artifact.ref === null &&
      cohort.artifact.sha256 === null && cohort.artifact.bytes === null &&
      cohort.capabilityState === "unsupported" && cohort.custody === "unverified" &&
      cohort.phaseReceipts.every(phase => phase.evidenceClass === "not_run" &&
        phase.receiptRef === null && phase.operationRef === null &&
        phase.attachmentGeneration === null && phase.result === "not_run") &&
      cohort.metrics.length === 0
    if (explicitlyUnclaimedProvider) continue
    if (cohort.evidenceClass !== expectedEvidenceClass[cohort.targetClass] ||
      cohort.journeyScope !== "full_move" || cohort.adapter.kind !== "production") {
      throw new Error(`IDE-13 acceptance lacks required real ${cohort.targetClass} evidence`)
    }
    if (cohort.adapter.ref === null || cohort.adapter.name === null ||
      cohort.adapter.version === null || cohort.targetRef === null ||
      cohort.artifact.ref === null || cohort.artifact.sha256 === null ||
      cohort.artifact.bytes === null) {
      throw new Error(`IDE-13 acceptance lacks exact ${cohort.targetClass} refs`)
    }
    if (cohort.phaseReceipts.some(phase =>
      phase.evidenceClass !== cohort.evidenceClass || phase.receiptRef === null ||
      phase.operationRef === null || phase.attachmentGeneration === null || phase.result !== "passed"
    )) {
      throw new Error(`IDE-13 acceptance lacks exact ${cohort.targetClass} phase receipts`)
    }
    requireCompleteSet(
      cohort.metrics.map(metric => metric.metric),
      IDE_PORTABLE_ACCEPTANCE_METRICS,
      `${cohort.targetClass} acceptance metrics`,
    )
    if (cohort.metrics.some(metric => !metric.passed)) {
      throw new Error(`IDE-13 acceptance contains a failed ${cohort.targetClass} metric`)
    }
  }
  if (receipt.review.independentDisposition !== "accepted" ||
    receipt.review.independentReviewerRef === null ||
    receipt.review.independentDispositionRef === null) {
    throw new Error("IDE-13 acceptance lacks an independent reviewer disposition")
  }
  if (receipt.review.ownerDisposition !== "accepted" || receipt.review.ownerRef === null ||
    receipt.review.ownerDispositionRef === null) {
    throw new Error("IDE-13 acceptance lacks an owner disposition")
  }
  if (receipt.remainingGaps.length > 0) {
    throw new Error("IDE-13 acceptance still records acceptance gaps")
  }
}

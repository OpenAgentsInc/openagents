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

export const IDE_PORTABLE_NON_PHASE_ACCEPTANCE_METRICS = [
  "checkpoint_size",
  "cpu",
  "memory",
  "network",
  "queue",
  "lease",
  "resource_cleanup",
  "teardown",
] as const

export const IDE_PORTABLE_ACCEPTANCE_METRICS = [
  "phase_latency",
  ...IDE_PORTABLE_NON_PHASE_ACCEPTANCE_METRICS,
] as const

export const IDE_PORTABLE_FAULT_SCENARIOS = [
  "transition_partition",
  "coordinator_crash",
  "checkpoint_store_crash",
  "provider_crash",
  "duplicate_event",
  "reordered_event",
  "old_generation_command",
  "lease_expiry_clock_skew",
  "corrupt_checkpoint",
  "truncated_checkpoint",
  "wrong_schema_checkpoint",
  "missing_artifact",
  "auth_expiry_revocation",
  "provider_capability_drift",
  "destination_boot_failure",
  "source_revocation_failure",
  "dual_attachment_claim",
  "target_offline_or_evicted",
  "cancellation_and_app_restart",
  "failback_to_older_recovery_point",
] as const

export const IDE_PORTABLE_REQUIRED_FAULT_CASES = [
  ...IDE_PORTABLE_PHASES.map(phase => ({
    scenario: "transition_partition" as const,
    phase,
  })),
  ...IDE_PORTABLE_FAULT_SCENARIOS.slice(1).map(scenario => ({ scenario, phase: null })),
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
  cohortRef: publicRef,
  targetClass: Schema.Literals(IDE_PORTABLE_TARGET_CLASSES),
  evidenceClass: IdePortableEvidenceClassSchema,
  journeyScope: Schema.Literals(["full_move", "foundation_only", "not_run"]),
  journeys: Schema.Struct({
    mainJourneyReceiptRef: Schema.NullOr(publicRef),
    failbackJourneyReceiptRef: Schema.NullOr(publicRef),
    faultMatrixReceiptRef: Schema.NullOr(publicRef),
  }),
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
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-evidence.v3"),
  issue: Schema.Literal("IDE-13"),
  candidateCommitSha: sha,
  baseCommitSha: sha,
  generatedAt: Schema.String,
  producerRef: publicRef,
  acceptanceRefs: Schema.Struct({
    candidateRef: Schema.NullOr(publicRef),
    mainCommitSha: Schema.NullOr(sha),
    mainRef: Schema.NullOr(publicRef),
    artifactReceiptRef: Schema.NullOr(publicRef),
    rollbackReceiptRef: Schema.NullOr(publicRef),
    verificationCommandRef: Schema.NullOr(publicRef),
    verificationResultRef: Schema.NullOr(publicRef),
  }),
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
    Schema.isMaxLength(32),
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
    cohortRef: publicRef,
    targetClass: Schema.Literals(IDE_PORTABLE_TARGET_CLASSES),
    scenario: bounded(200),
    evidenceClass: IdePortableEvidenceClassSchema,
    outcome: Schema.Literals(["passed", "failed", "not_run"]),
    recoveryPointRef: Schema.NullOr(publicRef),
    receiptRef: Schema.NullOr(publicRef),
  })).check(Schema.isMinLength(1), Schema.isMaxLength(256)),
  faultFacts: Schema.Array(Schema.Struct({
    faultRef: publicRef,
    cohortRef: publicRef,
    targetClass: Schema.Literals(IDE_PORTABLE_TARGET_CLASSES),
    scenario: Schema.Literals(IDE_PORTABLE_FAULT_SCENARIOS),
    phase: Schema.NullOr(IdePortablePhaseSchema),
    evidenceClass: IdePortableEvidenceClassSchema,
    outcome: Schema.Literals(["passed", "failed", "not_run"]),
    recoveryPointRef: Schema.NullOr(publicRef),
    receiptRef: Schema.NullOr(publicRef),
  })).check(Schema.isMaxLength(1_024)),
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

type BaseReceipt = typeof IdePortableEvidenceReceiptBaseSchema.Type
type PlacementCohort = BaseReceipt["placementCohorts"][number]

const hasCompleteSet = (
  values: ReadonlyArray<string>,
  expected: ReadonlyArray<string>,
): boolean => values.length === expected.length && new Set(values).size === expected.length &&
  expected.every(value => values.includes(value))

const isExplicitlyUnclaimedProvider = (cohort: PlacementCohort): boolean =>
  cohort.targetClass === "managed_provider" && cohort.evidenceClass === "not_run" &&
  cohort.journeyScope === "not_run" && cohort.adapter.kind === "not_run" &&
  cohort.operatingSystem === "unknown" && cohort.architecture === "unknown" &&
  cohort.journeys.mainJourneyReceiptRef === null &&
  cohort.journeys.failbackJourneyReceiptRef === null && cohort.journeys.faultMatrixReceiptRef === null &&
  cohort.adapter.ref === null && cohort.adapter.name === null && cohort.adapter.version === null &&
  cohort.targetRef === null && cohort.artifact.ref === null && cohort.artifact.sha256 === null &&
  cohort.artifact.bytes === null && cohort.capabilityState === "unsupported" &&
  cohort.custody === "unverified" && cohort.networkDestinations.length === 0 &&
  cohort.dataDestinations.length === 0 && cohort.retentionSeconds === 0 &&
  cohort.phaseReceipts.every(phase =>
    phase.evidenceClass === "not_run" && phase.receiptRef === null &&
    phase.operationRef === null && phase.attachmentGeneration === null && phase.result === "not_run"
  ) && cohort.metrics.length === 0

const acceptanceMetricError = (cohort: PlacementCohort): string | undefined => {
  const identities = cohort.metrics.map(metric => `${metric.metric}:${metric.phase ?? "all"}`)
  const expectedIdentities = [
    ...IDE_PORTABLE_PHASES.map(phase => `phase_latency:${phase}`),
    ...IDE_PORTABLE_NON_PHASE_ACCEPTANCE_METRICS.map(metric => `${metric}:all`),
  ]
  if (!hasCompleteSet(identities, expectedIdentities)) {
    return `IDE-13 ${cohort.cohortRef} acceptance metric and phase matrix is incomplete or duplicated`
  }
  if (cohort.metrics.some(metric =>
    (metric.metric === "phase_latency" && metric.phase === null) ||
    (metric.metric !== "phase_latency" && metric.phase !== null)
  )) return `IDE-13 ${cohort.cohortRef} acceptance metric phase binding is invalid`
  if (cohort.metrics.some(metric => !metric.passed)) {
    return `IDE-13 acceptance contains a failed ${cohort.cohortRef} metric`
  }
  return undefined
}

const receiptSemanticError = (receipt: BaseReceipt): string | undefined => {
  if (receipt.placementCohorts.some(cohort =>
    cohort.candidateCommitSha !== receipt.candidateCommitSha ||
    cohort.baseCommitSha !== receipt.baseCommitSha
  )) return "IDE-13 cohort candidate or base refs are stale"
  if (new Set(receipt.placementCohorts.map(cohort => cohort.cohortRef)).size !==
    receipt.placementCohorts.length) return "IDE-13 cohort refs contain a duplicate"
  if (IDE_PORTABLE_TARGET_CLASSES.some(targetClass =>
    !receipt.placementCohorts.some(cohort => cohort.targetClass === targetClass)
  )) return "IDE-13 target cohort matrix is incomplete"
  if (receipt.placementCohorts.some(cohort =>
    !hasCompleteSet(cohort.phaseReceipts.map(phase => phase.phase), IDE_PORTABLE_PHASES)
  )) return "IDE-13 phase receipt matrix is incomplete"
  if (receipt.placementCohorts.some(cohort =>
    (cohort.evidenceClass.startsWith("real_") && cohort.adapter.kind !== "production") ||
    (!cohort.evidenceClass.startsWith("real_") && cohort.adapter.kind === "production")
  )) return "IDE-13 simulated evidence cannot be classified as real"

  const cohortsByRef = new Map(receipt.placementCohorts.map(cohort => [cohort.cohortRef, cohort]))
  if ([...receipt.recoveryFacts, ...receipt.faultFacts].some(fact => {
    const cohort = cohortsByRef.get(fact.cohortRef)
    return cohort === undefined || cohort.targetClass !== fact.targetClass
  })) return "IDE-13 recovery or fault evidence uses an unknown or mismatched cohort ref"

  const reviewerRef = receipt.review.independentReviewerRef
  const ownerRef = receipt.review.ownerRef
  if (reviewerRef !== null && (reviewerRef === receipt.producerRef || reviewerRef === ownerRef)) {
    return "IDE-13 evidence requires a reviewer independent from producer and owner"
  }
  if (ownerRef !== null && ownerRef === receipt.producerRef) {
    return "IDE-13 evidence requires an owner distinct from the producer"
  }

  if (!receipt.acceptancePassed) return undefined
  if (receipt.omissions.some(omission => omission.disposition === "acceptance_gap")) {
    return "IDE-13 acceptance cannot contain an acceptance-gap omission"
  }
  if (Object.values(receipt.acceptanceRefs).some(reference => reference === null)) {
    return "IDE-13 acceptance requires exact candidate, main, artifact, rollback, and verification refs"
  }

  const unclaimedProviders = receipt.placementCohorts.filter(isExplicitlyUnclaimedProvider)
  const claimedCohorts = receipt.placementCohorts.filter(cohort => !isExplicitlyUnclaimedProvider(cohort))
  if (unclaimedProviders.length > 1 || (unclaimedProviders.length > 0 &&
    receipt.placementCohorts.some(cohort =>
      cohort.targetClass === "managed_provider" && !isExplicitlyUnclaimedProvider(cohort)
    ))) return "IDE-13 managed-provider matrix mixes claimed and unclaimed cohorts"

  for (const cohort of claimedCohorts) {
    if (cohort.evidenceClass !== expectedEvidenceClass[cohort.targetClass] ||
      cohort.journeyScope !== "full_move" || cohort.adapter.kind !== "production") {
      return `IDE-13 acceptance lacks required real ${cohort.cohortRef} evidence`
    }
    if (cohort.journeys.mainJourneyReceiptRef === null ||
      cohort.journeys.failbackJourneyReceiptRef === null ||
      cohort.journeys.faultMatrixReceiptRef === null) {
      return `IDE-13 acceptance lacks exact ${cohort.cohortRef} journey refs`
    }
    if (cohort.adapter.ref === null || cohort.adapter.name === null ||
      cohort.adapter.version === null || cohort.targetRef === null ||
      cohort.artifact.ref === null || cohort.artifact.sha256 === null ||
      cohort.artifact.bytes === null) {
      return `IDE-13 acceptance lacks exact ${cohort.cohortRef} target refs`
    }
    if (cohort.phaseReceipts.some(phase =>
      phase.evidenceClass !== cohort.evidenceClass || phase.receiptRef === null ||
      phase.operationRef === null || phase.attachmentGeneration === null || phase.result !== "passed"
    )) return `IDE-13 acceptance lacks exact ${cohort.cohortRef} phase receipts`
    const metricError = acceptanceMetricError(cohort)
    if (metricError !== undefined) return metricError

    const recoveryFacts = receipt.recoveryFacts.filter(fact => fact.cohortRef === cohort.cohortRef)
    if (recoveryFacts.length === 0) {
      return `IDE-13 acceptance lacks recovery coverage for ${cohort.cohortRef}`
    }
    if (recoveryFacts.some(fact => fact.evidenceClass !== cohort.evidenceClass ||
      fact.outcome !== "passed" || fact.recoveryPointRef === null || fact.receiptRef === null)) {
      return `IDE-13 acceptance contains incomplete or non-real recovery evidence for ${cohort.cohortRef}`
    }

    const faultFacts = receipt.faultFacts.filter(fact => fact.cohortRef === cohort.cohortRef)
    const faultIdentities = faultFacts.map(fact => `${fact.scenario}:${fact.phase ?? "all"}`)
    const expectedFaultIdentities = IDE_PORTABLE_REQUIRED_FAULT_CASES.map(fault =>
      `${fault.scenario}:${fault.phase ?? "all"}`
    )
    if (!hasCompleteSet(faultIdentities, expectedFaultIdentities)) {
      return `IDE-13 acceptance fault matrix is incomplete or duplicated for ${cohort.cohortRef}`
    }
    if (faultFacts.some(fact =>
      (fact.scenario === "transition_partition" && fact.phase === null) ||
      (fact.scenario !== "transition_partition" && fact.phase !== null)
    )) return `IDE-13 acceptance fault phase binding is invalid for ${cohort.cohortRef}`
    if (faultFacts.some(fact => fact.evidenceClass !== cohort.evidenceClass ||
      fact.outcome !== "passed" || fact.recoveryPointRef === null || fact.receiptRef === null)) {
      return `IDE-13 acceptance contains incomplete or non-real fault evidence for ${cohort.cohortRef}`
    }
  }

  if (receipt.recoveryFacts.some(fact => {
    const cohort = cohortsByRef.get(fact.cohortRef)
    return cohort === undefined || isExplicitlyUnclaimedProvider(cohort) ||
      fact.evidenceClass !== cohort.evidenceClass || fact.outcome !== "passed" ||
      fact.recoveryPointRef === null || fact.receiptRef === null
  })) return "IDE-13 acceptance contains failed, not-run, incomplete, or non-real recovery evidence"
  if (receipt.faultFacts.some(fact => {
    const cohort = cohortsByRef.get(fact.cohortRef)
    return cohort === undefined || isExplicitlyUnclaimedProvider(cohort) ||
      fact.evidenceClass !== cohort.evidenceClass || fact.outcome !== "passed" ||
      fact.recoveryPointRef === null || fact.receiptRef === null
  })) return "IDE-13 acceptance contains failed, not-run, incomplete, or non-real fault evidence"

  if (reviewerRef === null || receipt.review.independentDisposition !== "accepted" ||
    receipt.review.independentDispositionRef === null || ownerRef === null ||
    receipt.review.ownerDisposition !== "accepted" || receipt.review.ownerDispositionRef === null) {
    return "IDE-13 acceptance requires independent reviewer and owner dispositions"
  }
  if (receipt.remainingGaps.length > 0) return "IDE-13 acceptance cannot contain remaining gaps"
  return undefined
}

export const IdePortableEvidenceReceiptSchema = IdePortableEvidenceReceiptBaseSchema.check(
  Schema.makeFilter(receiptSemanticError),
)
export type IdePortableEvidenceReceipt = typeof IdePortableEvidenceReceiptSchema.Type

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
  const semanticError = receiptSemanticError(receipt)
  if (semanticError !== undefined) throw new Error(semanticError)
}

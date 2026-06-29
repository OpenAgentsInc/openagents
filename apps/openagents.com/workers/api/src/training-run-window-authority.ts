import { Option, Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  isRecord,
  optionalString,
  parseJsonRecord,
  parseJsonStringArray,
  stringArrayFromUnknown,
} from './json-boundary'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { isoTimestampAfterIso } from './runtime-primitives'
import {
  DurableCheckpointSeal,
  evaluateDurableCheckpointSeal,
} from './training-durable-checkpoint-seal'
import {
  type TrainingVerificationChallengeRecord,
  type TrainingVerificationRow,
  rowToTrainingVerificationChallenge,
} from './training-verification'

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
export const TrainingPublicSafeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/
export const TrainingPublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(TrainingPublicSafeRefPattern),
)
const PublicSafeRef = TrainingPublicSafeRef
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))
export const TrainingPublicSafePylonRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(120),
  S.isPattern(/^[a-z0-9][a-z0-9_.:-]*$/),
)
const PublicSafePylonRef = TrainingPublicSafePylonRef

export const TrainingRunState = S.Literals([
  'planned',
  'active',
  'sealed',
  'reconciled',
])
export type TrainingRunState = typeof TrainingRunState.Type

export const TrainingWindowState = S.Literals([
  'planned',
  'active',
  'sealed',
  'reconciled',
])
export type TrainingWindowState = typeof TrainingWindowState.Type

export const TrainingWindowHomeworkKind = S.Literals([
  'admin_dispatched_homework',
  'operator_planned_homework',
  'auto_starter',
])
export type TrainingWindowHomeworkKind = typeof TrainingWindowHomeworkKind.Type

/**
 * Default sync-reentry staleness trigger in optimizer steps.
 *
 * Rationale: a merged contribution that is more than this many steps behind
 * the sealed window head routes to sync re-entry instead of being merged or
 * rejected (Pluralis roadmap P0.2; forum post 6197bd1b). Pluralis node0 ships
 * `max_allowed_stale: 5` as prior art for this default. Ours is a stated
 * per-run contract value, not an inherited constant: the value is provisional
 * until R1 rehearsal seal records carry measured steps-behind distributions,
 * and any revision lands as a run-config change with its own receipt.
 */
export const DefaultMaxAllowedStaleSteps = 5

/**
 * Default seal/snapshot publication cadence in windows.
 *
 * Rationale: joiners bootstrap from the last durable seal only (Pluralis
 * roadmap P1.2; openagents issue #4850), so how often a sealed checkpoint
 * is published bounds how far behind a fresh joiner starts. The default of
 * one (every window publishes a durable seal) matches the current
 * window-lifecycle behavior; runs that seal less often must declare it
 * here so the joiner-staleness bound stays a stated per-run contract value.
 */
export const DefaultSealPublicationCadenceWindows = 1

export const MaxTrainingWindowSealContributionEntries = 64
export const MaxTrainingWindowSealChurnEventEntries = 64

const TrainingWindowSealStepsBehind = S.Number.check(
  S.isInt(),
  S.isBetween({ minimum: 0, maximum: 1_000_000 }),
)
const TrainingWindowSealNonNegativeCount = S.Number.check(
  S.isInt(),
  S.isBetween({ minimum: 0, maximum: 1_000_000 }),
)
const TrainingWindowSealPercentileSteps = S.Number.check(
  S.isFinite(),
  S.isBetween({ minimum: 0, maximum: 1_000_000 }),
)

export const TrainingWindowSealContributionStaleness = S.Struct({
  contributionRef: PublicSafeRef,
  stepsBehind: TrainingWindowSealStepsBehind,
})
export type TrainingWindowSealContributionStaleness =
  typeof TrainingWindowSealContributionStaleness.Type

export const TrainingWindowSealStalenessSummary = S.Struct({
  contributionCount: TrainingWindowSealNonNegativeCount,
  contributions: S.optionalKey(
    S.Array(TrainingWindowSealContributionStaleness).check(
      S.isMaxLength(MaxTrainingWindowSealContributionEntries),
    ),
  ),
  stepsBehindMax: TrainingWindowSealStepsBehind,
  stepsBehindMin: TrainingWindowSealStepsBehind,
  stepsBehindP50: TrainingWindowSealPercentileSteps,
  stepsBehindP90: TrainingWindowSealPercentileSteps,
})
export type TrainingWindowSealStalenessSummary =
  typeof TrainingWindowSealStalenessSummary.Type

export const TrainingWindowChurnEventKind = S.Literals([
  'join',
  'loss',
  'standby_promotion',
])
export type TrainingWindowChurnEventKind =
  typeof TrainingWindowChurnEventKind.Type

export const TrainingWindowChurnEvent = S.Struct({
  eventRef: PublicSafeRef,
  kind: TrainingWindowChurnEventKind,
})
export type TrainingWindowChurnEvent = typeof TrainingWindowChurnEvent.Type

export const TrainingWindowSealChurnSummary = S.Struct({
  events: S.optionalKey(
    S.Array(TrainingWindowChurnEvent).check(
      S.isMaxLength(MaxTrainingWindowSealChurnEventEntries),
    ),
  ),
  joinCount: TrainingWindowSealNonNegativeCount,
  lossCount: TrainingWindowSealNonNegativeCount,
  standbyPromotionCount: TrainingWindowSealNonNegativeCount,
})
export type TrainingWindowSealChurnSummary =
  typeof TrainingWindowSealChurnSummary.Type

export const TrainingWindowSealVerificationOverhead = S.Struct({
  fraction: S.Number.check(
    S.isFinite(),
    S.isBetween({ minimum: 0, maximum: 1 }),
  ),
  ladderRungRef: PublicSafeRef,
})
export type TrainingWindowSealVerificationOverhead =
  typeof TrainingWindowSealVerificationOverhead.Type

export const TrainingWindowSealMetadata = S.Struct({
  checkpointDigestRef: S.optionalKey(PublicSafeRef),
  churn: TrainingWindowSealChurnSummary,
  durableCheckpointSeal: S.optionalKey(DurableCheckpointSeal),
  staleness: TrainingWindowSealStalenessSummary,
  verificationOverhead: TrainingWindowSealVerificationOverhead,
})
export type TrainingWindowSealMetadata = typeof TrainingWindowSealMetadata.Type

/**
 * Typed caveat raised when a run is still `planned` while one or more of its
 * windows has reconciled. The public run projection must carry this blocker so
 * a reconciled-but-not-promoted run never reads as a clean `planned` state
 * (#5006). The run state-transition route (planned -> active -> ...) clears it.
 */
export const TrainingRunPlannedWithReconciledWindowsBlocker =
  'blocker.training.run_state_planned_with_reconciled_windows'

const ManifestText = NonEmptyTrimmedString.check(S.isMaxLength(600))

/**
 * Public-safe launch manifest for a training run (#5006). Every field is
 * optional so runs created before the manifest existed still decode. No raw
 * prompts, wallet material, hostnames, tokens, invoices, preimages, or local
 * paths belong here; refs are public-safe.
 */
export const TrainingRunManifest = S.Struct({
  abortRule: S.optionalKey(ManifestText),
  admissionRule: S.optionalKey(ManifestText),
  artifactDigestRefs: PublicSafeRefs,
  blockerRefs: PublicSafeRefs,
  maxParticipants: S.optionalKey(
    S.Number.check(
      S.isInt(),
      S.isBetween({ minimum: 1, maximum: 100_000_000 }),
    ),
  ),
  objective: S.optionalKey(ManifestText),
  paymentMode: S.optionalKey(PublicSafeRef),
  participantCountRule: S.optionalKey(ManifestText),
  settlementState: S.optionalKey(PublicSafeRef),
  spendCapSats: S.optionalKey(
    S.Number.check(
      S.isInt(),
      S.isBetween({ minimum: 0, maximum: 1_000_000_000 }),
    ),
  ),
  statusUrl: S.optionalKey(PublicSafeRef),
  verifierPolicy: S.optionalKey(PublicSafeRef),
  workloadFamily: S.optionalKey(PublicSafeRef),
})
export type TrainingRunManifest = typeof TrainingRunManifest.Type

export const TrainingRunPlanRequest = S.Struct({
  manifest: S.optionalKey(TrainingRunManifest),
  maxAllowedStale: S.optionalKey(
    S.Number.check(S.isInt(), S.isBetween({ minimum: 1, maximum: 100_000 })),
  ),
  promiseRef: PublicSafeRef,
  receiptRefs: PublicSafeRefs,
  sealPublicationCadenceWindows: S.optionalKey(
    S.Number.check(S.isInt(), S.isBetween({ minimum: 1, maximum: 10_000 })),
  ),
  sourceRefs: PublicSafeRefs,
  trainingRunRef: S.optionalKey(PublicSafeRef),
})
export type TrainingRunPlanRequest = typeof TrainingRunPlanRequest.Type

export const TrainingRunTransitionRequest = S.Struct({
  actorRef: S.optionalKey(PublicSafeRef),
  receiptRef: PublicSafeRef,
})
export type TrainingRunTransitionRequest =
  typeof TrainingRunTransitionRequest.Type

export const TrainingWindowPlanRequest = S.Struct({
  datasetRefs: PublicSafeRefs,
  homeworkKind: S.optionalKey(TrainingWindowHomeworkKind),
  priority: S.optionalKey(S.Number),
  receiptRefs: PublicSafeRefs,
  sourceRefs: PublicSafeRefs,
  trainingRunRef: PublicSafeRef,
  windowRef: S.optionalKey(PublicSafeRef),
})
export type TrainingWindowPlanRequest = typeof TrainingWindowPlanRequest.Type

export const TrainingWindowTransitionRequest = S.Struct({
  actorRef: S.optionalKey(PublicSafeRef),
  receiptRef: PublicSafeRef,
  sealMetadata: S.optionalKey(TrainingWindowSealMetadata),
})
export type TrainingWindowTransitionRequest =
  typeof TrainingWindowTransitionRequest.Type

export const TrainingWindowLeaseClaimRequest = S.Struct({
  leaseSeconds: S.optionalKey(S.Number),
  pylonRef: PublicSafePylonRef,
  receiptRefs: PublicSafeRefs,
})
export type TrainingWindowLeaseClaimRequest =
  typeof TrainingWindowLeaseClaimRequest.Type

export type TrainingRunRecord = Readonly<{
  createdAt: string
  id: string
  manifest: TrainingRunManifest | null
  maxAllowedStale: number
  promiseRef: string
  publicProjectionJson: string
  receiptRefs: ReadonlyArray<string>
  sealInFlightAt: string | null
  sealPublicationCadenceWindows: number
  sourceRefs: ReadonlyArray<string>
  state: TrainingRunState
  trainingRunRef: string
  updatedAt: string
}>

export type TrainingWindowRecord = Readonly<{
  activatedAt: string | null
  datasetRefs: ReadonlyArray<string>
  homeworkKind: TrainingWindowHomeworkKind
  id: string
  plannedAt: string
  priority: number
  publicProjectionJson: string
  receiptRefs: ReadonlyArray<string>
  reconciledAt: string | null
  sealMetadata: TrainingWindowSealMetadata | null
  sealedAt: string | null
  sourceRefs: ReadonlyArray<string>
  state: TrainingWindowState
  trainingRunRef: string
  updatedAt: string
  windowRef: string
}>

export type TrainingWindowLeaseRecord = Readonly<{
  claimedAt: string
  id: string
  leaseExpiresAt: string
  leaseRef: string
  publicProjectionJson: string
  pylonRef: string
  receiptRefs: ReadonlyArray<string>
  state: 'active' | 'released'
  trainingRunRef: string
  windowRef: string
}>

export type TrainingWindowEventRecord = Readonly<{
  actorRef: string
  createdAt: string
  id: string
  receiptRef: string
  stateFrom: TrainingWindowState | null
  stateTo: TrainingWindowState
  transitionKind: string
  windowRef: string
}>

export type TrainingRunProjection = Readonly<{
  blockers: ReadonlyArray<string>
  createdAtDisplay: string
  generatedAt: string
  manifest: TrainingRunManifest | null
  manifestSettlementStateNote: string | null
  maxAllowedStale: number
  maxStalenessSeconds: number
  promiseRef: string
  receiptRefs: ReadonlyArray<string>
  sealInFlight: boolean
  sealPublicationCadenceWindows: number
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
  state: TrainingRunState
  trainingRunRef: string
  updatedAtDisplay: string
}>

export type TrainingWindowProjection = Readonly<{
  datasetRefs: ReadonlyArray<string>
  homeworkKind: TrainingWindowHomeworkKind
  plannedAtDisplay: string
  priority: number
  receiptRefs: ReadonlyArray<string>
  sealMetadata: TrainingWindowSealMetadata | null
  sourceRefs: ReadonlyArray<string>
  state: TrainingWindowState
  trainingRunRef: string
  updatedAtDisplay: string
  windowRef: string
}>

export type TrainingWindowLeaseProjection = Readonly<{
  claimedAtDisplay: string
  leaseExpiresInSeconds: number
  leaseRef: string
  pylonRef: string
  receiptRefs: ReadonlyArray<string>
  state: 'active' | 'released'
  trainingRunRef: string
  windowRef: string
}>

export type TrainingRunPublicMetric = Readonly<{
  provenanceLabel: string
  sourceRefs: ReadonlyArray<string>
  value: number
}>

/**
 * Run-level reconciled settlement status (openagents #5316, the public
 * reconciliation gap an independent contributor found). The static
 * `manifest.settlementState` is a one-time owner launch-gate seed (migration
 * `0185`) and never recomputes; if it stays `pending` while real settled
 * receipts exist, a contributor reading the run projection sees `pending` next
 * to a real settled total. This computed status reconciles the live truth from
 * the SAME provider-confirmed settled-receipt source that feeds
 * `metrics.providerConfirmedSettledPayoutSats`:
 *  - `none`     -> zero settled sats / zero settled receipts for this run.
 *  - `settling` -> at least one provider-confirmed settled receipt is linked to
 *                  this run. Honest mid-state, NOT a "fully settled" / launch
 *                  claim (that judgment stays the owner's launch-gate field).
 *
 * `launchManifestSettlementState` carries the static manifest value verbatim,
 * explicitly labeled so the two can never be confused. Evidence only: grants no
 * payout, settlement, accepted-work, or public-claim authority; never mutates
 * the manifest.
 */
export type TrainingRunReconciledSettlementState = 'none' | 'settling'

export type TrainingRunSettlementReconciliation = Readonly<{
  launchManifestSettlementState: string | null
  launchManifestSettlementStateLabel: string
  provenanceLabel: string
  reconciledState: TrainingRunReconciledSettlementState
  settledPayoutSats: number
  settledReceiptCount: number
  sourceRefs: ReadonlyArray<string>
}>

/**
 * The run-scoped Tassadar verified-trace corpus (openagents #5010, W2 in
 * docs/tassadar/RESEARCH_PLAN.md §5). Counts accepted, replay-verified
 * `exact_trace_replay` closed-tick traces tied to the run — the tetrahedron
 * acceptance predicate (intent + execution + state delta + evaluation all
 * closed; "closed ticks _are_ training records"). It rebuilds on
 * verification-challenge transitions, never on registration; carries only
 * public-safe trace/verdict refs; and is bounded evidence, not a Tassadar
 * exactness or model-capability claim.
 */
export type TrainingRunCorpusAccumulation = Readonly<{
  acceptedTraceCount: number
  laneRef: string
  provenanceLabel: string
  staleness: PublicProjectionStalenessContract
  traceRefs: ReadonlyArray<string>
  verdictRefs: ReadonlyArray<string>
}>

export type TrainingRunLossPoint = Readonly<{
  provenanceLabel: string
  sourceRefs: ReadonlyArray<string>
  step: number
  validationLoss: number
}>

export type TrainingRunLeaderboardRow = Readonly<{
  bestValidationLoss: number | null
  provenanceLabel: string
  pylonRef: string
  rank: number
  settledPayoutSats: number
  sourceRefs: ReadonlyArray<string>
  trainingRunRef: string
  verifiedWindowCount: number
}>

export type TrainingRunVerifiedReplayPair = Readonly<{
  challengeRef: string
  provenanceLabel: string
  sourceRefs: ReadonlyArray<string>
  validatorRef: string
  verdictRefs: ReadonlyArray<string>
  workerRef: string
}>

export type TrainingRunRejectedReplayPair = Readonly<{
  challengeRef: string
  failureCodes: ReadonlyArray<string>
  provenanceLabel: string
  sourceRefs: ReadonlyArray<string>
  validatorRef: string | null
  verdictRefs: ReadonlyArray<string>
  workerRef: string
}>

export type TrainingRunRealGradientStatus = Readonly<{
  closeoutRequirement: Readonly<{
    evalRef: string | null
    freivaldsCommitmentRefs: ReadonlyArray<string>
    gradientCloseoutRefs: ReadonlyArray<string>
    mergeRef: string | null
    provenanceLabel: string
    satisfied: boolean
  }>
  deviceRequirement: Readonly<{
    observedDistinctContributorDevices: number
    provenanceLabel: string
    requiredDistinctContributorDevices: number
    satisfied: boolean
    sourceRefs: ReadonlyArray<string>
  }>
  externalAsk: Readonly<{
    blockerRefs: ReadonlyArray<string>
    psionicLaneRef: string
    requirementRefs: ReadonlyArray<string>
    status: 'blocked_external' | 'ready' | 'observed'
  }>
  leaderboardRows: ReadonlyArray<TrainingRunLeaderboardRow>
  lossCurve: ReadonlyArray<TrainingRunLossPoint>
  lossUnderBudget: Readonly<{
    budgetLabel: string
    budgetRef: string | null
    finalValidationLoss: number | null
    maxValidationLoss: number | null
    provenanceLabel: string
    satisfied: boolean
    sourceRefs: ReadonlyArray<string>
  }>
  scopeBoundaryRefs: ReadonlyArray<string>
  rejectedReplayPairs: ReadonlyArray<TrainingRunRejectedReplayPair>
  verifiedReplayPairs: ReadonlyArray<TrainingRunVerifiedReplayPair>
}>

export type TrainingRunPublicSummary = Readonly<{
  copyBoundaryRefs: ReadonlyArray<string>
  corpus: TrainingRunCorpusAccumulation
  emptyState: Readonly<{
    idle: boolean
    reason: string
  }>
  metrics: Readonly<{
    activeWindowCount: TrainingRunPublicMetric
    assignedContributorCount: TrainingRunPublicMetric
    pendingPayoutCount: TrainingRunPublicMetric
    plannedWindowCount: TrainingRunPublicMetric
    providerConfirmedSettledPayoutSats: TrainingRunPublicMetric
    qualifiedContributorCount: TrainingRunPublicMetric
    receiptRefCount: TrainingRunPublicMetric
    reconciledWindowCount: TrainingRunPublicMetric
    rejectedWorkCount: TrainingRunPublicMetric
    sealedWindowCount: TrainingRunPublicMetric
    verifiedWorkCount: TrainingRunPublicMetric
  }>
  realGradient: TrainingRunRealGradientStatus
  receiptRefs: ReadonlyArray<string>
  run: TrainingRunProjection
  settlement: TrainingRunSettlementReconciliation
  sourceRefs: ReadonlyArray<string>
  windows: ReadonlyArray<TrainingWindowProjection>
}>

export type TrainingAuthorityStore = Readonly<{
  attachRunEvidence: (run: TrainingRunRecord) => Promise<TrainingRunRecord>
  // Run-level merge/seal barrier (Pluralis roadmap P1.3, openagents issue
  // #4851). beginRunSealBarrier raises the durable seal-in-flight marker
  // before the seal mutation is persisted and clearRunSealBarrier lowers it
  // after; while the marker is up, the dispatcher queues joiner bootstrap
  // grants and join-lifecycle transitions instead of handing out
  // half-updated state. The marker is durable on purpose: a Worker that
  // dies mid-seal leaves the barrier up rather than letting a joiner
  // bootstrap from an unverified seal.
  beginRunSealBarrier: (trainingRunRef: string, nowIso: string) => Promise<void>
  claimLease: (
    lease: TrainingWindowLeaseRecord,
    nowIso: string,
  ) => Promise<TrainingWindowLeaseRecord>
  clearRunSealBarrier: (trainingRunRef: string) => Promise<void>
  listClaimableWindows: (
    nowIso: string,
    limit: number,
  ) => Promise<ReadonlyArray<TrainingWindowRecord>>
  listRuns: (limit: number) => Promise<ReadonlyArray<TrainingRunRecord>>
  listVerificationChallengesForRun: (
    trainingRunRef: string,
    limit: number,
  ) => Promise<ReadonlyArray<TrainingVerificationChallengeRecord>>
  listWindowLeasesForRun: (
    trainingRunRef: string,
    limit: number,
  ) => Promise<ReadonlyArray<TrainingWindowLeaseRecord>>
  listWindowsForRun: (
    trainingRunRef: string,
    limit: number,
  ) => Promise<ReadonlyArray<TrainingWindowRecord>>
  planRun: (run: TrainingRunRecord) => Promise<TrainingRunRecord>
  planWindow: (window: TrainingWindowRecord) => Promise<TrainingWindowRecord>
  readRun: (trainingRunRef: string) => Promise<TrainingRunRecord | undefined>
  // Read a single claimed lease by ref (#5052). Used by the agent-gated
  // worker -> validator trace-completion routes to enforce lease ownership.
  readWindowLease: (
    leaseRef: string,
  ) => Promise<TrainingWindowLeaseRecord | undefined>
  transitionRun: (run: TrainingRunRecord) => Promise<TrainingRunRecord>
  readWindow: (windowRef: string) => Promise<TrainingWindowRecord | undefined>
  transitionWindow: (
    window: TrainingWindowRecord,
    event: TrainingWindowEventRecord,
  ) => Promise<TrainingWindowRecord>
}>

export class TrainingAuthorityStoreError extends S.TaggedErrorClass<TrainingAuthorityStoreError>()(
  'TrainingAuthorityStoreError',
  {
    kind: S.Literals([
      'conflict',
      'forbidden',
      'not_found',
      'storage_error',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

export type TrainingRunRow = Readonly<{
  created_at: string
  id: string
  manifest_json: string | null
  max_allowed_stale: number
  promise_ref: string
  public_projection_json: string
  receipt_refs_json: string
  seal_in_flight_at: string | null
  seal_publication_cadence_windows: number
  source_refs_json: string
  state: TrainingRunState
  training_run_ref: string
  updated_at: string
}>

export type TrainingWindowRow = Readonly<{
  activated_at: string | null
  dataset_refs_json: string
  homework_kind: TrainingWindowHomeworkKind
  id: string
  planned_at: string
  priority: number
  public_projection_json: string
  receipt_refs_json: string
  reconciled_at: string | null
  seal_metadata_json: string | null
  sealed_at: string | null
  source_refs_json: string
  state: TrainingWindowState
  training_run_ref: string
  updated_at: string
  window_ref: string
}>

export type TrainingWindowLeaseRow = Readonly<{
  claimed_at: string
  id: string
  lease_expires_at: string
  lease_ref: string
  public_projection_json: string
  pylon_ref: string
  receipt_refs_json: string
  state: 'active' | 'released'
  training_run_ref: string
  window_ref: string
}>

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const homeworkKindRank = (kind: TrainingWindowHomeworkKind): number =>
  kind === 'admin_dispatched_homework'
    ? 3
    : kind === 'operator_planned_homework'
      ? 2
      : 1

export const selectTrainingLeaseCandidate = (
  windows: ReadonlyArray<TrainingWindowRecord>,
): TrainingWindowRecord | undefined =>
  [...windows].sort((left, right) => {
    const rankDelta =
      homeworkKindRank(right.homeworkKind) - homeworkKindRank(left.homeworkKind)

    if (rankDelta !== 0) {
      return rankDelta
    }

    const priorityDelta = right.priority - left.priority

    if (priorityDelta !== 0) {
      return priorityDelta
    }

    return left.plannedAt.localeCompare(right.plannedAt)
  })[0]

const trainingRunProjectionStaleness = (): PublicProjectionStalenessContract =>
  liveAtReadStaleness([
    'training_run_state_transition_recorded',
    'training_window_state_transition_recorded',
    'training_run_evidence_attached',
  ])

// The corpus count rebuilds on verification-challenge verdict transitions, never
// on registration or heartbeat events (openagents #5010; RESEARCH_PLAN §6.3 +
// Standing Order 5; case law #4744-#4747).
const trainingRunCorpusStaleness = (): PublicProjectionStalenessContract =>
  liveAtReadStaleness([
    'training_verification_challenge_verified_transition_recorded',
    'training_verification_challenge_state_transition_recorded',
  ])

/**
 * Project the run's accumulating Tassadar verified-trace corpus from its
 * verification challenges (openagents #5010, W2). A closed-tick corpus record is
 * a `Verified` `exact_trace_replay` challenge tied to the run: its trace is the
 * work product/state delta and the Verified replay verdict is the evaluation
 * corner of the tetrahedron. Generic `verifiedWorkCount` spans all verification
 * classes; this counts only the exact-replay corpus.
 */
export const publicTrainingRunCorpusAccumulation = (
  challenges: ReadonlyArray<TrainingVerificationChallengeRecord>,
): TrainingRunCorpusAccumulation => {
  const corpusChallenges = challenges.filter(
    challenge =>
      challenge.state === 'Verified' &&
      challenge.verificationClass === 'exact_trace_replay',
  )

  return {
    acceptedTraceCount: corpusChallenges.length,
    laneRef: 'tassadar.verified_trace_corpus',
    provenanceLabel:
      'Accepted, replay-verified exact_trace_replay closed-tick traces tied to this run (Verified verdicts only). Excludes queued, leased, retrying, rejected, timed-out, and non-exact-replay verification, and every registration or heartbeat signal. Rebuilds on verification-challenge transitions; bounded verified-trace evidence, not a Tassadar exactness or model-capability claim.',
    staleness: trainingRunCorpusStaleness(),
    traceRefs: uniqueRefs(
      corpusChallenges.map(challenge => challenge.challengeRef),
    ),
    verdictRefs: uniqueRefs(
      corpusChallenges.flatMap(challenge => challenge.verdictRefs),
    ),
  }
}

export const publicTrainingRunProjection = (
  record: TrainingRunRecord,
  nowIso: string,
  extraBlockers: ReadonlyArray<string> = [],
): TrainingRunProjection => {
  const staleness = trainingRunProjectionStaleness()

  return {
    blockers: uniqueRefs([
      ...(record.manifest?.blockerRefs ?? []),
      ...extraBlockers,
    ]),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAt,
      nowIso,
    ),
    generatedAt: nowIso,
    manifest: record.manifest,
    manifestSettlementStateNote:
      typeof record.manifest?.settlementState === 'string'
        ? 'manifest.settlementState is the static owner launch-gate field (seeded once in migration 0185); it is NOT the live settled status. See summary.settlement.reconciledState and summary.metrics.providerConfirmedSettledPayoutSats for the live provider-confirmed settlement truth.'
        : null,
    maxAllowedStale: record.maxAllowedStale,
    maxStalenessSeconds: staleness.maxStalenessSeconds,
    promiseRef: record.promiseRef,
    receiptRefs: uniqueRefs(record.receiptRefs),
    sealInFlight: record.sealInFlightAt !== null,
    sealPublicationCadenceWindows: record.sealPublicationCadenceWindows,
    sourceRefs: uniqueRefs(record.sourceRefs),
    staleness,
    state: record.state,
    trainingRunRef: record.trainingRunRef,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAt,
      nowIso,
    ),
  }
}

/**
 * Run-level state transition (#5006). Allowed edges mirror the window
 * lifecycle: planned -> active -> sealed -> reconciled. The run row's CHECK
 * constraint enforces the same four states. Returns the next record with its
 * receipt appended and projection regenerated; the route persists it through
 * `transitionRun`.
 */
export const transitionTrainingRunRecord = (
  input: Readonly<{
    nextState: TrainingRunState
    nowIso: string
    receiptRef: string
    run: TrainingRunRecord
  }>,
): Readonly<{ run: TrainingRunRecord }> => {
  const allowed =
    (input.run.state === 'planned' && input.nextState === 'active') ||
    (input.run.state === 'active' && input.nextState === 'sealed') ||
    (input.run.state === 'sealed' && input.nextState === 'reconciled')

  if (!allowed) {
    throw new TrainingAuthorityStoreError({
      kind: 'conflict',
      reason: `Cannot transition training run from ${input.run.state} to ${input.nextState}.`,
    })
  }

  const nextRun: TrainingRunRecord = {
    ...input.run,
    receiptRefs: uniqueRefs([...input.run.receiptRefs, input.receiptRef]),
    state: input.nextState,
    updatedAt: input.nowIso,
  }

  return {
    run: {
      ...nextRun,
      publicProjectionJson: JSON.stringify(
        publicTrainingRunProjection(nextRun, input.nowIso),
      ),
    },
  }
}

/**
 * Append settlement (or other) receipt refs to a run without changing its
 * state, regenerating the public projection (openagents #5009). The run keeps
 * its current state; only `receiptRefs`, the projection, and `updatedAt` move.
 * The route persists the returned record through `transitionRun`, whose UPDATE
 * writes `receipt_refs_json` (unlike `attachRunEvidence`, which only rewrites
 * the projection JSON and would drop the appended refs on the next read).
 */
export const appendTrainingRunReceiptRefs = (
  input: Readonly<{
    nowIso: string
    receiptRefs: ReadonlyArray<string>
    run: TrainingRunRecord
  }>,
): Readonly<{ run: TrainingRunRecord }> => {
  const nextRun: TrainingRunRecord = {
    ...input.run,
    receiptRefs: uniqueRefs([...input.run.receiptRefs, ...input.receiptRefs]),
    updatedAt: input.nowIso,
  }

  return {
    run: {
      ...nextRun,
      publicProjectionJson: JSON.stringify(
        publicTrainingRunProjection(nextRun, input.nowIso),
      ),
    },
  }
}

export const publicTrainingWindowProjection = (
  record: TrainingWindowRecord,
  nowIso: string,
): TrainingWindowProjection => ({
  datasetRefs: uniqueRefs(record.datasetRefs),
  homeworkKind: record.homeworkKind,
  plannedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.plannedAt,
    nowIso,
  ),
  priority: record.priority,
  receiptRefs: uniqueRefs(record.receiptRefs),
  sealMetadata: record.sealMetadata,
  sourceRefs: uniqueRefs(record.sourceRefs),
  state: record.state,
  trainingRunRef: record.trainingRunRef,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.updatedAt,
    nowIso,
  ),
  windowRef: record.windowRef,
})

export const publicTrainingWindowLeaseProjection = (
  record: TrainingWindowLeaseRecord,
  nowIso: string,
): TrainingWindowLeaseProjection => ({
  claimedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.claimedAt,
    nowIso,
  ),
  leaseExpiresInSeconds: Math.max(
    0,
    Math.floor((Date.parse(record.leaseExpiresAt) - Date.parse(nowIso)) / 1000),
  ),
  leaseRef: record.leaseRef,
  pylonRef: record.pylonRef,
  receiptRefs: uniqueRefs(record.receiptRefs),
  state: record.state,
  trainingRunRef: record.trainingRunRef,
  windowRef: record.windowRef,
})

const metric = (
  value: number,
  provenanceLabel: string,
  sourceRefs: ReadonlyArray<string>,
): TrainingRunPublicMetric => ({
  provenanceLabel,
  sourceRefs: uniqueRefs(sourceRefs),
  value,
})

const distinctPylonRefs = (
  leases: ReadonlyArray<TrainingWindowLeaseRecord>,
): ReadonlyArray<string> => uniqueRefs(leases.map(lease => lease.pylonRef))

const qualifiedContributorRefs = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    settlementReceiptRefsByContributor: ReadonlyMap<
      string,
      ReadonlyArray<string>
    >
  }>,
): ReadonlyArray<string> => {
  const verifiedExactReplayWindowRefs = new Set(
    input.challenges.flatMap(challenge =>
      challenge.state === 'Verified' &&
      challenge.verificationClass === 'exact_trace_replay' &&
      challenge.windowRef !== null
        ? [challenge.windowRef]
        : [],
    ),
  )

  return distinctPylonRefs(
    input.leases.filter(lease =>
      verifiedExactReplayWindowRefs.has(lease.windowRef),
    ),
  ).filter(
    pylonRef =>
      (input.settlementReceiptRefsByContributor.get(pylonRef) ?? []).length > 0,
  )
}

const optionalNumber = (value: unknown): number | undefined => {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value
  }

  if (typeof value !== 'string' || value.trim() === '') {
    return undefined
  }

  const parsed = Number(value)

  return Number.isFinite(parsed) ? parsed : undefined
}

const realGradientEvidenceRecord = (
  run: TrainingRunRecord,
): Record<string, unknown> | undefined => {
  const projection = parseJsonRecord(run.publicProjectionJson)
  const nested = projection?.realGradient

  return isRecord(nested) ? nested : undefined
}

const lossCurveFromEvidence = (
  evidence: Record<string, unknown> | undefined,
): ReadonlyArray<TrainingRunLossPoint> => {
  const curve = evidence?.lossCurve

  if (!Array.isArray(curve)) {
    return []
  }

  return curve
    .flatMap((point): ReadonlyArray<TrainingRunLossPoint> => {
      if (!isRecord(point)) {
        return []
      }

      const step = optionalNumber(point.step)
      const validationLoss = optionalNumber(point.validationLoss)

      if (step === undefined || validationLoss === undefined) {
        return []
      }

      return [
        {
          provenanceLabel:
            optionalString(point.provenanceLabel) ??
            'Public-safe validation-loss point from run projection evidence.',
          sourceRefs: uniqueRefs(stringArrayFromUnknown(point.sourceRefs)),
          step,
          validationLoss,
        },
      ]
    })
    .sort((left, right) => left.step - right.step)
}

const bestValidationLoss = (
  points: ReadonlyArray<TrainingRunLossPoint>,
): number | null =>
  points.length === 0
    ? null
    : Math.min(...points.map(point => point.validationLoss))

const verifiedReplayPairsFromChallenges = (
  challenges: ReadonlyArray<TrainingVerificationChallengeRecord>,
): ReadonlyArray<TrainingRunVerifiedReplayPair> =>
  challenges
    .filter(
      challenge =>
        challenge.state === 'Verified' &&
        challenge.verificationClass === 'exact_trace_replay',
    )
    .flatMap(challenge => {
      const payload = parseJsonRecord(challenge.payloadJson)
      const workerRef =
        optionalString(payload?.pylonDeviceRef) ?? challenge.contributionRef
      const validatorRef =
        optionalString(payload?.validatorDeviceRef) ?? challenge.leasedToRef

      if (workerRef === null || validatorRef === null) {
        return []
      }

      return [
        {
          challengeRef: challenge.challengeRef,
          provenanceLabel:
            'Verified exact_trace_replay pair. The worker side is the public worker/device ref and the validator side is the public validator ref recorded on the challenge payload or lease/finalization path.',
          sourceRefs: uniqueRefs([
            challenge.challengeRef,
            ...(challenge.contributionRef === null
              ? []
              : [challenge.contributionRef]),
            workerRef,
            validatorRef,
            ...challenge.verdictRefs,
          ]),
          validatorRef,
          verdictRefs: uniqueRefs(challenge.verdictRefs),
          workerRef,
        },
      ]
    })

const rejectedReplayPairsFromChallenges = (
  challenges: ReadonlyArray<TrainingVerificationChallengeRecord>,
): ReadonlyArray<TrainingRunRejectedReplayPair> =>
  challenges
    .filter(
      challenge =>
        challenge.state === 'Rejected' &&
        challenge.verificationClass === 'exact_trace_replay',
    )
    .flatMap(challenge => {
      const payload = parseJsonRecord(challenge.payloadJson)
      const workerRef =
        optionalString(payload?.pylonDeviceRef) ?? challenge.contributionRef
      const validatorRef =
        optionalString(payload?.validatorDeviceRef) ?? challenge.leasedToRef

      if (workerRef === null) {
        return []
      }

      return [
        {
          challengeRef: challenge.challengeRef,
          failureCodes: uniqueRefs(challenge.failureCodes),
          provenanceLabel:
            'Rejected exact_trace_replay pair. Public refs identify the worker/device, validator when publishable, challenge, verdict refs, and public-safe failure-code refs without exposing raw traces or private logs.',
          sourceRefs: uniqueRefs([
            challenge.challengeRef,
            ...(challenge.contributionRef === null
              ? []
              : [challenge.contributionRef]),
            workerRef,
            ...(validatorRef === null ? [] : [validatorRef]),
            ...challenge.verdictRefs,
            ...challenge.failureCodes,
          ]),
          validatorRef,
          verdictRefs: uniqueRefs(challenge.verdictRefs),
          workerRef,
        },
      ]
    })

const publicRealGradientStatus = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    run: TrainingRunRecord
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): TrainingRunRealGradientStatus => {
  const evidence = realGradientEvidenceRecord(input.run)
  const lossCurve = lossCurveFromEvidence(evidence)
  const finalLoss =
    lossCurve.length === 0
      ? null
      : lossCurve[lossCurve.length - 1]!.validationLoss
  const maxLoss = optionalNumber(evidence?.maxValidationLoss) ?? null
  const mergeRef = optionalString(evidence?.mergeRef) ?? null
  const evalRef = optionalString(evidence?.evalRef) ?? null
  const freivaldsCommitmentRefs = uniqueRefs(
    stringArrayFromUnknown(evidence?.freivaldsCommitmentRefs),
  )
  const gradientCloseoutRefs = uniqueRefs(
    stringArrayFromUnknown(evidence?.gradientCloseoutRefs),
  )
  const observedPylonRefs = distinctPylonRefs(input.leases)
  const verifiedChallenges = input.challenges.filter(
    challenge => challenge.state === 'Verified',
  )
  const closeoutSatisfied =
    freivaldsCommitmentRefs.length > 0 &&
    gradientCloseoutRefs.length > 0 &&
    mergeRef !== null &&
    evalRef !== null
  const lossSatisfied =
    finalLoss !== null && maxLoss !== null && finalLoss <= maxLoss
  const observed =
    observedPylonRefs.length >= 2 &&
    closeoutSatisfied &&
    lossSatisfied &&
    verifiedChallenges.length > 0
  const blockerRefs = observed
    ? []
    : [
        'blocker.cs336_a1.real_gradient_psionic_lane_external',
        'blocker.cs336_a1.requires_two_real_contributor_devices',
        'blocker.cs336_a1.operator_funded_settled_payouts_required',
      ]
  const bestLoss = bestValidationLoss(lossCurve)
  const verifiedWindowRefs = new Set(
    verifiedChallenges
      .map(challenge => challenge.windowRef)
      .filter((ref): ref is string => ref !== null),
  )

  return {
    closeoutRequirement: {
      evalRef,
      freivaldsCommitmentRefs,
      gradientCloseoutRefs,
      mergeRef,
      provenanceLabel:
        'Real-gradient closeout requires public Freivalds commitment refs, gradient closeout refs, merge refs, and eval refs from the Psionic lane.',
      satisfied: closeoutSatisfied,
    },
    deviceRequirement: {
      observedDistinctContributorDevices: observedPylonRefs.length,
      provenanceLabel:
        'Distinct contributor devices are counted from Worker D1 training_window_leases pylon_ref values; loopback/operator-only runs do not satisfy this issue.',
      requiredDistinctContributorDevices: 2,
      satisfied: observedPylonRefs.length >= 2,
      sourceRefs: input.leases.map(lease => lease.leaseRef),
    },
    externalAsk: {
      blockerRefs,
      psionicLaneRef:
        optionalString(evidence?.psionicLaneRef) ??
        'psion_cs336_a1_real_gradient_v1',
      requirementRefs: [
        'requirement.psionic.cs336_a1.real_gradient_training_lane',
        'requirement.psionic.cs336_a1.tinystories_owt_shards',
        'requirement.psionic.cs336_a1.freivalds_gradient_commitments',
        'requirement.psionic.cs336_a1.merge_eval_loss_curve_receipts',
      ],
      status: observed
        ? 'observed'
        : closeoutSatisfied
          ? 'ready'
          : 'blocked_external',
    },
    leaderboardRows: observedPylonRefs.map((pylonRef, index) => ({
      bestValidationLoss: bestLoss,
      provenanceLabel:
        'Public leaderboard row derived from D1 lease refs, verified challenge refs, and optional run projection loss evidence; settled sats stay zero until provider-confirmed settlement receipts are linked.',
      pylonRef,
      rank: index + 1,
      settledPayoutSats: 0,
      sourceRefs: uniqueRefs([
        ...input.leases
          .filter(lease => lease.pylonRef === pylonRef)
          .map(lease => lease.leaseRef),
        ...verifiedChallenges.map(challenge => challenge.challengeRef),
      ]),
      trainingRunRef: input.run.trainingRunRef,
      verifiedWindowCount: input.windows.filter(window =>
        verifiedWindowRefs.has(window.windowRef),
      ).length,
    })),
    lossCurve,
    lossUnderBudget: {
      budgetLabel:
        optionalString(evidence?.budgetLabel) ??
        'CS336 A1 validation loss under bounded compute budget.',
      budgetRef: optionalString(evidence?.budgetRef) ?? null,
      finalValidationLoss: finalLoss,
      maxValidationLoss: maxLoss,
      provenanceLabel:
        'Loss-under-budget is true only when the public run projection includes a final validation loss at or below the declared maxValidationLoss.',
      satisfied: lossSatisfied,
      sourceRefs: uniqueRefs([
        ...lossCurve.flatMap(point => point.sourceRefs),
        ...stringArrayFromUnknown(evidence?.lossSourceRefs),
      ]),
    },
    scopeBoundaryRefs: [
      'scope.cs336_a1.bounded_multi_device_training_evidence_only',
      'scope.cs336_a1.does_not_replace_qwen_finetune_gate_4670',
      'scope.cs336_a1.no_first_real_training_run_green_copy_from_this_issue_alone',
    ],
    rejectedReplayPairs: rejectedReplayPairsFromChallenges(input.challenges),
    verifiedReplayPairs: verifiedReplayPairsFromChallenges(input.challenges),
  }
}

export const publicTrainingRunSummary = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    nowIso: string
    run: TrainingRunRecord
    // Provider-confirmed settled sats keyed by receipt ref (openagents #5009).
    // Only refs already linked to this run/windows/leases/challenges are summed,
    // so a settlement that is not dereferenceable from this run contributes 0.
    settledSatsByReceiptRef?: ReadonlyMap<string, number>
    settlementReceiptRefsByContributor?: ReadonlyMap<
      string,
      ReadonlyArray<string>
    >
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): TrainingRunPublicSummary => {
  const windowProjections = input.windows.map(window =>
    publicTrainingWindowProjection(window, input.nowIso),
  )
  const receiptRefs = uniqueRefs([
    ...input.run.receiptRefs,
    ...input.windows.flatMap(window => window.receiptRefs),
    ...input.leases.flatMap(lease => lease.receiptRefs),
    ...input.challenges.flatMap(challenge => challenge.verdictRefs),
  ])
  const sourceRefs = uniqueRefs([
    ...input.run.sourceRefs,
    ...input.windows.flatMap(window => window.sourceRefs),
    'route:/api/training/runs',
    `route:/api/training/runs/${input.run.trainingRunRef}`,
  ])
  const windowMetricRefs = [
    `training.run.${input.run.trainingRunRef}.windows`,
    ...input.windows.map(window => window.windowRef),
  ]
  const challengeMetricRefs = [
    `training.run.${input.run.trainingRunRef}.verification_challenges`,
    ...input.challenges.map(challenge => challenge.challengeRef),
  ]
  const settledSatsByReceiptRef = input.settledSatsByReceiptRef
  const providerConfirmedSettledPayoutSats = receiptRefs.reduce(
    (total, ref) => total + (settledSatsByReceiptRef?.get(ref) ?? 0),
    0,
  )
  const settlementReceiptRefsByContributor =
    input.settlementReceiptRefsByContributor ??
    new Map<string, ReadonlyArray<string>>()
  const qualifiedContributors = qualifiedContributorRefs({
    challenges: input.challenges,
    leases: input.leases,
    settlementReceiptRefsByContributor,
  })
  const payoutMetricRefs = [
    `training.run.${input.run.trainingRunRef}.provider_confirmed_settlements`,
  ]
  const empty =
    input.windows.length === 0 &&
    input.leases.length === 0 &&
    input.challenges.length === 0
  const settledReceiptCount = receiptRefs.reduce(
    (count, ref) => count + ((settledSatsByReceiptRef?.get(ref) ?? 0) > 0 ? 1 : 0),
    0,
  )
  const reconciledSettlementState: TrainingRunReconciledSettlementState =
    providerConfirmedSettledPayoutSats > 0 && settledReceiptCount > 0
      ? 'settling'
      : 'none'
  const launchManifestSettlementState =
    typeof input.run.manifest?.settlementState === 'string'
      ? input.run.manifest.settlementState
      : null

  return {
    copyBoundaryRefs: [
      'copy.public.training.run_page.provenance_labeled_numbers',
      'copy.public.training.no_pending_as_paid',
      'copy.public.training.no_unbounded_model_training_claim',
    ],
    corpus: publicTrainingRunCorpusAccumulation(input.challenges),
    emptyState: {
      idle: empty,
      reason: empty
        ? 'No Worker-authoritative windows, leases, verification challenges, or provider-confirmed settlements are recorded for this run yet.'
        : 'Worker-authoritative run data is present.',
    },
    metrics: {
      activeWindowCount: metric(
        input.windows.filter(window => window.state === 'active').length,
        'Worker D1 training_windows rows with state active.',
        windowMetricRefs,
      ),
      assignedContributorCount: metric(
        distinctPylonRefs(input.leases).length,
        'Distinct pylon_ref values from Worker D1 training_window_leases rows.',
        input.leases.map(lease => lease.leaseRef),
      ),
      pendingPayoutCount: metric(
        0,
        'No pending payout rows are counted as paid on the public run page.',
        payoutMetricRefs,
      ),
      plannedWindowCount: metric(
        input.windows.filter(window => window.state === 'planned').length,
        'Worker D1 training_windows rows with state planned.',
        windowMetricRefs,
      ),
      providerConfirmedSettledPayoutSats: metric(
        providerConfirmedSettledPayoutSats,
        'Sum of provider-confirmed REAL-BITCOIN settlement receipts (receiptKind settlement_recorded, state settled, realBitcoinMoved true) linked to this run only; pending, offered, claimed, wallet-side, and settled-state SIMULATION receipts (realBitcoinMoved false) are excluded. This is real bitcoin actually moved, not a settled-state count.',
        payoutMetricRefs,
      ),
      qualifiedContributorCount: metric(
        qualifiedContributors.length,
        'Qualified contributor count equals admitted contributors with accepted, replay-verified useful work and public-safe provider-confirmed settlement receipt refs linked to this run. It is derived from Worker D1 verified exact_trace_replay challenges joined to run leases plus provider-confirmed settled receipt projections; raw registrations and stale heartbeats never count.',
        uniqueRefs([
          ...qualifiedContributors,
          ...qualifiedContributors.flatMap(
            pylonRef => settlementReceiptRefsByContributor.get(pylonRef) ?? [],
          ),
        ]),
      ),
      receiptRefCount: metric(
        receiptRefs.length,
        'Public-safe receipt and verdict refs linked to the run, windows, leases, or verification challenges.',
        receiptRefs,
      ),
      reconciledWindowCount: metric(
        input.windows.filter(window => window.state === 'reconciled').length,
        'Worker D1 training_windows rows with state reconciled.',
        windowMetricRefs,
      ),
      rejectedWorkCount: metric(
        input.challenges.filter(challenge => challenge.state === 'Rejected')
          .length,
        'Worker D1 training_verification_challenges rows with state Rejected.',
        challengeMetricRefs,
      ),
      sealedWindowCount: metric(
        input.windows.filter(window => window.state === 'sealed').length,
        'Worker D1 training_windows rows with state sealed.',
        windowMetricRefs,
      ),
      verifiedWorkCount: metric(
        input.challenges.filter(challenge => challenge.state === 'Verified')
          .length,
        'Worker D1 training_verification_challenges rows with state Verified.',
        challengeMetricRefs,
      ),
    },
    realGradient: publicRealGradientStatus(input),
    receiptRefs,
    run: publicTrainingRunProjection(
      input.run,
      input.nowIso,
      input.run.state === 'planned' &&
        input.windows.some(window => window.state === 'reconciled')
        ? [TrainingRunPlannedWithReconciledWindowsBlocker]
        : [],
    ),
    settlement: {
      launchManifestSettlementState,
      launchManifestSettlementStateLabel:
        'Static owner launch-gate field seeded once in the run manifest (migration 0185); it is NOT the live settled status and does not recompute. Read `reconciledState`/`settledPayoutSats` for the live provider-confirmed settlement truth.',
      provenanceLabel:
        'Reconciled from the same provider-confirmed REAL-BITCOIN settlement receipts (settlement_recorded, state settled, realBitcoinMoved true; settled-state simulation receipts excluded) that feed metrics.providerConfirmedSettledPayoutSats: `none` when zero real settled receipts, `settling` when one or more exist. Evidence only; not a fully-settled or launch claim.',
      reconciledState: reconciledSettlementState,
      settledPayoutSats: providerConfirmedSettledPayoutSats,
      settledReceiptCount,
      sourceRefs: payoutMetricRefs,
    },
    sourceRefs,
    windows: windowProjections,
  }
}

export const buildTrainingRunRecord = (
  input: Readonly<{
    makeId: () => string
    nowIso: string
    request: TrainingRunPlanRequest
  }>,
): TrainingRunRecord => {
  const id = input.makeId()
  const record: TrainingRunRecord = {
    createdAt: input.nowIso,
    id: `training_run_${id}`,
    manifest: input.request.manifest ?? null,
    maxAllowedStale:
      input.request.maxAllowedStale ?? DefaultMaxAllowedStaleSteps,
    promiseRef: input.request.promiseRef,
    publicProjectionJson: '{}',
    receiptRefs: uniqueRefs(input.request.receiptRefs),
    sealInFlightAt: null,
    sealPublicationCadenceWindows:
      input.request.sealPublicationCadenceWindows ??
      DefaultSealPublicationCadenceWindows,
    sourceRefs: uniqueRefs(input.request.sourceRefs),
    state: 'planned',
    trainingRunRef: input.request.trainingRunRef ?? `training.run.${id}`,
    updatedAt: input.nowIso,
  }

  return {
    ...record,
    publicProjectionJson: JSON.stringify(
      publicTrainingRunProjection(record, input.nowIso),
    ),
  }
}

export const buildTrainingWindowRecord = (
  input: Readonly<{
    makeId: () => string
    nowIso: string
    request: TrainingWindowPlanRequest
  }>,
): TrainingWindowRecord => {
  const id = input.makeId()
  const record: TrainingWindowRecord = {
    activatedAt: null,
    datasetRefs: uniqueRefs(input.request.datasetRefs),
    homeworkKind: input.request.homeworkKind ?? 'operator_planned_homework',
    id: `training_window_${id}`,
    plannedAt: input.nowIso,
    priority: Math.trunc(input.request.priority ?? 0),
    publicProjectionJson: '{}',
    receiptRefs: uniqueRefs(input.request.receiptRefs),
    reconciledAt: null,
    sealMetadata: null,
    sealedAt: null,
    sourceRefs: uniqueRefs(input.request.sourceRefs),
    state: 'planned',
    trainingRunRef: input.request.trainingRunRef,
    updatedAt: input.nowIso,
    windowRef: input.request.windowRef ?? `training.window.${id}`,
  }

  return {
    ...record,
    publicProjectionJson: JSON.stringify(
      publicTrainingWindowProjection(record, input.nowIso),
    ),
  }
}

const sealValidationError = (reason: string): TrainingAuthorityStoreError =>
  new TrainingAuthorityStoreError({ kind: 'validation_error', reason })

const requireNonNegativeStaleInteger = (value: number, label: string): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw sealValidationError(`${label} must be a non-negative integer.`)
  }
}

const requireNonNegativeFinite = (value: number, label: string): void => {
  if (!Number.isFinite(value) || value < 0) {
    throw sealValidationError(`${label} must be a non-negative finite number.`)
  }
}

export const assertValidTrainingWindowSealMetadata = (
  metadata: TrainingWindowSealMetadata,
): void => {
  const staleness = metadata.staleness
  requireNonNegativeStaleInteger(
    staleness.contributionCount,
    'staleness.contributionCount',
  )
  requireNonNegativeStaleInteger(
    staleness.stepsBehindMin,
    'staleness.stepsBehindMin',
  )
  requireNonNegativeStaleInteger(
    staleness.stepsBehindMax,
    'staleness.stepsBehindMax',
  )
  requireNonNegativeFinite(staleness.stepsBehindP50, 'staleness.stepsBehindP50')
  requireNonNegativeFinite(staleness.stepsBehindP90, 'staleness.stepsBehindP90')

  if (
    staleness.stepsBehindMin > staleness.stepsBehindP50 ||
    staleness.stepsBehindP50 > staleness.stepsBehindP90 ||
    staleness.stepsBehindP90 > staleness.stepsBehindMax
  ) {
    throw sealValidationError(
      'Staleness distribution must satisfy min <= p50 <= p90 <= max.',
    )
  }

  const contributions = staleness.contributions ?? []

  if (contributions.length > MaxTrainingWindowSealContributionEntries) {
    throw sealValidationError(
      `staleness.contributions is bounded to ${MaxTrainingWindowSealContributionEntries} entries.`,
    )
  }

  if (contributions.length > staleness.contributionCount) {
    throw sealValidationError(
      'staleness.contributions cannot exceed staleness.contributionCount.',
    )
  }

  contributions.forEach(contribution => {
    requireNonNegativeStaleInteger(
      contribution.stepsBehind,
      `staleness.contributions[${contribution.contributionRef}].stepsBehind`,
    )

    if (
      contribution.stepsBehind < staleness.stepsBehindMin ||
      contribution.stepsBehind > staleness.stepsBehindMax
    ) {
      throw sealValidationError(
        'Per-contribution stepsBehind must lie within the declared min/max distribution bounds.',
      )
    }
  })

  if (
    staleness.contributionCount === 0 &&
    (staleness.stepsBehindMin !== 0 ||
      staleness.stepsBehindP50 !== 0 ||
      staleness.stepsBehindP90 !== 0 ||
      staleness.stepsBehindMax !== 0 ||
      contributions.length > 0)
  ) {
    throw sealValidationError(
      'An empty staleness distribution must report all-zero steps-behind values.',
    )
  }

  const churn = metadata.churn
  requireNonNegativeStaleInteger(churn.joinCount, 'churn.joinCount')
  requireNonNegativeStaleInteger(churn.lossCount, 'churn.lossCount')
  requireNonNegativeStaleInteger(
    churn.standbyPromotionCount,
    'churn.standbyPromotionCount',
  )

  const churnEvents = churn.events ?? []

  if (churnEvents.length > MaxTrainingWindowSealChurnEventEntries) {
    throw sealValidationError(
      `churn.events is bounded to ${MaxTrainingWindowSealChurnEventEntries} entries.`,
    )
  }

  const declaredCountForKind: Record<TrainingWindowChurnEventKind, number> = {
    join: churn.joinCount,
    loss: churn.lossCount,
    standby_promotion: churn.standbyPromotionCount,
  }
  const sampledCountByKind = churnEvents.reduce<
    Record<TrainingWindowChurnEventKind, number>
  >((counts, event) => ({ ...counts, [event.kind]: counts[event.kind] + 1 }), {
    join: 0,
    loss: 0,
    standby_promotion: 0,
  })

  TrainingWindowChurnEventKind.literals.forEach(kind => {
    if (sampledCountByKind[kind] > declaredCountForKind[kind]) {
      throw sealValidationError(
        `churn.events carries more ${kind} refs than the declared ${kind} count.`,
      )
    }
  })

  const fraction = metadata.verificationOverhead.fraction

  if (!Number.isFinite(fraction) || fraction < 0 || fraction > 1) {
    throw sealValidationError(
      'verificationOverhead.fraction must be a number between 0 and 1.',
    )
  }

  const checkpointDigestRef = metadata.checkpointDigestRef
  const durableCheckpointSeal = metadata.durableCheckpointSeal

  if (
    checkpointDigestRef !== undefined &&
    (checkpointDigestRef.length < 3 ||
      checkpointDigestRef.length > 260 ||
      !TrainingPublicSafeRefPattern.test(checkpointDigestRef))
  ) {
    throw sealValidationError('checkpointDigestRef must be a public-safe ref.')
  }

  if (checkpointDigestRef !== undefined && durableCheckpointSeal === undefined) {
    throw sealValidationError(
      'checkpointDigestRef requires a durableCheckpointSeal descriptor.',
    )
  }

  if (durableCheckpointSeal !== undefined) {
    if (checkpointDigestRef === undefined) {
      throw sealValidationError(
        'durableCheckpointSeal requires checkpointDigestRef.',
      )
    }
    if (durableCheckpointSeal.checkpointDigestRef !== checkpointDigestRef) {
      throw sealValidationError(
        'durableCheckpointSeal.checkpointDigestRef must match checkpointDigestRef.',
      )
    }

    const gate = evaluateDurableCheckpointSeal(durableCheckpointSeal)
    if (!gate.durable) {
      throw sealValidationError(
        `durableCheckpointSeal must pass durable checkpoint evaluation before sealing. Reasons: ${gate.reasons.join(', ') || 'unknown'}.`,
      )
    }
  }
}

export const transitionTrainingWindowRecord = (
  input: Readonly<{
    actorRef: string
    eventId: string
    nextState: TrainingWindowState
    nowIso: string
    receiptRef: string
    sealMetadata?: TrainingWindowSealMetadata | undefined
    transitionKind: string
    window: TrainingWindowRecord
  }>,
): Readonly<{
  event: TrainingWindowEventRecord
  window: TrainingWindowRecord
}> => {
  const allowed =
    (input.window.state === 'planned' && input.nextState === 'active') ||
    (input.window.state === 'active' && input.nextState === 'sealed') ||
    (input.window.state === 'sealed' && input.nextState === 'reconciled')

  if (!allowed) {
    throw new TrainingAuthorityStoreError({
      kind: 'conflict',
      reason: `Cannot transition training window from ${input.window.state} to ${input.nextState}.`,
    })
  }

  if (input.sealMetadata !== undefined && input.nextState !== 'sealed') {
    throw sealValidationError(
      'Seal metadata is only accepted on the seal transition.',
    )
  }

  if (input.sealMetadata !== undefined) {
    assertValidTrainingWindowSealMetadata(input.sealMetadata)
    if (
      input.sealMetadata.durableCheckpointSeal !== undefined &&
      input.sealMetadata.durableCheckpointSeal.windowRef !==
        input.window.windowRef
    ) {
      throw sealValidationError(
        'durableCheckpointSeal.windowRef must match the sealed windowRef.',
      )
    }
  }

  const nextWindow: TrainingWindowRecord = {
    ...input.window,
    activatedAt:
      input.nextState === 'active' ? input.nowIso : input.window.activatedAt,
    receiptRefs: uniqueRefs([...input.window.receiptRefs, input.receiptRef]),
    reconciledAt:
      input.nextState === 'reconciled'
        ? input.nowIso
        : input.window.reconciledAt,
    sealMetadata:
      input.nextState === 'sealed'
        ? (input.sealMetadata ?? null)
        : input.window.sealMetadata,
    sealedAt:
      input.nextState === 'sealed' ? input.nowIso : input.window.sealedAt,
    state: input.nextState,
    updatedAt: input.nowIso,
  }

  return {
    event: {
      actorRef: input.actorRef,
      createdAt: input.nowIso,
      id: `training_window_event_${input.eventId}`,
      receiptRef: input.receiptRef,
      stateFrom: input.window.state,
      stateTo: input.nextState,
      transitionKind: input.transitionKind,
      windowRef: input.window.windowRef,
    },
    window: {
      ...nextWindow,
      publicProjectionJson: JSON.stringify(
        publicTrainingWindowProjection(nextWindow, input.nowIso),
      ),
    },
  }
}

const leaseSecondsForRequest = (
  request: TrainingWindowLeaseClaimRequest,
): number => {
  const leaseSeconds = request.leaseSeconds ?? 15 * 60

  if (
    !Number.isFinite(leaseSeconds) ||
    leaseSeconds < 60 ||
    leaseSeconds > 86_400
  ) {
    throw new TrainingAuthorityStoreError({
      kind: 'validation_error',
      reason: 'leaseSeconds must be between 60 and 86400.',
    })
  }

  return Math.floor(leaseSeconds)
}

export const buildTrainingWindowLeaseRecord = (
  input: Readonly<{
    makeId: () => string
    nowIso: string
    request: TrainingWindowLeaseClaimRequest
    window: TrainingWindowRecord
  }>,
): TrainingWindowLeaseRecord => {
  const id = input.makeId()
  const record: TrainingWindowLeaseRecord = {
    claimedAt: input.nowIso,
    id: `training_window_lease_${id}`,
    leaseExpiresAt: isoTimestampAfterIso(
      input.nowIso,
      leaseSecondsForRequest(input.request) * 1000,
    ),
    leaseRef: `training.lease.${id}`,
    publicProjectionJson: '{}',
    pylonRef: input.request.pylonRef,
    receiptRefs: uniqueRefs(input.request.receiptRefs),
    state: 'active',
    trainingRunRef: input.window.trainingRunRef,
    windowRef: input.window.windowRef,
  }

  return {
    ...record,
    publicProjectionJson: JSON.stringify(
      publicTrainingWindowLeaseProjection(record, input.nowIso),
    ),
  }
}

export const trainingAuthorityStoreErrorFromUnknown = (
  error: unknown,
): TrainingAuthorityStoreError => {
  if (error instanceof TrainingAuthorityStoreError) {
    return error
  }
  // Tagged errors (e.g. NexusTreasuryPayoutLedgerStorageError) carry their
  // detail in `reason`/`operation` fields rather than `Error.message`, which is
  // empty. Prefer those so storage failures are not surfaced blank.
  const detail =
    typeof error === 'object' && error !== null
      ? [
          (error as { operation?: unknown }).operation,
          (error as { reason?: unknown }).reason,
        ]
          .filter(
            (part): part is string => typeof part === 'string' && part !== '',
          )
          .join(': ')
      : ''
  const reason =
    detail !== ''
      ? detail
      : error instanceof Error && error.message !== ''
        ? error.message
        : String(error)
  return new TrainingAuthorityStoreError({ kind: 'storage_error', reason })
}

const decodeTrainingWindowSealMetadataOption = S.decodeUnknownOption(
  TrainingWindowSealMetadata,
)

const decodeTrainingRunManifestOption =
  S.decodeUnknownOption(TrainingRunManifest)

const manifestFromJson = (value: string | null): TrainingRunManifest | null => {
  const record = parseJsonRecord(value)

  return record === undefined
    ? null
    : Option.getOrNull(decodeTrainingRunManifestOption(record))
}

const sealMetadataFromJson = (
  value: string | null,
): TrainingWindowSealMetadata | null => {
  const record = parseJsonRecord(value)

  return record === undefined
    ? null
    : Option.getOrNull(decodeTrainingWindowSealMetadataOption(record))
}

export const rowToTrainingRun = (row: TrainingRunRow): TrainingRunRecord => ({
  createdAt: row.created_at,
  id: row.id,
  manifest: manifestFromJson(row.manifest_json ?? null),
  maxAllowedStale: row.max_allowed_stale ?? DefaultMaxAllowedStaleSteps,
  promiseRef: row.promise_ref,
  publicProjectionJson: row.public_projection_json,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  sealInFlightAt: row.seal_in_flight_at ?? null,
  sealPublicationCadenceWindows:
    row.seal_publication_cadence_windows ??
    DefaultSealPublicationCadenceWindows,
  sourceRefs: parseJsonStringArray(row.source_refs_json),
  state: row.state,
  trainingRunRef: row.training_run_ref,
  updatedAt: row.updated_at,
})

export const rowToTrainingWindow = (
  row: TrainingWindowRow,
): TrainingWindowRecord => ({
  activatedAt: row.activated_at,
  datasetRefs: parseJsonStringArray(row.dataset_refs_json),
  homeworkKind: row.homework_kind,
  id: row.id,
  plannedAt: row.planned_at,
  priority: row.priority,
  publicProjectionJson: row.public_projection_json,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  reconciledAt: row.reconciled_at,
  sealMetadata: sealMetadataFromJson(row.seal_metadata_json),
  sealedAt: row.sealed_at,
  sourceRefs: parseJsonStringArray(row.source_refs_json),
  state: row.state,
  trainingRunRef: row.training_run_ref,
  updatedAt: row.updated_at,
  windowRef: row.window_ref,
})

export const rowToTrainingWindowLease = (
  row: TrainingWindowLeaseRow,
): TrainingWindowLeaseRecord => ({
  claimedAt: row.claimed_at,
  id: row.id,
  leaseExpiresAt: row.lease_expires_at,
  leaseRef: row.lease_ref,
  publicProjectionJson: row.public_projection_json,
  pylonRef: row.pylon_ref,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  state: row.state,
  trainingRunRef: row.training_run_ref,
  windowRef: row.window_ref,
})

export const makeD1TrainingAuthorityStore = (
  db: D1Database,
): TrainingAuthorityStore => ({
  attachRunEvidence: async run => {
    const result = await db
      .prepare(
        `UPDATE training_runs
            SET public_projection_json = ?, updated_at = ?
          WHERE training_run_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(run.publicProjectionJson, run.updatedAt, run.trainingRunRef)
      .run()

    if ((result.meta?.changes ?? 0) === 0) {
      throw new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found for evidence attachment.',
      })
    }

    return run
  },
  beginRunSealBarrier: async (trainingRunRef, nowIso) => {
    const result = await db
      .prepare(
        `UPDATE training_runs
            SET seal_in_flight_at = ?
          WHERE training_run_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(nowIso, trainingRunRef)
      .run()

    if ((result.meta?.changes ?? 0) === 0) {
      throw new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found for seal barrier.',
      })
    }
  },
  claimLease: async lease => {
    await db
      .prepare(
        `INSERT INTO training_window_leases
          (id, lease_ref, window_ref, training_run_ref, pylon_ref, state,
           receipt_refs_json, public_projection_json, claimed_at,
           lease_expires_at, archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        lease.id,
        lease.leaseRef,
        lease.windowRef,
        lease.trainingRunRef,
        lease.pylonRef,
        lease.state,
        JSON.stringify(lease.receiptRefs),
        lease.publicProjectionJson,
        lease.claimedAt,
        lease.leaseExpiresAt,
      )
      .run()

    return lease
  },
  clearRunSealBarrier: async trainingRunRef => {
    await db
      .prepare(
        `UPDATE training_runs
            SET seal_in_flight_at = NULL
          WHERE training_run_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(trainingRunRef)
      .run()
  },
  listClaimableWindows: async (nowIso, limit) => {
    const result = await db
      .prepare(
        `SELECT w.*
           FROM training_windows w
          WHERE w.state = 'active'
            AND w.archived_at IS NULL
            AND NOT EXISTS (
              SELECT 1
                FROM training_window_leases l
               WHERE l.window_ref = w.window_ref
                 AND l.state = 'active'
                 AND l.lease_expires_at > ?
                 AND l.archived_at IS NULL
            )
          ORDER BY
            CASE w.homework_kind
              WHEN 'admin_dispatched_homework' THEN 3
              WHEN 'operator_planned_homework' THEN 2
              ELSE 1
            END DESC,
            w.priority DESC,
            w.planned_at ASC
          LIMIT ?`,
      )
      .bind(nowIso, limit)
      .all<TrainingWindowRow>()

    return (result.results ?? []).map(rowToTrainingWindow)
  },
  listRuns: async limit => {
    const result = await db
      .prepare(
        `SELECT *
           FROM training_runs
          WHERE archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<TrainingRunRow>()

    return (result.results ?? []).map(rowToTrainingRun)
  },
  listVerificationChallengesForRun: async (trainingRunRef, limit) => {
    const result = await db
      .prepare(
        `SELECT *
           FROM training_verification_challenges
          WHERE training_run_ref = ?
            AND archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(trainingRunRef, limit)
      .all<TrainingVerificationRow>()

    return (result.results ?? []).map(rowToTrainingVerificationChallenge)
  },
  listWindowLeasesForRun: async (trainingRunRef, limit) => {
    const result = await db
      .prepare(
        `SELECT *
           FROM training_window_leases
          WHERE training_run_ref = ?
            AND archived_at IS NULL
          ORDER BY claimed_at DESC
          LIMIT ?`,
      )
      .bind(trainingRunRef, limit)
      .all<TrainingWindowLeaseRow>()

    return (result.results ?? []).map(rowToTrainingWindowLease)
  },
  readWindowLease: async leaseRef => {
    const row = await db
      .prepare(
        `SELECT *
           FROM training_window_leases
          WHERE lease_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(leaseRef)
      .first<TrainingWindowLeaseRow>()

    return row === null ? undefined : rowToTrainingWindowLease(row)
  },
  listWindowsForRun: async (trainingRunRef, limit) => {
    const result = await db
      .prepare(
        `SELECT *
           FROM training_windows
          WHERE training_run_ref = ?
            AND archived_at IS NULL
          ORDER BY planned_at DESC
          LIMIT ?`,
      )
      .bind(trainingRunRef, limit)
      .all<TrainingWindowRow>()

    return (result.results ?? []).map(rowToTrainingWindow)
  },
  planRun: async run => {
    await db
      .prepare(
        `INSERT INTO training_runs
          (id, training_run_ref, promise_ref, state, max_allowed_stale,
           seal_publication_cadence_windows, seal_in_flight_at, manifest_json,
           source_refs_json, receipt_refs_json, public_projection_json,
           created_at, updated_at, archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        run.id,
        run.trainingRunRef,
        run.promiseRef,
        run.state,
        run.maxAllowedStale,
        run.sealPublicationCadenceWindows,
        run.sealInFlightAt,
        run.manifest === null ? null : JSON.stringify(run.manifest),
        JSON.stringify(run.sourceRefs),
        JSON.stringify(run.receiptRefs),
        run.publicProjectionJson,
        run.createdAt,
        run.updatedAt,
      )
      .run()

    return run
  },
  planWindow: async window => {
    await db
      .prepare(
        `INSERT INTO training_windows
          (id, window_ref, training_run_ref, state, homework_kind, priority,
           dataset_refs_json, source_refs_json, receipt_refs_json,
           seal_metadata_json, public_projection_json, planned_at,
           activated_at, sealed_at, reconciled_at, updated_at, archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        window.id,
        window.windowRef,
        window.trainingRunRef,
        window.state,
        window.homeworkKind,
        window.priority,
        JSON.stringify(window.datasetRefs),
        JSON.stringify(window.sourceRefs),
        JSON.stringify(window.receiptRefs),
        window.sealMetadata === null
          ? null
          : JSON.stringify(window.sealMetadata),
        window.publicProjectionJson,
        window.plannedAt,
        window.activatedAt,
        window.sealedAt,
        window.reconciledAt,
        window.updatedAt,
      )
      .run()

    return window
  },
  readRun: async trainingRunRef => {
    const row = await db
      .prepare(
        `SELECT *
           FROM training_runs
          WHERE training_run_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(trainingRunRef)
      .first<TrainingRunRow>()

    return row === null ? undefined : rowToTrainingRun(row)
  },
  readWindow: async windowRef => {
    const row = await db
      .prepare(
        `SELECT *
           FROM training_windows
          WHERE window_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(windowRef)
      .first<TrainingWindowRow>()

    return row === null ? undefined : rowToTrainingWindow(row)
  },
  transitionRun: async run => {
    const result = await db
      .prepare(
        `UPDATE training_runs
            SET state = ?,
                receipt_refs_json = ?,
                public_projection_json = ?,
                updated_at = ?
          WHERE training_run_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(
        run.state,
        JSON.stringify(run.receiptRefs),
        run.publicProjectionJson,
        run.updatedAt,
        run.trainingRunRef,
      )
      .run()

    if ((result.meta?.changes ?? 0) === 0) {
      throw new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found for state transition.',
      })
    }

    return run
  },
  transitionWindow: async (window, event) => {
    await db.batch([
      db
        .prepare(
          `UPDATE training_windows
              SET state = ?,
                  receipt_refs_json = ?,
                  seal_metadata_json = ?,
                  public_projection_json = ?,
                  activated_at = ?,
                  sealed_at = ?,
                  reconciled_at = ?,
                  updated_at = ?
            WHERE window_ref = ?
              AND archived_at IS NULL`,
        )
        .bind(
          window.state,
          JSON.stringify(window.receiptRefs),
          window.sealMetadata === null
            ? null
            : JSON.stringify(window.sealMetadata),
          window.publicProjectionJson,
          window.activatedAt,
          window.sealedAt,
          window.reconciledAt,
          window.updatedAt,
          window.windowRef,
        ),
      db
        .prepare(
          `INSERT INTO training_window_events
            (id, window_ref, transition_kind, state_from, state_to, actor_ref,
             receipt_ref, created_at, archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          event.id,
          event.windowRef,
          event.transitionKind,
          event.stateFrom,
          event.stateTo,
          event.actorRef,
          event.receiptRef,
          event.createdAt,
        ),
    ])

    return window
  },
})

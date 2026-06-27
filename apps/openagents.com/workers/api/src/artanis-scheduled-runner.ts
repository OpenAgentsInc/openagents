import { Effect } from 'effect'

import { TASSADAR_EXECUTOR_CAPABILITY_REF } from '@openagentsinc/tassadar-executor'

import {
  ArtanisActionProposalRecord,
  ArtanisApprovalRequirementRecord,
  ArtanisLoopRecord,
  ArtanisLoopTickRecord,
} from './artanis-loop'
import {
  type ArtanisPersistenceError,
  type ArtanisPersistenceWriteReceipt,
  closeArtanisPersistedLoopTick,
  readArtanisPersistedRecord,
  saveArtanisApprovalGate,
  saveArtanisForumPublicationIntent,
  saveArtanisHealthSnapshot,
  saveArtanisLoopRecord,
  saveArtanisLoopTick,
  saveArtanisRuntimeSnapshot,
  saveArtanisWorkRoutingProposal,
} from './artanis-persistence'
import {
  ArtanisApprovalGateRecord,
} from './artanis-approval-gates'
import {
  ArtanisForumPublicationIntentRecord,
  exampleArtanisForumPublicationQueue,
} from './artanis-forum-publication'
import {
  type ArtanisHealthOverallState,
  ArtanisHealthSignalRecord,
  type ArtanisHealthSignalState,
  ArtanisHealthSnapshotRecord,
  exampleArtanisHealthSnapshot,
} from './artanis-health'
import {
  type KhalaFeedbackRecord,
  type KhalaFeedbackStore,
} from './khala-feedback-routes'
import {
  buildKhalaTraceReviewReport,
  type KhalaTraceReviewReport,
  type KhalaTraceReviewStore,
  type KhalaTraceReviewTriageItem,
} from './khala-trace-review-routes'
import {
  type KhalaUnsupportedRequestRecord,
  type KhalaUnsupportedRequestStore,
  type KhalaUnsupportedRequestTriageKind,
} from './khala-unsupported-request-routes'
import { exampleArtanisRuntime } from './artanis-runtime'
import {
  ArtanisWorkRoutingProposalRecord,
} from './artanis-work-routing'
import {
  epochMillisToIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'
import {
  TassadarExactTraceReplayVerificationClass,
  TassadarExecutorTraceJobKind,
} from './tassadar-executor-trace-homework'

export type ArtanisScheduledRunnerState = 'blocked' | 'completed' | 'disabled'

export type ArtanisScheduledRunnerContext = Readonly<{
  modelLabPrivateContractRefs: ReadonlyArray<string>
  modelLabPublicContractRefs: ReadonlyArray<string>
  operatorSteeringRefs: ReadonlyArray<string>
  persistedStateRefs: ReadonlyArray<string>
  publicPylonStatRefs: ReadonlyArray<string>
  runnerBackendRefs: ReadonlyArray<string>
}>

export type ArtanisKhalaReadinessObservation = Readonly<{
  leakCount?: number | undefined
  publicModelIds: ReadonlyArray<string>
  readinessStatus: string
  servableModelCount: number
}>

export type ArtanisScheduledRunnerInput = Readonly<{
  context?: Partial<ArtanisScheduledRunnerContext> | undefined
  db: D1Database
  enabled: boolean
  khalaUnsupportedTriage?: ArtanisKhalaUnsupportedTriageDependencies | undefined
  khalaReadinessObservation?: ArtanisKhalaReadinessObservation | undefined
  nowIso: string
  scheduleRef: string
  scopeRef?: string | undefined
}>

export type ArtanisScheduledRunnerForbiddenAuthority = Readonly<{
  adapterInstallAllowed: false
  deploymentAllowed: false
  evalLaunchAllowed: false
  forumPublishAllowed: false
  l402RedemptionAllowed: false
  paymentSpendAllowed: false
  providerMutationAllowed: false
  pylonJobDispatchAllowed: false
  runtimePromotionAllowed: false
  settlementMutationAllowed: false
  trainingLaunchAllowed: false
  walletSpendAllowed: false
}>

export type ArtanisScheduledRunnerResult = Readonly<{
  approvalRequirementRefs: ReadonlyArray<string>
  closeoutReceiptRefs: ReadonlyArray<string>
  enabled: boolean
  forbiddenAuthority: ArtanisScheduledRunnerForbiddenAuthority
  forumIntentRefs: ReadonlyArray<string>
  healthSnapshotRef: string | null
  khalaUnsupportedRequestRefs: ReadonlyArray<string>
  loadedContextRefs: ReadonlyArray<string>
  loopRef: string | null
  persistedRefs: ReadonlyArray<string>
  scheduleRef: string
  state: ArtanisScheduledRunnerState
  storageReceipts: ReadonlyArray<ArtanisPersistenceWriteReceipt>
  tickRef: string | null
  workProposalRefs: ReadonlyArray<string>
}>

export type ArtanisKhalaUnsupportedTriageDependencies = Readonly<{
  feedbackStore: Pick<KhalaFeedbackStore, 'listRecent'>
  feedbackLimit?: number | undefined
  traceLimit?: number | undefined
  traceReviewStore: KhalaTraceReviewStore
  unsupportedRequestStore: Pick<KhalaUnsupportedRequestStore, 'upsert'>
}>

export type ArtanisKhalaUnsupportedTriageResult = Readonly<{
  feedbackGroupsConsidered: number
  reportRef: string
  skippedNoiseCount: number
  unsupportedRequests: ReadonlyArray<KhalaUnsupportedRequestRecord>
}>

const noRiskyExecutionAuthority: ArtanisScheduledRunnerForbiddenAuthority = {
  adapterInstallAllowed: false,
  deploymentAllowed: false,
  evalLaunchAllowed: false,
  forumPublishAllowed: false,
  l402RedemptionAllowed: false,
  paymentSpendAllowed: false,
  providerMutationAllowed: false,
  pylonJobDispatchAllowed: false,
  runtimePromotionAllowed: false,
  settlementMutationAllowed: false,
  trainingLaunchAllowed: false,
  walletSpendAllowed: false,
}

export const ARTANIS_TASSADAR_EXECUTOR_SAFE_COPY =
  'The proof of concept ran on 2026-06-10: a real registered Pylon executed a digest-pinned exact-program workload dispatched through the operator assignment route, the closeout carried the trace digest byte-identical to the psionic Rust executor fixture, the production worker re-executed the workload as a separate validator device with a Verified exact_trace_replay challenge receipt (and a Rejected receipt on a tampered digest), and one operator-funded paid closeout settled over real Lightning to the Pylon payout target with balance receipts on both sides. Bounded to one workload family and one Pylon; broad executor earning remains gated separately.'

const defaultContext: ArtanisScheduledRunnerContext = {
  modelLabPrivateContractRefs: [
    'context.private.artanis.model_lab.operator_contract_refs',
  ],
  modelLabPublicContractRefs: [
    'model_lab.public.report.autopilot_benchmark_loop',
    'contract.public.model_lab.retained_failure_loop',
    'contract.public.model_lab.training_run',
  ],
  operatorSteeringRefs: ['steering.public.autopilot_artanis'],
  persistedStateRefs: ['state.public.artanis.persistence'],
  publicPylonStatRefs: ['pylon.public.stats', 'nexus.public.stats'],
  runnerBackendRefs: ['runner_backend.public.artanis.worker_cron'],
}

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const contextForInput = (
  input: ArtanisScheduledRunnerInput,
): ArtanisScheduledRunnerContext => ({
  modelLabPrivateContractRefs:
    input.context?.modelLabPrivateContractRefs ??
    defaultContext.modelLabPrivateContractRefs,
  modelLabPublicContractRefs:
    input.context?.modelLabPublicContractRefs ??
    defaultContext.modelLabPublicContractRefs,
  operatorSteeringRefs:
    input.context?.operatorSteeringRefs ?? defaultContext.operatorSteeringRefs,
  persistedStateRefs:
    input.context?.persistedStateRefs ?? defaultContext.persistedStateRefs,
  publicPylonStatRefs:
    input.context?.publicPylonStatRefs ?? defaultContext.publicPylonStatRefs,
  runnerBackendRefs:
    input.context?.runnerBackendRefs ?? defaultContext.runnerBackendRefs,
})

const refSuffix = (value: string): string => {
  const suffix = value
    .trim()
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 96)

  return suffix === '' ? 'tick' : suffix
}

const KHALA_PUBLIC_MODEL_ID = 'openagents/khala'
const KHALA_READINESS_AUTHORITY_REFS = [
  'authority.public.khala_readiness.credentialless_read_only',
  'authority.public.khala_readiness.no_chat_call',
  'authority.public.khala_readiness.no_mutation',
  'authority.public.khala_readiness.no_paid_call',
]
const KHALA_READINESS_SOURCE_REFS = [
  'gateway.public.openagents.models',
  'gateway.public.openagents.readiness',
  'monitor.public.khala.no_spend_readiness',
]

const normalizedKhalaCatalog = (
  modelIds: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  uniqueRefs(modelIds.map(modelId => modelId.trim()).filter(Boolean))

const nonNegativeInteger = (value: number): number =>
  Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0

const khalaReadinessSignalNeedsAttention = (
  signal: ArtanisHealthSignalRecord,
): boolean => signal.state !== 'available' && signal.state !== 'fresh'

const scheduledKhalaReadinessSignal = (
  input: ArtanisScheduledRunnerInput,
): ArtanisHealthSignalRecord => {
  const observation = input.khalaReadinessObservation

  if (observation === undefined) {
    return new ArtanisHealthSignalRecord({
      blockerRefs: ['blocker.public.artanis.khala_readiness_not_observed'],
      caveatRefs: [
        ...KHALA_READINESS_AUTHORITY_REFS,
        'caveat.public.khala_readiness.requires_no_spend_observation',
      ],
      count: 1,
      kind: 'khala_readiness',
      label: 'Khala no-spend readiness has not been observed',
      observedAtIso: input.nowIso,
      operatorDetailRefs: ['health.operator.artanis.khala_readiness'],
      publicRecoveryActionRefs: [
        'recovery.public.artanis.run_khala_no_spend_monitor',
      ],
      publicStatusRefs: ['health.public.artanis.khala_readiness_unknown'],
      signalRef: 'health.public.artanis.khala_readiness',
      sourceRefs: KHALA_READINESS_SOURCE_REFS,
      state: 'unknown',
      subjectUpdatedAtIso: input.nowIso,
    })
  }

  const publicModelIds = normalizedKhalaCatalog(observation.publicModelIds)
  const hasKhala = publicModelIds.includes(KHALA_PUBLIC_MODEL_ID)
  const extraModelCount = publicModelIds.filter(
    modelId => modelId !== KHALA_PUBLIC_MODEL_ID,
  ).length
  const catalogLeakCount = Math.max(
    nonNegativeInteger(observation.leakCount ?? 0),
    extraModelCount,
  )
  const readinessReady = observation.readinessStatus.trim().toLowerCase() ===
    'ready'
  const servableModelCount = nonNegativeInteger(observation.servableModelCount)
  const catalogBlockerRefs = uniqueRefs([
    ...(hasKhala ? [] : ['blocker.public.artanis.khala_public_catalog_missing']),
    ...(publicModelIds.length === 1 && hasKhala
      ? []
      : ['blocker.public.artanis.khala_public_catalog_not_single_model']),
    ...(catalogLeakCount > 0
      ? ['blocker.public.artanis.khala_public_catalog_leak']
      : []),
  ])
  const availabilityBlockerRefs = uniqueRefs([
    ...(readinessReady
      ? []
      : ['blocker.public.artanis.khala_gateway_not_ready']),
    ...(servableModelCount > 0
      ? []
      : ['blocker.public.artanis.khala_no_servable_model']),
  ])
  const blockerRefs = uniqueRefs([
    ...catalogBlockerRefs,
    ...availabilityBlockerRefs,
  ])
  const state: ArtanisHealthSignalState = blockerRefs.length === 0
    ? 'available'
    : catalogBlockerRefs.length > 0
    ? 'blocked'
    : 'unavailable'
  const count = state === 'available'
    ? 0
    : Math.max(1, blockerRefs.length, catalogLeakCount)
  const publicStatusRefs = state === 'available'
    ? ['health.public.artanis.khala_ready']
    : state === 'blocked'
    ? ['health.public.artanis.khala_public_catalog_blocked']
    : ['health.public.artanis.khala_readiness_unavailable']

  return new ArtanisHealthSignalRecord({
    blockerRefs,
    caveatRefs: [
      ...KHALA_READINESS_AUTHORITY_REFS,
      'caveat.public.khala_public_catalog_single_model',
    ],
    count,
    kind: 'khala_readiness',
    label: state === 'available'
      ? 'Khala no-spend readiness is clean'
      : state === 'blocked'
      ? 'Khala public catalog is blocked'
      : 'Khala public readiness is unavailable',
    observedAtIso: input.nowIso,
    operatorDetailRefs: ['health.operator.artanis.khala_readiness'],
    publicRecoveryActionRefs: state === 'available'
      ? []
      : [
          'recovery.public.artanis.inspect_khala_gateway_catalog',
          'recovery.public.artanis.run_khala_no_spend_monitor',
        ],
    publicStatusRefs,
    signalRef: 'health.public.artanis.khala_readiness',
    sourceRefs: uniqueRefs([
      ...KHALA_READINESS_SOURCE_REFS,
      ...(hasKhala ? ['model.public.openagents.khala'] : []),
    ]),
    state,
    subjectUpdatedAtIso: input.nowIso,
  })
}

const overallStateWithKhalaSignal = (
  base: ArtanisHealthSnapshotRecord,
  khalaState: ArtanisHealthSignalState,
): ArtanisHealthOverallState => {
  if (khalaState === 'blocked') {
    return 'blocked'
  }

  if (base.overallState !== 'healthy') {
    return base.overallState
  }

  if (khalaState === 'unavailable') {
    return 'unavailable'
  }

  return khalaState === 'available' || khalaState === 'fresh'
    ? 'healthy'
    : 'degraded'
}

const nextTickIso = (nowIso: string): string =>
  isoTimestampAfterIso(nowIso, 15 * 60 * 1000)

const spendApprovalExpiryIso = (nowIso: string): string =>
  isoTimestampAfterIso(nowIso, 60 * 60 * 1000)

const forumPublicationSourceRef = (ref: string): boolean =>
  [
    'artifact.public.',
    'campaign.public.',
    'claim.public.',
    'context.public.',
    'evidence.public.',
    'forum.public.',
    'goal.public.',
    'loop.public.',
    'model_lab.public.',
    'nexus.public.',
    'pylon.public.',
    'receipt.public.',
    'report.public.',
  ].some(prefix => ref.startsWith(prefix))

const refSegment = (value: string): string =>
  refSuffix(value.toLowerCase()).slice(0, 80) || 'unknown'

const feedbackNoisePattern =
  /\b(thanks?|thank you|looks good|works great|nice|cool|wordy|verbose|tone|style|conversational|shorter|longer|friendly)\b/i
const feedbackBugPattern =
  /\b(500|bug|broken|crash(?:es|ed|ing)?|error|fail(?:s|ed|ing)?|hang(?:s|ing)?|stuck|timeout|wrong|regression|exception)\b/i
const feedbackCapabilityPattern =
  /\b(can't|cannot|could not|couldn't|does not support|unsupported|wish|need(?:s)?|local git|git diff|file|upload|repo|repository|browser|shell|terminal|integrat(?:e|ion)|connect)\b/i

const feedbackFingerprint = (feedback: string): string =>
  feedback
    .toLowerCase()
    .replace(/https?:\/\/\S+/g, ' ')
    .replace(/[^a-z0-9]+/g, ' ')
    .trim()
    .split(/\s+/)
    .filter(word => word.length > 2)
    .slice(0, 8)
    .join('_') || 'unknown'

const feedbackTriageKind = (
  record: KhalaFeedbackRecord,
): KhalaUnsupportedRequestTriageKind | null => {
  const feedback = record.feedback.trim()
  if (feedback.length < 12 || feedbackNoisePattern.test(feedback)) {
    return null
  }
  if (feedbackBugPattern.test(feedback)) {
    return 'bug'
  }
  if (feedbackCapabilityPattern.test(feedback)) {
    return 'missing_capability'
  }
  return null
}

const summarizeFeedbackGroup = (
  records: ReadonlyArray<KhalaFeedbackRecord>,
): string =>
  records
    .map(record => record.feedback.trim().replace(/\s+/g, ' '))
    .filter(Boolean)
    .slice(0, 3)
    .join(' / ')
    .slice(0, 1_000)

const traceTriageKind = (
  item: KhalaTraceReviewTriageItem,
): KhalaUnsupportedRequestTriageKind | null =>
  item.kind === 'bug' || item.kind === 'missing_capability' ? item.kind : null

const traceReviewWindowFor = (nowIso: string) => ({
  hours: 24,
  since: isoTimestampAfterIso(nowIso, -24 * 60 * 60 * 1000),
  until: nowIso,
})

const shouldRunHourlyKhalaUnsupportedTriage = (nowIso: string): boolean => {
  const parsed = new Date(nowIso)
  return Number.isFinite(parsed.getTime()) && parsed.getUTCMinutes() === 0
}

const uniqueUnsupportedRequests = (
  requests: ReadonlyArray<KhalaUnsupportedRequestRecord | null>,
): ReadonlyArray<KhalaUnsupportedRequestRecord> => {
  const byRef = requests.reduce(
    (acc, request) =>
      request === null ? acc : new Map(acc).set(request.requestRef, request),
    new Map<string, KhalaUnsupportedRequestRecord>(),
  )
  return [...byRef.values()].sort((a, b) =>
    a.requestRef.localeCompare(b.requestRef)
  )
}

const upsertTraceReviewUnsupportedRequest = (
  input: Readonly<{
    dependencies: ArtanisKhalaUnsupportedTriageDependencies
    nowIso: string
  }>,
  report: KhalaTraceReviewReport,
  item: KhalaTraceReviewTriageItem,
) =>
  Effect.gen(function* () {
    const triageKind = traceTriageKind(item)
    if (triageKind === null) {
      return null
    }
    const sourceRef = item.triageRef
    return yield* Effect.tryPromise({
      try: () =>
        input.dependencies.unsupportedRequestStore.upsert({
          createdAt: input.nowIso,
          evidenceRefs: uniqueRefs([
            sourceRef,
            report.reportRef,
            ...item.evidenceRefs,
          ]),
          forumTopicRef: null,
          githubIssueRef: null,
          requestRef:
            `khala_unsupported:artanis_trace_review:${refSegment(sourceRef)}`,
          sourceKind: 'trace_review',
          sourceRef,
          status: 'needs_issue',
          suggestedIssueTitle: item.suggestedIssueTitle,
          summary:
            `Recurring trace-review finding over ${report.window.hours}h: ${item.title}.`,
          title: item.title,
          triageKind,
          updatedAt: input.nowIso,
        }),
      catch: error => error,
    })
  })

const upsertFeedbackUnsupportedRequest = (
  input: Readonly<{
    dependencies: ArtanisKhalaUnsupportedTriageDependencies
    nowIso: string
  }>,
  key: string,
  triageKind: KhalaUnsupportedRequestTriageKind,
  records: ReadonlyArray<KhalaFeedbackRecord>,
) => {
  const sourceRef = `triage.khala_feedback.${refSegment(key)}`
  const title =
    triageKind === 'bug'
      ? 'Recurring Khala feedback reports a bug'
      : 'Recurring Khala feedback requests an unsupported capability'

  return Effect.tryPromise({
    try: () =>
      input.dependencies.unsupportedRequestStore.upsert({
        createdAt: input.nowIso,
        evidenceRefs: uniqueRefs(records.map(record => record.feedbackRef)),
        forumTopicRef: null,
        githubIssueRef: null,
        requestRef: `khala_unsupported:artanis_feedback:${refSegment(key)}`,
        sourceKind: 'khala_feedback',
        sourceRef,
        status: 'needs_issue',
        suggestedIssueTitle: `[Khala feedback] ${title}`,
        summary: summarizeFeedbackGroup(records),
        title,
        triageKind,
        updatedAt: input.nowIso,
      }),
    catch: error => error,
  })
}

export const runArtanisKhalaUnsupportedRequestTriage = Effect.fn(
  'runArtanisKhalaUnsupportedRequestTriage',
)(function* (
  input: Readonly<{
    dependencies: ArtanisKhalaUnsupportedTriageDependencies
    nowIso: string
  }>,
) {
  const traceLimit = Math.min(
    Math.max(Math.trunc(input.dependencies.traceLimit ?? 10), 1),
    50,
  )
  const feedbackLimit = Math.min(
    Math.max(Math.trunc(input.dependencies.feedbackLimit ?? 50), 1),
    100,
  )
  const window = traceReviewWindowFor(input.nowIso)
  const facts = yield* Effect.tryPromise({
    try: () =>
      input.dependencies.traceReviewStore.readFacts({
        limit: traceLimit,
        window,
      }),
    catch: error => error,
  })
  const report = yield* Effect.try({
    try: () =>
      buildKhalaTraceReviewReport({
        facts,
        generatedAt: input.nowIso,
        window,
      }),
    catch: error => error,
  })
  const traceRequests = yield* Effect.forEach(
    report.triageItems,
    item => upsertTraceReviewUnsupportedRequest(input, report, item),
    { concurrency: 1 },
  )
  const feedback = yield* Effect.tryPromise({
    try: () =>
      input.dependencies.feedbackStore.listRecent({
        limit: feedbackLimit,
      }),
    catch: error => error,
  })
  const classifiedFeedback = feedback
    .map(record => ({ kind: feedbackTriageKind(record), record }))
    .filter(
      (
        entry,
      ): entry is Readonly<{
        kind: KhalaUnsupportedRequestTriageKind
        record: KhalaFeedbackRecord
      }> => entry.kind !== null,
    )
  const groups = classifiedFeedback.reduce((acc, entry) => {
    const key = `${entry.kind}:${feedbackFingerprint(entry.record.feedback)}`
    const existing = acc.get(key)
    return new Map(acc).set(key, {
      kind: entry.kind,
      records:
        existing === undefined
          ? [entry.record]
          : [...existing.records, entry.record],
    })
  }, new Map<string, Readonly<{
    kind: KhalaUnsupportedRequestTriageKind
    records: ReadonlyArray<KhalaFeedbackRecord>
  }>>())
  const recurringFeedbackGroups = [...groups.entries()].filter(
    ([, group]) => group.records.length >= 2,
  )
  const feedbackRequests = yield* Effect.forEach(
    recurringFeedbackGroups,
    ([key, group]) =>
      upsertFeedbackUnsupportedRequest(input, key, group.kind, group.records),
    { concurrency: 1 },
  )

  const result: ArtanisKhalaUnsupportedTriageResult = {
    feedbackGroupsConsidered: groups.size,
    reportRef: report.reportRef,
    skippedNoiseCount: feedback.length - classifiedFeedback.length,
    unsupportedRequests: uniqueUnsupportedRequests([
      ...traceRequests,
      ...feedbackRequests,
    ]),
  }
  return result
})

const disabledResult = (
  scheduleRef: string,
): ArtanisScheduledRunnerResult => ({
  approvalRequirementRefs: [],
  closeoutReceiptRefs: [],
  enabled: false,
  forbiddenAuthority: noRiskyExecutionAuthority,
  forumIntentRefs: [],
  healthSnapshotRef: null,
  khalaUnsupportedRequestRefs: [],
  loadedContextRefs: [],
  loopRef: null,
  persistedRefs: [],
  scheduleRef,
  state: 'disabled',
  storageReceipts: [],
  tickRef: null,
  workProposalRefs: [],
})

const scheduledLoop = (
  input: ArtanisScheduledRunnerInput,
  selectedContextRefs: ReadonlyArray<string>,
): Readonly<{
  assignmentRef: string
  loop: ArtanisLoopRecord
  tick: ArtanisLoopTickRecord
}> => {
  const scheduleSuffix = refSuffix(input.scheduleRef)
  const scopeRef = input.scopeRef ?? 'scope.public.artanis.global'
  const loopRef = `loop.public.artanis.${refSuffix(scopeRef)}`
  const assignmentRef =
    `assignment.public.artanis.tassadar_executor_trace.${scheduleSuffix}`
  const dispatchActionRef =
    `action.public.artanis.tassadar_executor_dispatch.${scheduleSuffix}`
  const replayActionRef =
    `action.public.artanis.tassadar_executor_replay.${scheduleSuffix}`
  const paidSampleActionRef =
    `action.public.artanis.tassadar_executor_paid_sample.${scheduleSuffix}`
  const tickRef = `tick.public.artanis.${scheduleSuffix}`
  const dispatchReceiptRef =
    `receipt.public.artanis.tassadar_executor_dispatch.${scheduleSuffix}`
  const closeoutReceiptRef =
    `receipt.public.artanis.tassadar_executor_closeout.${scheduleSuffix}`
  const replayReceiptRef =
    `receipt.public.artanis.tassadar_executor_replay_verified.${scheduleSuffix}`
  const acceptanceReceiptRef =
    `receipt.public.artanis.tassadar_executor_acceptance.${scheduleSuffix}`
  const forumIntentQueuedReceiptRef =
    `receipt.public.artanis.tassadar_executor_forum_intent.${scheduleSuffix}`
  const tickCloseoutReceiptRef =
    `receipt.public.artanis.tassadar_executor_tick_closeout.${scheduleSuffix}`
  const forumIntentRef =
    `forum.public.artanis.tassadar_executor_trace_intent.${scheduleSuffix}`
  const payloadArtifactRef =
    `artifact.public.artanis.tassadar_executor_trace_payload.${scheduleSuffix}`
  const verdictArtifactRef =
    `artifact.public.artanis.tassadar_executor_replay_verdict.${scheduleSuffix}`
  const approvalRef =
    `approval.public.artanis.tassadar_executor_paid_sample.${scheduleSuffix}`
  const authorityRef = 'authority.public.artanis.operator_spend_enable'
  const publicEvidenceRefs = uniqueRefs([
    ...selectedContextRefs,
    assignmentRef,
    TASSADAR_EXECUTOR_CAPABILITY_REF,
    `job.public.${TassadarExecutorTraceJobKind}`,
    `verification.public.${TassadarExactTraceReplayVerificationClass}`,
  ])
  const tick = new ArtanisLoopTickRecord({
    actionProposals: [
      new ArtanisActionProposalRecord({
        actionRef: dispatchActionRef,
        approvalRequirementRefs: [],
        artifactRefs: [payloadArtifactRef],
        authorityReceiptRefs: [],
        caveatRefs: [
          'caveat.public.tassadar_executor_trace.no_spend_dispatch_only',
          'caveat.public.tassadar_executor_trace.operator_selected_pylon',
        ],
        evidenceRefs: publicEvidenceRefs,
        kind: 'pylon_triage',
        risk: 'safe',
      }),
      new ArtanisActionProposalRecord({
        actionRef: replayActionRef,
        approvalRequirementRefs: [],
        artifactRefs: [verdictArtifactRef],
        authorityReceiptRefs: [],
        caveatRefs: [
          'caveat.public.tassadar_executor_trace.digest_predicate_only',
          'caveat.public.tassadar_executor_trace.separate_worker_replay',
        ],
        evidenceRefs: [
          closeoutReceiptRef,
          replayReceiptRef,
          `verification.public.${TassadarExactTraceReplayVerificationClass}`,
        ],
        kind: 'executor_trace_replay',
        risk: 'safe',
      }),
      new ArtanisActionProposalRecord({
        actionRef: paidSampleActionRef,
        approvalRequirementRefs: [approvalRef],
        artifactRefs: [],
        authorityReceiptRefs: [authorityRef],
        caveatRefs: [
          'caveat.public.bitcoin_requires_operator_enable',
          'caveat.public.settlement_bridge_receipts_required',
        ],
        evidenceRefs: [
          acceptanceReceiptRef,
          'bridge.public.nexus_pylon_artanis_refs',
        ],
        kind: 'wallet_spend',
        risk: 'approval_required',
      }),
    ],
    approvalRequirements: [
      new ArtanisApprovalRequirementRecord({
        actionRef: paidSampleActionRef,
        approvalRef,
        authorityRef,
        caveatRefs: [
          'caveat.public.bitcoin_requires_operator_enable',
          'caveat.public.settlement_bridge_receipts_required',
        ],
        expiresAtIso: spendApprovalExpiryIso(input.nowIso),
        state: 'pending',
      }),
    ],
    artifactRefs: [payloadArtifactRef, verdictArtifactRef],
    blockerRefs: [],
    caveatRefs: [
      'caveat.public.tick_evidence_only',
      'caveat.public.runner_no_direct_dispatch_or_spend_authority',
      'caveat.public.tassadar_executor_trace.copy_limited_to_safeCopy',
    ],
    closeoutReceiptRefs: [tickCloseoutReceiptRef, closeoutReceiptRef],
    createdAtIso: input.nowIso,
    forumPublicationIntentRefs: [forumIntentRef],
    goalRef: 'goal.public.artanis.tassadar_executor_trace_loop',
    idempotencyKey: `artanis-scheduled-tick:${assignmentRef}:v1`,
    loopRef,
    nextTickAtIso: nextTickIso(input.nowIso),
    receiptRefs: [
      `receipt.public.artanis.context_loaded.${scheduleSuffix}`,
      dispatchReceiptRef,
      closeoutReceiptRef,
      replayReceiptRef,
      acceptanceReceiptRef,
      forumIntentQueuedReceiptRef,
    ],
    selectedContextRefs,
    state: 'completed',
    tickRef,
    updatedAtIso: input.nowIso,
  })

  return {
    assignmentRef,
    loop: new ArtanisLoopRecord({
      active: true,
      agentId: 'agent_artanis',
      blockerRefs: [],
      caveatRefs: [
        'caveat.public.loop_does_not_execute_risky_actions',
        'caveat.public.one_active_loop_per_scope',
      ],
      createdAtIso: input.nowIso,
      goalRefs: [tick.goalRef],
      loopRef,
      scopeRef,
      state: 'running',
      ticks: [tick],
      updatedAtIso: input.nowIso,
    }),
    tick,
  }
}

const scheduledExecutorTraceWorkProposal = (
  input: ArtanisScheduledRunnerInput,
  tick: ArtanisLoopTickRecord,
  assignmentRef: string,
): ArtanisWorkRoutingProposalRecord => {
  const scheduleSuffix = refSuffix(input.scheduleRef)

  return new ArtanisWorkRoutingProposalRecord({
    acceptanceCriteriaRefs: [
      'criteria.public.tassadar_executor_trace.digest_match',
      'criteria.public.tassadar_executor_trace.separate_replay_verdict',
    ],
    approvalRequirementRefs: [],
    blockerRefs: [],
    capability: 'executor_trace_validation',
    costCaveatRefs: ['cost.public.tassadar_executor_trace.no_spend_default'],
    createdAtIso: input.nowIso,
    decidedAtIso: input.nowIso,
    operatorDetailRefs: [
      'operator.artanis.route.tassadar_executor_trace',
    ],
    proposalRef: `work.public.artanis.tassadar_executor_trace.${scheduleSuffix}`,
    publicCaveatRefs: [
      'caveat.public.tassadar_executor_trace.no_spend_dispatch_only',
      'caveat.public.tassadar_executor_trace.copy_limited_to_safeCopy',
    ],
    receiptRefs: tick.receiptRefs,
    resourceMode: 'background',
    risk: 'safe_read_only',
    sourceEvidenceRefs: [
      'docs/artanis/2026-06-10-executor-trace-loop-candidate.md',
      'promise.public.compute.tassadar_executor_poc.v1',
      'pylon.public.stats',
    ],
    spendLimitRefs: [
      'spend_limit.public.tassadar_executor_trace.zero_sats_default',
    ],
    state: 'dispatched',
    target: 'pylon',
    targetCapabilityRefs: [TASSADAR_EXECUTOR_CAPABILITY_REF],
    traceableWorkRefs: [assignmentRef],
    updatedAtIso: input.nowIso,
    workClass: 'executor_trace_validation',
  })
}

const scheduledSpendApprovalGate = (
  input: ArtanisScheduledRunnerInput,
  tick: ArtanisLoopTickRecord,
): ArtanisApprovalGateRecord => {
  const scheduleSuffix = refSuffix(input.scheduleRef)
  const approval = tick.approvalRequirements[0]!

  return new ArtanisApprovalGateRecord({
    actionRef: approval.actionRef,
    authorityReceiptRefs: [],
    authoritySourceKinds: ['operator_policy'],
    caveatRefs: approval.caveatRefs,
    createdAtIso: input.nowIso,
    expiresAtIso: approval.expiresAtIso ?? spendApprovalExpiryIso(input.nowIso),
    gateRef:
      `gate.public.artanis.tassadar_executor_paid_sample.${scheduleSuffix}`,
    idempotencyKey:
      `artanis-approval:tassadar-executor-paid-sample:${scheduleSuffix}:v1`,
    kind: 'wallet_spend',
    operatorReceiptRefs: [
      `receipt.public.artanis.operator_spend_review.${scheduleSuffix}`,
    ],
    policyRefs: [
      'policy.public.artanis.tassadar_executor_paid_sample_operator_enable',
    ],
    privateEvidenceRefs: [],
    publicStatusRefs: [
      `approval.public.artanis.tassadar_executor_paid_sample.pending.${scheduleSuffix}`,
    ],
    resolvedAtIso: null,
    rollbackPosture: 'not_reversible',
    rollbackRefs: [],
    sourceRefs: [
      tick.tickRef,
      ...tick.closeoutReceiptRefs,
      'bridge.public.nexus_pylon_artanis_refs',
    ],
    state: 'pending',
    supersededByGateRef: null,
    updatedAtIso: input.nowIso,
  })
}

const scheduledForumIntent = (
  input: ArtanisScheduledRunnerInput,
  tick: ArtanisLoopTickRecord,
  publicLoadedContextRefs: ReadonlyArray<string>,
): ArtanisForumPublicationIntentRecord => {
  const base = exampleArtanisForumPublicationQueue().intents[0]!
  const scheduleSuffix = refSuffix(input.scheduleRef)

  return new ArtanisForumPublicationIntentRecord({
    ...base,
    artifactRefs: tick.artifactRefs,
    blockerRefs: [],
    bodyText: ARTANIS_TASSADAR_EXECUTOR_SAFE_COPY,
    caveatRefs: [
      'caveat.public.copy_limited_to_promise_safeCopy',
      'caveat.public.no_broader_executor_or_earning_claim',
    ],
    createdAtIso: input.nowIso,
    deliveredAtIso: null,
    deliveryReceiptRefs: [],
    deliveryState: 'ready',
    goalRefs: [tick.goalRef],
    idempotencyKey:
      `artanis-forum:tassadar-executor-trace:${scheduleSuffix}:v1`,
    intentRef: tick.forumPublicationIntentRefs[0]!,
    modelLabReportRefs: [
      'report.public.model_lab.tassadar_executor_trace_loop',
    ],
    pageUrls: [
      'https://openagents.com/docs/product-promises',
      'https://openagents.com/forum/f/artanis',
    ],
    postRef: null,
    pylonNexusPublicRefs: [
      'campaign.public.tassadar_executor_trace',
      'pylon.public.stats',
    ],
    r10ClaimRefs: [],
    receiptRefs: tick.receiptRefs,
    sourceRefs: uniqueRefs([
      ...tick.artifactRefs,
      ...tick.receiptRefs,
      ...publicLoadedContextRefs.filter(forumPublicationSourceRef),
    ]),
    targetTopicRef: 'topic.public.forum.artanis.status',
    targetTopicState: 'open',
    updatedAtIso: input.nowIso,
  })
}

const scheduledHealthSnapshot = (
  input: ArtanisScheduledRunnerInput,
  loopRef: string,
  tickRef: string,
): ArtanisHealthSnapshotRecord => {
  const base = exampleArtanisHealthSnapshot
  const scheduleSuffix = refSuffix(input.scheduleRef)
  const khalaSignal = scheduledKhalaReadinessSignal(input)
  const khalaNeedsAttention = khalaReadinessSignalNeedsAttention(khalaSignal)

  return new ArtanisHealthSnapshotRecord({
    ...base,
    blockerRefs: uniqueRefs([
      ...base.blockerRefs,
      ...khalaSignal.blockerRefs,
    ]),
    caveatRefs: uniqueRefs([
      ...base.caveatRefs,
      ...KHALA_READINESS_AUTHORITY_REFS,
    ]),
    createdAtIso: input.nowIso,
    latestTickRef: tickRef,
    loopRef,
    operatorRecoveryActionRefs: uniqueRefs([
      ...base.operatorRecoveryActionRefs,
      ...(khalaNeedsAttention
        ? ['recovery.operator.artanis.inspect_khala_readiness']
        : []),
    ]),
    overallState: overallStateWithKhalaSignal(base, khalaSignal.state),
    overclaimBlockerRefs: uniqueRefs([
      ...base.overclaimBlockerRefs,
      ...(khalaNeedsAttention
        ? ['overclaim.public.artanis.khala_readiness_attention']
        : []),
    ]),
    publicStatusRefs: uniqueRefs([
      ...base.publicStatusRefs,
      ...khalaSignal.publicStatusRefs,
    ]),
    signals: base.signals.map(signal =>
      signal.kind === 'last_tick'
        ? new ArtanisHealthSignalRecord({
            ...signal,
            observedAtIso: input.nowIso,
            sourceRefs: [tickRef],
            subjectUpdatedAtIso: input.nowIso,
          })
        : signal.kind === 'khala_readiness'
        ? khalaSignal
        : signal,
    ),
    snapshotRef: `health.public.artanis.snapshot.${scheduleSuffix}`,
    sourceRefs: uniqueRefs([
      ...base.sourceRefs,
      ...khalaSignal.sourceRefs,
      loopRef,
      tickRef,
    ]),
    updatedAtIso: input.nowIso,
  })
}

export const runArtanisScheduledTick = Effect.fn('runArtanisScheduledTick')(
  function* (input: ArtanisScheduledRunnerInput) {
    if (!input.enabled) {
      return disabledResult(input.scheduleRef)
    }

    const context = contextForInput(input)
    const loadedContextRefs = uniqueRefs([
      ...context.publicPylonStatRefs,
      ...context.modelLabPublicContractRefs,
      ...context.modelLabPrivateContractRefs,
      ...context.persistedStateRefs,
      ...context.operatorSteeringRefs,
      ...context.runnerBackendRefs,
    ])
    const publicLoadedContextRefs = uniqueRefs([
      ...context.publicPylonStatRefs,
      ...context.modelLabPublicContractRefs,
      ...context.persistedStateRefs,
      ...context.operatorSteeringRefs,
      ...context.runnerBackendRefs,
    ])
    const { assignmentRef, loop, tick } = scheduledLoop(
      input,
      publicLoadedContextRefs,
    )
    const priorLoop = yield* readArtanisPersistedRecord(
      input.db,
      'loop_record',
      loop.loopRef,
    )
    const runtime = {
      ...exampleArtanisRuntime(),
      runtimeRef: `runtime.public.artanis.scheduled.${refSuffix(input.scheduleRef)}`,
      updatedAtIso: input.nowIso,
      workLoopRefs: [loop.loopRef],
    }
    const workProposal = scheduledExecutorTraceWorkProposal(
      input,
      tick,
      assignmentRef,
    )
    const approvalGate = scheduledSpendApprovalGate(input, tick)
    const forumIntent = scheduledForumIntent(input, tick, publicLoadedContextRefs)
    const healthSnapshot = scheduledHealthSnapshot(
      input,
      loop.loopRef,
      tick.tickRef,
    )

    const runtimeReceipt = yield* saveArtanisRuntimeSnapshot(
      input.db,
      runtime,
      `artanis-runtime-scheduled:${refSuffix(input.scheduleRef)}:v1`,
      input.nowIso,
    )
    const maybeLoopReceipt = priorLoop === null
      ? yield* saveArtanisLoopRecord(
          input.db,
          loop,
          `artanis-loop-scheduled:${refSuffix(loop.scopeRef)}:v1`,
          input.nowIso,
        )
      : null
    const tickReceipt = yield* saveArtanisLoopTick(
      input.db,
      tick,
      input.nowIso,
    )
    const healthReceipt = yield* saveArtanisHealthSnapshot(
      input.db,
      healthSnapshot,
      input.nowIso,
    )
    const workProposalReceipt = yield* saveArtanisWorkRoutingProposal(
      input.db,
      workProposal,
      input.nowIso,
    )
    const approvalGateReceipt = yield* saveArtanisApprovalGate(
      input.db,
      approvalGate,
      input.nowIso,
    )
    const forumIntentReceipt = yield* saveArtanisForumPublicationIntent(
      input.db,
      forumIntent,
      input.nowIso,
    )
    const closeoutReceipt = yield* closeArtanisPersistedLoopTick(
      input.db,
      tick.tickRef,
      {
        closedAtIso: input.nowIso,
        closeoutReceiptRefs: tick.closeoutReceiptRefs,
        state: 'completed',
        updatedAtIso: input.nowIso,
      },
    )
    const storageReceipts = [
      runtimeReceipt,
      ...(maybeLoopReceipt === null ? [] : [maybeLoopReceipt]),
      tickReceipt,
      healthReceipt,
      workProposalReceipt,
      approvalGateReceipt,
      forumIntentReceipt,
      closeoutReceipt,
    ]
    const khalaUnsupportedTriage =
      input.khalaUnsupportedTriage !== undefined &&
      shouldRunHourlyKhalaUnsupportedTriage(input.nowIso)
        ? yield* runArtanisKhalaUnsupportedRequestTriage({
            dependencies: input.khalaUnsupportedTriage,
            nowIso: input.nowIso,
          }).pipe(
            Effect.catch(() =>
              Effect.succeed<ArtanisKhalaUnsupportedTriageResult>({
                feedbackGroupsConsidered: 0,
                reportRef: `khala_trace_review.${refSegment(input.nowIso)}`,
                skippedNoiseCount: 0,
                unsupportedRequests: [],
              })
            ),
          )
        : null

    const result: ArtanisScheduledRunnerResult = {
      approvalRequirementRefs: uniqueRefs([
        ...tick.approvalRequirements.map(approval => approval.approvalRef),
        ...workProposal.approvalRequirementRefs,
        approvalGate.gateRef,
      ]),
      closeoutReceiptRefs: tick.closeoutReceiptRefs,
      enabled: true,
      forbiddenAuthority: noRiskyExecutionAuthority,
      forumIntentRefs: tick.forumPublicationIntentRefs,
      healthSnapshotRef: healthSnapshot.snapshotRef,
      khalaUnsupportedRequestRefs:
        khalaUnsupportedTriage?.unsupportedRequests.map(
          request => request.requestRef,
        ) ?? [],
      loadedContextRefs: uniqueRefs([
        ...loadedContextRefs,
        ...(input.khalaUnsupportedTriage === undefined
          ? []
          : [
              'operator.khala.trace_review',
              'operator.khala.feedback',
              'operator.khala.unsupported_requests',
            ]),
        ...(priorLoop === null ? [] : [priorLoop.recordRef]),
      ]),
      loopRef: loop.loopRef,
      persistedRefs: storageReceipts.map(receipt => receipt.recordRef),
      scheduleRef: input.scheduleRef,
      state: 'completed',
      storageReceipts,
      tickRef: tick.tickRef,
      workProposalRefs: [workProposal.proposalRef],
    }

    return result
  },
)

export const runArtanisScheduledTickForWorker = (
  input: Readonly<{
    db: D1Database
    khalaUnsupportedTriage?: ArtanisKhalaUnsupportedTriageDependencies | undefined
    khalaReadinessObservation?: ArtanisKhalaReadinessObservation | undefined
    scheduledRunnerEnabled: boolean
    scheduledTime: number
  }>,
): Effect.Effect<ArtanisScheduledRunnerResult, ArtanisPersistenceError> => {
  const nowIso = epochMillisToIsoTimestamp(input.scheduledTime)

  return runArtanisScheduledTick({
    db: input.db,
    enabled: input.scheduledRunnerEnabled,
    khalaUnsupportedTriage: input.khalaUnsupportedTriage,
    khalaReadinessObservation: input.khalaReadinessObservation,
    nowIso,
    scheduleRef: `cron.public.artanis.${refSuffix(nowIso)}`,
  })
}

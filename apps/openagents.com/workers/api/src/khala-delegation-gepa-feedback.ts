import { Schema as S } from 'effect'

import {
  KhalaDelegationExample,
  type KhalaDelegationExample as KhalaDelegationExampleType,
} from './khala-delegation-example-dataset'
import { assertPylonGepaMetricCallPublicRefs } from './pylon-gepa-metric-call-assignments'
import { publicRefSegment, uniqueRefs } from './public-ref-format'

export const KHALA_DELEGATION_GEPA_FEEDBACK_SCHEMA_REF =
  'openagents.khala.delegation_gepa_feedback.v0'

export const KHALA_DELEGATION_GEPA_TARGET_SUITE_REFS = [
  'target_suite.khala_delegation.public_safe.v0',
] as const

export const KHALA_DELEGATION_OPTIMIZER_BOUNDARY_REF =
  'boundary.psionic.gepa.optimizer_acceptance_not_runtime_promotion.v0'

export const KhalaDelegationGepaMetricVector = S.Struct({
  single_prompt_success: S.Number,
  merged_clean: S.Number,
  admitted_first_try: S.Number,
  wall_clock_seconds: S.Number,
  token_cost_tokens: S.Number,
  idle_gap_seconds: S.Number,
  conflict_churn: S.Number,
})
export type KhalaDelegationGepaMetricVector =
  typeof KhalaDelegationGepaMetricVector.Type

export const KhalaDelegationGepaFeedback = S.Struct({
  schemaRef: S.Literal(KHALA_DELEGATION_GEPA_FEEDBACK_SCHEMA_REF),
  assignmentRef: S.String,
  candidateRef: S.String,
  feedbackRef: S.String,
  metricName: S.Literal('khala.fleet.delegation'),
  scoreBps: S.Number,
  scalarRewardBps: S.Number,
  dimensions: KhalaDelegationGepaMetricVector,
  failureRefs: S.Array(S.String),
  admissionBlockerRefs: S.Array(S.String),
  verificationBlockerRefs: S.Array(S.String),
  coordinationBlockerRefs: S.Array(S.String),
  preconditionRefs: S.Array(S.String),
  traceProvenanceRefs: S.Array(S.String),
  optimizerAcceptanceBoundaryRef: S.Literal(
    KHALA_DELEGATION_OPTIMIZER_BOUNDARY_REF,
  ),
  targetSuiteRefs: S.Array(S.Literals(KHALA_DELEGATION_GEPA_TARGET_SUITE_REFS)),
  payoutAuthorityAllowed: S.Literal(false),
  publicClaimAuthorityAllowed: S.Literal(false),
  runtimePromotionAllowed: S.Literal(false),
  rawPromptIncluded: S.Literal(false),
  rawTraceIncluded: S.Literal(false),
  rawJudgeRationaleIncluded: S.Literal(false),
})
export type KhalaDelegationGepaFeedback =
  typeof KhalaDelegationGepaFeedback.Type

export class KhalaDelegationGepaFeedbackUnsafe extends S.TaggedErrorClass<KhalaDelegationGepaFeedbackUnsafe>()(
  'KhalaDelegationGepaFeedbackUnsafe',
  {
    reason: S.String,
  },
) {}

export type BuildKhalaDelegationGepaFeedbackInput = Readonly<{
  candidateRef?: string
  example: KhalaDelegationExampleType | unknown
  feedbackRef?: string
}>

type StringSignal = Readonly<{
  path: string
  value: string
}>

type NumberSignal = Readonly<{
  path: string
  value: number
}>

const decodeExample = S.decodeUnknownSync(KhalaDelegationExample)
const decodeFeedback = S.decodeUnknownSync(KhalaDelegationGepaFeedback)

const CANONICAL_FAILURE_REFS = {
  duplicateActiveAssignment:
    'blocker.public.pylon_dispatch.duplicate_active_assignment',
  noAvailableCodexCapacity:
    'blocker.public.pylon_dispatch.no_available_codex_capacity',
  objectiveTooVague: 'blocker.public.khala_delegation.objective_too_vague',
  prConflicted: 'blocker.public.khala_delegation.pr_conflicted',
  pylonStale: 'blocker.public.pylon_dispatch.pylon_stale',
  vacuousPr: 'blocker.public.khala_delegation.vacuous_pr',
  verifyFailed: 'blocker.public.pylon_assignment.verify_failed',
} as const

const isSuccessState = (state: string): boolean =>
  /accepted|accepted_work|merged|closed|completed|result_submitted/i.test(state)

const hasMergedOutcome = (example: KhalaDelegationExampleType): boolean =>
  example.outcome.mergeRefs.length > 0 &&
  example.outcome.acceptedWorkRefs.length > 0

const clampBps = (value: number): number =>
  Math.max(0, Math.min(10_000, Math.round(value)))

const secondsBetween = (startIso: string, endIso: string): number => {
  const start = Date.parse(startIso)
  const end = Date.parse(endIso)
  if (!Number.isFinite(start) || !Number.isFinite(end) || end < start) {
    return 0
  }
  return Math.round((end - start) / 1000)
}

const collectSignals = (value: unknown): {
  numbers: ReadonlyArray<NumberSignal>
  strings: ReadonlyArray<StringSignal>
} => {
  const strings: Array<StringSignal> = []
  const numbers: Array<NumberSignal> = []

  const walk = (current: unknown, path: string): void => {
    if (typeof current === 'string') {
      strings.push({ path, value: current })
      return
    }
    if (typeof current === 'number' && Number.isFinite(current)) {
      numbers.push({ path, value: current })
      return
    }
    if (Array.isArray(current)) {
      current.forEach((item, index) => walk(item, `${path}.${index}`))
      return
    }
    if (current !== null && typeof current === 'object') {
      for (const [key, child] of Object.entries(
        current as Record<string, unknown>,
      )) {
        walk(child, path === '' ? key : `${path}.${key}`)
      }
    }
  }

  walk(value, '')
  return { numbers, strings }
}

const haystack = (signals: ReadonlyArray<StringSignal>): string =>
  signals.map(signal => signal.value.toLowerCase()).join('\n')

const hasNumberAtPath = (
  numbers: ReadonlyArray<NumberSignal>,
  pattern: RegExp,
  predicate: (value: number) => boolean,
): boolean =>
  numbers.some(signal => pattern.test(signal.path) && predicate(signal.value))

const hasZeroOfOneCodexCapacity = (
  numbers: ReadonlyArray<NumberSignal>,
): boolean =>
  hasNumberAtPath(
    numbers,
    /(availablecodexassignments|available_codex_assignments|available.*codex)/i,
    value => value === 0,
  ) &&
  hasNumberAtPath(
    numbers,
    /(maxcodexassignments|max_codex_assignments|max.*codex)/i,
    value => value >= 1,
  )

const maxNumberAtPath = (
  numbers: ReadonlyArray<NumberSignal>,
  pattern: RegExp,
): number =>
  Math.max(
    0,
    ...numbers
      .filter(signal => pattern.test(signal.path))
      .map(signal => Math.max(0, signal.value)),
  )

const directRefSignals = (
  example: KhalaDelegationExampleType,
): ReadonlyArray<string> =>
  uniqueRefs([
    example.assignmentRef,
    example.exampleRef,
    example.pylonRef,
    ...example.input.taskRefs,
    ...example.input.acceptanceCriteriaRefs,
    ...example.input.resultExpectationRefs,
    ...example.input.artifactRefs,
    ...example.input.proofRefs,
    ...example.input.closeoutRefs,
    ...example.outcome.acceptedWorkRefs,
    ...example.outcome.rejectionRefs,
    ...example.outcome.closeoutRefs,
    ...example.outcome.pullRequestRefs,
    ...example.outcome.mergeRefs,
    ...example.evidenceRefs,
    ...example.rolloutTrace.lifecycleEvents.map(event => event.eventRef),
    ...example.rolloutTrace.exactTokenUsage.map(
      (_row, index) =>
        `usage_event.public.khala_delegation.${publicRefSegment(
          example.assignmentRef,
          'assignment',
        )}.${index}`,
    ),
    ...example.rolloutTrace.redactedAtif.map(trace => `atif:${trace.traceUuid}`),
  ])

const deriveFailureRefs = (
  example: KhalaDelegationExampleType,
): {
  admissionBlockerRefs: ReadonlyArray<string>
  coordinationBlockerRefs: ReadonlyArray<string>
  failureRefs: ReadonlyArray<string>
  preconditionRefs: ReadonlyArray<string>
  verificationBlockerRefs: ReadonlyArray<string>
} => {
  const signals = collectSignals(example)
  const text = haystack(signals.strings)
  const failureRefs: Array<string> = []
  const admissionBlockerRefs: Array<string> = []
  const verificationBlockerRefs: Array<string> = []
  const coordinationBlockerRefs: Array<string> = []
  const preconditionRefs: Array<string> = []

  const addAdmission = (ref: string): void => {
    admissionBlockerRefs.push(ref)
    failureRefs.push(ref)
  }
  const addVerification = (ref: string): void => {
    verificationBlockerRefs.push(ref)
    failureRefs.push(ref)
  }
  const addCoordination = (ref: string): void => {
    coordinationBlockerRefs.push(ref)
    failureRefs.push(ref)
  }

  const noAvailableCodexCapacity =
    text.includes('no_available_codex_capacity') ||
    hasZeroOfOneCodexCapacity(signals.numbers)

  if (noAvailableCodexCapacity) {
    preconditionRefs.push(CANONICAL_FAILURE_REFS.noAvailableCodexCapacity)
    if (!hasMergedOutcome(example)) {
      addAdmission(CANONICAL_FAILURE_REFS.noAvailableCodexCapacity)
    }
  }
  if (text.includes('duplicate_active_assignment')) {
    addAdmission(CANONICAL_FAILURE_REFS.duplicateActiveAssignment)
  }
  if (
    text.includes('stale_or_missing_heartbeat') ||
    text.includes('stale_heartbeat') ||
    text.includes('pylon_stale')
  ) {
    addAdmission(CANONICAL_FAILURE_REFS.pylonStale)
  }
  if (text.includes('verify_failed')) {
    addVerification(CANONICAL_FAILURE_REFS.verifyFailed)
  }
  if (
    text.includes('vacuous_pr') ||
    (isSuccessState(example.state) &&
      example.outcome.pullRequestRefs.length === 0 &&
      example.outcome.acceptedWorkRefs.length === 0)
  ) {
    addVerification(CANONICAL_FAILURE_REFS.vacuousPr)
  }
  if (
    text.includes('pr_conflicted') ||
    text.includes('conflict_resolution') ||
    text.includes('merge_conflict') ||
    text.includes('rebase_required')
  ) {
    addCoordination(CANONICAL_FAILURE_REFS.prConflicted)
  }
  if (text.includes('objective_too_vague')) {
    addVerification(CANONICAL_FAILURE_REFS.objectiveTooVague)
  }

  return {
    admissionBlockerRefs: uniqueRefs(admissionBlockerRefs),
    coordinationBlockerRefs: uniqueRefs(coordinationBlockerRefs),
    failureRefs: uniqueRefs(failureRefs),
    preconditionRefs: uniqueRefs(preconditionRefs),
    verificationBlockerRefs: uniqueRefs(verificationBlockerRefs),
  }
}

const scoreMetricVector = (
  example: KhalaDelegationExampleType,
  failures: ReturnType<typeof deriveFailureRefs>,
): KhalaDelegationGepaMetricVector => {
  const signals = collectSignals(example)
  const tokenCostTokens = example.rolloutTrace.exactTokenUsage.reduce(
    (sum, row) => sum + Math.max(0, row.totalTokens),
    0,
  )
  const conflictChurn = failures.coordinationBlockerRefs.includes(
    CANONICAL_FAILURE_REFS.prConflicted,
  )
    ? Math.max(1, maxNumberAtPath(signals.numbers, /conflict.*churn/i))
    : maxNumberAtPath(signals.numbers, /conflict.*churn/i)
  const mergedClean =
    example.outcome.mergeRefs.length > 0 &&
    example.outcome.acceptedWorkRefs.length > 0 &&
    failures.verificationBlockerRefs.length === 0 &&
    conflictChurn === 0
      ? 1
      : 0
  const singlePromptSuccess =
    mergedClean === 1 &&
    failures.failureRefs.length === 0 &&
    !haystack(signals.strings).match(/manual_intervention|human_conflict_help/)
      ? 1
      : 0

  return {
    admitted_first_try: failures.admissionBlockerRefs.length === 0 ? 1 : 0,
    conflict_churn: conflictChurn,
    idle_gap_seconds: Math.round(
      maxNumberAtPath(signals.numbers, /idle[_-]?gap.*seconds/i),
    ),
    merged_clean: mergedClean,
    single_prompt_success: singlePromptSuccess,
    token_cost_tokens: tokenCostTokens,
    wall_clock_seconds: secondsBetween(example.createdAt, example.updatedAt),
  }
}

const scoreBps = (dimensions: KhalaDelegationGepaMetricVector): number => {
  const successScore =
    dimensions.single_prompt_success * 2_500 +
    dimensions.merged_clean * 3_500 +
    dimensions.admitted_first_try * 1_500 +
    (dimensions.conflict_churn === 0 ? 1_000 : 0) +
    (dimensions.idle_gap_seconds === 0 ? 500 : 0)
  const costPenalty =
    Math.min(1_500, Math.floor(dimensions.token_cost_tokens / 250)) +
    Math.min(1_000, Math.floor(dimensions.wall_clock_seconds / 60)) +
    Math.min(1_000, Math.floor(dimensions.idle_gap_seconds / 30)) +
    Math.min(1_000, dimensions.conflict_churn * 500)

  return clampBps(successScore - costPenalty)
}

const assertFeedbackPublicSafe = (
  feedback: KhalaDelegationGepaFeedback,
): void => {
  const refs = [
    feedback.assignmentRef,
    feedback.candidateRef,
    feedback.feedbackRef,
    feedback.optimizerAcceptanceBoundaryRef,
    ...feedback.failureRefs,
    ...feedback.admissionBlockerRefs,
    ...feedback.verificationBlockerRefs,
    ...feedback.coordinationBlockerRefs,
    ...feedback.preconditionRefs,
    ...feedback.traceProvenanceRefs,
    ...feedback.targetSuiteRefs,
  ]

  try {
    assertPylonGepaMetricCallPublicRefs(
      'khala delegation GEPA feedback refs',
      refs,
    )
  } catch (error) {
    throw new KhalaDelegationGepaFeedbackUnsafe({
      reason:
        error instanceof Error
          ? error.message
          : 'khala delegation GEPA feedback refs are unsafe.',
    })
  }

  const unsafeRef = refs.find(ref => /\s|because|critique|rationale/i.test(ref))
  if (unsafeRef !== undefined) {
    throw new KhalaDelegationGepaFeedbackUnsafe({
      reason: 'khala delegation GEPA feedback must expose opaque refs only.',
    })
  }
}

export const buildKhalaDelegationGepaFeedback = (
  input: BuildKhalaDelegationGepaFeedbackInput,
): KhalaDelegationGepaFeedback => {
  const example = decodeExample(input.example)
  const failures = deriveFailureRefs(example)
  const dimensions = scoreMetricVector(example, failures)
  const candidateRef =
    input.candidateRef ?? 'candidate.khala.fleet.delegation.baseline'
  const feedbackRef =
    input.feedbackRef ??
    `gepa_feedback.khala_delegation.${publicRefSegment(
      example.assignmentRef,
      'assignment',
    )}.${publicRefSegment(candidateRef, 'candidate')}`
  const score = scoreBps(dimensions)
  const feedback = decodeFeedback({
    schemaRef: KHALA_DELEGATION_GEPA_FEEDBACK_SCHEMA_REF,
    admissionBlockerRefs: failures.admissionBlockerRefs,
    assignmentRef: example.assignmentRef,
    candidateRef,
    coordinationBlockerRefs: failures.coordinationBlockerRefs,
    dimensions,
    failureRefs: failures.failureRefs,
    feedbackRef,
    metricName: 'khala.fleet.delegation',
    optimizerAcceptanceBoundaryRef: KHALA_DELEGATION_OPTIMIZER_BOUNDARY_REF,
    payoutAuthorityAllowed: false,
    preconditionRefs: failures.preconditionRefs,
    publicClaimAuthorityAllowed: false,
    rawJudgeRationaleIncluded: false,
    rawPromptIncluded: false,
    rawTraceIncluded: false,
    runtimePromotionAllowed: false,
    scalarRewardBps: score,
    scoreBps: score,
    targetSuiteRefs: [...KHALA_DELEGATION_GEPA_TARGET_SUITE_REFS],
    traceProvenanceRefs: directRefSignals(example),
    verificationBlockerRefs: failures.verificationBlockerRefs,
  })

  assertFeedbackPublicSafe(feedback)
  return feedback
}

import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import {
  BlueprintMissionBriefingMetricAggregate,
  BlueprintMissionBriefingMetricProjection,
  BlueprintMissionBriefingMetricRecord,
} from './blueprint/schemas/mission-briefing-metric'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  aggregateBlueprintMissionBriefingMetrics,
  projectBlueprintMissionBriefingMetric,
} from './blueprint/services/mission-briefing-metric'

export class CodingAutopilotSituationalAwarenessRecord extends S.Class<CodingAutopilotSituationalAwarenessRecord>(
  'CodingAutopilotSituationalAwarenessRecord',
)({
  accountFailoverRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  decisionActionRefs: S.Array(S.String),
  id: S.String,
  metric: BlueprintMissionBriefingMetricRecord,
  metricRef: S.String,
  missionRefs: S.Array(S.String),
  repoTrustRefs: S.Array(S.String),
  summaryRef: S.String,
}) {}

export class CodingAutopilotSituationalAwarenessProjection extends S.Class<CodingAutopilotSituationalAwarenessProjection>(
  'CodingAutopilotSituationalAwarenessProjection',
)({
  accountFailoverRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  comprehensionResult: S.Literals([
    'not_understood',
    'partially_understood',
    'understood',
  ]),
  createdAtDisplay: S.String,
  decisionActionRefs: S.Array(S.String),
  elapsedTimeBucket: S.Literals([
    'under_30s',
    'under_1m',
    'under_2m',
    'over_2m',
    'not_understood',
  ]),
  followUpAction: S.Literals([
    'accepted',
    'asked_followup',
    'escalated',
    'none',
    'requested_revision',
    'resumed_work',
  ]),
  id: S.String,
  metric: BlueprintMissionBriefingMetricProjection,
  metricRef: S.String,
  missingContextRefs: S.Array(S.String),
  missionRefs: S.Array(S.String),
  repoTrustRefs: S.Array(S.String),
  reviewerKind: S.Literals(['agent', 'customer', 'operator', 'team']),
  summaryRef: S.String,
  underTwoMinuteTargetMet: S.Boolean,
}) {}

export class CodingAutopilotSituationalAwarenessAggregate extends S.Class<CodingAutopilotSituationalAwarenessAggregate>(
  'CodingAutopilotSituationalAwarenessAggregate',
)({
  accountFailoverRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  decisionActionRefs: S.Array(S.String),
  improvementNeeded: S.Boolean,
  metricAggregate: BlueprintMissionBriefingMetricAggregate,
  missionRefs: S.Array(S.String),
  repoTrustRefs: S.Array(S.String),
  totalCount: S.Number,
  understoodPercent: S.Number,
  underTwoMinutePercent: S.Number,
}) {}

export class CodingAutopilotSituationalAwarenessUnsafe extends S.TaggedErrorClass<CodingAutopilotSituationalAwarenessUnsafe>()(
  'CodingAutopilotSituationalAwarenessUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(email|invoice|payment|payload|patch|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet|webhook[_-]?secret|workroom[_-]?private)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(account[_-]?lease|artifact\.private|decision_action\.private|provider[_-]?account|repo\.private|repo_trust\.private|route[_-]?scorecard|source[_-]?authority|workroom\.(?!redacted))/i
const customerUnsafeRefPattern =
  /(account[_-]?lease|artifact\.private|decision_action\.private|provider[_-]?account|repo\.private|repo_trust\.private|route[_-]?scorecard|source[_-]?authority|workroom\.private)/i
const teamUnsafeRefPattern =
  /(account[_-]?lease|provider[_-]?account|repo\.private|repo_trust\.private|source[_-]?authority|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const audienceUnsafePattern = (
  audience: BlueprintMissionBriefingAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref) ||
    isoTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new CodingAutopilotSituationalAwarenessUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, raw timestamp, or raw artifact material.`,
    })
  }
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const projectionText = (
  projection: CodingAutopilotSituationalAwarenessProjection,
): string =>
  [
    projection.id,
    projection.metricRef,
    projection.summaryRef,
    ...projection.missionRefs,
    ...projection.artifactRefs,
    ...projection.decisionActionRefs,
    ...projection.accountFailoverRefs,
    ...projection.repoTrustRefs,
    projection.metric.briefingRef,
    projection.metric.feedbackSummaryRef ?? '',
    projection.metric.id,
    ...projection.metric.missingContextRefs,
    projection.metric.privateFeedbackNoteRef ?? '',
    projection.metric.programRunRef ?? '',
    ...projection.metric.receiptRefs,
    ...projection.metric.scorecardRefs,
    projection.metric.workroomRef,
  ].join(' ')

const aggregateText = (
  aggregate: CodingAutopilotSituationalAwarenessAggregate,
): string =>
  [
    ...aggregate.missionRefs,
    ...aggregate.artifactRefs,
    ...aggregate.decisionActionRefs,
    ...aggregate.accountFailoverRefs,
    ...aggregate.repoTrustRefs,
  ].join(' ')

export const codingAutopilotSituationalAwarenessProjectionHasPrivateMaterial = (
  projection: CodingAutopilotSituationalAwarenessProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return (
    universallyUnsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
  )
}

export const codingAutopilotSituationalAwarenessAggregateHasPrivateMaterial = (
  aggregate: CodingAutopilotSituationalAwarenessAggregate,
): boolean => {
  const text = aggregateText(aggregate)
  const pattern = audienceUnsafePattern(aggregate.audience)

  return (
    universallyUnsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
  )
}

const assertRecordSafe = (
  record: CodingAutopilotSituationalAwarenessRecord,
): void => {
  assertSafeRefs('situational-awareness identity refs', [
    record.id,
    record.metricRef,
    record.summaryRef,
  ])
  assertSafeRefs('situational-awareness mission refs', record.missionRefs)
  assertSafeRefs('situational-awareness artifact refs', record.artifactRefs)
  assertSafeRefs(
    'situational-awareness decision action refs',
    record.decisionActionRefs,
  )
  assertSafeRefs(
    'situational-awareness account failover refs',
    record.accountFailoverRefs,
  )
  assertSafeRefs('situational-awareness repo trust refs', record.repoTrustRefs)
}

export const projectCodingAutopilotSituationalAwarenessRecord = (
  record: CodingAutopilotSituationalAwarenessRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): CodingAutopilotSituationalAwarenessProjection => {
  assertRecordSafe(record)

  const baseMetric = projectBlueprintMissionBriefingMetric(record.metric, audience)
  const metric: BlueprintMissionBriefingMetricProjection = audience === 'public'
    ? {
      ...baseMetric,
      workroomRef: 'workroom.redacted',
    }
    : baseMetric
  const projection: CodingAutopilotSituationalAwarenessProjection = {
    accountFailoverRefs: safeRefsForAudience(
      'situational-awareness account failover refs',
      record.accountFailoverRefs,
      audience,
    ),
    artifactRefs: safeRefsForAudience(
      'situational-awareness artifact refs',
      record.artifactRefs,
      audience,
    ),
    audience,
    comprehensionResult: metric.comprehensionResult,
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.metric.createdAt,
      nowIso,
    ),
    decisionActionRefs: safeRefsForAudience(
      'situational-awareness decision action refs',
      record.decisionActionRefs,
      audience,
    ),
    elapsedTimeBucket: metric.elapsedTimeBucket,
    followUpAction: metric.followUpAction,
    id: safeRefsForAudience(
      'situational-awareness identity refs',
      [record.id],
      audience,
    )[0] ?? 'situational_awareness.redacted',
    metric,
    metricRef: safeRefsForAudience(
      'situational-awareness metric refs',
      [record.metricRef],
      audience,
    )[0] ?? 'briefing_metric.redacted',
    missingContextRefs: metric.missingContextRefs,
    missionRefs: safeRefsForAudience(
      'situational-awareness mission refs',
      record.missionRefs,
      audience,
    ),
    repoTrustRefs: safeRefsForAudience(
      'situational-awareness repo trust refs',
      record.repoTrustRefs,
      audience,
    ),
    reviewerKind: metric.reviewerKind,
    summaryRef: safeRefsForAudience(
      'situational-awareness summary refs',
      [record.summaryRef],
      audience,
    )[0] ?? 'summary.redacted',
    underTwoMinuteTargetMet: metric.underTwoMinuteTargetMet,
  }

  if (codingAutopilotSituationalAwarenessProjectionHasPrivateMaterial(projection)) {
    throw new CodingAutopilotSituationalAwarenessUnsafe({
      reason:
        'Situational-awareness projection contains private material or raw timestamps.',
    })
  }

  return projection
}

export const aggregateCodingAutopilotSituationalAwarenessRecords = (
  records: ReadonlyArray<CodingAutopilotSituationalAwarenessRecord>,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): CodingAutopilotSituationalAwarenessAggregate => {
  const projections = records.map(record =>
    projectCodingAutopilotSituationalAwarenessRecord(record, audience, nowIso),
  )
  const metricAggregate = aggregateBlueprintMissionBriefingMetrics(
    records.map(record => record.metric),
    audience,
  )
  const aggregate: CodingAutopilotSituationalAwarenessAggregate = {
    accountFailoverRefs: uniqueRefs(
      projections.flatMap(projection => projection.accountFailoverRefs),
    ),
    artifactRefs: uniqueRefs(
      projections.flatMap(projection => projection.artifactRefs),
    ),
    audience,
    decisionActionRefs: uniqueRefs(
      projections.flatMap(projection => projection.decisionActionRefs),
    ),
    improvementNeeded: metricAggregate.improvementNeeded,
    metricAggregate,
    missionRefs: uniqueRefs(
      projections.flatMap(projection => projection.missionRefs),
    ),
    repoTrustRefs: uniqueRefs(
      projections.flatMap(projection => projection.repoTrustRefs),
    ),
    totalCount: metricAggregate.totalCount,
    understoodPercent: metricAggregate.understoodPercent,
    underTwoMinutePercent: metricAggregate.underTwoMinutePercent,
  }

  if (codingAutopilotSituationalAwarenessAggregateHasPrivateMaterial(aggregate)) {
    throw new CodingAutopilotSituationalAwarenessUnsafe({
      reason:
        'Situational-awareness aggregate contains private material or raw timestamps.',
    })
  }

  return aggregate
}

export const exampleCodingAutopilotSituationalAwarenessRecord =
  (): CodingAutopilotSituationalAwarenessRecord => ({
    accountFailoverRefs: ['account_failover.provider_rate_limit.redacted'],
    artifactRefs: [
      'artifact.diff_summary.otec_revision_4',
      'artifact.test_run.otec_revision_4',
    ],
    decisionActionRefs: [
      'decision_action.otec_revision_4.continue',
      'decision_action.otec_revision_4.retry_account',
    ],
    id: 'situational_awareness.otec_revision_4.operator_1',
    metric: {
      briefingRef: 'briefing.continuation.otec_revision_4.latest',
      comprehensionResult: 'understood',
      createdAt: '2026-06-06T21:00:00.000Z',
      elapsedTimeBucket: 'under_2m',
      feedbackSummaryRef: 'feedback_summary.otec_revision_4.clear',
      followUpAction: 'resumed_work',
      id: 'briefing_metric.otec_revision_4.operator_1',
      missingContextRefs: [],
      privateFeedbackNoteRef: 'private_feedback.operator_note.redacted',
      programRunRef: 'program_run.continuation.otec_revision_4',
      receiptRefs: ['receipt.briefing_metric.otec_revision_4.operator_1'],
      reviewerKind: 'operator',
      scorecardRefs: ['scorecard.briefing.two_minute_state_understanding'],
      workroomRef: 'workroom.otec_site_revision_4',
    },
    metricRef: 'briefing_metric.otec_revision_4.operator_1',
    missionRefs: ['mission.otec_revision_4'],
    repoTrustRefs: ['repo_trust.public_repo.low'],
    summaryRef: 'summary.situational_awareness.otec_revision_4',
  })

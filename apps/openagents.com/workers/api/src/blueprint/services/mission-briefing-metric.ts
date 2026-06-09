import type {
  BlueprintMissionBriefingAudience,
} from '../schemas/continuation-mission-briefing'
import type {
  BlueprintMissionBriefingElapsedBucket,
  BlueprintMissionBriefingMetricAggregate,
  BlueprintMissionBriefingMetricCount,
  BlueprintMissionBriefingMetricProjection,
  BlueprintMissionBriefingMetricRecord,
} from '../schemas/mission-briefing-metric'

const universallyUnsafeMetricTextPattern =
  /(bearer\s+|cookie|customer[_-]?email|customer[_-]?name|email[_-]?body|mnemonic|oauth|oa_agent_|openagents_admin|password|preimage|private[_-]?key|raw[_-]?email|raw[_-]?runner|raw[_-]?run[_-]?log|runner[_-]?log|secret|sk-[a-z0-9]|token|wallet[_-]?secret|\S+@\S+)/i
const nonOperatorPrivateMetricTextPattern =
  /(private[_-]?feedback|provider[_-]?account|provider[_-]?payload|provider[_-]?token|source[_-]?authority)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const underTwoMinuteBuckets: ReadonlySet<BlueprintMissionBriefingElapsedBucket> =
  new Set(['under_30s', 'under_1m', 'under_2m'])

export const blueprintMissionBriefingMetricMetTwoMinuteTarget = (
  metric: Pick<BlueprintMissionBriefingMetricRecord, 'elapsedTimeBucket'>,
): boolean => underTwoMinuteBuckets.has(metric.elapsedTimeBucket)

const audienceCanSeeOperatorMetricRefs = (
  audience: BlueprintMissionBriefingAudience,
): boolean => audience === 'operator'

const textIsSafeForMetricAudience = (
  value: string,
  audience: BlueprintMissionBriefingAudience,
): boolean =>
  value.trim() !== '' &&
  !universallyUnsafeMetricTextPattern.test(value) &&
  !isoTimestampPattern.test(value) &&
  (audienceCanSeeOperatorMetricRefs(audience) ||
    !nonOperatorPrivateMetricTextPattern.test(value))

const safeRef = (
  ref: string | null,
  audience: BlueprintMissionBriefingAudience,
): string | null =>
  ref !== null && textIsSafeForMetricAudience(ref, audience) ? ref : null

const safeRefs = (
  refs: ReadonlyArray<string>,
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> =>
  [...new Set(refs)].filter(ref => textIsSafeForMetricAudience(ref, audience))

const privateFeedbackNoteRefForAudience = (
  metric: BlueprintMissionBriefingMetricRecord,
  audience: BlueprintMissionBriefingAudience,
): string | null =>
  audienceCanSeeOperatorMetricRefs(audience)
    ? safeRef(metric.privateFeedbackNoteRef, audience)
    : null

export const projectBlueprintMissionBriefingMetric = (
  metric: BlueprintMissionBriefingMetricRecord,
  audience: BlueprintMissionBriefingAudience,
): BlueprintMissionBriefingMetricProjection => ({
  audience,
  briefingRef:
    safeRef(metric.briefingRef, audience) ?? 'briefing_metric.redacted',
  comprehensionResult: metric.comprehensionResult,
  elapsedTimeBucket: metric.elapsedTimeBucket,
  feedbackSummaryRef: safeRef(metric.feedbackSummaryRef, audience),
  followUpAction: metric.followUpAction,
  id: safeRef(metric.id, audience) ?? 'briefing_metric.redacted',
  missingContextRefs: safeRefs(metric.missingContextRefs, audience),
  privateFeedbackNoteRef: privateFeedbackNoteRefForAudience(metric, audience),
  programRunRef: safeRef(metric.programRunRef, audience),
  receiptRefs: safeRefs(metric.receiptRefs, audience),
  reviewerKind: metric.reviewerKind,
  scorecardRefs: safeRefs(metric.scorecardRefs, audience),
  underTwoMinuteTargetMet:
    blueprintMissionBriefingMetricMetTwoMinuteTarget(metric),
  workroomRef:
    safeRef(metric.workroomRef, audience) ?? 'workroom.redacted',
})

const countBy = (
  values: ReadonlyArray<string>,
): ReadonlyArray<BlueprintMissionBriefingMetricCount> =>
  [...values.reduce((counts, value) => {
    counts.set(value, (counts.get(value) ?? 0) + 1)

    return counts
  }, new Map<string, number>())]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, count]) => ({ count, key }))

const percent = (count: number, total: number): number =>
  total === 0 ? 0 : Math.round((count / total) * 100)

export const aggregateBlueprintMissionBriefingMetrics = (
  metrics: ReadonlyArray<BlueprintMissionBriefingMetricRecord>,
  audience: BlueprintMissionBriefingAudience,
): BlueprintMissionBriefingMetricAggregate => {
  const projections = metrics.map(metric =>
    projectBlueprintMissionBriefingMetric(metric, audience),
  )
  const totalCount = projections.length
  const underTwoMinuteCount = projections.filter(
    projection => projection.underTwoMinuteTargetMet,
  ).length
  const understoodCount = projections.filter(
    projection => projection.comprehensionResult === 'understood',
  ).length
  const partialCount = projections.filter(
    projection => projection.comprehensionResult === 'partially_understood',
  ).length
  const notUnderstoodCount = projections.filter(
    projection => projection.comprehensionResult === 'not_understood',
  ).length

  return {
    audience,
    byElapsedBucket: countBy(
      projections.map(projection => projection.elapsedTimeBucket),
    ),
    byFollowUpAction: countBy(
      projections.map(projection => projection.followUpAction),
    ),
    byReviewerKind: countBy(
      projections.map(projection => projection.reviewerKind),
    ),
    improvementNeeded:
      totalCount > 0 &&
      (underTwoMinuteCount < totalCount || understoodCount < totalCount),
    notUnderstoodCount,
    partialCount,
    totalCount,
    understoodCount,
    understoodPercent: percent(understoodCount, totalCount),
    underTwoMinuteCount,
    underTwoMinutePercent: percent(underTwoMinuteCount, totalCount),
  }
}

export const blueprintMissionBriefingMetricProjectionHasPrivateMaterial = (
  projection: BlueprintMissionBriefingMetricProjection,
): boolean => {
  const projectedText = [
    projection.briefingRef,
    projection.feedbackSummaryRef ?? '',
    projection.id,
    ...projection.missingContextRefs,
    projection.privateFeedbackNoteRef ?? '',
    projection.programRunRef ?? '',
    ...projection.receiptRefs,
    ...projection.scorecardRefs,
    projection.workroomRef,
  ].join(' ')

  return (
    universallyUnsafeMetricTextPattern.test(projectedText) ||
    isoTimestampPattern.test(projectedText) ||
    (!audienceCanSeeOperatorMetricRefs(projection.audience) &&
      nonOperatorPrivateMetricTextPattern.test(projectedText))
  )
}

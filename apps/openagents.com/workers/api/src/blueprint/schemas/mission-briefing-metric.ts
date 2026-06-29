import { Schema as S } from 'effect'

import { BlueprintMissionBriefingAudience } from './continuation-mission-briefing'

export const BlueprintMissionBriefingReviewerKind = S.Literals([
  'agent',
  'customer',
  'operator',
  'team',
])
export type BlueprintMissionBriefingReviewerKind =
  typeof BlueprintMissionBriefingReviewerKind.Type

export const BlueprintMissionBriefingElapsedBucket = S.Literals([
  'under_30s',
  'under_1m',
  'under_2m',
  'over_2m',
  'not_understood',
])
export type BlueprintMissionBriefingElapsedBucket =
  typeof BlueprintMissionBriefingElapsedBucket.Type

export const BlueprintMissionBriefingComprehensionResult = S.Literals([
  'not_understood',
  'partially_understood',
  'understood',
])
export type BlueprintMissionBriefingComprehensionResult =
  typeof BlueprintMissionBriefingComprehensionResult.Type

export const BlueprintMissionBriefingFollowUpAction = S.Literals([
  'accepted',
  'asked_followup',
  'escalated',
  'none',
  'requested_revision',
  'resumed_work',
])
export type BlueprintMissionBriefingFollowUpAction =
  typeof BlueprintMissionBriefingFollowUpAction.Type

export const BlueprintMissionBriefingMetricRecord = S.Struct({
  briefingRef: S.String,
  comprehensionResult: BlueprintMissionBriefingComprehensionResult,
  createdAt: S.String,
  elapsedTimeBucket: BlueprintMissionBriefingElapsedBucket,
  feedbackSummaryRef: S.NullOr(S.String),
  followUpAction: BlueprintMissionBriefingFollowUpAction,
  id: S.String,
  missingContextRefs: S.Array(S.String),
  privateFeedbackNoteRef: S.NullOr(S.String),
  programRunRef: S.NullOr(S.String),
  receiptRefs: S.Array(S.String),
  reviewerKind: BlueprintMissionBriefingReviewerKind,
  scorecardRefs: S.Array(S.String),
  workroomRef: S.String,
})
export type BlueprintMissionBriefingMetricRecord =
  typeof BlueprintMissionBriefingMetricRecord.Type

export const BlueprintMissionBriefingMetricProjection = S.Struct({
  audience: BlueprintMissionBriefingAudience,
  briefingRef: S.String,
  comprehensionResult: BlueprintMissionBriefingComprehensionResult,
  elapsedTimeBucket: BlueprintMissionBriefingElapsedBucket,
  feedbackSummaryRef: S.NullOr(S.String),
  followUpAction: BlueprintMissionBriefingFollowUpAction,
  id: S.String,
  missingContextRefs: S.Array(S.String),
  privateFeedbackNoteRef: S.NullOr(S.String),
  programRunRef: S.NullOr(S.String),
  receiptRefs: S.Array(S.String),
  reviewerKind: BlueprintMissionBriefingReviewerKind,
  scorecardRefs: S.Array(S.String),
  underTwoMinuteTargetMet: S.Boolean,
  workroomRef: S.String,
})
export type BlueprintMissionBriefingMetricProjection =
  typeof BlueprintMissionBriefingMetricProjection.Type

export const BlueprintMissionBriefingMetricCount = S.Struct({
  count: S.Number,
  key: S.String,
})
export type BlueprintMissionBriefingMetricCount =
  typeof BlueprintMissionBriefingMetricCount.Type

export const BlueprintMissionBriefingMetricAggregate = S.Struct({
  audience: BlueprintMissionBriefingAudience,
  byElapsedBucket: S.Array(BlueprintMissionBriefingMetricCount),
  byFollowUpAction: S.Array(BlueprintMissionBriefingMetricCount),
  byReviewerKind: S.Array(BlueprintMissionBriefingMetricCount),
  improvementNeeded: S.Boolean,
  notUnderstoodCount: S.Number,
  partialCount: S.Number,
  totalCount: S.Number,
  understoodCount: S.Number,
  understoodPercent: S.Number,
  underTwoMinuteCount: S.Number,
  underTwoMinutePercent: S.Number,
})
export type BlueprintMissionBriefingMetricAggregate =
  typeof BlueprintMissionBriefingMetricAggregate.Type

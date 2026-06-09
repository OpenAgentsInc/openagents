import { Schema as S } from 'effect'

import {
  BlueprintContinuationDecision,
  BlueprintContinuationDecisionKind,
  BlueprintContinuationTurnResult,
} from './continuation-decision'

export const BlueprintDecisionQueueAudience = S.Literals([
  'customer',
  'operator',
])
export type BlueprintDecisionQueueAudience =
  typeof BlueprintDecisionQueueAudience.Type

export const BlueprintDecisionQueueItemStatus = S.Literals([
  'blocked',
  'needs_review',
  'pending',
  'retrying',
  'terminal',
])
export type BlueprintDecisionQueueItemStatus =
  typeof BlueprintDecisionQueueItemStatus.Type

export const BlueprintContinuationDecisionQueueSource = S.Struct({
  decision: BlueprintContinuationDecision,
  orderRefs: S.Array(S.String),
  programRunRef: S.NullOr(S.String),
  safeSummaryRef: S.String,
  siteRefs: S.Array(S.String),
  turnResult: BlueprintContinuationTurnResult,
  workroomRefs: S.Array(S.String),
})
export type BlueprintContinuationDecisionQueueSource =
  typeof BlueprintContinuationDecisionQueueSource.Type

export const BlueprintContinuationDecisionQueueItem = S.Struct({
  accountFailoverNeeded: S.Boolean,
  accountFailoverRefs: S.Array(S.String),
  action: BlueprintContinuationDecisionKind,
  approvalRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  constraintRefs: S.Array(S.String),
  customerVisible: S.Boolean,
  decisionRef: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  orderRefs: S.Array(S.String),
  programRunRef: S.NullOr(S.String),
  programSignatureId: S.String,
  recommendedNextOrderRef: S.String,
  receiptRefs: S.Array(S.String),
  retryRefs: S.Array(S.String),
  safeSummaryRef: S.String,
  siteRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  status: BlueprintDecisionQueueItemStatus,
  stopConditionRefs: S.Array(S.String),
  workRef: S.String,
  workroomRefs: S.Array(S.String),
})
export type BlueprintContinuationDecisionQueueItem =
  typeof BlueprintContinuationDecisionQueueItem.Type

export const BlueprintContinuationDecisionQueueProjection = S.Struct({
  audience: BlueprintDecisionQueueAudience,
  blockerCount: S.Number,
  empty: S.Boolean,
  items: S.Array(BlueprintContinuationDecisionQueueItem),
  pendingCount: S.Number,
  retryCount: S.Number,
  reviewCount: S.Number,
  stopCount: S.Number,
})
export type BlueprintContinuationDecisionQueueProjection =
  typeof BlueprintContinuationDecisionQueueProjection.Type

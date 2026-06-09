import { Schema as S } from 'effect'

export const BlueprintMissionBriefingAudience = S.Literals([
  'public',
  'customer',
  'team',
  'operator',
])
export type BlueprintMissionBriefingAudience =
  typeof BlueprintMissionBriefingAudience.Type

export const BlueprintMissionBriefingWorkKind = S.Literals(['site', 'coding'])
export type BlueprintMissionBriefingWorkKind =
  typeof BlueprintMissionBriefingWorkKind.Type

export const BlueprintMissionBriefingSectionKind = S.Literals([
  'acceptance_request',
  'blocked',
  'changed',
  'cost',
  'email',
  'evidence',
  'link',
  'next_action',
  'route',
  'verification',
])
export type BlueprintMissionBriefingSectionKind =
  typeof BlueprintMissionBriefingSectionKind.Type

export const BlueprintMissionBriefingItemStatus = S.Literals([
  'blocked',
  'done',
  'needs_review',
  'pending',
  'ready',
  'retrying',
  'sent',
])
export type BlueprintMissionBriefingItemStatus =
  typeof BlueprintMissionBriefingItemStatus.Type

export const BlueprintMissionBriefingItem = S.Struct({
  displayTime: S.NullOr(S.String),
  kind: BlueprintMissionBriefingSectionKind,
  linkRefs: S.Array(S.String),
  ref: S.String,
  status: BlueprintMissionBriefingItemStatus,
  summaryRef: S.String,
})
export type BlueprintMissionBriefingItem =
  typeof BlueprintMissionBriefingItem.Type

export const BlueprintMissionBriefingProjection = S.Struct({
  audience: BlueprintMissionBriefingAudience,
  empty: S.Boolean,
  generatedAtDisplay: S.String,
  sections: S.Struct({
    acceptanceRequest: S.Array(BlueprintMissionBriefingItem),
    blocked: S.Array(BlueprintMissionBriefingItem),
    changed: S.Array(BlueprintMissionBriefingItem),
    costs: S.Array(BlueprintMissionBriefingItem),
    email: S.Array(BlueprintMissionBriefingItem),
    evidence: S.Array(BlueprintMissionBriefingItem),
    links: S.Array(BlueprintMissionBriefingItem),
    nextAction: S.Array(BlueprintMissionBriefingItem),
    route: S.Array(BlueprintMissionBriefingItem),
    verification: S.Array(BlueprintMissionBriefingItem),
  }),
  status: S.String,
  workKind: BlueprintMissionBriefingWorkKind,
  workroomRef: S.String,
})
export type BlueprintMissionBriefingProjection =
  typeof BlueprintMissionBriefingProjection.Type

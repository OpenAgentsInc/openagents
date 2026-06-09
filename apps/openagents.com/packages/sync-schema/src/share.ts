import { Schema as S } from 'effect'

export const ShareAudiencePublic = S.TaggedStruct('Public', {})
export type ShareAudiencePublic = typeof ShareAudiencePublic.Type

export const ShareAudienceTeamMembers = S.TaggedStruct('TeamMembers', {
  teamId: S.String,
  teamName: S.String,
})
export type ShareAudienceTeamMembers = typeof ShareAudienceTeamMembers.Type

export const ShareAudienceRecipient = S.Struct({
  userId: S.NullOr(S.String),
  email: S.NullOr(S.String),
  displayName: S.String,
})
export type ShareAudienceRecipient = typeof ShareAudienceRecipient.Type

export const ShareAudienceUsers = S.TaggedStruct('Users', {
  recipients: S.Array(ShareAudienceRecipient),
})
export type ShareAudienceUsers = typeof ShareAudienceUsers.Type

export const ShareAudience = S.Union([
  ShareAudiencePublic,
  ShareAudienceTeamMembers,
  ShareAudienceUsers,
])
export type ShareAudience = typeof ShareAudience.Type

export const ShareSource = S.Union([
  S.Struct({
    kind: S.Literal('agent-run'),
    id: S.String,
  }),
  S.Struct({
    kind: S.Literal('team-thread'),
    id: S.String,
    teamId: S.optionalKey(S.String),
  }),
  S.Struct({
    kind: S.Literal('team-project-thread'),
    id: S.String,
    teamId: S.String,
    projectId: S.optionalKey(S.String),
  }),
])
export type ShareSource = typeof ShareSource.Type

export const ShareStatus = S.Literals(['active', 'revoked', 'expired'])
export type ShareStatus = typeof ShareStatus.Type

export const WorkroomTimelineTextPart = S.Struct({
  kind: S.Literal('text'),
  body: S.Array(S.String),
  tone: S.optionalKey(S.Literals(['normal', 'muted'])),
})
export type WorkroomTimelineTextPart = typeof WorkroomTimelineTextPart.Type

export const WorkroomTimelineToolPart = S.Struct({
  kind: S.Literal('tool'),
  title: S.String,
  subtitle: S.String,
  status: S.Literals(['queued', 'running', 'completed', 'failed']),
  detail: S.Array(S.String),
  href: S.optionalKey(S.String),
  actionHref: S.optionalKey(S.String),
  actionLabel: S.optionalKey(S.String),
})
export type WorkroomTimelineToolPart = typeof WorkroomTimelineToolPart.Type

export const WorkroomTimelineDiffPart = S.Struct({
  kind: S.Literal('diff'),
  files: S.Array(
    S.Struct({
      path: S.String,
      added: S.Int,
      removed: S.Int,
      status: S.Literals(['modified', 'added']),
    }),
  ),
})
export type WorkroomTimelineDiffPart = typeof WorkroomTimelineDiffPart.Type

export const WorkroomTimelineFilePart = S.Struct({
  kind: S.Literal('file'),
  path: S.String,
  language: S.String,
  excerpt: S.Array(S.String),
})
export type WorkroomTimelineFilePart = typeof WorkroomTimelineFilePart.Type

export const WorkroomTimelinePart = S.Union([
  WorkroomTimelineTextPart,
  WorkroomTimelineToolPart,
  WorkroomTimelineDiffPart,
  WorkroomTimelineFilePart,
])
export type WorkroomTimelinePart = typeof WorkroomTimelinePart.Type

export const WorkroomTimelineMessage = S.Struct({
  id: S.String,
  author: S.Literals(['user', 'assistant', 'system']),
  label: S.String,
  time: S.String,
  parts: S.Array(WorkroomTimelinePart),
  avatarUrl: S.optionalKey(S.String),
  status: S.optionalKey(S.Literals(['complete', 'streaming'])),
})
export type WorkroomTimelineMessage = typeof WorkroomTimelineMessage.Type

export const WorkroomFileItem = S.Struct({
  label: S.String,
  meta: S.String,
  depth: S.optionalKey(S.Union([S.Literal(0), S.Literal(1)])),
  active: S.optionalKey(S.Boolean),
})
export type WorkroomFileItem = typeof WorkroomFileItem.Type

export const ShareProjectionMetrics = S.Struct({
  eventCount: S.Int,
  tokenTotal: S.Int,
  toolCallCount: S.Int,
})
export type ShareProjectionMetrics = typeof ShareProjectionMetrics.Type

export const ShareProjectionV1 = S.Struct({
  schemaVersion: S.Literal('openagents.share_projection.v1'),
  id: S.String,
  url: S.String,
  audience: ShareAudience,
  audienceLabel: S.String,
  title: S.String,
  subtitle: S.String,
  source: ShareSource,
  status: ShareStatus,
  createdAt: S.String,
  updatedAt: S.String,
  messages: S.Array(WorkroomTimelineMessage),
  files: S.Array(WorkroomFileItem),
  artifacts: S.Array(S.String),
  approvals: S.Array(S.String),
  receipts: S.Array(S.String),
  metrics: ShareProjectionMetrics,
})
export type ShareProjectionV1 = typeof ShareProjectionV1.Type

export const ShareCreateRequest = S.Struct({
  source: ShareSource,
  audience: ShareAudience,
  title: S.optionalKey(S.String),
  redactionPolicyId: S.optionalKey(S.String),
  expiresAt: S.optionalKey(S.NullOr(S.String)),
})
export type ShareCreateRequest = typeof ShareCreateRequest.Type

export const ShareUpdateRequest = S.Struct({
  audience: S.optionalKey(ShareAudience),
  title: S.optionalKey(S.String),
  expiresAt: S.optionalKey(S.NullOr(S.String)),
})
export type ShareUpdateRequest = typeof ShareUpdateRequest.Type

export const ShareCreateResponse = S.Struct({
  id: S.String,
  url: S.String,
  audienceLabel: S.String,
  status: ShareStatus,
})
export type ShareCreateResponse = typeof ShareCreateResponse.Type

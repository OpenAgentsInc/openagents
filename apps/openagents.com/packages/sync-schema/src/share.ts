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

// ---------------------------------------------------------------------------
// T14 (#8871, epic #8857 Wave 3) widened kinds.
//
// `text` | `tool` | `diff` | `file` above were the CLOSED 4-kind union every
// share/team-thread projection had to collapse into (every agent/Codex event
// became one generic `tool` part). The kinds below mirror the desktop
// `WorkbenchItem` model 1:1 (`apps/openagents-desktop/src/
// workbench-item-contract.ts`, #8859) so the server can emit distinct typed
// parts and the web `/share` and `/khala/chat-sync` routes can render them
// through the SAME shared `@openagentsinc/ui/desktop-workbench` components
// desktop uses, instead of a parallel Foldkit-ported card renderer.
//
// This widening is purely ADDITIVE: the union keeps every pre-existing kind
// (and their exact shapes) so already-persisted share rows and older cached
// responses keep decoding unchanged. `S.Union` tries each member in order and
// every member is tagged by a distinct `kind` literal, so old and new kinds
// coexist without ambiguity.
//
// Bound constants mirror the desktop contract's `WORKBENCH_*` ceilings
// exactly (same numbers, re-declared locally): `packages/sync-schema` is a
// browser-consumed package and cannot depend on the Desktop Electron app
// package, so the bounds are duplicated rather than imported — keep them in
// sync by hand if the desktop contract's numbers ever change.
export const SHARE_COMMAND_LIMIT = 4_000
export const SHARE_PATH_LIMIT = 1_024
export const SHARE_OUTPUT_TAIL_LIMIT = 4_000
export const SHARE_DIFF_LIMIT = 20_000
export const SHARE_CHANGE_LIMIT = 64
export const SHARE_ARG_LIMIT = 24
export const SHARE_ARG_KEY_LIMIT = 80
export const SHARE_ARG_VALUE_LIMIT = 400
export const SHARE_RESULT_SNIPPET_LIMIT = 2_000
export const SHARE_ERROR_MESSAGE_LIMIT = 400
export const SHARE_REASONING_SUMMARY_LIMIT = 4_000
export const SHARE_PLAN_ENTRY_LIMIT = 64
export const SHARE_PLAN_STEP_LIMIT = 400
export const SHARE_PLAN_PROSE_LIMIT = 4_000
export const SHARE_APPROVAL_DETAIL_LIMIT = 400
export const SHARE_AGENT_PROMPT_LIMIT = 2_000
export const SHARE_AGENT_CHILD_LIMIT = 16
export const SHARE_NOTICE_TEXT_LIMIT = 400

const BoundedString = (limit: number) => S.String.check(S.isMaxLength(limit))

const WorkroomTimelineStatus = S.Literals(['queued', 'running', 'completed', 'failed'])

export const WorkroomTimelineReasoningPart = S.Struct({
  kind: S.Literal('reasoning'),
  summary: BoundedString(SHARE_REASONING_SUMMARY_LIMIT),
})
export type WorkroomTimelineReasoningPart = typeof WorkroomTimelineReasoningPart.Type

export const WorkroomTimelineCommandPart = S.Struct({
  kind: S.Literal('command'),
  command: BoundedString(SHARE_COMMAND_LIMIT),
  cwd: S.optionalKey(BoundedString(SHARE_PATH_LIMIT)),
  status: WorkroomTimelineStatus,
  exitCode: S.optionalKey(S.NullOr(S.Int)),
  durationMs: S.optionalKey(S.Number),
  /** Bounded TAIL of aggregated stdout+stderr (the end carries the verdict). */
  outputTail: S.optionalKey(BoundedString(SHARE_OUTPUT_TAIL_LIMIT)),
  /** True when earlier output was discarded to preserve the bounded tail. */
  outputCapReached: S.optionalKey(S.Boolean),
})
export type WorkroomTimelineCommandPart = typeof WorkroomTimelineCommandPart.Type

export const WorkroomTimelineFileChangeEntry = S.Struct({
  path: BoundedString(SHARE_PATH_LIMIT),
  kind: S.Literals(['add', 'delete', 'update']),
  adds: S.optionalKey(S.Int),
  dels: S.optionalKey(S.Int),
  diff: S.optionalKey(BoundedString(SHARE_DIFF_LIMIT)),
  diffCapReached: S.optionalKey(S.Boolean),
})
export type WorkroomTimelineFileChangeEntry = typeof WorkroomTimelineFileChangeEntry.Type

export const WorkroomTimelineFileChangePart = S.Struct({
  kind: S.Literal('fileChange'),
  status: WorkroomTimelineStatus,
  changes: S.Array(WorkroomTimelineFileChangeEntry).check(
    S.isMaxLength(SHARE_CHANGE_LIMIT),
  ),
})
export type WorkroomTimelineFileChangePart = typeof WorkroomTimelineFileChangePart.Type

export const WorkroomTimelineToolCallArg = S.Struct({
  key: BoundedString(SHARE_ARG_KEY_LIMIT),
  value: BoundedString(SHARE_ARG_VALUE_LIMIT),
})
export type WorkroomTimelineToolCallArg = typeof WorkroomTimelineToolCallArg.Type

export const WorkroomTimelineToolCallPart = S.Struct({
  kind: S.Literal('toolCall'),
  callKind: S.Literals(['mcp', 'dynamic', 'web', 'image']),
  tool: BoundedString(120),
  server: S.optionalKey(BoundedString(120)),
  namespace: S.optionalKey(BoundedString(120)),
  args: S.Array(WorkroomTimelineToolCallArg).check(S.isMaxLength(SHARE_ARG_LIMIT)),
  resultSnippet: S.optionalKey(BoundedString(SHARE_RESULT_SNIPPET_LIMIT)),
  errorMessage: S.optionalKey(BoundedString(SHARE_ERROR_MESSAGE_LIMIT)),
  durationMs: S.optionalKey(S.Number),
  status: WorkroomTimelineStatus,
  /** web: the search query. */
  query: S.optionalKey(BoundedString(SHARE_ERROR_MESSAGE_LIMIT)),
  /** web: how many structured results came back. */
  resultCount: S.optionalKey(S.Int),
  /** image: the viewed/saved image path (already host-redacted). */
  path: S.optionalKey(BoundedString(SHARE_PATH_LIMIT)),
})
export type WorkroomTimelineToolCallPart = typeof WorkroomTimelineToolCallPart.Type

export const WorkroomTimelinePlanEntry = S.Struct({
  step: BoundedString(SHARE_PLAN_STEP_LIMIT),
  status: S.Literals(['pending', 'in_progress', 'completed']),
})
export type WorkroomTimelinePlanEntry = typeof WorkroomTimelinePlanEntry.Type

export const WorkroomTimelinePlanPart = S.Struct({
  kind: S.Literal('plan'),
  entries: S.Array(WorkroomTimelinePlanEntry).check(
    S.isMaxLength(SHARE_PLAN_ENTRY_LIMIT),
  ),
  /** Free-form plan narrative; a plan part may carry either, or both. */
  prose: S.optionalKey(BoundedString(SHARE_PLAN_PROSE_LIMIT)),
})
export type WorkroomTimelinePlanPart = typeof WorkroomTimelinePlanPart.Type

export const WorkroomTimelineApprovalPart = S.Struct({
  kind: S.Literal('approval'),
  /** Public/historical and read-only — never an interactive decision on web. */
  decision: S.optionalKey(S.Literals(['approved', 'denied'])),
  detail: S.optionalKey(BoundedString(SHARE_APPROVAL_DETAIL_LIMIT)),
})
export type WorkroomTimelineApprovalPart = typeof WorkroomTimelineApprovalPart.Type

export const WorkroomTimelineAgentChildStatus = S.Literals([
  'pendingInit',
  'running',
  'interrupted',
  'completed',
  'errored',
  'shutdown',
  'notFound',
])
export type WorkroomTimelineAgentChildStatus =
  typeof WorkroomTimelineAgentChildStatus.Type

export const WorkroomTimelineAgentChild = S.Struct({
  threadRef: BoundedString(120),
  status: WorkroomTimelineAgentChildStatus,
  nickname: S.optionalKey(BoundedString(120)),
})
export type WorkroomTimelineAgentChild = typeof WorkroomTimelineAgentChild.Type

export const WorkroomTimelineAgentPart = S.Struct({
  kind: S.Literal('agent'),
  tool: S.optionalKey(BoundedString(40)),
  prompt: S.optionalKey(BoundedString(SHARE_AGENT_PROMPT_LIMIT)),
  status: WorkroomTimelineStatus,
  children: S.optionalKey(
    S.Array(WorkroomTimelineAgentChild).check(
      S.isMaxLength(SHARE_AGENT_CHILD_LIMIT),
    ),
  ),
})
export type WorkroomTimelineAgentPart = typeof WorkroomTimelineAgentPart.Type

export const WorkroomTimelineNoticePart = S.Struct({
  kind: S.Literal('notice'),
  severity: S.optionalKey(S.Literals(['info', 'warning', 'error'])),
  text: BoundedString(SHARE_NOTICE_TEXT_LIMIT),
})
export type WorkroomTimelineNoticePart = typeof WorkroomTimelineNoticePart.Type

export const WorkroomTimelineCompactionPart = S.Struct({
  kind: S.Literal('compaction'),
})
export type WorkroomTimelineCompactionPart =
  typeof WorkroomTimelineCompactionPart.Type

export const WorkroomTimelineMeterPart = S.Struct({
  kind: S.Literal('meter'),
  inputTokens: S.optionalKey(S.Int),
  cachedInputTokens: S.optionalKey(S.Int),
  outputTokens: S.optionalKey(S.Int),
  reasoningTokens: S.optionalKey(S.Int),
  totalTokens: S.optionalKey(S.Int),
})
export type WorkroomTimelineMeterPart = typeof WorkroomTimelineMeterPart.Type

export const WorkroomTimelinePart = S.Union([
  WorkroomTimelineTextPart,
  WorkroomTimelineToolPart,
  WorkroomTimelineDiffPart,
  WorkroomTimelineFilePart,
  WorkroomTimelineReasoningPart,
  WorkroomTimelineCommandPart,
  WorkroomTimelineFileChangePart,
  WorkroomTimelineToolCallPart,
  WorkroomTimelinePlanPart,
  WorkroomTimelineApprovalPart,
  WorkroomTimelineAgentPart,
  WorkroomTimelineNoticePart,
  WorkroomTimelineCompactionPart,
  WorkroomTimelineMeterPart,
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

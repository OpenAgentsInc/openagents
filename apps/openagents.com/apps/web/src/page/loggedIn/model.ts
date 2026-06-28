import {
  SyncPatch,
  SyncSnapshot,
  InferenceAnalyticsResponse,
  TokenUsageAggregateResponse,
  TokenUsageLeaderboardPreferenceResponse,
  TokenUsageLeaderboardsResponse,
} from '@openagentsinc/sync-schema'
import { Option, Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

import {
  AuthBootstrap,
  BillingSummary,
  OnboardingGitHubRepository,
  OnboardingRepositoriesResponse,
  OnboardingStatus,
  Session,
  Team,
  type TeamProject,
  type TokenLeaderboards,
  emptyProviderAccountBundle,
} from '../../domain/session'
import {
  loggedInMulletAccessAllowed,
  projectMissionVisible,
  projectWorkroomsEnabled,
} from '../../product-policy'
import {
  LoggedInRoute,
  adminRouter,
  autopilotWorkRouter,
  decisionsRouter,
  forgeRouter,
  mulletRouter,
  statsRouter,
  teamChatRouter,
  teamProjectChatRouter,
} from '../../route'
import { MulletModel, init as initMullet } from './mullet/model'
import { NotificationsModel, initNotifications } from './notifications/model'
import {
  Model as WorkroomModel,
  OverviewTab as WorkroomOverviewTab,
  init as initWorkroom,
} from './page/workroom'
import { SiteElementContext } from './site-element-context'
import { ThreadRouteIdle, ThreadRouteState } from './thread-route'

export {
  MulletBootstrapFailed,
  MulletBootstrapIdle,
  MulletBootstrapLoaded,
  MulletBootstrapLoading,
  MulletBootstrapResponse,
  MulletModel,
} from './mullet/model'

// MODEL

export const ChatMessageAuthor = S.Literals(['user', 'assistant', 'system'])
export type ChatMessageAuthor = typeof ChatMessageAuthor.Type

export const ChatMessageStatus = S.Literals(['complete', 'streaming'])
export type ChatMessageStatus = typeof ChatMessageStatus.Type

export const ChatMessage = S.Struct({
  author: ChatMessageAuthor,
  body: S.String,
  id: S.String,
  label: S.String,
  status: ChatMessageStatus,
})
export type ChatMessage = typeof ChatMessage.Type

export const ThreadFileScope = S.Literals(['personal', 'team'])
export type ThreadFileScope = typeof ThreadFileScope.Type

export const PersonalThreadFileOwnership = ts('ThreadFileOwnershipPersonal', {})
export const TeamThreadFileOwnership = ts('ThreadFileOwnershipTeam', {
  teamId: S.String,
})
export const ThreadFileOwnership = S.Union([
  PersonalThreadFileOwnership,
  TeamThreadFileOwnership,
])
export type ThreadFileOwnership = typeof ThreadFileOwnership.Type

export const threadFileOwnershipFromNullableTeamId = (
  teamId: string | null | undefined,
): ThreadFileOwnership =>
  typeof teamId === 'string' && teamId.trim() !== ''
    ? TeamThreadFileOwnership({ teamId })
    : PersonalThreadFileOwnership()

export const threadFileOwnershipTeamId = (
  ownership: ThreadFileOwnership,
): string | undefined =>
  ownership._tag === 'ThreadFileOwnershipTeam' ? ownership.teamId : undefined

export const ThreadFileApiRecord = S.Struct({
  id: S.String,
  scope: ThreadFileScope,
  threadId: S.String,
  teamId: S.NullOr(S.String),
  ownerUserId: S.String,
  filename: S.String,
  contentType: S.String,
  sizeBytes: S.Number,
  downloadUrl: S.String,
  detailUrl: S.optionalKey(S.String),
  downloadEnabled: S.optionalKey(S.Boolean),
  createdAt: S.String,
})
export type ThreadFileApiRecord = typeof ThreadFileApiRecord.Type

export const ThreadFileRecord = S.Struct({
  id: S.String,
  scope: ThreadFileScope,
  threadId: S.String,
  ownership: ThreadFileOwnership,
  ownerUserId: S.String,
  filename: S.String,
  contentType: S.String,
  sizeBytes: S.Number,
  downloadUrl: S.String,
  detailUrl: S.optionalKey(S.String),
  downloadEnabled: S.optionalKey(S.Boolean),
  createdAt: S.String,
})
export type ThreadFileRecord = typeof ThreadFileRecord.Type

export const ThreadFileReferenceAuthor = S.Struct({
  userId: S.String,
  name: S.String,
  avatarUrl: S.NullOr(S.String),
  githubUsername: S.NullOr(S.String),
})
export type ThreadFileReferenceAuthor = typeof ThreadFileReferenceAuthor.Type

export const ThreadFileReferenceApiRecord = S.Struct({
  id: S.String,
  fileId: S.String,
  teamId: S.NullOr(S.String),
  threadId: S.String,
  messageId: S.String,
  messageKind: S.String,
  referenceKind: S.String,
  body: S.String,
  excerpt: S.String,
  href: S.String,
  createdAt: S.String,
  author: ThreadFileReferenceAuthor,
})
export type ThreadFileReferenceApiRecord =
  typeof ThreadFileReferenceApiRecord.Type

export const ThreadFileReferenceRecord = S.Struct({
  id: S.String,
  fileId: S.String,
  ownership: ThreadFileOwnership,
  threadId: S.String,
  messageId: S.String,
  messageKind: S.String,
  referenceKind: S.String,
  body: S.String,
  excerpt: S.String,
  href: S.String,
  createdAt: S.String,
  author: ThreadFileReferenceAuthor,
})
export type ThreadFileReferenceRecord = typeof ThreadFileReferenceRecord.Type

export const ThreadFileDetailApiRecord = S.Struct({
  canManage: S.Boolean,
  file: ThreadFileApiRecord,
  references: S.Array(ThreadFileReferenceApiRecord),
})
export type ThreadFileDetailApiRecord = typeof ThreadFileDetailApiRecord.Type

export const ThreadFileDetailRecord = S.Struct({
  canManage: S.Boolean,
  file: ThreadFileRecord,
  references: S.Array(ThreadFileReferenceRecord),
})
export type ThreadFileDetailRecord = typeof ThreadFileDetailRecord.Type

export const ThreadFilesResponse = S.Struct({
  files: S.Array(ThreadFileApiRecord),
})
export type ThreadFilesResponse = typeof ThreadFilesResponse.Type

export const ThreadFileDetailResponse = S.Struct({
  detail: ThreadFileDetailApiRecord,
})
export type ThreadFileDetailResponse = typeof ThreadFileDetailResponse.Type

export const ThreadFileUploadResponse = S.Struct({
  file: ThreadFileApiRecord,
})
export type ThreadFileUploadResponse = typeof ThreadFileUploadResponse.Type

export const threadFileRecordFromDto = (
  file: ThreadFileApiRecord,
): ThreadFileRecord => ({
  contentType: file.contentType,
  createdAt: file.createdAt,
  downloadUrl: file.downloadUrl,
  filename: file.filename,
  id: file.id,
  ownerUserId: file.ownerUserId,
  ownership: threadFileOwnershipFromNullableTeamId(file.teamId),
  scope: file.scope,
  sizeBytes: file.sizeBytes,
  threadId: file.threadId,
  ...(file.detailUrl === undefined ? {} : { detailUrl: file.detailUrl }),
  ...(file.downloadEnabled === undefined
    ? {}
    : { downloadEnabled: file.downloadEnabled }),
})

export const threadFileReferenceRecordFromDto = (
  reference: ThreadFileReferenceApiRecord,
): ThreadFileReferenceRecord => ({
  author: reference.author,
  body: reference.body,
  createdAt: reference.createdAt,
  excerpt: reference.excerpt,
  fileId: reference.fileId,
  href: reference.href,
  id: reference.id,
  messageId: reference.messageId,
  messageKind: reference.messageKind,
  ownership: threadFileOwnershipFromNullableTeamId(reference.teamId),
  referenceKind: reference.referenceKind,
  threadId: reference.threadId,
})

export const threadFileDetailFromDto = (
  detail: ThreadFileDetailApiRecord,
): ThreadFileDetailRecord => ({
  canManage: detail.canManage,
  file: threadFileRecordFromDto(detail.file),
  references: detail.references.map(threadFileReferenceRecordFromDto),
})

export const TeamChatKind = S.Literals([
  'message',
  'autopilot_intent',
  'adjutant_intent',
  'system',
])
export type TeamChatKind = typeof TeamChatKind.Type

export const TeamChatAuthor = S.Struct({
  userId: S.String,
  name: S.String,
  avatarUrl: S.NullOr(S.String),
  githubUsername: S.NullOr(S.String),
})
export type TeamChatAuthor = typeof TeamChatAuthor.Type

export const AgentRunStatus = S.Literals([
  'queued',
  'running',
  'waiting_for_input',
  'completed',
  'failed',
  'canceled',
])
export type AgentRunStatus = typeof AgentRunStatus.Type

export const UnknownRunDuration = ts('RunDurationUnknown', {})
export const KnownRunDuration = ts('RunDurationKnownSeconds', {
  seconds: S.Number,
})
export const RunDuration = S.Union([UnknownRunDuration, KnownRunDuration])
export type RunDuration = typeof RunDuration.Type

export const runDurationFromNullable = (
  seconds: number | null | undefined,
): RunDuration =>
  typeof seconds === 'number' && Number.isFinite(seconds)
    ? KnownRunDuration({ seconds: Math.max(0, Math.round(seconds)) })
    : UnknownRunDuration()

export const MissingAgentRunExternalRef = ts('AgentRunExternalRefMissing', {})
export const PresentAgentRunExternalRef = ts('AgentRunExternalRefPresent', {
  value: S.String,
})
export const AgentRunExternalRef = S.Union([
  MissingAgentRunExternalRef,
  PresentAgentRunExternalRef,
])
export type AgentRunExternalRef = typeof AgentRunExternalRef.Type

export const agentRunExternalRefFromNullable = (
  externalRunId: string | null | undefined,
): AgentRunExternalRef =>
  typeof externalRunId === 'string' && externalRunId.trim() !== ''
    ? PresentAgentRunExternalRef({ value: externalRunId })
    : MissingAgentRunExternalRef()

export const optionFromNullableString = (
  value: string | null | undefined,
): Option.Option<string> =>
  typeof value === 'string' && value.trim() !== ''
    ? Option.some(value)
    : Option.none()

export const TeamChatRunSummary = S.Struct({
  runId: S.String,
  status: AgentRunStatus,
  runtime: S.String,
  backend: S.String,
  repository: S.String,
  eventCount: S.Number,
  toolCallCount: S.Number,
  tokenTotal: S.Number,
  durationSeconds: S.NullOr(S.Number),
  updatedAt: S.String,
})
export type TeamChatRunSummary = typeof TeamChatRunSummary.Type

export const TeamChatMessageRecord = S.Struct({
  id: S.String,
  teamId: S.String,
  projectId: S.optionalKey(S.NullOr(S.String)),
  kind: TeamChatKind,
  body: S.String,
  autopilotThreadId: S.NullOr(S.String),
  agentRunId: S.NullOr(S.String),
  launchError: S.optionalKey(S.String),
  runSummary: S.optionalKey(TeamChatRunSummary),
  createdAt: S.String,
  author: TeamChatAuthor,
})
export type TeamChatMessageRecord = typeof TeamChatMessageRecord.Type

export const TeamChatMessagesResponse = S.Struct({
  messages: S.Array(TeamChatMessageRecord),
  projectId: S.optionalKey(S.NullOr(S.String)),
  teamId: S.String,
})
export type TeamChatMessagesResponse = typeof TeamChatMessagesResponse.Type

export const IdleThreadFileUpload = ts('ThreadFileUploadIdle', {})
export const UploadingThreadFile = ts('ThreadFileUploading', {
  scopeKey: S.String,
})
export const SucceededThreadFileUpload = ts('ThreadFileUploadSucceeded', {
  message: S.String,
})
export const FailedThreadFileUpload = ts('ThreadFileUploadFailed', {
  error: S.String,
})
export const ThreadFileUpload = S.Union([
  IdleThreadFileUpload,
  UploadingThreadFile,
  SucceededThreadFileUpload,
  FailedThreadFileUpload,
])
export type ThreadFileUpload = typeof ThreadFileUpload.Type

export const SidebarNavItem = S.Struct({
  href: S.String,
  label: S.String,
  meta: S.optionalKey(S.String),
})
export type SidebarNavItem = typeof SidebarNavItem.Type

export const SidebarSessionStatus = S.Literals([
  'active',
  'complete',
  'failed',
  'queued',
])
export type SidebarSessionStatus = typeof SidebarSessionStatus.Type

export const SidebarSessionOwner = S.Literals(['personal', 'team', 'project'])
export type SidebarSessionOwner = typeof SidebarSessionOwner.Type

export const SidebarSessionItem = S.Struct({
  active: S.Boolean,
  attention: S.Boolean,
  detail: S.String,
  href: S.String,
  owner: SidebarSessionOwner,
  ownerUserId: S.optionalKey(S.String),
  projectId: S.optionalKey(S.String),
  status: SidebarSessionStatus,
  teamId: S.optionalKey(S.String),
  title: S.String,
  updatedAt: S.optionalKey(S.String),
})
export type SidebarSessionItem = typeof SidebarSessionItem.Type

export const SidebarSessionSection = S.Struct({
  title: S.String,
  items: S.Array(SidebarSessionItem),
})
export type SidebarSessionSection = typeof SidebarSessionSection.Type

export const SidebarFooterRow = S.Struct({
  label: S.String,
  value: S.String,
})
export type SidebarFooterRow = typeof SidebarFooterRow.Type

export const SidebarModel = ts('LoggedInSidebar', {
  footerRows: S.Array(SidebarFooterRow),
  primaryItems: S.Array(SidebarNavItem),
  sessionSections: S.Array(SidebarSessionSection),
})
export type SidebarModel = typeof SidebarModel.Type

const sidebarStatusForProject = (project: TeamProject): SidebarSessionStatus =>
  project.agent?.status === 'active' ? 'active' : 'queued'

export const SyncConnectionStatus = S.Literals([
  'idle',
  'connecting',
  'open',
  'closed',
  'failed',
])
export type SyncConnectionStatus = typeof SyncConnectionStatus.Type

export const SyncConnectionModel = S.Struct({
  error: S.optionalKey(S.String),
  status: SyncConnectionStatus,
})
export type SyncConnectionModel = typeof SyncConnectionModel.Type

export const SyncMutationModel = S.Struct({
  command: S.String,
  payload: S.Unknown,
  scope: S.String,
})
export type SyncMutationModel = typeof SyncMutationModel.Type

export const SyncClientModel = ts('LoggedInSync', {
  collectionByScope: S.Record(
    S.String,
    S.Record(S.String, S.Record(S.String, S.Unknown)),
  ),
  connectionByScope: S.Record(S.String, SyncConnectionModel),
  cursors: S.Record(S.String, S.Number),
  pendingMutations: S.Record(S.String, SyncMutationModel),
  workspaceScope: S.String,
})
export type SyncClientModel = typeof SyncClientModel.Type

export const TokenUsageStatsFilterKey = S.Literals([
  'actorTeamId',
  'actorUserId',
  'leaderboardEligible',
  'leaderboardWindow',
  'model',
  'producerSystem',
  'provider',
  'since',
  'sourceRoute',
  'until',
  'usageTruth',
])
export type TokenUsageStatsFilterKey = typeof TokenUsageStatsFilterKey.Type

export const TokenUsageStatsFilters = ts('TokenUsageStatsFilters', {
  actorTeamId: S.String,
  actorUserId: S.String,
  leaderboardEligible: S.String,
  leaderboardWindow: S.String,
  model: S.String,
  producerSystem: S.String,
  provider: S.String,
  since: S.String,
  sourceRoute: S.String,
  until: S.String,
  usageTruth: S.String,
})
export type TokenUsageStatsFilters = typeof TokenUsageStatsFilters.Type

export const TokenUsageStatsIdle = ts('TokenUsageStatsIdle', {
  filters: TokenUsageStatsFilters,
})
export const TokenUsageStatsLoading = ts('TokenUsageStatsLoading', {
  filters: TokenUsageStatsFilters,
})
export const TokenUsageStatsLoaded = ts('TokenUsageStatsLoaded', {
  analytics: InferenceAnalyticsResponse,
  filters: TokenUsageStatsFilters,
  leaderboards: TokenUsageLeaderboardsResponse,
  preference: TokenUsageLeaderboardPreferenceResponse,
  response: TokenUsageAggregateResponse,
})
export const TokenUsageStatsFailed = ts('TokenUsageStatsFailed', {
  error: S.String,
  filters: TokenUsageStatsFilters,
})
export const TokenUsageStatsState = S.Union([
  TokenUsageStatsIdle,
  TokenUsageStatsLoading,
  TokenUsageStatsLoaded,
  TokenUsageStatsFailed,
])
export type TokenUsageStatsState = typeof TokenUsageStatsState.Type

export const PrefilledWorkspaceStatus = S.Literals([
  'draft',
  'invited',
  'active',
  'archived',
])
export type PrefilledWorkspaceStatus = typeof PrefilledWorkspaceStatus.Type

export const PrefilledWorkspaceStarterWorkflowStatus = S.Literals([
  'queued',
  'ready',
  'completed',
  'dismissed',
])
export type PrefilledWorkspaceStarterWorkflowStatus =
  typeof PrefilledWorkspaceStarterWorkflowStatus.Type

export const PrefilledWorkspaceSeededMemoryEntry = S.Struct({
  label: S.String,
  value: S.String,
  publicSourceRef: S.String,
})
export type PrefilledWorkspaceSeededMemoryEntry =
  typeof PrefilledWorkspaceSeededMemoryEntry.Type

export const PrefilledWorkspaceStarterWorkflow = S.Struct({
  title: S.String,
  description: S.String,
  outcomeKind: S.String,
  status: PrefilledWorkspaceStarterWorkflowStatus,
})
export type PrefilledWorkspaceStarterWorkflow =
  typeof PrefilledWorkspaceStarterWorkflow.Type

export const PrefilledWorkspaceIntroReceipt = S.Struct({
  summary: S.String,
  publicSourceRefs: S.Array(S.String),
})
export type PrefilledWorkspaceIntroReceipt =
  typeof PrefilledWorkspaceIntroReceipt.Type

export const PrefilledWorkspaceEngagement = S.Struct({
  invitedAt: S.NullOr(S.String),
  firstViewedAt: S.NullOr(S.String),
  firstClaimedAt: S.NullOr(S.String),
  firstRunAt: S.NullOr(S.String),
  lastViewedAt: S.NullOr(S.String),
  revisitCount: S.Number,
})
export type PrefilledWorkspaceEngagement =
  typeof PrefilledWorkspaceEngagement.Type

export const PrefilledWorkspace = S.Struct({
  id: S.String,
  projectName: S.String,
  status: PrefilledWorkspaceStatus,
  seededMemory: S.Array(PrefilledWorkspaceSeededMemoryEntry),
  starterWorkflows: S.Array(PrefilledWorkspaceStarterWorkflow),
  introReceipt: PrefilledWorkspaceIntroReceipt,
  engagement: S.optionalKey(PrefilledWorkspaceEngagement),
})
export type PrefilledWorkspace = typeof PrefilledWorkspace.Type

export const PrefilledWorkspaceViewer = S.Literals(['holder', 'operator'])
export type PrefilledWorkspaceViewer = typeof PrefilledWorkspaceViewer.Type

export const PrefilledWorkspaceResponse = S.Struct({
  generatedAt: S.String,
  viewer: PrefilledWorkspaceViewer,
  workspace: PrefilledWorkspace,
})
export type PrefilledWorkspaceResponse = typeof PrefilledWorkspaceResponse.Type

export const PrefilledWorkspaceIdle = ts('PrefilledWorkspaceIdle', {})
export const PrefilledWorkspaceLoading = ts('PrefilledWorkspaceLoading', {
  workspaceId: S.String,
})
export const PrefilledWorkspaceLoaded = ts('PrefilledWorkspaceLoaded', {
  generatedAt: S.String,
  viewer: PrefilledWorkspaceViewer,
  workspace: PrefilledWorkspace,
})
export const PrefilledWorkspaceFailed = ts('PrefilledWorkspaceFailed', {
  error: S.String,
  workspaceId: S.String,
})
export const PrefilledWorkspaceState = S.Union([
  PrefilledWorkspaceIdle,
  PrefilledWorkspaceLoading,
  PrefilledWorkspaceLoaded,
  PrefilledWorkspaceFailed,
])
export type PrefilledWorkspaceState = typeof PrefilledWorkspaceState.Type

export const AgentRunApiRepository = S.Struct({
  provider: S.String,
  owner: S.String,
  repo: S.String,
  ref: S.String,
})
export type AgentRunApiRepository = typeof AgentRunApiRepository.Type

export const AgentRunApiRun = S.Struct({
  id: S.String,
  runtime: S.String,
  backend: S.String,
  runnerId: S.String,
  userId: S.optionalKey(S.NullOr(S.String)),
  teamId: S.optionalKey(S.NullOr(S.String)),
  projectId: S.optionalKey(S.NullOr(S.String)),
  repository: AgentRunApiRepository,
  goal: S.String,
  externalRunId: S.NullOr(S.String),
  status: AgentRunStatus,
  eventCursor: S.Number,
  createdAt: S.String,
  updatedAt: S.String,
})
export type AgentRunApiRun = typeof AgentRunApiRun.Type

export const AgentRunApiEvent = S.Struct({
  id: S.String,
  parentId: S.String,
  sequence: S.Number,
  type: S.String,
  summary: S.String,
  status: S.NullOr(S.String),
  source: S.String,
  payloadJson: S.NullOr(S.String),
  artifactRefs: S.Array(S.String),
  externalEventId: S.NullOr(S.String),
  createdAt: S.String,
})
export type AgentRunApiEvent = typeof AgentRunApiEvent.Type

export const AgentGoalStatus = S.Literals([
  'pending',
  'active',
  'paused',
  'blocked',
  'completed',
  'failed',
  'usage_limited',
  'budget_limited',
  'archived',
])
export type AgentGoalStatus = typeof AgentGoalStatus.Type

export const AgentGoalVisibility = S.Literals(['private', 'team', 'public'])
export type AgentGoalVisibility = typeof AgentGoalVisibility.Type

export const AgentGoalApiGoal = S.Struct({
  id: S.String,
  agentId: S.String,
  userId: S.NullOr(S.String),
  teamId: S.NullOr(S.String),
  projectId: S.NullOr(S.String),
  objective: S.String,
  status: AgentGoalStatus,
  visibility: AgentGoalVisibility,
  currentRunId: S.NullOr(S.String),
  tokenBudget: S.NullOr(S.Int),
  tokensUsed: S.Int,
  timeUsedSeconds: S.Int,
  remainingTokens: S.NullOr(S.Int),
  createdAt: S.String,
  updatedAt: S.String,
  completedAt: S.NullOr(S.String),
  pausedAt: S.NullOr(S.String),
  blockedAt: S.NullOr(S.String),
  canEdit: S.Boolean,
  canPause: S.Boolean,
  canResume: S.Boolean,
  canMakePublic: S.Boolean,
  publicUrl: S.NullOr(S.String),
})
export type AgentGoalApiGoal = typeof AgentGoalApiGoal.Type

export const AgentGoalResponse = S.Struct({
  goal: S.NullOr(AgentGoalApiGoal),
})
export type AgentGoalResponse = typeof AgentGoalResponse.Type

export const AgentGoalAction = S.Literals([
  'pause',
  'resume',
  'clear',
  'make_public',
])
export type AgentGoalAction = typeof AgentGoalAction.Type

export const ArtanisOperatorApprovalAction = S.Literals(['approve', 'reject'])
export type ArtanisOperatorApprovalAction =
  typeof ArtanisOperatorApprovalAction.Type

export const AgentGoalPanelModel = ts('AgentGoalPanel', {
  budgetDraft: S.String,
  error: S.Option(S.String),
  goal: S.Option(AgentGoalApiGoal),
  isEditing: S.Boolean,
  objectiveDraft: S.String,
  pendingAction: S.Option(S.String),
  scopeKey: S.String,
})
export type AgentGoalPanelModel = typeof AgentGoalPanelModel.Type

export const ArtanisOperatorConsoleStatus = S.Struct({
  blockerRefs: S.Array(S.String),
  healthState: S.String,
  lastTickRef: S.NullOr(S.String),
  loopState: S.String,
  nextTickDisplay: S.NullOr(S.String),
  pendingApprovalCount: S.Number,
  publicationLagLabel: S.String,
  publicationLagState: S.String,
  runtimeState: S.String,
})
export type ArtanisOperatorConsoleStatus =
  typeof ArtanisOperatorConsoleStatus.Type

export const ArtanisOperatorConsoleCommand = S.Struct({
  blockerRefs: S.Array(S.String),
  commandRef: S.String,
  goalRef: S.String,
  kind: S.String,
  operatorReceiptRefs: S.Array(S.String),
  priority: S.Number,
  privateEvidenceRefs: S.Array(S.String),
  rawWorkroomRefs: S.Array(S.String),
  state: S.String,
  updatedAtDisplay: S.String,
})
export type ArtanisOperatorConsoleCommand =
  typeof ArtanisOperatorConsoleCommand.Type

export const ArtanisOperatorConsoleDecision = S.Struct({
  actionRef: S.String,
  authorityReceiptRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  decisionRef: S.String,
  operatorReceiptRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  rawWorkroomRefs: S.Array(S.String),
  state: S.String,
  updatedAtDisplay: S.String,
})
export type ArtanisOperatorConsoleDecision =
  typeof ArtanisOperatorConsoleDecision.Type

export const ArtanisOperatorConsoleApprovalGate = S.Struct({
  actionRef: S.String,
  authorityReceiptRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  effective: S.Boolean,
  expiresAtDisplay: S.String,
  gateRef: S.String,
  kind: S.String,
  label: S.String,
  operatorReceiptRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  rollbackPosture: S.String,
  rollbackRefs: S.Array(S.String),
  state: S.String,
  updatedAtDisplay: S.String,
})
export type ArtanisOperatorConsoleApprovalGate =
  typeof ArtanisOperatorConsoleApprovalGate.Type

export const ArtanisOperatorConsoleWorkProposal = S.Struct({
  approvalRequirementRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  capability: S.String,
  costCaveatRefs: S.Array(S.String),
  operatorDetailRefs: S.Array(S.String),
  proposalRef: S.String,
  resourceMode: S.String,
  risk: S.String,
  sourceEvidenceRefs: S.Array(S.String),
  spendLimitRefs: S.Array(S.String),
  state: S.String,
  target: S.String,
  updatedAtDisplay: S.String,
  workClass: S.String,
})
export type ArtanisOperatorConsoleWorkProposal =
  typeof ArtanisOperatorConsoleWorkProposal.Type

export const ArtanisOperatorConsoleForumIntent = S.Struct({
  blockerRefs: S.Array(S.String),
  deliveryState: S.String,
  intentRef: S.String,
  postRef: S.NullOr(S.String),
  targetForumRef: S.String,
  targetTopicRef: S.String,
  targetTopicState: S.String,
  updatedAtDisplay: S.String,
})
export type ArtanisOperatorConsoleForumIntent =
  typeof ArtanisOperatorConsoleForumIntent.Type

export const ArtanisOperatorConsoleRowSummary = S.Struct({
  kind: S.String,
  recordRef: S.String,
  state: S.String,
  updatedAtDisplay: S.String,
})
export type ArtanisOperatorConsoleRowSummary =
  typeof ArtanisOperatorConsoleRowSummary.Type

export const ArtanisOperatorConsoleResponse = S.Struct({
  agentId: S.String,
  consoleRef: S.String,
  status: ArtanisOperatorConsoleStatus,
  latestRows: S.Array(ArtanisOperatorConsoleRowSummary),
  steering: S.Struct({
    approvalDecisions: S.Array(ArtanisOperatorConsoleDecision),
    goalCommands: S.Array(ArtanisOperatorConsoleCommand),
    privateEvidencePackRefs: S.Array(S.String),
    rawWorkroomStateRefs: S.Array(S.String),
    supportedApprovalActions: S.Array(S.String),
    supportedGoalActions: S.Array(S.String),
  }),
  approvalGates: S.NullOr(
    S.Struct({
      effectiveGateRefs: S.Array(S.String),
      gateCount: S.Number,
      gates: S.Array(ArtanisOperatorConsoleApprovalGate),
    }),
  ),
  workRouting: S.NullOr(
    S.Struct({
      proposalCount: S.Number,
      proposals: S.Array(ArtanisOperatorConsoleWorkProposal),
      riskyProposalRefs: S.Array(S.String),
    }),
  ),
  publicationQueue: S.NullOr(
    S.Struct({
      deliverableIntentRefs: S.Array(S.String),
      deliveredCount: S.Number,
      intentCount: S.Number,
      intents: S.Array(ArtanisOperatorConsoleForumIntent),
    }),
  ),
})
export type ArtanisOperatorConsoleResponse =
  typeof ArtanisOperatorConsoleResponse.Type

export const ArtanisOperatorConsoleIdle = ts('ArtanisOperatorConsoleIdle', {})
export const ArtanisOperatorConsoleLoading = ts(
  'ArtanisOperatorConsoleLoading',
  {},
)
export const ArtanisOperatorConsoleLoaded = ts('ArtanisOperatorConsoleLoaded', {
  response: ArtanisOperatorConsoleResponse,
})
export const ArtanisOperatorConsoleFailed = ts('ArtanisOperatorConsoleFailed', {
  error: S.String,
})
export const ArtanisOperatorConsoleModel = S.Union([
  ArtanisOperatorConsoleIdle,
  ArtanisOperatorConsoleLoading,
  ArtanisOperatorConsoleLoaded,
  ArtanisOperatorConsoleFailed,
])
export type ArtanisOperatorConsoleModel =
  typeof ArtanisOperatorConsoleModel.Type

export const ArtanisOperatorDashboardThread = S.Struct({
  callerId: S.String,
  callerKind: S.String,
  createdAt: S.String,
  lastMessageAt: S.String,
  messageCount: S.Number,
  sourceRef: S.NullOr(S.String),
  status: S.String,
  subjectAgentKind: S.String,
  subjectAgentRef: S.String,
  threadRef: S.String,
  title: S.String,
  updatedAt: S.String,
})
export type ArtanisOperatorDashboardThread =
  typeof ArtanisOperatorDashboardThread.Type

export const ArtanisOperatorDashboardMessage = S.Struct({
  authorId: S.String,
  authorKind: S.String,
  body: S.String,
  callerId: S.String,
  createdAt: S.String,
  messageRef: S.String,
  threadRef: S.String,
})
export type ArtanisOperatorDashboardMessage =
  typeof ArtanisOperatorDashboardMessage.Type

export const ArtanisOperatorDashboardAccountUsageWindow = S.Struct({
  cap: S.NullOr(S.Number),
  label: S.Literals(['hourly', 'weekly']),
  percentUsed: S.Number,
  remaining: S.NullOr(S.Number),
  used: S.NullOr(S.Number),
})
export type ArtanisOperatorDashboardAccountUsageWindow =
  typeof ArtanisOperatorDashboardAccountUsageWindow.Type

export const ArtanisOperatorDashboardAccountUsageEntry = S.Struct({
  accountRefHash: S.String,
  cooldownExpiresAt: S.NullOr(S.String),
  isRateLimited: S.Boolean,
  manualResetsRemaining: S.NullOr(S.Number),
  provider: S.String,
  windows: S.Array(ArtanisOperatorDashboardAccountUsageWindow),
})
export type ArtanisOperatorDashboardAccountUsageEntry =
  typeof ArtanisOperatorDashboardAccountUsageEntry.Type

export const ArtanisOperatorDashboardAccountUsage = S.Struct({
  accounts: S.Array(ArtanisOperatorDashboardAccountUsageEntry),
  observedAt: S.String,
})
export type ArtanisOperatorDashboardAccountUsage =
  typeof ArtanisOperatorDashboardAccountUsage.Type

export const ArtanisOperatorDashboardResponse = S.Struct({
  accountUsage: S.optionalKey(ArtanisOperatorDashboardAccountUsage),
  callerIdFilter: S.NullOr(S.String),
  dashboardRef: S.String,
  messages: S.Array(ArtanisOperatorDashboardMessage),
  selectedThread: S.NullOr(ArtanisOperatorDashboardThread),
  threads: S.Array(ArtanisOperatorDashboardThread),
})
export type ArtanisOperatorDashboardResponse =
  typeof ArtanisOperatorDashboardResponse.Type

export const ArtanisOperatorDashboardIdle = ts(
  'ArtanisOperatorDashboardIdle',
  {},
)
export const ArtanisOperatorDashboardLoading = ts(
  'ArtanisOperatorDashboardLoading',
  {
    callerIdFilter: S.String,
    threadRef: S.String,
  },
)
export const ArtanisOperatorDashboardLoaded = ts(
  'ArtanisOperatorDashboardLoaded',
  {
    response: ArtanisOperatorDashboardResponse,
  },
)
export const ArtanisOperatorDashboardFailed = ts(
  'ArtanisOperatorDashboardFailed',
  {
    error: S.String,
  },
)
export const ArtanisOperatorDashboardModel = S.Union([
  ArtanisOperatorDashboardIdle,
  ArtanisOperatorDashboardLoading,
  ArtanisOperatorDashboardLoaded,
  ArtanisOperatorDashboardFailed,
])
export type ArtanisOperatorDashboardModel =
  typeof ArtanisOperatorDashboardModel.Type

export const ArtanisOperatorGoalPanelModel = ts('ArtanisOperatorGoalPanel', {
  error: S.Option(S.String),
  goal: S.Option(AgentGoalApiGoal),
  objectiveDraft: S.String,
  pendingAction: S.Option(S.String),
  scopeKey: S.String,
})
export type ArtanisOperatorGoalPanelModel =
  typeof ArtanisOperatorGoalPanelModel.Type

export const AgentRunDetailResponse = S.Struct({
  run: AgentRunApiRun,
  events: S.Array(AgentRunApiEvent),
})
export type AgentRunDetailResponse = typeof AgentRunDetailResponse.Type

export const AgentRunLaunchResponse = S.Struct({
  run: AgentRunApiRun,
  events: S.Array(AgentRunApiEvent),
  statusUrl: S.String,
  streamUrl: S.String,
})
export type AgentRunLaunchResponse = typeof AgentRunLaunchResponse.Type

export const TeamChatPostResponse = S.Struct({
  teamId: S.String,
  projectId: S.optionalKey(S.NullOr(S.String)),
  message: TeamChatMessageRecord,
  launchError: S.optionalKey(S.String),
  run: S.optionalKey(AgentRunApiRun),
  events: S.optionalKey(S.Array(AgentRunApiEvent)),
  statusUrl: S.optionalKey(S.String),
  streamUrl: S.optionalKey(S.String),
  threadId: S.optionalKey(S.String),
  threadUrl: S.optionalKey(S.String),
})
export type TeamChatPostResponse = typeof TeamChatPostResponse.Type

export const ChatRunEvent = S.Struct({
  id: S.String,
  sequence: S.Number,
  type: S.String,
  summary: S.String,
  status: S.Option(S.String),
  source: S.String,
  payloadJson: S.Option(S.String),
  artifactRefs: S.Array(S.String),
  externalEventId: S.Option(S.String),
  createdAt: S.String,
  tokenTotal: S.Number,
  tokenProvider: S.Option(S.String),
  tokenModel: S.Option(S.String),
})
export type ChatRunEvent = typeof ChatRunEvent.Type

export const ChatRunMetadata = S.Struct({
  runId: S.String,
  displayRunId: S.String,
  externalRunRef: AgentRunExternalRef,
  status: AgentRunStatus,
  runtime: S.String,
  backend: S.String,
  runnerId: S.String,
  repository: S.String,
  goal: S.String,
  eventCursor: S.Number,
  statusUrl: S.String,
  streamUrl: S.String,
  tokenTotal: S.Number,
  tokenUsageEvents: S.Number,
  createdAt: S.String,
  updatedAt: S.String,
})
export type ChatRunMetadata = typeof ChatRunMetadata.Type

export const IdleChatRun = ts('Idle', {})
export const LoadingChatRun = ts('Loading', {
  runId: S.String,
})
export const LaunchingChatRun = ts('Launching', {
  prompt: S.String,
  requestId: S.String,
})
export const ActiveChatRun = ts('Active', {
  metadata: ChatRunMetadata,
  events: S.Array(ChatRunEvent),
})
export const FailedChatRun = ts('Failed', {
  error: S.String,
})
export const ChatRun = S.Union([
  IdleChatRun,
  LoadingChatRun,
  LaunchingChatRun,
  ActiveChatRun,
  FailedChatRun,
])
export type ChatRun = typeof ChatRun.Type

export const ClosedRunMetadataDialog = ts('Closed', {})
export const OpenRunMetadataDialog = ts('Open', {})
export const RunMetadataDialog = S.Union([
  ClosedRunMetadataDialog,
  OpenRunMetadataDialog,
])
export type RunMetadataDialog = typeof RunMetadataDialog.Type

export const IdleBillingAction = ts('BillingIdle', {})
export const RedeemingBillingCoupon = ts('BillingRedeemingCoupon', {
  code: S.String,
})
export const OpeningBillingCheckout = ts('BillingOpeningCheckout', {
  packageId: S.String,
})
export const PreparingBillingCardSetup = ts('BillingPreparingCardSetup', {})
export const SavingBillingAutoTopUpPolicy = ts(
  'BillingSavingAutoTopUpPolicy',
  {},
)
export const RunningBillingAutoTopUp = ts('BillingRunningAutoTopUp', {})
export const SucceededBillingAction = ts('BillingSucceeded', {
  message: S.String,
})
export const FailedBillingAction = ts('BillingFailed', {
  error: S.String,
})
export const BillingAction = S.Union([
  IdleBillingAction,
  RedeemingBillingCoupon,
  OpeningBillingCheckout,
  PreparingBillingCardSetup,
  SavingBillingAutoTopUpPolicy,
  RunningBillingAutoTopUp,
  SucceededBillingAction,
  FailedBillingAction,
])
export type BillingAction = typeof BillingAction.Type

export const IdleProviderConnectionAction = ts('ProviderConnectionIdle', {})
export const StartingProviderDeviceLogin = ts('ProviderConnectionStarting', {})
export const PollingProviderDeviceLogin = ts('ProviderConnectionPolling', {
  attemptId: S.String,
})
export const SucceededProviderConnectionAction = ts(
  'ProviderConnectionSucceeded',
  {
    message: S.String,
  },
)
export const FailedProviderConnectionAction = ts('ProviderConnectionFailed', {
  error: S.String,
})
export const ProviderConnectionAction = S.Union([
  IdleProviderConnectionAction,
  StartingProviderDeviceLogin,
  PollingProviderDeviceLogin,
  SucceededProviderConnectionAction,
  FailedProviderConnectionAction,
])
export type ProviderConnectionAction = typeof ProviderConnectionAction.Type

export const ProviderAccountPoolReconnect = S.Struct({
  needed: S.Boolean,
  reason: S.NullOr(S.String),
})
export type ProviderAccountPoolReconnect =
  typeof ProviderAccountPoolReconnect.Type

export const ProviderAccountPoolAccount = S.Struct({
  providerAccountRef: S.String,
  provider: S.String,
  accountLabel: S.NullOr(S.String),
  status: S.String,
  health: S.String,
  eligibility: S.Literals(['eligible', 'ineligible']),
  eligibilityReasons: S.Array(S.String),
  operatorPriority: S.Number,
  activeLeaseCount: S.Number,
  leaseLimit: S.Number,
  cooldownUntil: S.NullOr(S.String),
  cooldownRemainingSeconds: S.NullOr(S.Number),
  lowCredit: S.Boolean,
  recentFailureClass: S.NullOr(S.String),
  lastSelectedAt: S.NullOr(S.String),
  lastSanityCheckAt: S.NullOr(S.String),
  lastSanityCheckResult: S.NullOr(S.String),
  lastParallelProbeAt: S.NullOr(S.String),
  lastParallelProbeResult: S.NullOr(S.String),
  lastSuccessfulLaunchAt: S.NullOr(S.String),
  lastFailedLaunchAt: S.NullOr(S.String),
  connectedAt: S.NullOr(S.String),
  reconnect: ProviderAccountPoolReconnect,
})
export type ProviderAccountPoolAccount = typeof ProviderAccountPoolAccount.Type

export const ProviderAccountPoolLease = S.Struct({
  leaseRef: S.String,
  providerAccountRef: S.String,
  provider: S.String,
  accountLabel: S.NullOr(S.String),
  requestedAction: S.String,
  runId: S.NullOr(S.String),
  assignmentId: S.NullOr(S.String),
  orderId: S.NullOr(S.String),
  startedAt: S.String,
  expiresAt: S.String,
  lastTouchedAt: S.NullOr(S.String),
  status: S.String,
})
export type ProviderAccountPoolLease = typeof ProviderAccountPoolLease.Type

export const ProviderAccountPoolNextSelection = S.Struct({
  status: S.Literals(['selected', 'none']),
  providerAccountRef: S.NullOr(S.String),
  provider: S.NullOr(S.String),
  accountLabel: S.NullOr(S.String),
  selectionReason: S.String,
  activeLeaseCount: S.NullOr(S.Number),
  leaseLimit: S.NullOr(S.Number),
})
export type ProviderAccountPoolNextSelection =
  typeof ProviderAccountPoolNextSelection.Type

export const ProviderAccountPoolSummary = S.Struct({
  total: S.Number,
  eligible: S.Number,
  activeLeaseCount: S.Number,
  lowCredit: S.Number,
  requiresReauth: S.Number,
  cooldown: S.Number,
  unhealthy: S.Number,
})
export type ProviderAccountPoolSummary = typeof ProviderAccountPoolSummary.Type

export const ProviderAccountPoolResponse = S.Struct({
  generatedAt: S.String,
  provider: S.String,
  policyVersion: S.String,
  accounts: S.Array(ProviderAccountPoolAccount),
  activeLeases: S.Array(ProviderAccountPoolLease),
  nextSelection: ProviderAccountPoolNextSelection,
  summary: ProviderAccountPoolSummary,
})
export type ProviderAccountPoolResponse =
  typeof ProviderAccountPoolResponse.Type

export const ProviderAccountPoolManualResetResponse = S.Struct({
  ok: S.Literal(true),
  providerAccountRef: S.String,
  resetAt: S.String,
})
export type ProviderAccountPoolManualResetResponse =
  typeof ProviderAccountPoolManualResetResponse.Type

export const ProviderAccountPoolIdle = ts('ProviderAccountPoolIdle', {})
export const ProviderAccountPoolLoading = ts('ProviderAccountPoolLoading', {})
export const ProviderAccountPoolLoaded = ts('ProviderAccountPoolLoaded', {
  response: ProviderAccountPoolResponse,
})
export const ProviderAccountPoolFailed = ts('ProviderAccountPoolFailed', {
  error: S.String,
})
export const ProviderAccountPoolState = S.Union([
  ProviderAccountPoolIdle,
  ProviderAccountPoolLoading,
  ProviderAccountPoolLoaded,
  ProviderAccountPoolFailed,
])
export type ProviderAccountPoolState = typeof ProviderAccountPoolState.Type

export const IdleOnboardingRepositories = ts('OnboardingRepositoriesIdle', {})
export const LoadingOnboardingRepositories = ts(
  'OnboardingRepositoriesLoading',
  {},
)
export const LoadedOnboardingRepositories = ts('OnboardingRepositoriesLoaded', {
  repositories: S.Array(OnboardingGitHubRepository),
  tokenStatus: S.Literals(['available', 'missing']),
})
export const FailedOnboardingRepositories = ts('OnboardingRepositoriesFailed', {
  error: S.String,
})
export const OnboardingRepositoryList = S.Union([
  IdleOnboardingRepositories,
  LoadingOnboardingRepositories,
  LoadedOnboardingRepositories,
  FailedOnboardingRepositories,
])
export type OnboardingRepositoryList = typeof OnboardingRepositoryList.Type

export const ONBOARDING_REPOSITORY_PAGE_SIZE = 6

const normalizedRepositoryQuery = (value: string): string =>
  value.trim().toLowerCase()

export const filteredOnboardingRepositories = (
  repositories: ReadonlyArray<OnboardingGitHubRepository>,
  search: string,
): ReadonlyArray<OnboardingGitHubRepository> => {
  const query = normalizedRepositoryQuery(search)

  return query === ''
    ? repositories
    : repositories.filter(repository =>
        [
          repository.fullName,
          repository.owner,
          repository.name,
          repository.description ?? '',
        ].some(value => value.toLowerCase().includes(query)),
      )
}

export const onboardingRepositoryPageCount = (
  repositories: ReadonlyArray<OnboardingGitHubRepository>,
  search: string,
): number =>
  Math.max(
    1,
    Math.ceil(
      filteredOnboardingRepositories(repositories, search).length /
        ONBOARDING_REPOSITORY_PAGE_SIZE,
    ),
  )

export const clampOnboardingRepositoryPageIndex = (
  pageIndex: number,
  repositories: ReadonlyArray<OnboardingGitHubRepository>,
  search: string,
): number =>
  Math.min(
    Math.max(0, pageIndex),
    onboardingRepositoryPageCount(repositories, search) - 1,
  )

export const IdleOnboardingAction = ts('OnboardingActionIdle', {})
export const SubmittingOnboardingAction = ts('OnboardingActionSubmitting', {
  label: S.String,
})
export const FailedOnboardingAction = ts('OnboardingActionFailed', {
  error: S.String,
})
export const OnboardingAction = S.Union([
  IdleOnboardingAction,
  SubmittingOnboardingAction,
  FailedOnboardingAction,
])
export type OnboardingAction = typeof OnboardingAction.Type

export const OnboardingFlowModel = ts('LoggedInOnboardingFlow', {
  action: OnboardingAction,
  goalValue: S.String,
  manualRepositoryName: S.String,
  manualRepositoryOwner: S.String,
  repositoryPageIndex: S.Number,
  repositorySearch: S.String,
  repositories: OnboardingRepositoryList,
  selectedRepositoryId: S.String,
})
export type OnboardingFlowModel = typeof OnboardingFlowModel.Type

export const CustomerOrderStatus = S.Literals([
  'submitted',
  'scoping',
  'free_slice_ready',
  'quote_ready',
  'agent_queued',
  'agent_running',
  'delivered',
  'needs_customer_input',
  'declined',
  'unavailable',
])
export type CustomerOrderStatus = typeof CustomerOrderStatus.Type

export const CustomerOrderRepository = S.Struct({
  provider: S.Literal('github'),
  owner: S.String,
  name: S.String,
  fullName: S.String,
  private: S.Boolean,
  defaultBranch: S.String,
  htmlUrl: S.String,
})
export type CustomerOrderRepository = typeof CustomerOrderRepository.Type

export const CustomerOrderSite = S.Struct({
  id: S.String,
  status: S.String,
  activeUrl: S.NullOr(S.String),
  activeVersionId: S.NullOr(S.String),
  activeDeploymentId: S.NullOr(S.String),
  latestSavedVersionId: S.NullOr(S.String),
  latestBuildStatus: S.NullOr(S.String),
  feedbackCount: S.Number,
  openFeedbackCount: S.Number,
})
export type CustomerOrderSite = typeof CustomerOrderSite.Type

export const CustomerSiteRevisionReviewState = S.Literals([
  'runtime_verified',
  'internal_draft',
  'customer_review_ready',
  'customer_accepted',
])
export type CustomerSiteRevisionReviewState =
  typeof CustomerSiteRevisionReviewState.Type

export const CustomerSiteRevision = S.Struct({
  id: S.String,
  siteId: S.String,
  buildStatus: S.String,
  deploymentId: S.NullOr(S.String),
  deploymentStatus: S.NullOr(S.String),
  url: S.NullOr(S.String),
  active: S.Boolean,
  sourceCommitSha: S.NullOr(S.String),
  sourceHash: S.NullOr(S.String),
  reviewState: CustomerSiteRevisionReviewState,
  originSummary: S.NullOr(S.String),
  originCreatedAt: S.NullOr(S.String),
  createdAt: S.String,
  savedAt: S.NullOr(S.String),
  activatedAt: S.NullOr(S.String),
})
export type CustomerSiteRevision = typeof CustomerSiteRevision.Type

export const CustomerSiteFeedbackStatus = S.Literals([
  'submitted',
  'queued',
  'running',
  'addressed',
  'closed',
  'rejected',
])
export type CustomerSiteFeedbackStatus = typeof CustomerSiteFeedbackStatus.Type

export const CustomerSiteFeedback = S.Struct({
  id: S.String,
  orderId: S.String,
  siteId: S.NullOr(S.String),
  versionId: S.NullOr(S.String),
  deploymentId: S.NullOr(S.String),
  body: S.String,
  status: CustomerSiteFeedbackStatus,
  createdAt: S.String,
  updatedAt: S.String,
})
export type CustomerSiteFeedback = typeof CustomerSiteFeedback.Type

export const CustomerSiteRevisionsResponse = S.Struct({
  revisions: S.Array(CustomerSiteRevision),
})
export type CustomerSiteRevisionsResponse =
  typeof CustomerSiteRevisionsResponse.Type

export const CustomerSiteFeedbackResponse = S.Struct({
  feedback: S.Array(CustomerSiteFeedback),
})
export type CustomerSiteFeedbackResponse =
  typeof CustomerSiteFeedbackResponse.Type

export const SubmitCustomerSiteFeedbackResponse = S.Struct({
  feedback: CustomerSiteFeedback,
})
export type SubmitCustomerSiteFeedbackResponse =
  typeof SubmitCustomerSiteFeedbackResponse.Type

export const CustomerSiteBuilderSessionStatus = S.Literals([
  'draft',
  'planning',
  'building',
  'preview_ready',
  'review_ready',
  'saved',
  'deploying',
  'deployed',
  'failed',
  'archived',
])
export type CustomerSiteBuilderSessionStatus =
  typeof CustomerSiteBuilderSessionStatus.Type

export const CustomerSiteBuilderActorKind = S.Literals([
  'customer',
  'agent',
  'operator',
  'system',
])
export type CustomerSiteBuilderActorKind =
  typeof CustomerSiteBuilderActorKind.Type

export const CustomerSiteBuilderPhaseKind = S.Literals([
  'planning',
  'foundation',
  'core',
  'styling',
  'integration',
  'optimization',
  'preview',
  'save',
  'deploy',
])
export type CustomerSiteBuilderPhaseKind =
  typeof CustomerSiteBuilderPhaseKind.Type

export const CustomerSiteBuilderPhaseStatus = S.Literals([
  'queued',
  'running',
  'succeeded',
  'failed',
  'blocked',
  'skipped',
])
export type CustomerSiteBuilderPhaseStatus =
  typeof CustomerSiteBuilderPhaseStatus.Type

export const CustomerSiteBuilderEventKind = S.Literals([
  'session_created',
  'message_added',
  'phase_started',
  'phase_updated',
  'phase_completed',
  'file_changed',
  'preview_created',
  'artifact_created',
  'build_failed',
  'build_repaired',
  'save_requested',
  'deploy_requested',
  'error',
])
export type CustomerSiteBuilderEventKind =
  typeof CustomerSiteBuilderEventKind.Type

export const CustomerSiteBuilderPhase = S.Struct({
  phaseKind: CustomerSiteBuilderPhaseKind,
  sequence: S.Number,
  status: CustomerSiteBuilderPhaseStatus,
  summary: S.String,
  title: S.String,
})
export type CustomerSiteBuilderPhase = typeof CustomerSiteBuilderPhase.Type

export const CustomerSiteBuilderMessage = S.Struct({
  actorKind: CustomerSiteBuilderActorKind,
  body: S.String,
  createdAt: S.String,
  id: S.String,
  sequence: S.Number,
})
export type CustomerSiteBuilderMessage = typeof CustomerSiteBuilderMessage.Type

export const CustomerSiteBuilderPreview = S.Struct({
  id: S.String,
  previewUrl: S.NullOr(S.String),
  status: S.String,
  updatedAt: S.String,
})
export type CustomerSiteBuilderPreview = typeof CustomerSiteBuilderPreview.Type

export const CustomerSiteBuilderSession = S.Struct({
  activePreview: S.NullOr(CustomerSiteBuilderPreview),
  activePreviewId: S.NullOr(S.String),
  createdAt: S.String,
  currentPhase: S.NullOr(CustomerSiteBuilderPhase),
  id: S.String,
  messages: S.Array(CustomerSiteBuilderMessage),
  orderId: S.NullOr(S.String),
  phases: S.Array(CustomerSiteBuilderPhase),
  promptSummary: S.String,
  siteId: S.NullOr(S.String),
  status: CustomerSiteBuilderSessionStatus,
  updatedAt: S.String,
})
export type CustomerSiteBuilderSession = typeof CustomerSiteBuilderSession.Type

export const CustomerSiteBuilderSessionResponse = S.Struct({
  siteBuilderSession: CustomerSiteBuilderSession,
})
export type CustomerSiteBuilderSessionResponse =
  typeof CustomerSiteBuilderSessionResponse.Type

export const CustomerSiteBuilderFile = S.Struct({
  byteSize: S.Number,
  contentHash: S.String,
  createdAt: S.String,
  hasPreview: S.Boolean,
  id: S.String,
  language: S.NullOr(S.String),
  path: S.String,
  sequence: S.Number,
  updatedAt: S.String,
  visibility: S.String,
})
export type CustomerSiteBuilderFile = typeof CustomerSiteBuilderFile.Type

export const CustomerSiteBuilderFileTreeItem = S.Struct({
  ...CustomerSiteBuilderFile.fields,
  segments: S.Array(S.String),
})
export type CustomerSiteBuilderFileTreeItem =
  typeof CustomerSiteBuilderFileTreeItem.Type

export const CustomerSiteBuilderFileListResponse = S.Struct({
  files: S.Array(CustomerSiteBuilderFile),
  siteBuilderSessionId: S.String,
})
export type CustomerSiteBuilderFileListResponse =
  typeof CustomerSiteBuilderFileListResponse.Type

export const CustomerSiteBuilderFileTreeResponse = S.Struct({
  fileTree: S.Array(CustomerSiteBuilderFileTreeItem),
  siteBuilderSessionId: S.String,
})
export type CustomerSiteBuilderFileTreeResponse =
  typeof CustomerSiteBuilderFileTreeResponse.Type

export const CustomerSiteBuilderFileRead = S.Struct({
  ...CustomerSiteBuilderFile.fields,
  previewText: S.NullOr(S.String),
})
export type CustomerSiteBuilderFileRead =
  typeof CustomerSiteBuilderFileRead.Type

export const CustomerSiteBuilderFileReadResponse = S.Struct({
  file: CustomerSiteBuilderFileRead,
  siteBuilderSessionId: S.String,
})
export type CustomerSiteBuilderFileReadResponse =
  typeof CustomerSiteBuilderFileReadResponse.Type

export const CustomerSiteBuilderEvent = S.Struct({
  createdAt: S.String,
  eventKind: CustomerSiteBuilderEventKind,
  id: S.String,
  phaseKind: S.NullOr(CustomerSiteBuilderPhaseKind),
  sequence: S.Number,
  status: CustomerSiteBuilderPhaseStatus,
  summary: S.String,
  title: S.String,
})
export type CustomerSiteBuilderEvent = typeof CustomerSiteBuilderEvent.Type

export const CustomerSiteBuilderEventsResponse = S.Struct({
  events: S.Array(CustomerSiteBuilderEvent),
  siteBuilderSessionId: S.String,
})
export type CustomerSiteBuilderEventsResponse =
  typeof CustomerSiteBuilderEventsResponse.Type

export const CustomerOrderTriageProjection = S.Struct({
  status: S.String,
  summary: S.String,
  nextAction: S.String,
})
export type CustomerOrderTriageProjection =
  typeof CustomerOrderTriageProjection.Type

export const CustomerOrderAdjutantStage = S.Literals([
  'queued',
  'running',
  'reviewing',
  'deployed',
  'waiting_for_input',
  'unavailable',
])
export type CustomerOrderAdjutantStage = typeof CustomerOrderAdjutantStage.Type

export const CustomerOrderAdjutantProgress = S.Struct({
  stage: CustomerOrderAdjutantStage,
  orderStatus: CustomerOrderStatus,
  siteStatus: S.NullOr(S.String),
  activeUrl: S.NullOr(S.String),
  adjustmentStatus: S.NullOr(S.String),
  reviewNeeded: S.Boolean,
  inputNeeded: S.Boolean,
  nextAction: S.String,
})
export type CustomerOrderAdjutantProgress =
  typeof CustomerOrderAdjutantProgress.Type

export const AdjutantUsageReceiptCategory = S.Literals([
  'generation',
  'build',
  'hosting',
  'storage',
  'adjustment',
])
export type AdjutantUsageReceiptCategory =
  typeof AdjutantUsageReceiptCategory.Type

export const AdjutantUsageReceiptBillingMode = S.Literals([
  'public_beta_free',
  'paid_credits',
])
export type AdjutantUsageReceiptBillingMode =
  typeof AdjutantUsageReceiptBillingMode.Type

export const AdjutantUsageReceiptCategoryTotal = S.Struct({
  category: AdjutantUsageReceiptCategory,
  creditsChargedCents: S.Number,
  creditsChargedFormatted: S.String,
  quantity: S.Number,
  receiptCount: S.Number,
  unit: S.NullOr(S.String),
})
export type AdjutantUsageReceiptCategoryTotal =
  typeof AdjutantUsageReceiptCategoryTotal.Type

export const AdjutantUsageReceiptSummary = S.Struct({
  billingMode: AdjutantUsageReceiptBillingMode,
  categories: S.Array(AdjutantUsageReceiptCategoryTotal),
  totalCreditsChargedCents: S.Number,
  totalCreditsChargedFormatted: S.String,
})
export type AdjutantUsageReceiptSummary =
  typeof AdjutantUsageReceiptSummary.Type

export const CustomerOrderUsageReceipt = S.Struct({
  billingMode: AdjutantUsageReceiptBillingMode,
  category: AdjutantUsageReceiptCategory,
  createdAt: S.String,
  creditsChargedCents: S.Number,
  creditsChargedFormatted: S.String,
  details: S.Record(S.String, S.Unknown),
  id: S.String,
  quantity: S.Number,
  summary: S.String,
  unit: S.String,
})
export type CustomerOrderUsageReceipt = typeof CustomerOrderUsageReceipt.Type

export const CustomerOrder = S.Struct({
  id: S.String,
  status: CustomerOrderStatus,
  visibility: S.Literal('public'),
  request: S.String,
  repository: S.NullOr(CustomerOrderRepository),
  site: S.NullOr(CustomerOrderSite),
  triage: S.NullOr(CustomerOrderTriageProjection),
  adjutant: CustomerOrderAdjutantProgress,
  usageReceipts: S.Array(CustomerOrderUsageReceipt),
  usageSummary: AdjutantUsageReceiptSummary,
  publicWorkAcknowledgedAt: S.String,
  dataUseAcknowledgedAt: S.String,
  computePaymentAcknowledgedAt: S.String,
  providerAccountRequired: S.Boolean,
  freeSliceCents: S.Number,
  quoteCents: S.NullOr(S.Number),
  createdAt: S.String,
  updatedAt: S.String,
})
export type CustomerOrder = typeof CustomerOrder.Type

export const CustomerOrderResponse = S.Struct({
  order: S.NullOr(CustomerOrder),
})
export type CustomerOrderResponse = typeof CustomerOrderResponse.Type

export const CustomerOrdersResponse = S.Struct({
  orders: S.Array(CustomerOrder),
})
export type CustomerOrdersResponse = typeof CustomerOrdersResponse.Type

export const CustomerFulfillmentArtifactKind = S.Literals([
  'pull_request',
  'branch',
  'commit',
  'diff',
  'preview',
  'notes',
  'attachment',
])
export type CustomerFulfillmentArtifactKind =
  typeof CustomerFulfillmentArtifactKind.Type

export const CustomerFulfillmentArtifactStatus = S.Literals([
  'draft',
  'customer_review_ready',
  'customer_accepted',
  'superseded',
  'rejected',
])
export type CustomerFulfillmentArtifactStatus =
  typeof CustomerFulfillmentArtifactStatus.Type

export const CustomerFulfillmentArtifact = S.Struct({
  id: S.String,
  orderId: S.String,
  kind: CustomerFulfillmentArtifactKind,
  title: S.String,
  summary: S.String,
  url: S.NullOr(S.String),
  repositoryFullName: S.NullOr(S.String),
  sourceBranch: S.NullOr(S.String),
  targetBranch: S.NullOr(S.String),
  commitSha: S.NullOr(S.String),
  status: CustomerFulfillmentArtifactStatus,
  createdAt: S.String,
  updatedAt: S.String,
})
export type CustomerFulfillmentArtifact =
  typeof CustomerFulfillmentArtifact.Type

export const CustomerFulfillmentArtifactsResponse = S.Struct({
  artifacts: S.Array(CustomerFulfillmentArtifact),
})
export type CustomerFulfillmentArtifactsResponse =
  typeof CustomerFulfillmentArtifactsResponse.Type

export const CustomerOrderIdle = ts('CustomerOrderIdle', {})
export const CustomerOrderLoading = ts('CustomerOrderLoading', {})
export const CustomerOrderLoaded = ts('CustomerOrderLoaded', {
  order: S.NullOr(CustomerOrder),
})
export const CustomerOrderFailed = ts('CustomerOrderFailed', {
  error: S.String,
})
export const CustomerOrderState = S.Union([
  CustomerOrderIdle,
  CustomerOrderLoading,
  CustomerOrderLoaded,
  CustomerOrderFailed,
])
export type CustomerOrderState = typeof CustomerOrderState.Type

export const CustomerOrdersIdle = ts('CustomerOrdersIdle', {})
export const CustomerOrdersLoading = ts('CustomerOrdersLoading', {})
export const CustomerOrdersLoaded = ts('CustomerOrdersLoaded', {
  orders: S.Array(CustomerOrder),
})
export const CustomerOrdersFailed = ts('CustomerOrdersFailed', {
  error: S.String,
})
export const CustomerOrdersState = S.Union([
  CustomerOrdersIdle,
  CustomerOrdersLoading,
  CustomerOrdersLoaded,
  CustomerOrdersFailed,
])
export type CustomerOrdersState = typeof CustomerOrdersState.Type

export const CustomerOrderCreateIdle = ts('CustomerOrderCreateIdle', {})
export const CustomerOrderCreateSubmitting = ts(
  'CustomerOrderCreateSubmitting',
  {},
)
export const CustomerOrderCreateSucceeded = ts('CustomerOrderCreateSucceeded', {
  order: CustomerOrder,
})
export const CustomerOrderCreateFailed = ts('CustomerOrderCreateFailed', {
  error: S.String,
})
export const CustomerOrderCreateState = S.Union([
  CustomerOrderCreateIdle,
  CustomerOrderCreateSubmitting,
  CustomerOrderCreateSucceeded,
  CustomerOrderCreateFailed,
])
export type CustomerOrderCreateState = typeof CustomerOrderCreateState.Type

export const CustomerFulfillmentArtifactsIdle = ts(
  'CustomerFulfillmentArtifactsIdle',
  {},
)
export const CustomerFulfillmentArtifactsLoading = ts(
  'CustomerFulfillmentArtifactsLoading',
  {},
)
export const CustomerFulfillmentArtifactsLoaded = ts(
  'CustomerFulfillmentArtifactsLoaded',
  {
    artifacts: S.Array(CustomerFulfillmentArtifact),
  },
)
export const CustomerFulfillmentArtifactsFailed = ts(
  'CustomerFulfillmentArtifactsFailed',
  {
    error: S.String,
  },
)
export const CustomerFulfillmentArtifactsState = S.Union([
  CustomerFulfillmentArtifactsIdle,
  CustomerFulfillmentArtifactsLoading,
  CustomerFulfillmentArtifactsLoaded,
  CustomerFulfillmentArtifactsFailed,
])
export type CustomerFulfillmentArtifactsState =
  typeof CustomerFulfillmentArtifactsState.Type

export const AUTOPILOT_WORK_LIST_PROMISE_ID = 'autopilot.mission_briefing.v1'

export const AutopilotWorkState = S.Literals([
  'access_required',
  'accepted',
  'accepted_free_slice',
  'blocked',
  'delivered',
  'invalid',
  'paid_ready',
  'payment_required',
  'queued_or_running',
  'rejected',
  'revision_required',
  'scheduled',
])
export type AutopilotWorkState = typeof AutopilotWorkState.Type

export const AutopilotWorkEventKind = S.Literals([
  'accepted',
  'blocked',
  'delivered',
  'needs_access',
  'payment_required',
  'queued',
  'rejected',
  'revision_required',
  'running',
  'scheduled',
  'settled',
])
export type AutopilotWorkEventKind = typeof AutopilotWorkEventKind.Type

export const AutopilotMorningReportGroup = S.Literals([
  'awaiting_decision',
  'blocked',
  'launched',
  'reviewed',
  'running',
  'scheduled',
])
export type AutopilotMorningReportGroup =
  typeof AutopilotMorningReportGroup.Type

export const AutopilotMorningReportWorkItem = S.Struct({
  group: AutopilotMorningReportGroup,
  scheduledLaunchAt: S.NullOr(S.String),
  state: AutopilotWorkState,
  taskRefs: S.Array(S.String),
  updatedAt: S.String,
  workOrderRef: S.String,
})
export type AutopilotMorningReportWorkItem =
  typeof AutopilotMorningReportWorkItem.Type

export const AutopilotMorningReportContinuation = S.Struct({
  attempt: S.Number,
  decision: S.Literals(['dispatched', 'failed', 'skipped']),
  mode: S.Literals(['follow_up_turn', 'goal_continuation']),
  occurredAt: S.String,
  reasonRef: S.String,
  runId: S.String,
})
export type AutopilotMorningReportContinuation =
  typeof AutopilotMorningReportContinuation.Type

export const AutopilotMorningReport = S.Struct({
  continuations: S.Array(AutopilotMorningReportContinuation),
  counts: S.Struct({
    awaitingDecision: S.Number,
    blocked: S.Number,
    continuations: S.Number,
    launched: S.Number,
    reviewed: S.Number,
    running: S.Number,
    scheduled: S.Number,
  }),
  generatedAt: S.String,
  reportRef: S.String,
  sinceIso: S.String,
  workItems: S.Array(AutopilotMorningReportWorkItem),
})
export type AutopilotMorningReport = typeof AutopilotMorningReport.Type

export const AutopilotMorningReportResponse = S.Struct({
  report: AutopilotMorningReport,
})
export type AutopilotMorningReportResponse =
  typeof AutopilotMorningReportResponse.Type

export const AutopilotMorningReportIdle = ts('AutopilotMorningReportIdle', {})
export const AutopilotMorningReportLoading = ts(
  'AutopilotMorningReportLoading',
  {},
)
export const AutopilotMorningReportLoaded = ts('AutopilotMorningReportLoaded', {
  response: AutopilotMorningReportResponse,
})
export const AutopilotMorningReportFailed = ts('AutopilotMorningReportFailed', {
  error: S.String,
})
export const AutopilotMorningReportState = S.Union([
  AutopilotMorningReportIdle,
  AutopilotMorningReportLoading,
  AutopilotMorningReportLoaded,
  AutopilotMorningReportFailed,
])
export type AutopilotMorningReportState =
  typeof AutopilotMorningReportState.Type

export const AutopilotWorkReviewAction = S.Literals([
  'accept',
  'reject',
  'request_changes',
])
export type AutopilotWorkReviewAction = typeof AutopilotWorkReviewAction.Type

export const AutopilotWorkPromiseRef = S.Struct({
  blockerRefs: S.Array(S.String),
  promiseId: S.String,
  registryVersion: S.NullOr(S.String),
})
export type AutopilotWorkPromiseRef = typeof AutopilotWorkPromiseRef.Type

export const AutopilotWorkRoutingSummary = S.Struct({
  availabilityState: S.Literals(['needs_input', 'retry_later', 'selected']),
  buyerDebitRequired: S.Boolean,
  fallbackLeaseIntentCount: S.Number,
  fallbackRunnerKind: S.NullOr(S.String),
  laneRef: S.NullOr(S.String),
  meterKind: S.NullOr(S.String),
  pylonAssignmentIntentCount: S.Number,
  selectedRunnerKind: S.NullOr(S.String),
  source: S.Literals(['fallback', 'none_available', 'requester_pylon']),
})
export type AutopilotWorkRoutingSummary =
  typeof AutopilotWorkRoutingSummary.Type

export const AutopilotWorkSummary = S.Struct({
  createdAt: S.String,
  generatedAt: S.optionalKey(S.String),
  issueRefs: S.optionalKey(S.Array(S.String)),
  promiseRef: AutopilotWorkPromiseRef,
  routing: S.optionalKey(AutopilotWorkRoutingSummary),
  state: AutopilotWorkState,
  taskRefs: S.optionalKey(S.Array(S.String)),
  updatedAt: S.String,
  workOrderRef: S.String,
})
export type AutopilotWorkSummary = typeof AutopilotWorkSummary.Type

export const AutopilotWorkListResponse = S.Struct({
  generatedAt: S.String,
  promiseId: S.String,
  workOrders: S.Array(AutopilotWorkSummary),
})
export type AutopilotWorkListResponse = typeof AutopilotWorkListResponse.Type

export const AutopilotWorkExecutionCloseout = S.Struct({
  acceptedWorkAuthority: S.Boolean,
  artifactRefs: S.optionalKey(S.Array(S.String)),
  assignmentRefs: S.Array(S.String),
  authorityReceiptRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  buildRefs: S.optionalKey(S.Array(S.String)),
  changeCaptureRefs: S.optionalKey(S.Array(S.String)),
  changeCaptureStatus: S.optionalKey(
    S.Literals(['blocked', 'review_ready', 'stale']),
  ),
  closeoutRefs: S.Array(S.String),
  deliveryReadinessFreshness: S.optionalKey(S.Literals(['fresh', 'stale'])),
  deliveryReadinessRefs: S.optionalKey(S.Array(S.String)),
  deliveryReadinessStatus: S.optionalKey(
    S.Literals(['blocked', 'ready', 'scoped_exception']),
  ),
  fileCount: S.optionalKey(S.Number),
  forumAutoPublishAllowed: S.Boolean,
  addedLineCount: S.optionalKey(S.Number),
  patchDigestRef: S.optionalKey(S.NullOr(S.String)),
  previewRefs: S.optionalKey(S.Array(S.String)),
  proofRefs: S.Array(S.String),
  publicSafe: S.Boolean,
  removedLineCount: S.optionalKey(S.Number),
  resultRefs: S.Array(S.String),
  reviewCaveatRefs: S.optionalKey(S.Array(S.String)),
  runnerKind: S.String,
  summaryRefs: S.optionalKey(S.Array(S.String)),
  testRefs: S.optionalKey(S.Array(S.String)),
  verificationRefs: S.optionalKey(S.Array(S.String)),
  worktreeIdentityStatus: S.optionalKey(S.Literals(['blocked', 'ready', 'stale'])),
  writebackRequired: S.optionalKey(S.Boolean),
  workerPayoutAuthority: S.Boolean,
})
export type AutopilotWorkExecutionCloseout =
  typeof AutopilotWorkExecutionCloseout.Type

export const AutopilotWorkSessionState = S.Literals([
  'cancelled',
  'completed',
  'failed',
  'queued',
  'running',
  'unknown',
])
export type AutopilotWorkSessionState = typeof AutopilotWorkSessionState.Type

export const AutopilotWorkSessionControlAction = S.Literals([
  'cancel',
  'fork',
  'resume',
  'rewind',
])
export type AutopilotWorkSessionControlAction =
  typeof AutopilotWorkSessionControlAction.Type

export const AutopilotWorkSessionControlFreshness = S.Literals([
  'fresh',
  'stale',
])
export type AutopilotWorkSessionControlFreshness =
  typeof AutopilotWorkSessionControlFreshness.Type

export const AutopilotWorkSessionControlOutcome = S.Literals([
  'applied',
  'blocked',
  'queued',
  'stale',
])
export type AutopilotWorkSessionControlOutcome =
  typeof AutopilotWorkSessionControlOutcome.Type

export const AutopilotWorkSessionSummary = S.Struct({
  artifactRefs: S.optionalKey(S.Array(S.String)),
  bridgeRefs: S.optionalKey(S.Array(S.String)),
  checkpointRefs: S.optionalKey(S.Array(S.String)),
  controlAuthorityRefs: S.optionalKey(S.Array(S.String)),
  controlBlockerRefs: S.optionalKey(S.Array(S.String)),
  controlFreshness: S.optionalKey(AutopilotWorkSessionControlFreshness),
  controlPolicyRefs: S.optionalKey(S.Array(S.String)),
  eventRefs: S.optionalKey(S.Array(S.String)),
  observedAt: S.optionalKey(S.NullOr(S.String)),
  sessionRef: S.String,
  state: S.optionalKey(AutopilotWorkSessionState),
  supportedControlActions: S.optionalKey(S.Array(AutopilotWorkSessionControlAction)),
  title: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkSessionSummary =
  typeof AutopilotWorkSessionSummary.Type

export const AutopilotWorkSessionControlReceipt = S.Struct({
  action: AutopilotWorkSessionControlAction,
  actorRef: S.String,
  blockerRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.String,
  outcome: AutopilotWorkSessionControlOutcome,
  provenanceRefs: S.optionalKey(S.Array(S.String)),
  publicSafe: S.Boolean,
  receiptRef: S.String,
  requestRef: S.String,
  sessionRef: S.String,
})
export type AutopilotWorkSessionControlReceipt =
  typeof AutopilotWorkSessionControlReceipt.Type

export const AutopilotWorkSessionNavigation = S.Struct({
  bridgeSessions: S.optionalKey(S.Array(AutopilotWorkSessionSummary)),
  claudeSessions: S.optionalKey(S.Array(AutopilotWorkSessionSummary)),
  codexSessions: S.optionalKey(S.Array(AutopilotWorkSessionSummary)),
  controlReceipts: S.optionalKey(S.Array(AutopilotWorkSessionControlReceipt)),
  localPylonSessions: S.optionalKey(S.Array(AutopilotWorkSessionSummary)),
})
export type AutopilotWorkSessionNavigation =
  typeof AutopilotWorkSessionNavigation.Type

export const AutopilotWorkDoctorSeverity = S.Literals([
  'error',
  'info',
  'ok',
  'warning',
])
export type AutopilotWorkDoctorSeverity =
  typeof AutopilotWorkDoctorSeverity.Type

export const AutopilotWorkDoctorCategory = S.Literals([
  'extension',
  'install',
  'integration',
  'keybinding',
  'network',
  'sandbox',
  'search',
  'settings',
  'shell',
  'update',
])
export type AutopilotWorkDoctorCategory =
  typeof AutopilotWorkDoctorCategory.Type

export const AutopilotWorkSupportBundleConsent = S.Literals([
  'consented',
  'declined',
  'pending',
])
export type AutopilotWorkSupportBundleConsent =
  typeof AutopilotWorkSupportBundleConsent.Type

export const AutopilotWorkDoctorCheck = S.Struct({
  category: S.optionalKey(AutopilotWorkDoctorCategory),
  checkRef: S.String,
  evidenceRefs: S.optionalKey(S.Array(S.String)),
  fixRefs: S.optionalKey(S.Array(S.String)),
  severity: S.optionalKey(AutopilotWorkDoctorSeverity),
})
export type AutopilotWorkDoctorCheck = typeof AutopilotWorkDoctorCheck.Type

export const AutopilotWorkSupportBundleSection = S.Struct({
  consent: S.optionalKey(AutopilotWorkSupportBundleConsent),
  evidenceRefs: S.optionalKey(S.Array(S.String)),
  sectionRef: S.String,
})
export type AutopilotWorkSupportBundleSection =
  typeof AutopilotWorkSupportBundleSection.Type

export const AutopilotWorkSupportDiagnostics = S.Struct({
  diagnosticLogRefs: S.optionalKey(S.Array(S.String)),
  doctorChecks: S.optionalKey(S.Array(AutopilotWorkDoctorCheck)),
  helpCommandRefs: S.optionalKey(S.Array(S.String)),
  preflightRefs: S.optionalKey(S.Array(S.String)),
  supportBundleSections: S.optionalKey(S.Array(AutopilotWorkSupportBundleSection)),
})
export type AutopilotWorkSupportDiagnostics =
  typeof AutopilotWorkSupportDiagnostics.Type

export const AutopilotWorkContextFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkContextFreshness =
  typeof AutopilotWorkContextFreshness.Type

export const AutopilotWorkContextDirtyState = S.Literals([
  'clean',
  'dirty',
  'unknown',
])
export type AutopilotWorkContextDirtyState =
  typeof AutopilotWorkContextDirtyState.Type

export const AutopilotWorkContextRefGroup = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  refs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkContextRefGroup =
  typeof AutopilotWorkContextRefGroup.Type

export const AutopilotWorkContextRepo = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  changedCount: S.optionalKey(S.NullOr(S.Number)),
  dirtyState: S.optionalKey(AutopilotWorkContextDirtyState),
  dirtyStateRefs: S.optionalKey(S.Array(S.String)),
  identityRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkContextRepo = typeof AutopilotWorkContextRepo.Type

export const AutopilotWorkContextAdapters = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  refs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkContextAdapters =
  typeof AutopilotWorkContextAdapters.Type

export const AutopilotWorkContextCurrentJob = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  jobRefs: S.optionalKey(S.Array(S.String)),
  verificationRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkContextCurrentJob =
  typeof AutopilotWorkContextCurrentJob.Type

export const AutopilotWorkContextInstructions = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  configRefs: S.optionalKey(S.Array(S.String)),
  refs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkContextInstructions =
  typeof AutopilotWorkContextInstructions.Type

export const AutopilotWorkRepositoryProfileKind = S.Literals([
  'command',
  'instruction',
  'invariant',
  'test',
])
export type AutopilotWorkRepositoryProfileKind =
  typeof AutopilotWorkRepositoryProfileKind.Type

export const AutopilotWorkRepositoryProfileRefreshEvent = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  changedProfileKinds: S.optionalKey(S.Array(AutopilotWorkRepositoryProfileKind)),
  commandProfileRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkContextFreshness),
  generatedAt: S.String,
  instructionRefs: S.optionalKey(S.Array(S.String)),
  invariantRefs: S.optionalKey(S.Array(S.String)),
  refreshedAt: S.optionalKey(S.NullOr(S.String)),
  repoIdentityRefs: S.optionalKey(S.Array(S.String)),
  testProfileRefs: S.optionalKey(S.Array(S.String)),
  workOrderRef: S.String,
})
export type AutopilotWorkRepositoryProfileRefreshEvent =
  typeof AutopilotWorkRepositoryProfileRefreshEvent.Type

export const AutopilotWorkRepositoryMemoryProfile = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  blockedClaimRefs: S.optionalKey(S.Array(S.String)),
  changedProfileKinds: S.optionalKey(S.Array(AutopilotWorkRepositoryProfileKind)),
  commandProfileRefs: S.optionalKey(S.Array(S.String)),
  corpusManifestRef: S.optionalKey(S.NullOr(S.String)),
  currentInstructionRefs: S.optionalKey(S.Array(S.String)),
  datasetRefs: S.optionalKey(S.Array(S.String)),
  devDoctorRefs: S.optionalKey(S.Array(S.String)),
  dirtyState: S.optionalKey(AutopilotWorkContextDirtyState),
  freshness: S.optionalKey(AutopilotWorkContextFreshness),
  generatedAt: S.String,
  holdoutEvaluationRef: S.optionalKey(S.NullOr(S.String)),
  instructionRefs: S.optionalKey(S.Array(S.String)),
  invariantRefs: S.optionalKey(S.Array(S.String)),
  privateValidationTrendRef: S.optionalKey(S.NullOr(S.String)),
  profileRef: S.String,
  publicRetainedScoreRef: S.optionalKey(S.NullOr(S.String)),
  refreshedAt: S.optionalKey(S.NullOr(S.String)),
  refreshEvents: S.optionalKey(S.Array(AutopilotWorkRepositoryProfileRefreshEvent)),
  refreshReceiptRefs: S.optionalKey(S.Array(S.String)),
  repoIdentityRefs: S.optionalKey(S.Array(S.String)),
  studyPacketFreshness: S.optionalKey(AutopilotWorkContextFreshness),
  studyPacketRef: S.optionalKey(S.NullOr(S.String)),
  testProfileRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkRepositoryMemoryProfile =
  typeof AutopilotWorkRepositoryMemoryProfile.Type

export const AutopilotWorkContextSnapshot = S.Struct({
  adapters: S.optionalKey(AutopilotWorkContextAdapters),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  currentJob: S.optionalKey(AutopilotWorkContextCurrentJob),
  devDoctor: S.optionalKey(AutopilotWorkContextRefGroup),
  freshness: S.optionalKey(AutopilotWorkContextFreshness),
  instructions: S.optionalKey(AutopilotWorkContextInstructions),
  observedAt: S.optionalKey(S.NullOr(S.String)),
  repo: S.optionalKey(AutopilotWorkContextRepo),
  repositoryMemoryProfile: S.optionalKey(AutopilotWorkRepositoryMemoryProfile),
})
export type AutopilotWorkContextSnapshot =
  typeof AutopilotWorkContextSnapshot.Type

export const AutopilotWorkRetrievalMode = S.Literals([
  'exact',
  'hybrid',
  'model_selected',
  'semantic',
  'structured',
])
export type AutopilotWorkRetrievalMode =
  typeof AutopilotWorkRetrievalMode.Type

export const AutopilotWorkRetrievalFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkRetrievalFreshness =
  typeof AutopilotWorkRetrievalFreshness.Type

export const AutopilotWorkRetrievalSkipReason = S.Literals([
  'duplicate',
  'filtered_private',
  'low_score',
  'missing_source',
  'stale',
  'unsupported_mode',
])
export type AutopilotWorkRetrievalSkipReason =
  typeof AutopilotWorkRetrievalSkipReason.Type

export const AutopilotWorkRetrievalCandidate = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  candidateRef: S.String,
  freshness: S.optionalKey(AutopilotWorkRetrievalFreshness),
  mode: S.optionalKey(AutopilotWorkRetrievalMode),
  provenanceRefs: S.optionalKey(S.Array(S.String)),
  rank: S.optionalKey(S.NullOr(S.Number)),
  score: S.optionalKey(S.NullOr(S.Number)),
  sourceRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkRetrievalCandidate =
  typeof AutopilotWorkRetrievalCandidate.Type

export const AutopilotWorkRetrievalSkippedCandidate = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  candidateRef: S.String,
  reason: AutopilotWorkRetrievalSkipReason,
  sourceRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkRetrievalSkippedCandidate =
  typeof AutopilotWorkRetrievalSkippedCandidate.Type

export const AutopilotWorkLiveRetrievalSourceKind = S.Literals([
  'diagnostic',
  'documentation',
  'file',
  'unsupported',
])
export type AutopilotWorkLiveRetrievalSourceKind =
  typeof AutopilotWorkLiveRetrievalSourceKind.Type

export const AutopilotWorkLiveRetrievalSource = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  candidateRef: S.String,
  exactRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkRetrievalFreshness),
  provenanceRefs: S.optionalKey(S.Array(S.String)),
  score: S.optionalKey(S.NullOr(S.Number)),
  sourceKind: AutopilotWorkLiveRetrievalSourceKind,
  sourceRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkLiveRetrievalSource =
  typeof AutopilotWorkLiveRetrievalSource.Type

export const AutopilotWorkLiveRetrievalAdapter = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkRetrievalFreshness),
  generatedAt: S.optionalKey(S.String),
  minimumScore: S.optionalKey(S.Number),
  mode: S.optionalKey(AutopilotWorkRetrievalMode),
  planRef: S.optionalKey(S.String),
  providerEvidenceRefs: S.optionalKey(S.Array(S.String)),
  queryRefs: S.optionalKey(S.Array(S.String)),
  requestRef: S.optionalKey(S.String),
  sourceRefs: S.optionalKey(S.Array(S.String)),
  sources: S.optionalKey(S.Array(AutopilotWorkLiveRetrievalSource)),
  workspaceBoundaryRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkLiveRetrievalAdapter =
  typeof AutopilotWorkLiveRetrievalAdapter.Type

export const AutopilotWorkRetrievalPlan = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  candidates: S.optionalKey(S.Array(AutopilotWorkRetrievalCandidate)),
  freshness: S.optionalKey(AutopilotWorkRetrievalFreshness),
  generatedAt: S.optionalKey(S.String),
  liveAdapter: S.optionalKey(AutopilotWorkLiveRetrievalAdapter),
  mode: S.optionalKey(AutopilotWorkRetrievalMode),
  planRef: S.optionalKey(S.String),
  queryRefs: S.optionalKey(S.Array(S.String)),
  requestRef: S.optionalKey(S.String),
  skippedCandidates: S.optionalKey(S.Array(AutopilotWorkRetrievalSkippedCandidate)),
  sourceRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkRetrievalPlan =
  typeof AutopilotWorkRetrievalPlan.Type

export const AutopilotWorkExtensibilityDomain = S.Literals([
  'hooks',
  'mcp',
  'plugins',
  'skills',
])
export type AutopilotWorkExtensibilityDomain =
  typeof AutopilotWorkExtensibilityDomain.Type

export const AutopilotWorkExtensibilityEffectiveState = S.Literals([
  'blocked',
  'disabled',
  'enabled',
  'needs_auth',
  'needs_trust',
  'pending',
])
export type AutopilotWorkExtensibilityEffectiveState =
  typeof AutopilotWorkExtensibilityEffectiveState.Type

export const AutopilotWorkExtensibilityFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkExtensibilityFreshness =
  typeof AutopilotWorkExtensibilityFreshness.Type

export const AutopilotWorkExtensibilityConfigEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  catalogRefs: S.optionalKey(S.Array(S.String)),
  configRefs: S.optionalKey(S.Array(S.String)),
  domain: AutopilotWorkExtensibilityDomain,
  effectiveState: S.optionalKey(AutopilotWorkExtensibilityEffectiveState),
  freshness: S.optionalKey(AutopilotWorkExtensibilityFreshness),
  policyRefs: S.optionalKey(S.Array(S.String)),
  sourceRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkExtensibilityConfigEntry =
  typeof AutopilotWorkExtensibilityConfigEntry.Type

export const AutopilotWorkExtensibilityEffectiveConfig = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  configRef: S.String,
  entries: S.optionalKey(S.Array(AutopilotWorkExtensibilityConfigEntry)),
  freshness: S.optionalKey(AutopilotWorkExtensibilityFreshness),
  generatedAt: S.optionalKey(S.String),
  workOrderRef: S.optionalKey(S.String),
})
export type AutopilotWorkExtensibilityEffectiveConfig =
  typeof AutopilotWorkExtensibilityEffectiveConfig.Type

export const AutopilotWorkExtensibilityExecutionRequestKind = S.Literals([
  'hook_enablement',
  'mcp_resource_read',
  'mcp_tool_call',
  'plugin_activation',
  'settings_activation',
  'skill_body_disclosure',
])
export type AutopilotWorkExtensibilityExecutionRequestKind =
  typeof AutopilotWorkExtensibilityExecutionRequestKind.Type

export const AutopilotWorkExtensibilityExecutionRequest = S.Struct({
  actorRef: S.optionalKey(S.NullOr(S.String)),
  authRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  catalogRefs: S.optionalKey(S.Array(S.String)),
  configRefs: S.optionalKey(S.Array(S.String)),
  domain: AutopilotWorkExtensibilityDomain,
  explicitDisclosure: S.optionalKey(S.Boolean),
  failureRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.optionalKey(S.String),
  observedState: S.optionalKey(S.NullOr(S.Literals(['failed']))),
  policyRefs: S.optionalKey(S.Array(S.String)),
  providerAccountRefs: S.optionalKey(S.Array(S.String)),
  requestKind: AutopilotWorkExtensibilityExecutionRequestKind,
  requestRef: S.String,
  sourceRefs: S.optionalKey(S.Array(S.String)),
  targetRef: S.String,
  workspaceTrustRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkExtensibilityExecutionRequest =
  typeof AutopilotWorkExtensibilityExecutionRequest.Type

export const AutopilotWorkExtensibility = S.Struct({
  effectiveConfig: S.optionalKey(AutopilotWorkExtensibilityEffectiveConfig),
  executionRequests: S.optionalKey(
    S.Array(AutopilotWorkExtensibilityExecutionRequest),
  ),
})
export type AutopilotWorkExtensibility = typeof AutopilotWorkExtensibility.Type

export const AutopilotWorkPlanMutationAction = S.Literals([
  'add',
  'block',
  'complete',
  'unblock',
  'update',
])
export type AutopilotWorkPlanMutationAction =
  typeof AutopilotWorkPlanMutationAction.Type

export const AutopilotWorkPlanMutationState = S.Literals([
  'applied',
  'blocked',
  'requested',
  'stale',
])
export type AutopilotWorkPlanMutationState =
  typeof AutopilotWorkPlanMutationState.Type

export const AutopilotWorkPlanMutationRequest = S.Struct({
  action: AutopilotWorkPlanMutationAction,
  actorRef: S.String,
  generatedAt: S.String,
  itemRef: S.String,
  provenanceRefs: S.optionalKey(S.Array(S.String)),
  publicSafe: S.Boolean,
  requestRef: S.String,
})
export type AutopilotWorkPlanMutationRequest =
  typeof AutopilotWorkPlanMutationRequest.Type

export const AutopilotWorkPlanMutationReceipt = S.Struct({
  action: AutopilotWorkPlanMutationAction,
  actorRef: S.String,
  blockerRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.String,
  itemRef: S.String,
  provenanceRefs: S.optionalKey(S.Array(S.String)),
  publicSafe: S.Boolean,
  receiptRef: S.String,
  requestRef: S.String,
  state: AutopilotWorkPlanMutationState,
})
export type AutopilotWorkPlanMutationReceipt =
  typeof AutopilotWorkPlanMutationReceipt.Type

export const AutopilotWorkErrorCategory = S.Literals([
  'ApprovalUnavailable',
  'ArtifactWriteFailed',
  'ContextAssemblyFailed',
  'ContextTooLarge',
  'ExternalAdapterFailed',
  'InputInvalid',
  'InternalBug',
  'InvariantViolation',
  'ModelOutputInvalid',
  'ModelRequestFailed',
  'ModelStreamTimeout',
  'NetworkPermanent',
  'NetworkTransient',
  'PermissionDenied',
  'ProcessKilled',
  'ProcessTimeout',
  'ProviderAuthFailed',
  'ProviderOverloaded',
  'ProviderRateLimited',
  'ResumeConflict',
  'StorageCorrupt',
  'StorageReadFailed',
  'StorageWriteFailed',
  'TaskFailed',
  'ToolExecutionFailed',
  'ToolValidationFailed',
  'WorkspaceBoundaryViolation',
])
export type AutopilotWorkErrorCategory =
  typeof AutopilotWorkErrorCategory.Type

export const AutopilotWorkErrorSeverity = S.Literals([
  'error',
  'fatal',
  'info',
  'warning',
])
export type AutopilotWorkErrorSeverity =
  typeof AutopilotWorkErrorSeverity.Type

export const AutopilotWorkErrorRetryability = S.Literals([
  'conditional',
  'not_retryable',
  'retryable',
])
export type AutopilotWorkErrorRetryability =
  typeof AutopilotWorkErrorRetryability.Type

export const AutopilotWorkErrorRecoveryStrategy = S.Literals([
  'alternate_adapter',
  'ask_user',
  'backoff_retry',
  'compact_context',
  'deny',
  'none',
  'preserve_partial',
  'stop_fail_closed',
  'structured_tool_error',
])
export type AutopilotWorkErrorRecoveryStrategy =
  typeof AutopilotWorkErrorRecoveryStrategy.Type

export const AutopilotWorkErrorRedactionClass = S.Literals([
  'local_only',
  'private_ref',
  'public',
])
export type AutopilotWorkErrorRedactionClass =
  typeof AutopilotWorkErrorRedactionClass.Type

export const AutopilotWorkRecoveryEventKind = S.Literals([
  'error.recorded',
  'recovery.alternate_adapter_selected',
  'recovery.compaction_attempted',
  'recovery.failed',
  'recovery.partial_result_preserved',
  'recovery.permission_denial_recorded',
  'recovery.retry_scheduled',
  'recovery.started',
  'recovery.succeeded',
  'recovery.user_input_requested',
  'run.failed_closed',
])
export type AutopilotWorkRecoveryEventKind =
  typeof AutopilotWorkRecoveryEventKind.Type

export const AutopilotWorkErrorRecoveryError = S.Struct({
  category: AutopilotWorkErrorCategory,
  causeRef: S.optionalKey(S.NullOr(S.String)),
  diagnosticRef: S.optionalKey(S.NullOr(S.String)),
  errorRef: S.String,
  occurredAt: S.optionalKey(S.NullOr(S.String)),
  originServiceRef: S.optionalKey(S.NullOr(S.String)),
  publicMessage: S.optionalKey(S.NullOr(S.String)),
  recoveryStrategy: S.optionalKey(AutopilotWorkErrorRecoveryStrategy),
  redactionClass: S.optionalKey(AutopilotWorkErrorRedactionClass),
  relatedRefs: S.optionalKey(S.Array(S.String)),
  retryability: S.optionalKey(AutopilotWorkErrorRetryability),
  severity: S.optionalKey(AutopilotWorkErrorSeverity),
})
export type AutopilotWorkErrorRecoveryError =
  typeof AutopilotWorkErrorRecoveryError.Type

export const AutopilotWorkRecoveryEvent = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  errorRef: S.optionalKey(S.NullOr(S.String)),
  eventRef: S.String,
  kind: AutopilotWorkRecoveryEventKind,
  occurredAt: S.String,
  publicSafe: S.Boolean,
  recoveryStrategy: S.optionalKey(AutopilotWorkErrorRecoveryStrategy),
  receiptRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkRecoveryEvent =
  typeof AutopilotWorkRecoveryEvent.Type

export const AutopilotWorkErrorRecovery = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  errors: S.optionalKey(S.Array(AutopilotWorkErrorRecoveryError)),
  events: S.optionalKey(S.Array(AutopilotWorkRecoveryEvent)),
  recoveryRef: S.optionalKey(S.String),
})
export type AutopilotWorkErrorRecovery =
  typeof AutopilotWorkErrorRecovery.Type

export const AutopilotWorkCompactionTrigger = S.Literals([
  'automatic',
  'manual',
  'reactive',
  'recovery',
  'session_memory',
])
export type AutopilotWorkCompactionTrigger =
  typeof AutopilotWorkCompactionTrigger.Type

export const AutopilotWorkCompactionStrategy = S.Literals([
  'microcompact',
  'partial_compact',
  'reactive_compact',
  'session_memory_compact',
  'summary_compact',
])
export type AutopilotWorkCompactionStrategy =
  typeof AutopilotWorkCompactionStrategy.Type

export const AutopilotWorkCompactionState = S.Literals([
  'cancelled',
  'compacted',
  'failed',
  'pending',
  'skipped',
])
export type AutopilotWorkCompactionState =
  typeof AutopilotWorkCompactionState.Type

export const AutopilotWorkCompactionEstimate = S.Struct({
  contextWindow: S.optionalKey(S.NullOr(S.Number)),
  estimateRef: S.optionalKey(S.NullOr(S.String)),
  messageCount: S.optionalKey(S.NullOr(S.Number)),
  tokenCount: S.optionalKey(S.NullOr(S.Number)),
})
export type AutopilotWorkCompactionEstimate =
  typeof AutopilotWorkCompactionEstimate.Type

export const AutopilotWorkCompactionToolPair = S.Struct({
  requestRef: S.String,
  resultRef: S.optionalKey(S.NullOr(S.String)),
  summaryRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkCompactionToolPair =
  typeof AutopilotWorkCompactionToolPair.Type

export const AutopilotWorkCompactionBoundary = S.Struct({
  automaticFailureCount: S.optionalKey(S.NullOr(S.Number)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  boundaryRef: S.String,
  failureRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.String,
  hookRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  postEstimate: S.optionalKey(AutopilotWorkCompactionEstimate),
  preEstimate: S.optionalKey(AutopilotWorkCompactionEstimate),
  preservedAdapterRefs: S.optionalKey(S.Array(S.String)),
  preservedPlanRefs: S.optionalKey(S.Array(S.String)),
  preservedRecentMessageRefs: S.optionalKey(S.Array(S.String)),
  preservedTaskRefs: S.optionalKey(S.Array(S.String)),
  preservedToolPairs: S.optionalKey(S.Array(AutopilotWorkCompactionToolPair)),
  publicMessage: S.optionalKey(S.NullOr(S.String)),
  publicSafe: S.Boolean,
  restoredAdapterRefs: S.optionalKey(S.Array(S.String)),
  restoredFileRefs: S.optionalKey(S.Array(S.String)),
  restoredPlanRefs: S.optionalKey(S.Array(S.String)),
  restoredSkillRefs: S.optionalKey(S.Array(S.String)),
  restoredTaskRefs: S.optionalKey(S.Array(S.String)),
  retryRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkCompactionState,
  strategy: AutopilotWorkCompactionStrategy,
  summarySourceRefs: S.optionalKey(S.Array(S.String)),
  trigger: AutopilotWorkCompactionTrigger,
})
export type AutopilotWorkCompactionBoundary =
  typeof AutopilotWorkCompactionBoundary.Type

export const AutopilotWorkCompaction = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  boundaries: S.optionalKey(S.Array(AutopilotWorkCompactionBoundary)),
  compactionRef: S.optionalKey(S.String),
})
export type AutopilotWorkCompaction = typeof AutopilotWorkCompaction.Type

export const AutopilotWorkUsagePricingState = S.Literals([
  'known',
  'unknown',
])
export type AutopilotWorkUsagePricingState =
  typeof AutopilotWorkUsagePricingState.Type

export const AutopilotWorkUsageTruth = S.Literals([
  'estimated',
  'exact',
  'synthetic',
  'unknown',
])
export type AutopilotWorkUsageTruth = typeof AutopilotWorkUsageTruth.Type

export const AutopilotWorkBudgetAction = S.Literals([
  'ask',
  'compact',
  'continue',
  'stop',
  'warn',
])
export type AutopilotWorkBudgetAction = typeof AutopilotWorkBudgetAction.Type

export const AutopilotWorkBudgetState = S.Literals([
  'blocked',
  'exceeded',
  'near_limit',
  'unknown',
  'within',
])
export type AutopilotWorkBudgetState = typeof AutopilotWorkBudgetState.Type

export const AutopilotWorkUsageTokenCounts = S.Struct({
  cacheReadTokens: S.optionalKey(S.NullOr(S.Number)),
  cacheWriteTokens: S.optionalKey(S.NullOr(S.Number)),
  contextWindowTokens: S.optionalKey(S.NullOr(S.Number)),
  inputTokens: S.optionalKey(S.NullOr(S.Number)),
  outputTokens: S.optionalKey(S.NullOr(S.Number)),
  serverToolRequestCount: S.optionalKey(S.NullOr(S.Number)),
  totalTokens: S.optionalKey(S.NullOr(S.Number)),
})
export type AutopilotWorkUsageTokenCounts =
  typeof AutopilotWorkUsageTokenCounts.Type

export const AutopilotWorkUsageCostEstimate = S.Struct({
  costRef: S.String,
  currency: S.optionalKey(S.NullOr(S.String)),
  estimatedCostCents: S.optionalKey(S.NullOr(S.Number)),
  pricingRef: S.optionalKey(S.NullOr(S.String)),
  pricingState: AutopilotWorkUsagePricingState,
})
export type AutopilotWorkUsageCostEstimate =
  typeof AutopilotWorkUsageCostEstimate.Type

export const AutopilotWorkBudgetThreshold = S.Struct({
  action: AutopilotWorkBudgetAction,
  budgetRef: S.String,
  limitCostCents: S.optionalKey(S.NullOr(S.Number)),
  limitTokens: S.optionalKey(S.NullOr(S.Number)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkBudgetState,
})
export type AutopilotWorkBudgetThreshold =
  typeof AutopilotWorkBudgetThreshold.Type

export const AutopilotWorkUsageBudget = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  budgetThresholds: S.optionalKey(S.Array(AutopilotWorkBudgetThreshold)),
  contextEstimateRef: S.optionalKey(S.NullOr(S.String)),
  costEstimate: S.optionalKey(AutopilotWorkUsageCostEstimate),
  generatedAt: S.optionalKey(S.String),
  modelRef: S.optionalKey(S.NullOr(S.String)),
  providerRef: S.optionalKey(S.NullOr(S.String)),
  quotaBlockerRefs: S.optionalKey(S.Array(S.String)),
  rateLimitRefs: S.optionalKey(S.Array(S.String)),
  tokenCounts: S.optionalKey(AutopilotWorkUsageTokenCounts),
  usageRef: S.String,
  usageTruth: S.optionalKey(AutopilotWorkUsageTruth),
})
export type AutopilotWorkUsageBudget =
  typeof AutopilotWorkUsageBudget.Type

export const AutopilotWorkModelProviderState = S.Literals([
  'blocked',
  'fallback_selected',
  'selected',
  'unavailable',
  'unknown',
])
export type AutopilotWorkModelProviderState =
  typeof AutopilotWorkModelProviderState.Type

export const AutopilotWorkModelValidationState = S.Literals([
  'failed',
  'passed',
  'pending',
  'unknown',
])
export type AutopilotWorkModelValidationState =
  typeof AutopilotWorkModelValidationState.Type

export const AutopilotWorkModelResolutionSource = S.Literals([
  'agent_override',
  'built_in_default',
  'environment_override',
  'entitlement_default',
  'runtime_mode',
  'session_override',
  'settings',
  'skill_or_command_override',
  'startup_flag',
])
export type AutopilotWorkModelResolutionSource =
  typeof AutopilotWorkModelResolutionSource.Type

export const AutopilotWorkModelCapabilities = S.Struct({
  cacheSupport: S.optionalKey(S.Boolean),
  contextWindowTokens: S.optionalKey(S.NullOr(S.Number)),
  documentSupport: S.optionalKey(S.Boolean),
  maxOutputTokens: S.optionalKey(S.NullOr(S.Number)),
  parallelToolCallSupport: S.optionalKey(S.Boolean),
  reasoningSupport: S.optionalKey(S.Boolean),
  serverToolSupport: S.optionalKey(S.Boolean),
  structuredOutputSupport: S.optionalKey(S.Boolean),
  toolCallSupport: S.optionalKey(S.Boolean),
  visionSupport: S.optionalKey(S.Boolean),
})
export type AutopilotWorkModelCapabilities =
  typeof AutopilotWorkModelCapabilities.Type

export const AutopilotWorkModelProviderResolution = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  capabilities: S.optionalKey(AutopilotWorkModelCapabilities),
  entitlementRefs: S.optionalKey(S.Array(S.String)),
  fallbackRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.optionalKey(S.String),
  modelRef: S.optionalKey(S.NullOr(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  pricingRefs: S.optionalKey(S.Array(S.String)),
  privacyRefs: S.optionalKey(S.Array(S.String)),
  providerFacingModelRef: S.optionalKey(S.NullOr(S.String)),
  providerRef: S.optionalKey(S.NullOr(S.String)),
  requestedAliasRef: S.optionalKey(S.NullOr(S.String)),
  resolutionRef: S.String,
  resolutionSource: S.optionalKey(AutopilotWorkModelResolutionSource),
  state: AutopilotWorkModelProviderState,
  telemetryRefs: S.optionalKey(S.Array(S.String)),
  validationRefs: S.optionalKey(S.Array(S.String)),
  validationState: S.optionalKey(AutopilotWorkModelValidationState),
})
export type AutopilotWorkModelProviderResolution =
  typeof AutopilotWorkModelProviderResolution.Type

export const AutopilotWorkInstructionLayerKind = S.Literals([
  'agent_instruction',
  'append_operator_instruction',
  'command_instruction',
  'custom_system_prompt',
  'execution_mode_override',
  'local_private_instruction',
  'memory_instruction',
  'mode_instruction',
  'output_style',
  'product_default',
  'provider_capability_instruction',
  'runtime_policy',
  'skill_instruction',
  'team_instruction',
  'tool_instruction',
  'user_instruction',
  'workspace_instruction',
])
export type AutopilotWorkInstructionLayerKind =
  typeof AutopilotWorkInstructionLayerKind.Type

export const AutopilotWorkInstructionLayerState = S.Literals([
  'appended',
  'applied',
  'replaced',
  'skipped',
])
export type AutopilotWorkInstructionLayerState =
  typeof AutopilotWorkInstructionLayerState.Type

export const AutopilotWorkInstructionFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkInstructionFreshness =
  typeof AutopilotWorkInstructionFreshness.Type

export const AutopilotWorkInstructionRedactionClass = S.Literals([
  'local_only',
  'private_ref',
  'public',
])
export type AutopilotWorkInstructionRedactionClass =
  typeof AutopilotWorkInstructionRedactionClass.Type

export const AutopilotWorkInstructionLayer = S.Struct({
  allowedToolRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  capabilityDeltaRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkInstructionFreshness),
  kind: AutopilotWorkInstructionLayerKind,
  layerRef: S.String,
  metadataRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  precedence: S.Number,
  redactionClass: S.optionalKey(AutopilotWorkInstructionRedactionClass),
  replacementSourceRef: S.optionalKey(S.NullOr(S.String)),
  sourceRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkInstructionLayerState,
  tokenEstimate: S.optionalKey(S.NullOr(S.Number)),
})
export type AutopilotWorkInstructionLayer =
  typeof AutopilotWorkInstructionLayer.Type

export const AutopilotWorkInstructionLayering = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.optionalKey(S.String),
  layers: S.optionalKey(S.Array(AutopilotWorkInstructionLayer)),
  projectionRef: S.optionalKey(S.NullOr(S.String)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkInstructionLayering =
  typeof AutopilotWorkInstructionLayering.Type

export const AutopilotWorkSessionMemoryKind = S.Literals([
  'decision',
  'fact',
  'operator_preference',
  'progress_note',
  'repository_note',
  'task_context',
  'user_preference',
])
export type AutopilotWorkSessionMemoryKind =
  typeof AutopilotWorkSessionMemoryKind.Type

export const AutopilotWorkSessionMemoryLifecycleState = S.Literals([
  'active',
  'expired',
  'forgotten',
  'pending',
  'superseded',
])
export type AutopilotWorkSessionMemoryLifecycleState =
  typeof AutopilotWorkSessionMemoryLifecycleState.Type

export const AutopilotWorkSessionMemoryScope = S.Literals([
  'repository',
  'run',
  'session',
  'team',
  'user',
  'workspace',
])
export type AutopilotWorkSessionMemoryScope =
  typeof AutopilotWorkSessionMemoryScope.Type

export const AutopilotWorkSessionMemoryFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkSessionMemoryFreshness =
  typeof AutopilotWorkSessionMemoryFreshness.Type

export const AutopilotWorkSessionMemoryRetentionClass = S.Literals([
  'delete_requested',
  'ephemeral',
  'long_term',
  'project',
  'session',
])
export type AutopilotWorkSessionMemoryRetentionClass =
  typeof AutopilotWorkSessionMemoryRetentionClass.Type

export const AutopilotWorkSessionMemoryRedactionClass = S.Literals([
  'local_only',
  'private_ref',
  'public',
])
export type AutopilotWorkSessionMemoryRedactionClass =
  typeof AutopilotWorkSessionMemoryRedactionClass.Type

export const AutopilotWorkSessionMemoryEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  compactionRefs: S.optionalKey(S.Array(S.String)),
  conflictRefs: S.optionalKey(S.Array(S.String)),
  entryRef: S.String,
  freshness: S.optionalKey(AutopilotWorkSessionMemoryFreshness),
  kind: AutopilotWorkSessionMemoryKind,
  lifecycleState: AutopilotWorkSessionMemoryLifecycleState,
  policyRefs: S.optionalKey(S.Array(S.String)),
  redactionClass: S.optionalKey(AutopilotWorkSessionMemoryRedactionClass),
  retentionClass: S.optionalKey(AutopilotWorkSessionMemoryRetentionClass),
  retrievalRefs: S.optionalKey(S.Array(S.String)),
  scope: AutopilotWorkSessionMemoryScope,
  sourceRefs: S.optionalKey(S.Array(S.String)),
  summaryRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkSessionMemoryEntry =
  typeof AutopilotWorkSessionMemoryEntry.Type

export const AutopilotWorkSessionMemory = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkSessionMemoryEntry)),
  generatedAt: S.optionalKey(S.String),
  projectionRef: S.optionalKey(S.NullOr(S.String)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkSessionMemory =
  typeof AutopilotWorkSessionMemory.Type

export const AutopilotWorkDiagnosticSeverity = S.Literals([
  'error',
  'hint',
  'info',
  'warning',
])
export type AutopilotWorkDiagnosticSeverity =
  typeof AutopilotWorkDiagnosticSeverity.Type

export const AutopilotWorkDiagnosticsFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkDiagnosticsFreshness =
  typeof AutopilotWorkDiagnosticsFreshness.Type

export const AutopilotWorkDiagnosticsSeverityCounts = S.Struct({
  errorCount: S.optionalKey(S.NullOr(S.Number)),
  hintCount: S.optionalKey(S.NullOr(S.Number)),
  infoCount: S.optionalKey(S.NullOr(S.Number)),
  warningCount: S.optionalKey(S.NullOr(S.Number)),
})
export type AutopilotWorkDiagnosticsSeverityCounts =
  typeof AutopilotWorkDiagnosticsSeverityCounts.Type

export const AutopilotWorkDiagnosticEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  diagnosticRef: S.String,
  freshness: S.optionalKey(AutopilotWorkDiagnosticsFreshness),
  languageServerRef: S.optionalKey(S.NullOr(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  remediationRefs: S.optionalKey(S.Array(S.String)),
  severity: AutopilotWorkDiagnosticSeverity,
  sourceRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkDiagnosticEntry =
  typeof AutopilotWorkDiagnosticEntry.Type

export const AutopilotWorkDiagnostics = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  diagnosticRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkDiagnosticEntry)),
  freshness: S.optionalKey(AutopilotWorkDiagnosticsFreshness),
  generatedAt: S.optionalKey(S.String),
  indexedAt: S.optionalKey(S.NullOr(S.String)),
  indexedAtRef: S.optionalKey(S.NullOr(S.String)),
  languageServerRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  remediationRefs: S.optionalKey(S.Array(S.String)),
  severityCounts: S.optionalKey(AutopilotWorkDiagnosticsSeverityCounts),
  skippedDiagnosticRefs: S.optionalKey(S.Array(S.String)),
  snapshotRef: S.String,
  sourceRefs: S.optionalKey(S.Array(S.String)),
  versionRef: S.optionalKey(S.NullOr(S.String)),
  workspaceBoundaryRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkDiagnostics =
  typeof AutopilotWorkDiagnostics.Type

export const AutopilotWorkTerminalSurfaceMode = S.Literals([
  'headless',
  'interactive',
  'non_interactive',
  'remote',
  'web_companion',
])
export type AutopilotWorkTerminalSurfaceMode =
  typeof AutopilotWorkTerminalSurfaceMode.Type

export const AutopilotWorkTerminalSurfaceState = S.Literals([
  'available',
  'blocked',
  'degraded',
  'missing',
  'unknown',
])
export type AutopilotWorkTerminalSurfaceState =
  typeof AutopilotWorkTerminalSurfaceState.Type

export const AutopilotWorkTerminalSurfaceFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkTerminalSurfaceFreshness =
  typeof AutopilotWorkTerminalSurfaceFreshness.Type

export const AutopilotWorkTerminalSurface = S.Struct({
  accessibilityRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  commandDescriptorRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkTerminalSurfaceFreshness),
  inputDescriptorRefs: S.optionalKey(S.Array(S.String)),
  mode: AutopilotWorkTerminalSurfaceMode,
  nonInteractiveRefs: S.optionalKey(S.Array(S.String)),
  paneRefs: S.optionalKey(S.Array(S.String)),
  parityRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  shellRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkTerminalSurfaceState,
  streamRefs: S.optionalKey(S.Array(S.String)),
  surfaceRef: S.String,
  transcriptSummaryRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkTerminalSurface =
  typeof AutopilotWorkTerminalSurface.Type

export const AutopilotWorkTerminalUiShell = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  surfaces: S.optionalKey(S.Array(AutopilotWorkTerminalSurface)),
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkTerminalUiShell =
  typeof AutopilotWorkTerminalUiShell.Type

export const AutopilotWorkInputMode = S.Literals([
  'accessibility',
  'command_palette',
  'form',
  'headless_json',
  'keyboard',
  'remote_control',
  'slash_command',
])
export type AutopilotWorkInputMode = typeof AutopilotWorkInputMode.Type

export const AutopilotWorkInputKeybindingState = S.Literals([
  'available',
  'blocked',
  'degraded',
  'missing',
  'unknown',
])
export type AutopilotWorkInputKeybindingState =
  typeof AutopilotWorkInputKeybindingState.Type

export const AutopilotWorkInputKeybindingFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkInputKeybindingFreshness =
  typeof AutopilotWorkInputKeybindingFreshness.Type

export const AutopilotWorkInputKeybindingEntry = S.Struct({
  accessibilityRefs: S.optionalKey(S.Array(S.String)),
  bindingMapRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  commandDescriptorRefs: S.optionalKey(S.Array(S.String)),
  conflictRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkInputKeybindingFreshness),
  inputModeRef: S.String,
  keymapRefs: S.optionalKey(S.Array(S.String)),
  mode: AutopilotWorkInputMode,
  nonInteractiveFallbackRefs: S.optionalKey(S.Array(S.String)),
  platformRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkInputKeybindingState,
})
export type AutopilotWorkInputKeybindingEntry =
  typeof AutopilotWorkInputKeybindingEntry.Type

export const AutopilotWorkInputKeybinding = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkInputKeybindingEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkInputKeybinding =
  typeof AutopilotWorkInputKeybinding.Type

export const AutopilotWorkCommandKind = S.Literals([
  'automation',
  'built_in',
  'command_palette',
  'extension',
  'keybinding',
  'slash_command',
])
export type AutopilotWorkCommandKind = typeof AutopilotWorkCommandKind.Type

export const AutopilotWorkCommandState = S.Literals([
  'available',
  'blocked',
  'conflicted',
  'disabled',
  'unavailable',
  'unknown',
])
export type AutopilotWorkCommandState = typeof AutopilotWorkCommandState.Type

export const AutopilotWorkCommandFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkCommandFreshness =
  typeof AutopilotWorkCommandFreshness.Type

export const AutopilotWorkCommandEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  commandDescriptorRefs: S.optionalKey(S.Array(S.String)),
  commandRef: S.String,
  conflictRefs: S.optionalKey(S.Array(S.String)),
  fallbackRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkCommandFreshness),
  inputModeRefs: S.optionalKey(S.Array(S.String)),
  kind: AutopilotWorkCommandKind,
  parserRefs: S.optionalKey(S.Array(S.String)),
  plannerRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  selectorRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkCommandState,
})
export type AutopilotWorkCommandEntry =
  typeof AutopilotWorkCommandEntry.Type

export const AutopilotWorkCommandSystem = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  commands: S.optionalKey(S.Array(AutopilotWorkCommandEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkCommandSystem =
  typeof AutopilotWorkCommandSystem.Type

export const AutopilotWorkAttentionState = S.Literals([
  'active',
  'invalidated',
  'resolved',
  'unknown',
  'waiting',
])
export type AutopilotWorkAttentionState =
  typeof AutopilotWorkAttentionState.Type

export const AutopilotWorkAttentionSeverity = S.Literals([
  'critical',
  'info',
  'warning',
])
export type AutopilotWorkAttentionSeverity =
  typeof AutopilotWorkAttentionSeverity.Type

export const AutopilotWorkAttentionFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkAttentionFreshness =
  typeof AutopilotWorkAttentionFreshness.Type

export const AutopilotWorkAttentionEntry = S.Struct({
  actionRefs: S.optionalKey(S.Array(S.String)),
  attentionRef: S.String,
  blockerRefs: S.optionalKey(S.Array(S.String)),
  channelRefs: S.optionalKey(S.Array(S.String)),
  decisionRefs: S.optionalKey(S.Array(S.String)),
  deliveryRefs: S.optionalKey(S.Array(S.String)),
  dedupeRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkAttentionFreshness),
  invalidationRefs: S.optionalKey(S.Array(S.String)),
  notificationRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  resolutionRefs: S.optionalKey(S.Array(S.String)),
  severity: AutopilotWorkAttentionSeverity,
  state: AutopilotWorkAttentionState,
})
export type AutopilotWorkAttentionEntry =
  typeof AutopilotWorkAttentionEntry.Type

export const AutopilotWorkNotificationAttention = S.Struct({
  attention: S.optionalKey(S.Array(AutopilotWorkAttentionEntry)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkNotificationAttention =
  typeof AutopilotWorkNotificationAttention.Type

export const AutopilotWorkHelpDoctorState = S.Literals([
  'blocked',
  'failed',
  'passed',
  'unknown',
  'warning',
])
export type AutopilotWorkHelpDoctorState =
  typeof AutopilotWorkHelpDoctorState.Type

export const AutopilotWorkHelpDoctorSeverity = S.Literals([
  'critical',
  'error',
  'info',
  'warning',
])
export type AutopilotWorkHelpDoctorSeverity =
  typeof AutopilotWorkHelpDoctorSeverity.Type

export const AutopilotWorkHelpDoctorFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkHelpDoctorFreshness =
  typeof AutopilotWorkHelpDoctorFreshness.Type

export const AutopilotWorkHelpDoctorEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  debugBundleRefs: S.optionalKey(S.Array(S.String)),
  diagnosticRefs: S.optionalKey(S.Array(S.String)),
  doctorCheckRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkHelpDoctorFreshness),
  helpTopicRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  remediationRefs: S.optionalKey(S.Array(S.String)),
  severity: AutopilotWorkHelpDoctorSeverity,
  sourceRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkHelpDoctorState,
  surfaceRef: S.String,
})
export type AutopilotWorkHelpDoctorEntry =
  typeof AutopilotWorkHelpDoctorEntry.Type

export const AutopilotWorkHelpDoctorDebug = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkHelpDoctorEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkHelpDoctorDebug =
  typeof AutopilotWorkHelpDoctorDebug.Type

export const AutopilotWorkMcpServerExposureState = S.Literals([
  'blocked',
  'disabled',
  'exposed',
  'internal_only',
  'planned',
  'unknown',
])
export type AutopilotWorkMcpServerExposureState =
  typeof AutopilotWorkMcpServerExposureState.Type

export const AutopilotWorkMcpServerFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkMcpServerFreshness =
  typeof AutopilotWorkMcpServerFreshness.Type

export const AutopilotWorkMcpServerEntry = S.Struct({
  audienceRefs: S.optionalKey(S.Array(S.String)),
  authPolicyRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  exportedPromptRefs: S.optionalKey(S.Array(S.String)),
  exportedResourceRefs: S.optionalKey(S.Array(S.String)),
  exportedToolRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkMcpServerFreshness),
  invocationReceiptRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  schemaRefs: S.optionalKey(S.Array(S.String)),
  serverRef: S.String,
  sourceRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkMcpServerExposureState,
  transportRefs: S.optionalKey(S.Array(S.String)),
  trustTierRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkMcpServerEntry =
  typeof AutopilotWorkMcpServerEntry.Type

export const AutopilotWorkMcpServerExport = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkMcpServerEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkMcpServerExport =
  typeof AutopilotWorkMcpServerExport.Type

export const AutopilotWorkSettingsConfigurationState = S.Literals([
  'blocked',
  'defaulted',
  'enabled',
  'overridden',
  'unknown',
])
export type AutopilotWorkSettingsConfigurationState =
  typeof AutopilotWorkSettingsConfigurationState.Type

export const AutopilotWorkSettingsConfigurationFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkSettingsConfigurationFreshness =
  typeof AutopilotWorkSettingsConfigurationFreshness.Type

export const AutopilotWorkSettingsConfigurationRedactionClass = S.Literals([
  'local_only',
  'private_ref',
  'public',
])
export type AutopilotWorkSettingsConfigurationRedactionClass =
  typeof AutopilotWorkSettingsConfigurationRedactionClass.Type

export const AutopilotWorkSettingsConfigurationEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  defaultRefs: S.optionalKey(S.Array(S.String)),
  effectiveValueRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkSettingsConfigurationFreshness),
  overrideRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  redactionClass: S.optionalKey(AutopilotWorkSettingsConfigurationRedactionClass),
  redactionRefs: S.optionalKey(S.Array(S.String)),
  scopeRefs: S.optionalKey(S.Array(S.String)),
  settingRef: S.String,
  sourceRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkSettingsConfigurationState,
  validationRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkSettingsConfigurationEntry =
  typeof AutopilotWorkSettingsConfigurationEntry.Type

export const AutopilotWorkSettingsConfiguration = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkSettingsConfigurationEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkSettingsConfiguration =
  typeof AutopilotWorkSettingsConfiguration.Type

export const AutopilotWorkCredentialKind = S.Literals([
  'api_key',
  'oauth_token',
  'session',
  'ssh_key',
  'unknown',
  'wallet',
])
export type AutopilotWorkCredentialKind =
  typeof AutopilotWorkCredentialKind.Type

export const AutopilotWorkCredentialState = S.Literals([
  'blocked',
  'expired',
  'missing',
  'revoked',
  'unknown',
  'usable',
])
export type AutopilotWorkCredentialState =
  typeof AutopilotWorkCredentialState.Type

export const AutopilotWorkCredentialFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkCredentialFreshness =
  typeof AutopilotWorkCredentialFreshness.Type

export const AutopilotWorkCredentialRedactionClass = S.Literals([
  'local_only',
  'private_ref',
  'public',
])
export type AutopilotWorkCredentialRedactionClass =
  typeof AutopilotWorkCredentialRedactionClass.Type

export const AutopilotWorkCredentialStorageEntry = S.Struct({
  accountRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  credentialRef: S.String,
  entitlementRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkCredentialFreshness),
  kind: AutopilotWorkCredentialKind,
  leaseRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  redactionClass: S.optionalKey(AutopilotWorkCredentialRedactionClass),
  redactionRefs: S.optionalKey(S.Array(S.String)),
  revocationRefs: S.optionalKey(S.Array(S.String)),
  rotationRefs: S.optionalKey(S.Array(S.String)),
  scopeRefs: S.optionalKey(S.Array(S.String)),
  sessionRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkCredentialState,
  storageBackendRefs: S.optionalKey(S.Array(S.String)),
  validationRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkCredentialStorageEntry =
  typeof AutopilotWorkCredentialStorageEntry.Type

export const AutopilotWorkCredentialStorage = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkCredentialStorageEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkCredentialStorage =
  typeof AutopilotWorkCredentialStorage.Type

export const AutopilotWorkGitWorkflowState = S.Literals([
  'blocked',
  'checks_pending',
  'pr_ready',
  'review_ready',
  'unknown',
  'writeback_ready',
])
export type AutopilotWorkGitWorkflowState =
  typeof AutopilotWorkGitWorkflowState.Type

export const AutopilotWorkGitWorkflowFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkGitWorkflowFreshness =
  typeof AutopilotWorkGitWorkflowFreshness.Type

export const AutopilotWorkGitWorkflowEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  branchRefs: S.optionalKey(S.Array(S.String)),
  checkRefs: S.optionalKey(S.Array(S.String)),
  commitRefs: S.optionalKey(S.Array(S.String)),
  diffRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkGitWorkflowFreshness),
  issueRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  prRefs: S.optionalKey(S.Array(S.String)),
  repositoryRefs: S.optionalKey(S.Array(S.String)),
  reviewRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkGitWorkflowState,
  statusRefs: S.optionalKey(S.Array(S.String)),
  workflowRef: S.String,
  worktreeRefs: S.optionalKey(S.Array(S.String)),
  writebackRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkGitWorkflowEntry =
  typeof AutopilotWorkGitWorkflowEntry.Type

export const AutopilotWorkGitWorkflow = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkGitWorkflowEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkGitWorkflow =
  typeof AutopilotWorkGitWorkflow.Type

export const AutopilotWorkEditorIntegrationState = S.Literals([
  'blocked',
  'connected',
  'disconnected',
  'ready',
  'unknown',
])
export type AutopilotWorkEditorIntegrationState =
  typeof AutopilotWorkEditorIntegrationState.Type

export const AutopilotWorkEditorIntegrationFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkEditorIntegrationFreshness =
  typeof AutopilotWorkEditorIntegrationFreshness.Type

export const AutopilotWorkEditorIntegrationEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  commandRefs: S.optionalKey(S.Array(S.String)),
  deepLinkRefs: S.optionalKey(S.Array(S.String)),
  diagnosticHandoffRefs: S.optionalKey(S.Array(S.String)),
  diagnosticRefs: S.optionalKey(S.Array(S.String)),
  editorRefs: S.optionalKey(S.Array(S.String)),
  extensionRefs: S.optionalKey(S.Array(S.String)),
  fileOpenRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkEditorIntegrationFreshness),
  integrationRef: S.String,
  policyRefs: S.optionalKey(S.Array(S.String)),
  selectionRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkEditorIntegrationState,
  statusRefs: S.optionalKey(S.Array(S.String)),
  workspaceRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkEditorIntegrationEntry =
  typeof AutopilotWorkEditorIntegrationEntry.Type

export const AutopilotWorkEditorIntegration = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkEditorIntegrationEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkEditorIntegration =
  typeof AutopilotWorkEditorIntegration.Type

export const AutopilotWorkBrowserDesktopIntegrationState = S.Literals([
  'blocked',
  'connected',
  'installed',
  'ready',
  'unavailable',
  'unknown',
])
export type AutopilotWorkBrowserDesktopIntegrationState =
  typeof AutopilotWorkBrowserDesktopIntegrationState.Type

export const AutopilotWorkBrowserDesktopIntegrationFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkBrowserDesktopIntegrationFreshness =
  typeof AutopilotWorkBrowserDesktopIntegrationFreshness.Type

export const AutopilotWorkBrowserDesktopIntegrationEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  browserRefs: S.optionalKey(S.Array(S.String)),
  companionRefs: S.optionalKey(S.Array(S.String)),
  deepLinkRefs: S.optionalKey(S.Array(S.String)),
  desktopAppRefs: S.optionalKey(S.Array(S.String)),
  extensionRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkBrowserDesktopIntegrationFreshness),
  installRefs: S.optionalKey(S.Array(S.String)),
  integrationRef: S.String,
  notificationRefs: S.optionalKey(S.Array(S.String)),
  permissionRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkBrowserDesktopIntegrationState,
  statusRefs: S.optionalKey(S.Array(S.String)),
  surfaceRefs: S.optionalKey(S.Array(S.String)),
  updateRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkBrowserDesktopIntegrationEntry =
  typeof AutopilotWorkBrowserDesktopIntegrationEntry.Type

export const AutopilotWorkBrowserDesktopIntegration = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkBrowserDesktopIntegrationEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkBrowserDesktopIntegration =
  typeof AutopilotWorkBrowserDesktopIntegration.Type

export const AutopilotWorkMultimodalInputState = S.Literals([
  'blocked',
  'capture_ready',
  'ingested',
  'pending',
  'unknown',
])
export type AutopilotWorkMultimodalInputState =
  typeof AutopilotWorkMultimodalInputState.Type

export const AutopilotWorkMultimodalInputModality = S.Literals([
  'audio',
  'image',
  'screen',
  'text',
  'unknown',
  'video',
])
export type AutopilotWorkMultimodalInputModality =
  typeof AutopilotWorkMultimodalInputModality.Type

export const AutopilotWorkMultimodalInputFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkMultimodalInputFreshness =
  typeof AutopilotWorkMultimodalInputFreshness.Type

export const AutopilotWorkMultimodalInputEntry = S.Struct({
  attachmentRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  captureSurfaceRefs: S.optionalKey(S.Array(S.String)),
  consentRefs: S.optionalKey(S.Array(S.String)),
  contextIngestionRefs: S.optionalKey(S.Array(S.String)),
  endpointRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkMultimodalInputFreshness),
  inputRef: S.String,
  modality: AutopilotWorkMultimodalInputModality,
  policyRefs: S.optionalKey(S.Array(S.String)),
  redactionRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkMultimodalInputState,
  transcriptRefs: S.optionalKey(S.Array(S.String)),
  vadRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkMultimodalInputEntry =
  typeof AutopilotWorkMultimodalInputEntry.Type

export const AutopilotWorkMultimodalInput = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkMultimodalInputEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkMultimodalInput =
  typeof AutopilotWorkMultimodalInput.Type

export const AutopilotWorkRemoteSessionBridgeState = S.Literals([
  'blocked',
  'connected',
  'ready',
  'reconnecting',
  'unknown',
])
export type AutopilotWorkRemoteSessionBridgeState =
  typeof AutopilotWorkRemoteSessionBridgeState.Type

export const AutopilotWorkRemoteSessionBridgeFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkRemoteSessionBridgeFreshness =
  typeof AutopilotWorkRemoteSessionBridgeFreshness.Type

export const AutopilotWorkRemoteSessionBridgeEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  bridgeRef: S.String,
  controllerRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkRemoteSessionBridgeFreshness),
  heartbeatRefs: S.optionalKey(S.Array(S.String)),
  permissionRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  protocolRefs: S.optionalKey(S.Array(S.String)),
  reconnectRefs: S.optionalKey(S.Array(S.String)),
  sessionRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkRemoteSessionBridgeState,
  transportRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkRemoteSessionBridgeEntry =
  typeof AutopilotWorkRemoteSessionBridgeEntry.Type

export const AutopilotWorkRemoteSessionBridge = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkRemoteSessionBridgeEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkRemoteSessionBridge =
  typeof AutopilotWorkRemoteSessionBridge.Type

export const AutopilotWorkCompanionSurfaceState = S.Literals([
  'blocked',
  'offline',
  'read_only',
  'ready',
  'unknown',
  'waiting',
])
export type AutopilotWorkCompanionSurfaceState =
  typeof AutopilotWorkCompanionSurfaceState.Type

export const AutopilotWorkCompanionSurfaceFreshness = S.Literals([
  'fresh',
  'lagged',
  'stale',
  'unknown',
])
export type AutopilotWorkCompanionSurfaceFreshness =
  typeof AutopilotWorkCompanionSurfaceFreshness.Type

export const AutopilotWorkCompanionSurfaceEntry = S.Struct({
  actionRefs: S.optionalKey(S.Array(S.String)),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  budgetRefs: S.optionalKey(S.Array(S.String)),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  closeoutRefs: S.optionalKey(S.Array(S.String)),
  companionRef: S.String,
  cursorRefs: S.optionalKey(S.Array(S.String)),
  decisionRefs: S.optionalKey(S.Array(S.String)),
  deliveryTierRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkCompanionSurfaceFreshness),
  idempotencyRefs: S.optionalKey(S.Array(S.String)),
  lagRefs: S.optionalKey(S.Array(S.String)),
  notificationRefs: S.optionalKey(S.Array(S.String)),
  pairingRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  progressRefs: S.optionalKey(S.Array(S.String)),
  receiptRefs: S.optionalKey(S.Array(S.String)),
  runRefs: S.optionalKey(S.Array(S.String)),
  sessionRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkCompanionSurfaceState,
  streamRefs: S.optionalKey(S.Array(S.String)),
  surfaceRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkCompanionSurfaceEntry =
  typeof AutopilotWorkCompanionSurfaceEntry.Type

export const AutopilotWorkCompanionSurface = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkCompanionSurfaceEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkCompanionSurface =
  typeof AutopilotWorkCompanionSurface.Type

export const AutopilotWorkTeamSharedMemoryScope = S.Literals([
  'mission',
  'personal',
  'project',
  'public',
  'repository',
  'team',
  'unknown',
])
export type AutopilotWorkTeamSharedMemoryScope =
  typeof AutopilotWorkTeamSharedMemoryScope.Type

export const AutopilotWorkTeamSharedMemoryKind = S.Literals([
  'accepted_fix',
  'budget_caveat',
  'build_command',
  'denied_path',
  'flaky_test',
  'onboarding_note',
  'product_policy',
  'provider_caveat',
  'repo_style',
  'reviewer_preference',
  'run_caveat',
  'unknown',
])
export type AutopilotWorkTeamSharedMemoryKind =
  typeof AutopilotWorkTeamSharedMemoryKind.Type

export const AutopilotWorkTeamSharedMemoryReviewState = S.Literals([
  'accepted',
  'corrected',
  'deleted',
  'pending_review',
  'rejected',
  'tentative',
  'unknown',
])
export type AutopilotWorkTeamSharedMemoryReviewState =
  typeof AutopilotWorkTeamSharedMemoryReviewState.Type

export const AutopilotWorkTeamSharedMemoryVisibility = S.Literals([
  'private',
  'public',
  'team',
])
export type AutopilotWorkTeamSharedMemoryVisibility =
  typeof AutopilotWorkTeamSharedMemoryVisibility.Type

export const AutopilotWorkTeamSharedMemoryRedactionClass = S.Literals([
  'local_only',
  'private_ref',
  'public_safe',
  'team_ref',
])
export type AutopilotWorkTeamSharedMemoryRedactionClass =
  typeof AutopilotWorkTeamSharedMemoryRedactionClass.Type

export const AutopilotWorkTeamSharedMemoryFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkTeamSharedMemoryFreshness =
  typeof AutopilotWorkTeamSharedMemoryFreshness.Type

export const AutopilotWorkTeamSharedMemoryEntry = S.Struct({
  applicationReceiptRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  consentRefs: S.optionalKey(S.Array(S.String)),
  deletionReceiptRefs: S.optionalKey(S.Array(S.String)),
  evidenceRefs: S.optionalKey(S.Array(S.String)),
  expiryRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkTeamSharedMemoryFreshness),
  kind: AutopilotWorkTeamSharedMemoryKind,
  memoryRef: S.String,
  ownerRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  promotionRefs: S.optionalKey(S.Array(S.String)),
  redactionClass: S.optionalKey(AutopilotWorkTeamSharedMemoryRedactionClass),
  retrievalPolicyRefs: S.optionalKey(S.Array(S.String)),
  reviewRefs: S.optionalKey(S.Array(S.String)),
  reviewState: AutopilotWorkTeamSharedMemoryReviewState,
  scope: AutopilotWorkTeamSharedMemoryScope,
  semanticQueryRefs: S.optionalKey(S.Array(S.String)),
  teamRefs: S.optionalKey(S.Array(S.String)),
  tombstoneRefs: S.optionalKey(S.Array(S.String)),
  typedQueryRefs: S.optionalKey(S.Array(S.String)),
  visibility: AutopilotWorkTeamSharedMemoryVisibility,
})
export type AutopilotWorkTeamSharedMemoryEntry =
  typeof AutopilotWorkTeamSharedMemoryEntry.Type

export const AutopilotWorkTeamSharedMemory = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkTeamSharedMemoryEntry)),
  generatedAt: S.optionalKey(S.String),
  projectionRef: S.optionalKey(S.NullOr(S.String)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkTeamSharedMemory =
  typeof AutopilotWorkTeamSharedMemory.Type

export const AutopilotWorkMultiAgentCoordinationState = S.Literals([
  'blocked',
  'cancelled',
  'completed',
  'failed',
  'merged',
  'planned',
  'running',
  'unknown',
  'waiting',
])
export type AutopilotWorkMultiAgentCoordinationState =
  typeof AutopilotWorkMultiAgentCoordinationState.Type

export const AutopilotWorkMultiAgentCoordinationFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkMultiAgentCoordinationFreshness =
  typeof AutopilotWorkMultiAgentCoordinationFreshness.Type

export const AutopilotWorkMultiAgentCoordinationLaneKind = S.Literals([
  'external',
  'hosted',
  'local',
  'market',
  'pylon',
  'unknown',
])
export type AutopilotWorkMultiAgentCoordinationLaneKind =
  typeof AutopilotWorkMultiAgentCoordinationLaneKind.Type

export const AutopilotWorkMultiAgentCoordinationCriticality = S.Literals([
  'mandatory',
  'optional',
])
export type AutopilotWorkMultiAgentCoordinationCriticality =
  typeof AutopilotWorkMultiAgentCoordinationCriticality.Type

export const AutopilotWorkMultiAgentCoordinationLane = S.Struct({
  acceptancePolicyRefs: S.optionalKey(S.Array(S.String)),
  adapterRefs: S.optionalKey(S.Array(S.String)),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  assignmentRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  budgetCapRefs: S.optionalKey(S.Array(S.String)),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  closeoutRefs: S.optionalKey(S.Array(S.String)),
  conflictRefs: S.optionalKey(S.Array(S.String)),
  criticality: AutopilotWorkMultiAgentCoordinationCriticality,
  dependencyRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkMultiAgentCoordinationFreshness),
  inboxRefs: S.optionalKey(S.Array(S.String)),
  kind: AutopilotWorkMultiAgentCoordinationLaneKind,
  laneRef: S.String,
  mergeStrategyRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  providerRefs: S.optionalKey(S.Array(S.String)),
  receiptRefs: S.optionalKey(S.Array(S.String)),
  state: AutopilotWorkMultiAgentCoordinationState,
  steeringReceiptRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkMultiAgentCoordinationLane =
  typeof AutopilotWorkMultiAgentCoordinationLane.Type

export const AutopilotWorkMultiAgentCoordination = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkMultiAgentCoordinationLane)),
  generatedAt: S.optionalKey(S.String),
  parentRunRef: S.optionalKey(S.NullOr(S.String)),
  planRef: S.String,
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkMultiAgentCoordination =
  typeof AutopilotWorkMultiAgentCoordination.Type

export const AutopilotWorkExternalWorkIntakeStatus = S.Literals([
  'admitted',
  'blocked',
  'delivered',
  'expired',
  'pending',
  'rejected',
  'routed',
  'unknown',
])
export type AutopilotWorkExternalWorkIntakeStatus =
  typeof AutopilotWorkExternalWorkIntakeStatus.Type

export const AutopilotWorkExternalWorkIntakeFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkExternalWorkIntakeFreshness =
  typeof AutopilotWorkExternalWorkIntakeFreshness.Type

export const AutopilotWorkExternalWorkIntakeChannel = S.Literals([
  'agent_api',
  'api',
  'autonomous_admin',
  'forum',
  'issue',
  'pylon',
  'schedule',
  'ui',
  'unknown',
])
export type AutopilotWorkExternalWorkIntakeChannel =
  typeof AutopilotWorkExternalWorkIntakeChannel.Type

export const AutopilotWorkExternalWorkIntakeKind = S.Literals([
  'coding_task',
  'debug',
  'migration',
  'research',
  'review',
  'unknown',
])
export type AutopilotWorkExternalWorkIntakeKind =
  typeof AutopilotWorkExternalWorkIntakeKind.Type

export const AutopilotWorkExternalWorkIntakeEntry = S.Struct({
  acceptancePolicyRefs: S.optionalKey(S.Array(S.String)),
  accountRefs: S.optionalKey(S.Array(S.String)),
  adapterPreferenceRefs: S.optionalKey(S.Array(S.String)),
  admissionReceiptRefs: S.optionalKey(S.Array(S.String)),
  apiParityRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  budgetRefs: S.optionalKey(S.Array(S.String)),
  budgetRequired: S.optionalKey(S.Boolean),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  channel: AutopilotWorkExternalWorkIntakeChannel,
  dataClassificationRefs: S.optionalKey(S.Array(S.String)),
  deliveryReceiptRefs: S.optionalKey(S.Array(S.String)),
  expirationRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkExternalWorkIntakeFreshness),
  idempotencyRefs: S.optionalKey(S.Array(S.String)),
  intakeRef: S.String,
  paymentRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  rejectionReceiptRefs: S.optionalKey(S.Array(S.String)),
  requestRefs: S.optionalKey(S.Array(S.String)),
  requesterRefs: S.optionalKey(S.Array(S.String)),
  reviewPolicyRefs: S.optionalKey(S.Array(S.String)),
  routingReceiptRefs: S.optionalKey(S.Array(S.String)),
  scopeRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkExternalWorkIntakeStatus,
  statusReceiptRefs: S.optionalKey(S.Array(S.String)),
  verificationRefs: S.optionalKey(S.Array(S.String)),
  workKind: AutopilotWorkExternalWorkIntakeKind,
  workOrderRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkExternalWorkIntakeEntry =
  typeof AutopilotWorkExternalWorkIntakeEntry.Type

export const AutopilotWorkExternalWorkIntake = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkExternalWorkIntakeEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkExternalWorkIntake =
  typeof AutopilotWorkExternalWorkIntake.Type

export const AutopilotWorkArtifactReceiptArtifactKind = S.Literals([
  'assignment_event',
  'build_log',
  'closeout',
  'diff',
  'patch',
  'payment_evidence',
  'preview',
  'pr_draft',
  'screenshot',
  'settlement_projection',
  'test_result',
  'unknown',
])
export type AutopilotWorkArtifactReceiptArtifactKind =
  typeof AutopilotWorkArtifactReceiptArtifactKind.Type

export const AutopilotWorkArtifactReceiptVisibility = S.Literals([
  'private',
  'public',
  'team',
])
export type AutopilotWorkArtifactReceiptVisibility =
  typeof AutopilotWorkArtifactReceiptVisibility.Type

export const AutopilotWorkArtifactReceiptRedactionClass = S.Literals([
  'local_only',
  'private_ref',
  'public_safe',
  'team_ref',
])
export type AutopilotWorkArtifactReceiptRedactionClass =
  typeof AutopilotWorkArtifactReceiptRedactionClass.Type

export const AutopilotWorkArtifactReceiptFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkArtifactReceiptFreshness =
  typeof AutopilotWorkArtifactReceiptFreshness.Type

export const AutopilotWorkArtifactReceiptTransitionKind = S.Literals([
  'acceptance',
  'admission',
  'delivery',
  'deploy',
  'execution',
  'merge',
  'payment',
  'pr_draft',
  'public_claim',
  'rejection',
  'settlement',
  'unknown',
  'verification',
])
export type AutopilotWorkArtifactReceiptTransitionKind =
  typeof AutopilotWorkArtifactReceiptTransitionKind.Type

export const AutopilotWorkArtifactReceiptArtifact = S.Struct({
  artifactRef: S.String,
  assignmentRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  digestRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkArtifactReceiptFreshness),
  kind: AutopilotWorkArtifactReceiptArtifactKind,
  laneRefs: S.optionalKey(S.Array(S.String)),
  mediaTypeRefs: S.optionalKey(S.Array(S.String)),
  missionRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  producerRefs: S.optionalKey(S.Array(S.String)),
  redactionClass: S.optionalKey(AutopilotWorkArtifactReceiptRedactionClass),
  relatedReceiptRefs: S.optionalKey(S.Array(S.String)),
  retentionRefs: S.optionalKey(S.Array(S.String)),
  runRefs: S.optionalKey(S.Array(S.String)),
  sizeRefs: S.optionalKey(S.Array(S.String)),
  subjectRefs: S.optionalKey(S.Array(S.String)),
  summaryRefs: S.optionalKey(S.Array(S.String)),
  visibility: AutopilotWorkArtifactReceiptVisibility,
  workOrderRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkArtifactReceiptArtifact =
  typeof AutopilotWorkArtifactReceiptArtifact.Type

export const AutopilotWorkArtifactReceiptReceipt = S.Struct({
  actorRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  claimRequirementRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkArtifactReceiptFreshness),
  idempotencyRefs: S.optionalKey(S.Array(S.String)),
  inputRefs: S.optionalKey(S.Array(S.String)),
  outputRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  receiptRef: S.String,
  satisfyingReceiptRefs: S.optionalKey(S.Array(S.String)),
  serviceRefs: S.optionalKey(S.Array(S.String)),
  subjectRefs: S.optionalKey(S.Array(S.String)),
  transitionKind: AutopilotWorkArtifactReceiptTransitionKind,
  verificationRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkArtifactReceiptReceipt =
  typeof AutopilotWorkArtifactReceiptReceipt.Type

export const AutopilotWorkArtifactReceiptIndex = S.Struct({
  artifacts: S.optionalKey(S.Array(AutopilotWorkArtifactReceiptArtifact)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.optionalKey(S.String),
  receipts: S.optionalKey(S.Array(AutopilotWorkArtifactReceiptReceipt)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkArtifactReceiptIndex =
  typeof AutopilotWorkArtifactReceiptIndex.Type

export const AutopilotWorkSchedulingCronTriggerKind = S.Literals([
  'continuation',
  'maintenance',
  'one_shot',
  'overnight',
  'recurring',
  'retry_window',
  'unknown',
])
export type AutopilotWorkSchedulingCronTriggerKind =
  typeof AutopilotWorkSchedulingCronTriggerKind.Type

export const AutopilotWorkSchedulingCronStatus = S.Literals([
  'active',
  'blocked',
  'cancelled',
  'failed',
  'fired',
  'paused',
  'skipped',
  'unknown',
])
export type AutopilotWorkSchedulingCronStatus =
  typeof AutopilotWorkSchedulingCronStatus.Type

export const AutopilotWorkSchedulingCronFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkSchedulingCronFreshness =
  typeof AutopilotWorkSchedulingCronFreshness.Type

export const AutopilotWorkSchedulingCronSchedule = S.Struct({
  adapterPreferenceRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  budgetPolicyRefs: S.optionalKey(S.Array(S.String)),
  cancelReceiptRefs: S.optionalKey(S.Array(S.String)),
  continuationPolicyRefs: S.optionalKey(S.Array(S.String)),
  failureReceiptRefs: S.optionalKey(S.Array(S.String)),
  fireReceiptRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkSchedulingCronFreshness),
  lastRunRefs: S.optionalKey(S.Array(S.String)),
  nextRunRefs: S.optionalKey(S.Array(S.String)),
  noDoubleFireReceiptRefs: S.optionalKey(S.Array(S.String)),
  notificationPolicyRefs: S.optionalKey(S.Array(S.String)),
  ownerRefs: S.optionalKey(S.Array(S.String)),
  permissionPolicyRefs: S.optionalKey(S.Array(S.String)),
  providerPreferenceRefs: S.optionalKey(S.Array(S.String)),
  repoRefs: S.optionalKey(S.Array(S.String)),
  retentionPolicyRefs: S.optionalKey(S.Array(S.String)),
  runReceiptRefs: S.optionalKey(S.Array(S.String)),
  scheduleRef: S.String,
  skipReceiptRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkSchedulingCronStatus,
  teamRefs: S.optionalKey(S.Array(S.String)),
  timezoneRefs: S.optionalKey(S.Array(S.String)),
  triggerKind: AutopilotWorkSchedulingCronTriggerKind,
  workOrderTemplateRefs: S.optionalKey(S.Array(S.String)),
  workspaceRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkSchedulingCronSchedule =
  typeof AutopilotWorkSchedulingCronSchedule.Type

export const AutopilotWorkSchedulingCron = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.optionalKey(S.String),
  schedules: S.optionalKey(S.Array(AutopilotWorkSchedulingCronSchedule)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkSchedulingCron =
  typeof AutopilotWorkSchedulingCron.Type

export const AutopilotWorkStructuredEventLogKind = S.Literals([
  'approval_denied',
  'approval_granted',
  'approval_prompt',
  'artifact_created',
  'cancellation',
  'compaction',
  'error',
  'file_edit',
  'model_stream',
  'receipt_created',
  'shell_execution',
  'status_transition',
  'tool_proposal',
  'tool_result',
  'unknown',
])
export type AutopilotWorkStructuredEventLogKind =
  typeof AutopilotWorkStructuredEventLogKind.Type

export const AutopilotWorkStructuredEventLogVisibility = S.Literals([
  'private',
  'public',
  'team',
])
export type AutopilotWorkStructuredEventLogVisibility =
  typeof AutopilotWorkStructuredEventLogVisibility.Type

export const AutopilotWorkStructuredEventLogRedactionClass = S.Literals([
  'local_only',
  'private_ref',
  'public_safe',
  'team_ref',
])
export type AutopilotWorkStructuredEventLogRedactionClass =
  typeof AutopilotWorkStructuredEventLogRedactionClass.Type

export const AutopilotWorkStructuredEventLogStatus = S.Literals([
  'appended',
  'failed',
  'projected',
  'replayed',
  'skipped',
  'unknown',
])
export type AutopilotWorkStructuredEventLogStatus =
  typeof AutopilotWorkStructuredEventLogStatus.Type

export const AutopilotWorkStructuredEventLogFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkStructuredEventLogFreshness =
  typeof AutopilotWorkStructuredEventLogFreshness.Type

export const AutopilotWorkStructuredEventLogEntry = S.Struct({
  actorRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  correlationRefs: S.optionalKey(S.Array(S.String)),
  eventKind: AutopilotWorkStructuredEventLogKind,
  eventRef: S.String,
  exportRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkStructuredEventLogFreshness),
  idempotencyRefs: S.optionalKey(S.Array(S.String)),
  occurredAt: S.optionalKey(S.String),
  parentRefs: S.optionalKey(S.Array(S.String)),
  payloadSchemaVersionRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  projectionRefs: S.optionalKey(S.Array(S.String)),
  redactionClass: AutopilotWorkStructuredEventLogRedactionClass,
  replayRefs: S.optionalKey(S.Array(S.String)),
  retentionRefs: S.optionalKey(S.Array(S.String)),
  runRefs: S.optionalKey(S.Array(S.String)),
  sequence: S.Number,
  sequenceRef: S.optionalKey(S.NullOr(S.String)),
  serviceRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkStructuredEventLogStatus,
  subjectRefs: S.optionalKey(S.Array(S.String)),
  timestampRefs: S.optionalKey(S.Array(S.String)),
  visibility: AutopilotWorkStructuredEventLogVisibility,
})
export type AutopilotWorkStructuredEventLogEntry =
  typeof AutopilotWorkStructuredEventLogEntry.Type

export const AutopilotWorkStructuredEventLog = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  eventStreamRefs: S.optionalKey(S.Array(S.String)),
  events: S.optionalKey(S.Array(AutopilotWorkStructuredEventLogEntry)),
  exportRefs: S.optionalKey(S.Array(S.String)),
  generatedAt: S.optionalKey(S.String),
  policyRefs: S.optionalKey(S.Array(S.String)),
  projectionRefs: S.optionalKey(S.Array(S.String)),
  replayRefs: S.optionalKey(S.Array(S.String)),
  retentionRefs: S.optionalKey(S.Array(S.String)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkStructuredEventLog =
  typeof AutopilotWorkStructuredEventLog.Type

export const AutopilotWorkTelemetryPrivacyMode = S.Literals([
  'disabled',
  'local_only',
  'private_account',
  'product_improvement',
  'team',
  'unknown',
])
export type AutopilotWorkTelemetryPrivacyMode =
  typeof AutopilotWorkTelemetryPrivacyMode.Type

export const AutopilotWorkTelemetryPrivacyClassKind = S.Literals([
  'account_usage',
  'cost_events',
  'health_events',
  'local_diagnostics',
  'product_metrics',
  'safety_events',
  'unknown',
])
export type AutopilotWorkTelemetryPrivacyClassKind =
  typeof AutopilotWorkTelemetryPrivacyClassKind.Type

export const AutopilotWorkTelemetryPrivacyStatus = S.Literals([
  'blocked',
  'disabled',
  'enabled',
  'failed',
  'unknown',
])
export type AutopilotWorkTelemetryPrivacyStatus =
  typeof AutopilotWorkTelemetryPrivacyStatus.Type

export const AutopilotWorkTelemetryPrivacyFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkTelemetryPrivacyFreshness =
  typeof AutopilotWorkTelemetryPrivacyFreshness.Type

export const AutopilotWorkTelemetryPrivacyClass = S.Struct({
  aggregateRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  classKind: AutopilotWorkTelemetryPrivacyClassKind,
  diagnosticBundleRefs: S.optionalKey(S.Array(S.String)),
  deliveryRefs: S.optionalKey(S.Array(S.String)),
  exportabilityRefs: S.optionalKey(S.Array(S.String)),
  failureRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkTelemetryPrivacyFreshness),
  mode: AutopilotWorkTelemetryPrivacyMode,
  optOutRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  privacyFilterRefs: S.optionalKey(S.Array(S.String)),
  redactionScanRefs: S.optionalKey(S.Array(S.String)),
  retentionRefs: S.optionalKey(S.Array(S.String)),
  sinkRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkTelemetryPrivacyStatus,
  telemetryRef: S.String,
  visibilityRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkTelemetryPrivacyClass =
  typeof AutopilotWorkTelemetryPrivacyClass.Type

export const AutopilotWorkTelemetryPrivacy = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  classes: S.optionalKey(S.Array(AutopilotWorkTelemetryPrivacyClass)),
  generatedAt: S.optionalKey(S.String),
  modeRefs: S.optionalKey(S.Array(S.String)),
  optOutRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  privacyFilterRefs: S.optionalKey(S.Array(S.String)),
  redactionScanRefs: S.optionalKey(S.Array(S.String)),
  retentionRefs: S.optionalKey(S.Array(S.String)),
  sinkRefs: S.optionalKey(S.Array(S.String)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkTelemetryPrivacy =
  typeof AutopilotWorkTelemetryPrivacy.Type

export const AutopilotWorkPerformanceLatencyClass = S.Literals([
  'blocked',
  'normal',
  'slow',
  'unknown',
])
export type AutopilotWorkPerformanceLatencyClass =
  typeof AutopilotWorkPerformanceLatencyClass.Type

export const AutopilotWorkPerformanceResourceClass = S.Literals([
  'context',
  'event_log',
  'local_resource',
  'model',
  'output',
  'provider',
  'queue',
  'shell',
  'tool',
  'unknown',
  'verification',
])
export type AutopilotWorkPerformanceResourceClass =
  typeof AutopilotWorkPerformanceResourceClass.Type

export const AutopilotWorkPerformanceStatus = S.Literals([
  'blocked',
  'failed',
  'ok',
  'slow',
  'truncated',
  'unknown',
])
export type AutopilotWorkPerformanceStatus =
  typeof AutopilotWorkPerformanceStatus.Type

export const AutopilotWorkPerformanceFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkPerformanceFreshness =
  typeof AutopilotWorkPerformanceFreshness.Type

export const AutopilotWorkPerformanceEntry = S.Struct({
  artifactRefs: S.optionalKey(S.Array(S.String)),
  backpressureRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  budgetStopRefs: S.optionalKey(S.Array(S.String)),
  counterRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkPerformanceFreshness),
  latencyClass: AutopilotWorkPerformanceLatencyClass,
  localResourcePressureRefs: S.optionalKey(S.Array(S.String)),
  outputVolumeRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  profileRefs: S.optionalKey(S.Array(S.String)),
  providerRateLimitRefs: S.optionalKey(S.Array(S.String)),
  redactionRefs: S.optionalKey(S.Array(S.String)),
  resourceClass: AutopilotWorkPerformanceResourceClass,
  runRefs: S.optionalKey(S.Array(S.String)),
  spanRef: S.String,
  status: AutopilotWorkPerformanceStatus,
  timeoutRefs: S.optionalKey(S.Array(S.String)),
  truncationRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkPerformanceEntry =
  typeof AutopilotWorkPerformanceEntry.Type

export const AutopilotWorkPerformanceDiagnostics = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkPerformanceEntry)),
  generatedAt: S.optionalKey(S.String),
  profileRefs: S.optionalKey(S.Array(S.String)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkPerformanceDiagnostics =
  typeof AutopilotWorkPerformanceDiagnostics.Type

export const AutopilotWorkUpdateReleaseChannel = S.Literals([
  'beta',
  'canary',
  'dev',
  'managed',
  'nightly',
  'stable',
  'unknown',
])
export type AutopilotWorkUpdateReleaseChannel =
  typeof AutopilotWorkUpdateReleaseChannel.Type

export const AutopilotWorkUpdateReleaseStatus = S.Literals([
  'available',
  'blocked',
  'current',
  'failed',
  'recommended',
  'required',
  'unknown',
])
export type AutopilotWorkUpdateReleaseStatus =
  typeof AutopilotWorkUpdateReleaseStatus.Type

export const AutopilotWorkUpdateReleaseFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkUpdateReleaseFreshness =
  typeof AutopilotWorkUpdateReleaseFreshness.Type

export const AutopilotWorkUpdateReleaseEntry = S.Struct({
  activeRunRefs: S.optionalKey(S.Array(S.String)),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  channel: AutopilotWorkUpdateReleaseChannel,
  channelRefs: S.optionalKey(S.Array(S.String)),
  checksumRefs: S.optionalKey(S.Array(S.String)),
  compatibilityRefs: S.optionalKey(S.Array(S.String)),
  deprecationRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkUpdateReleaseFreshness),
  knownBlockerRefs: S.optionalKey(S.Array(S.String)),
  managedPinRefs: S.optionalKey(S.Array(S.String)),
  manifestRefs: S.optionalKey(S.Array(S.String)),
  migrationRefs: S.optionalKey(S.Array(S.String)),
  migrationRequired: S.optionalKey(S.Boolean),
  platformRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  releaseNoteRefs: S.optionalKey(S.Array(S.String)),
  releaseRef: S.String,
  restorePointRefs: S.optionalKey(S.Array(S.String)),
  rollbackRefs: S.optionalKey(S.Array(S.String)),
  rolloutRefs: S.optionalKey(S.Array(S.String)),
  runtimeRequirementRefs: S.optionalKey(S.Array(S.String)),
  safeUpdateWindowRefs: S.optionalKey(S.Array(S.String)),
  signatureRefs: S.optionalKey(S.Array(S.String)),
  smokeReceiptRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkUpdateReleaseStatus,
  supportRefs: S.optionalKey(S.Array(S.String)),
  versionRef: S.String,
})
export type AutopilotWorkUpdateReleaseEntry =
  typeof AutopilotWorkUpdateReleaseEntry.Type

export const AutopilotWorkUpdateRelease = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkUpdateReleaseEntry)),
  generatedAt: S.optionalKey(S.String),
  manifestRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkUpdateRelease =
  typeof AutopilotWorkUpdateRelease.Type

export const AutopilotWorkMigrationDomain = S.Literals([
  'artifact_indexes',
  'credentials_metadata',
  'event_logs',
  'memory_records',
  'plugin_skill_registries',
  'public_projections',
  'release_channel_metadata',
  'repository_profiles',
  'session_summaries',
  'settings',
  'telemetry_preferences',
  'tool_permission_caches',
  'unknown',
])
export type AutopilotWorkMigrationDomain =
  typeof AutopilotWorkMigrationDomain.Type

export const AutopilotWorkMigrationStatus = S.Literals([
  'blocked',
  'completed',
  'failed',
  'pending',
  'rebuildable',
  'required',
  'rolled_back',
  'skipped',
  'unknown',
])
export type AutopilotWorkMigrationStatus =
  typeof AutopilotWorkMigrationStatus.Type

export const AutopilotWorkMigrationFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkMigrationFreshness =
  typeof AutopilotWorkMigrationFreshness.Type

export const AutopilotWorkMigrationEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  domain: AutopilotWorkMigrationDomain,
  domainRef: S.String,
  downgradeRefs: S.optionalKey(S.Array(S.String)),
  downgradeRequired: S.optionalKey(S.Boolean),
  freshness: S.optionalKey(AutopilotWorkMigrationFreshness),
  idempotencyRefs: S.optionalKey(S.Array(S.String)),
  migrationRefs: S.optionalKey(S.Array(S.String)),
  optionalCache: S.optionalKey(S.Boolean),
  optionalCacheRebuildRefs: S.optionalKey(S.Array(S.String)),
  policyRefs: S.optionalKey(S.Array(S.String)),
  receiptRefs: S.optionalKey(S.Array(S.String)),
  recoveryRefs: S.optionalKey(S.Array(S.String)),
  redactionRefs: S.optionalKey(S.Array(S.String)),
  registryRefs: S.optionalKey(S.Array(S.String)),
  required: S.optionalKey(S.Boolean),
  restorePointRefs: S.optionalKey(S.Array(S.String)),
  rollbackBoundaryRefs: S.optionalKey(S.Array(S.String)),
  schemaFromRef: S.String,
  schemaToRef: S.String,
  status: AutopilotWorkMigrationStatus,
  validationRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkMigrationEntry =
  typeof AutopilotWorkMigrationEntry.Type

export const AutopilotWorkMigrationEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkMigrationEntry)),
  generatedAt: S.optionalKey(S.String),
  registryRefs: S.optionalKey(S.Array(S.String)),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkMigrationEvidence =
  typeof AutopilotWorkMigrationEvidence.Type

export const AutopilotWorkTestingSmokeLayer = S.Literals([
  'adapter_contract',
  'ci_smoke',
  'fixture',
  'integration',
  'live_smoke',
  'local_device_smoke',
  'regression',
  'schema',
  'service',
  'staging_smoke',
  'unit',
  'unknown',
])
export type AutopilotWorkTestingSmokeLayer =
  typeof AutopilotWorkTestingSmokeLayer.Type

export const AutopilotWorkTestingSmokeClassification = S.Literals([
  'ci_safe',
  'deploy',
  'live',
  'local',
  'no_spend',
  'paid',
  'settlement',
  'staging',
  'unknown',
  'write',
])
export type AutopilotWorkTestingSmokeClassification =
  typeof AutopilotWorkTestingSmokeClassification.Type

export const AutopilotWorkTestingSmokeStatus = S.Literals([
  'blocked',
  'failed',
  'passed',
  'pending',
  'skipped',
  'stale',
  'unknown',
])
export type AutopilotWorkTestingSmokeStatus =
  typeof AutopilotWorkTestingSmokeStatus.Type

export const AutopilotWorkTestingSmokeFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkTestingSmokeFreshness =
  typeof AutopilotWorkTestingSmokeFreshness.Type

export const AutopilotWorkTestingSmokeEntry = S.Struct({
  adapterAvailabilityRefs: S.optionalKey(S.Array(S.String)),
  approvalRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  classifications: S.optionalKey(S.Array(AutopilotWorkTestingSmokeClassification)),
  commandRefs: S.optionalKey(S.Array(S.String)),
  credentialAvailabilityRefs: S.optionalKey(S.Array(S.String)),
  environmentRefs: S.optionalKey(S.Array(S.String)),
  failureRefs: S.optionalKey(S.Array(S.String)),
  fixtureRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkTestingSmokeFreshness),
  layer: AutopilotWorkTestingSmokeLayer,
  policyRefs: S.optionalKey(S.Array(S.String)),
  productClaimRefs: S.optionalKey(S.Array(S.String)),
  proofBoundaryRefs: S.optionalKey(S.Array(S.String)),
  providerAvailabilityRefs: S.optionalKey(S.Array(S.String)),
  redactionScanRefs: S.optionalKey(S.Array(S.String)),
  smokeReceiptRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkTestingSmokeStatus,
  testRef: S.String,
  versionRefs: S.optionalKey(S.Array(S.String)),
  workspaceAvailabilityRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkTestingSmokeEntry =
  typeof AutopilotWorkTestingSmokeEntry.Type

export const AutopilotWorkTestingSmokeEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkTestingSmokeEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkTestingSmokeEvidence =
  typeof AutopilotWorkTestingSmokeEvidence.Type

export const AutopilotWorkEvaluationRegressionStatus = S.Literals([
  'blocked',
  'failed',
  'passed',
  'pending',
  'regressed',
  'unknown',
])
export type AutopilotWorkEvaluationRegressionStatus =
  typeof AutopilotWorkEvaluationRegressionStatus.Type

export const AutopilotWorkEvaluationRegressionFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkEvaluationRegressionFreshness =
  typeof AutopilotWorkEvaluationRegressionFreshness.Type

export const AutopilotWorkEvaluationRegressionEntry = S.Struct({
  adapterRefs: S.optionalKey(S.Array(S.String)),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  budgetPolicyRefs: S.optionalKey(S.Array(S.String)),
  costSummaryRefs: S.optionalKey(S.Array(S.String)),
  failureRefs: S.optionalKey(S.Array(S.String)),
  firstDivergenceRefs: S.optionalKey(S.Array(S.String)),
  fixturePromotionRefs: S.optionalKey(S.Array(S.String)),
  fixtureProvenanceRefs: S.optionalKey(S.Array(S.String)),
  fixtureRedactionRefs: S.optionalKey(S.Array(S.String)),
  fixtureRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkEvaluationRegressionFreshness),
  latencySummaryRefs: S.optionalKey(S.Array(S.String)),
  modelRefs: S.optionalKey(S.Array(S.String)),
  privateReportRefs: S.optionalKey(S.Array(S.String)),
  productClaimRefs: S.optionalKey(S.Array(S.String)),
  providerRefs: S.optionalKey(S.Array(S.String)),
  publicReportRefs: S.optionalKey(S.Array(S.String)),
  regressionGateRefs: S.optionalKey(S.Array(S.String)),
  resultVerdictRefs: S.optionalKey(S.Array(S.String)),
  reviewRefs: S.optionalKey(S.Array(S.String)),
  safetyVerdictRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkEvaluationRegressionStatus,
  suiteRefs: S.optionalKey(S.Array(S.String)),
  thresholdRefs: S.optionalKey(S.Array(S.String)),
  toolPolicyRefs: S.optionalKey(S.Array(S.String)),
  evaluationRef: S.String,
  versionRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkEvaluationRegressionEntry =
  typeof AutopilotWorkEvaluationRegressionEntry.Type

export const AutopilotWorkEvaluationRegressionEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkEvaluationRegressionEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkEvaluationRegressionEvidence =
  typeof AutopilotWorkEvaluationRegressionEvidence.Type

export const AutopilotWorkSecurityReviewDomain = S.Literals([
  'browser_desktop_integration',
  'data_retention_deletion',
  'filesystem_workspace',
  'mcp_plugins_hooks_skills',
  'payment_wallet_settlement',
  'provider_credentials',
  'public_projection_claims',
  'release_artifacts',
  'remote_session_bridge',
  'shell_execution',
  'unknown',
])
export type AutopilotWorkSecurityReviewDomain =
  typeof AutopilotWorkSecurityReviewDomain.Type

export const AutopilotWorkSecurityReviewRisk = S.Literals([
  'blocked',
  'critical',
  'high',
  'low',
  'medium',
  'unknown',
])
export type AutopilotWorkSecurityReviewRisk =
  typeof AutopilotWorkSecurityReviewRisk.Type

export const AutopilotWorkSecurityReviewStatus = S.Literals([
  'approved',
  'blocked',
  'denied',
  'expired',
  'needs_review',
  'unknown',
])
export type AutopilotWorkSecurityReviewStatus =
  typeof AutopilotWorkSecurityReviewStatus.Type

export const AutopilotWorkSecurityReviewFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkSecurityReviewFreshness =
  typeof AutopilotWorkSecurityReviewFreshness.Type

export const AutopilotWorkSecurityReviewEntry = S.Struct({
  approvalGateRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  diagnosticBundleRefs: S.optionalKey(S.Array(S.String)),
  domain: AutopilotWorkSecurityReviewDomain,
  domainRef: S.String,
  denialReceiptRefs: S.optionalKey(S.Array(S.String)),
  exceptionExpiryRefs: S.optionalKey(S.Array(S.String)),
  exceptionRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkSecurityReviewFreshness),
  ownerPolicyRefs: S.optionalKey(S.Array(S.String)),
  providerCredentialPolicyRefs: S.optionalKey(S.Array(S.String)),
  publicProjectionScanRefs: S.optionalKey(S.Array(S.String)),
  redactionScanRefs: S.optionalKey(S.Array(S.String)),
  regressionFixtureRefs: S.optionalKey(S.Array(S.String)),
  releaseIntegrityRefs: S.optionalKey(S.Array(S.String)),
  risk: AutopilotWorkSecurityReviewRisk,
  status: AutopilotWorkSecurityReviewStatus,
  threatModelRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkSecurityReviewEntry =
  typeof AutopilotWorkSecurityReviewEntry.Type

export const AutopilotWorkSecurityReviewEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkSecurityReviewEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkSecurityReviewEvidence =
  typeof AutopilotWorkSecurityReviewEvidence.Type

export const AutopilotWorkDataRetentionClass = S.Literals([
  'artifact_indexes',
  'artifact_payloads',
  'credential_metadata',
  'ephemeral_capture_data',
  'memory_records',
  'private_event_log_payloads',
  'product_receipts',
  'public_projections',
  'public_safe_event_refs',
  'session_summaries',
  'telemetry_aggregates',
  'temporary_workspace_material',
  'unknown',
])
export type AutopilotWorkDataRetentionClass =
  typeof AutopilotWorkDataRetentionClass.Type

export const AutopilotWorkDataRetentionStatus = S.Literals([
  'active',
  'blocked',
  'delete_requested',
  'deleted',
  'expired',
  'legal_hold',
  'retained',
  'tombstoned',
  'unknown',
])
export type AutopilotWorkDataRetentionStatus =
  typeof AutopilotWorkDataRetentionStatus.Type

export const AutopilotWorkDataRetentionFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkDataRetentionFreshness =
  typeof AutopilotWorkDataRetentionFreshness.Type

export const AutopilotWorkDataRetentionEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  dataClass: AutopilotWorkDataRetentionClass,
  dataClassRef: S.String,
  deletionReceiptRefs: S.optionalKey(S.Array(S.String)),
  deletionRequestRefs: S.optionalKey(S.Array(S.String)),
  exportManifestRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkDataRetentionFreshness),
  legalHoldRefs: S.optionalKey(S.Array(S.String)),
  projectionFreshnessRefs: S.optionalKey(S.Array(S.String)),
  projectionInvalidationRefs: S.optionalKey(S.Array(S.String)),
  retentionPolicyRefs: S.optionalKey(S.Array(S.String)),
  retentionSweepRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkDataRetentionStatus,
  tombstoneRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkDataRetentionEntry =
  typeof AutopilotWorkDataRetentionEntry.Type

export const AutopilotWorkDataRetentionDeletionEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkDataRetentionEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkDataRetentionDeletionEvidence =
  typeof AutopilotWorkDataRetentionDeletionEvidence.Type

export const AutopilotWorkOnboardingCapabilityMode = S.Literals([
  'api_connected',
  'local_only',
  'managed',
  'pylon_provider',
  'team',
  'unknown',
])
export type AutopilotWorkOnboardingCapabilityMode =
  typeof AutopilotWorkOnboardingCapabilityMode.Type

export const AutopilotWorkOnboardingCapabilityStepKind = S.Literals([
  'capability_probe',
  'data_scope',
  'first_run_smoke',
  'instructions_invariants',
  'integration',
  'permission',
  'provider',
  'repository_profile',
  'workspace',
  'unknown',
])
export type AutopilotWorkOnboardingCapabilityStepKind =
  typeof AutopilotWorkOnboardingCapabilityStepKind.Type

export const AutopilotWorkOnboardingCapabilityStatus = S.Literals([
  'blocked',
  'completed',
  'in_progress',
  'planned',
  'ready',
  'skipped',
  'unknown',
])
export type AutopilotWorkOnboardingCapabilityStatus =
  typeof AutopilotWorkOnboardingCapabilityStatus.Type

export const AutopilotWorkOnboardingCapabilityFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkOnboardingCapabilityFreshness =
  typeof AutopilotWorkOnboardingCapabilityFreshness.Type

export const AutopilotWorkOnboardingCapabilityEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  capabilityProbeRefs: S.optionalKey(S.Array(S.String)),
  completionReceiptRefs: S.optionalKey(S.Array(S.String)),
  credentialPolicyRefs: S.optionalKey(S.Array(S.String)),
  dataScopeRefs: S.optionalKey(S.Array(S.String)),
  firstRunSmokeRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkOnboardingCapabilityFreshness),
  instructionRefs: S.optionalKey(S.Array(S.String)),
  integrationRefs: S.optionalKey(S.Array(S.String)),
  invariantRefs: S.optionalKey(S.Array(S.String)),
  mode: AutopilotWorkOnboardingCapabilityMode,
  permissionDecisionRefs: S.optionalKey(S.Array(S.String)),
  providerReadinessRefs: S.optionalKey(S.Array(S.String)),
  repositoryProfileRefs: S.optionalKey(S.Array(S.String)),
  skipReceiptRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkOnboardingCapabilityStatus,
  stepKind: AutopilotWorkOnboardingCapabilityStepKind,
  stepRef: S.String,
  userDeviceRefs: S.optionalKey(S.Array(S.String)),
  workspaceRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkOnboardingCapabilityEntry =
  typeof AutopilotWorkOnboardingCapabilityEntry.Type

export const AutopilotWorkOnboardingCapabilityEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkOnboardingCapabilityEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkOnboardingCapabilityEvidence =
  typeof AutopilotWorkOnboardingCapabilityEvidence.Type

export const AutopilotWorkOutputStyleVerbosity = S.Literals([
  'concise',
  'detailed',
  'normal',
  'unknown',
])
export type AutopilotWorkOutputStyleVerbosity =
  typeof AutopilotWorkOutputStyleVerbosity.Type

export const AutopilotWorkOutputStyleDomainMode = S.Literals([
  'implementation',
  'planning',
  'review',
  'status',
  'support',
  'unknown',
])
export type AutopilotWorkOutputStyleDomainMode =
  typeof AutopilotWorkOutputStyleDomainMode.Type

export const AutopilotWorkOutputStyleStatus = S.Literals([
  'blocked',
  'conflicted',
  'planned',
  'ready',
  'unknown',
])
export type AutopilotWorkOutputStyleStatus =
  typeof AutopilotWorkOutputStyleStatus.Type

export const AutopilotWorkOutputStyleFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkOutputStyleFreshness =
  typeof AutopilotWorkOutputStyleFreshness.Type

export const AutopilotWorkOutputStyleEntry = S.Struct({
  accessibilityRefs: S.optionalKey(S.Array(S.String)),
  audienceRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  citationRequirementRefs: S.optionalKey(S.Array(S.String)),
  claimReceiptRefs: S.optionalKey(S.Array(S.String)),
  conflictResolutionRefs: S.optionalKey(S.Array(S.String)),
  disallowedClaimRefs: S.optionalKey(S.Array(S.String)),
  domainMode: AutopilotWorkOutputStyleDomainMode,
  evidenceRequirementRefs: S.optionalKey(S.Array(S.String)),
  finalAnswerExpectationRefs: S.optionalKey(S.Array(S.String)),
  formattingRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkOutputStyleFreshness),
  managedPolicyRefs: S.optionalKey(S.Array(S.String)),
  overrideRefs: S.optionalKey(S.Array(S.String)),
  personaConstraintRefs: S.optionalKey(S.Array(S.String)),
  productDefaultRefs: S.optionalKey(S.Array(S.String)),
  projectConstraintRefs: S.optionalKey(S.Array(S.String)),
  safetyPolicyRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkOutputStyleStatus,
  styleAuditRefs: S.optionalKey(S.Array(S.String)),
  stylePolicyRef: S.String,
  toolAuthorityBoundaryRefs: S.optionalKey(S.Array(S.String)),
  userPreferenceRefs: S.optionalKey(S.Array(S.String)),
  verbosity: AutopilotWorkOutputStyleVerbosity,
})
export type AutopilotWorkOutputStyleEntry =
  typeof AutopilotWorkOutputStyleEntry.Type

export const AutopilotWorkOutputStylePersonaEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkOutputStyleEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkOutputStylePersonaEvidence =
  typeof AutopilotWorkOutputStylePersonaEvidence.Type

export const AutopilotWorkPromptSuggestionKind = S.Literals([
  'artifact',
  'command_argument',
  'file',
  'follow_up_action',
  'issue',
  'prompt_starter',
  'session',
  'slash_command',
  'symbol',
  'unknown',
  'workflow',
])
export type AutopilotWorkPromptSuggestionKind =
  typeof AutopilotWorkPromptSuggestionKind.Type

export const AutopilotWorkPromptSuggestionPrivacy = S.Literals([
  'public_safe',
  'scoped_private',
  'team_scoped',
  'unknown',
])
export type AutopilotWorkPromptSuggestionPrivacy =
  typeof AutopilotWorkPromptSuggestionPrivacy.Type

export const AutopilotWorkPromptSuggestionStatus = S.Literals([
  'blocked',
  'disabled',
  'expired',
  'ready',
  'stale',
  'unknown',
])
export type AutopilotWorkPromptSuggestionStatus =
  typeof AutopilotWorkPromptSuggestionStatus.Type

export const AutopilotWorkPromptSuggestionFreshness = S.Literals([
  'expired',
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkPromptSuggestionFreshness =
  typeof AutopilotWorkPromptSuggestionFreshness.Type

export const AutopilotWorkPromptSuggestionEntry = S.Struct({
  actionRef: S.optionalKey(S.NullOr(S.String)),
  actionSeparationRefs: S.optionalKey(S.Array(S.String)),
  auditRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  confidenceRefs: S.optionalKey(S.Array(S.String)),
  destructiveActionRefs: S.optionalKey(S.Array(S.String)),
  disablementRefs: S.optionalKey(S.Array(S.String)),
  displayRefs: S.optionalKey(S.Array(S.String)),
  expirationRefs: S.optionalKey(S.Array(S.String)),
  externalActionRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkPromptSuggestionFreshness),
  insertTextRefs: S.optionalKey(S.Array(S.String)),
  kind: AutopilotWorkPromptSuggestionKind,
  permissionRefs: S.optionalKey(S.Array(S.String)),
  privacy: AutopilotWorkPromptSuggestionPrivacy,
  privacyRefs: S.optionalKey(S.Array(S.String)),
  provenanceRefs: S.optionalKey(S.Array(S.String)),
  rankingRefs: S.optionalKey(S.Array(S.String)),
  scopeRefs: S.optionalKey(S.Array(S.String)),
  semanticSelectorRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkPromptSuggestionStatus,
  suggestionRef: S.String,
  validationRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkPromptSuggestionEntry =
  typeof AutopilotWorkPromptSuggestionEntry.Type

export const AutopilotWorkPromptSuggestionsEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkPromptSuggestionEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkPromptSuggestionsEvidence =
  typeof AutopilotWorkPromptSuggestionsEvidence.Type

export const AutopilotWorkTipsEducationTopic = S.Literals([
  'approval',
  'capability',
  'command',
  'first_run',
  'payment',
  'payout',
  'privacy',
  'provider',
  'receipt',
  'settlement',
  'unknown',
  'workflow',
])
export type AutopilotWorkTipsEducationTopic =
  typeof AutopilotWorkTipsEducationTopic.Type

export const AutopilotWorkTipsEducationStatus = S.Literals([
  'blocked',
  'dismissed',
  'expired',
  'ready',
  'unsupported',
  'unknown',
])
export type AutopilotWorkTipsEducationStatus =
  typeof AutopilotWorkTipsEducationStatus.Type

export const AutopilotWorkTipsEducationFreshness = S.Literals([
  'expired',
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkTipsEducationFreshness =
  typeof AutopilotWorkTipsEducationFreshness.Type

export const AutopilotWorkTipsEducationEntry = S.Struct({
  audienceRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  capabilityRefs: S.optionalKey(S.Array(S.String)),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  dismissalReceiptRefs: S.optionalKey(S.Array(S.String)),
  docsRefs: S.optionalKey(S.Array(S.String)),
  expirationRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkTipsEducationFreshness),
  helpTopicRefs: S.optionalKey(S.Array(S.String)),
  liveStateRefs: S.optionalKey(S.Array(S.String)),
  nonInteractiveDocsRefs: S.optionalKey(S.Array(S.String)),
  nonInteractiveModeRefs: S.optionalKey(S.Array(S.String)),
  requiredWarningRefs: S.optionalKey(S.Array(S.String)),
  scopeRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkTipsEducationStatus,
  tipRef: S.String,
  topic: AutopilotWorkTipsEducationTopic,
  triggerRefs: S.optionalKey(S.Array(S.String)),
  unsupportedClaimRefs: S.optionalKey(S.Array(S.String)),
  versionRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkTipsEducationEntry =
  typeof AutopilotWorkTipsEducationEntry.Type

export const AutopilotWorkTipsEducationEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkTipsEducationEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkTipsEducationEvidence =
  typeof AutopilotWorkTipsEducationEvidence.Type

export const AutopilotWorkThemeVisualSurface = S.Literals([
  'mobile',
  'operator',
  'terminal',
  'unknown',
  'web',
])
export type AutopilotWorkThemeVisualSurface =
  typeof AutopilotWorkThemeVisualSurface.Type

export const AutopilotWorkThemeVisualStatus = S.Literals([
  'blocked',
  'ready',
  'stale',
  'unknown',
])
export type AutopilotWorkThemeVisualStatus =
  typeof AutopilotWorkThemeVisualStatus.Type

export const AutopilotWorkThemeVisualFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkThemeVisualFreshness =
  typeof AutopilotWorkThemeVisualFreshness.Type

export const AutopilotWorkThemeVisualEntry = S.Struct({
  attentionColorRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  contrastCheckRefs: S.optionalKey(S.Array(S.String)),
  crossSurfaceRefs: S.optionalKey(S.Array(S.String)),
  densityRefs: S.optionalKey(S.Array(S.String)),
  diffColorRefs: S.optionalKey(S.Array(S.String)),
  focusRingRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkThemeVisualFreshness),
  highContrastRefs: S.optionalKey(S.Array(S.String)),
  managedPolicyRefs: S.optionalKey(S.Array(S.String)),
  monochromeRefs: S.optionalKey(S.Array(S.String)),
  progressColorRefs: S.optionalKey(S.Array(S.String)),
  reducedMotionRefs: S.optionalKey(S.Array(S.String)),
  runtimeReceiptRefs: S.optionalKey(S.Array(S.String)),
  snapshotRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkThemeVisualStatus,
  statusIconRefs: S.optionalKey(S.Array(S.String)),
  statusLabelRefs: S.optionalKey(S.Array(S.String)),
  statusVisualRefs: S.optionalKey(S.Array(S.String)),
  surface: AutopilotWorkThemeVisualSurface,
  themeRef: S.String,
  tokenRefs: S.optionalKey(S.Array(S.String)),
  typographyRefs: S.optionalKey(S.Array(S.String)),
  warningPreservationRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkThemeVisualEntry =
  typeof AutopilotWorkThemeVisualEntry.Type

export const AutopilotWorkThemeVisualEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkThemeVisualEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkThemeVisualEvidence =
  typeof AutopilotWorkThemeVisualEvidence.Type

export const AutopilotWorkAccessibilityInteractionMode = S.Literals([
  'ci',
  'headless_service',
  'interactive_tui',
  'json_output',
  'non_interactive_command',
  'plain_terminal',
  'screen_reader',
  'unknown',
])
export type AutopilotWorkAccessibilityInteractionMode =
  typeof AutopilotWorkAccessibilityInteractionMode.Type

export const AutopilotWorkAccessibilityNonInteractiveStatus = S.Literals([
  'blocked',
  'ready',
  'stale',
  'unknown',
])
export type AutopilotWorkAccessibilityNonInteractiveStatus =
  typeof AutopilotWorkAccessibilityNonInteractiveStatus.Type

export const AutopilotWorkAccessibilityNonInteractiveFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkAccessibilityNonInteractiveFreshness =
  typeof AutopilotWorkAccessibilityNonInteractiveFreshness.Type

export const AutopilotWorkAccessibilityNonInteractiveEntry = S.Struct({
  approvalResolverRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  ciPolicyRefs: S.optionalKey(S.Array(S.String)),
  deployCaveatRefs: S.optionalKey(S.Array(S.String)),
  exitCodeRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkAccessibilityNonInteractiveFreshness),
  highContrastRefs: S.optionalKey(S.Array(S.String)),
  keyboardNavigationRefs: S.optionalKey(S.Array(S.String)),
  mode: AutopilotWorkAccessibilityInteractionMode,
  modeRef: S.String,
  noColorRefs: S.optionalKey(S.Array(S.String)),
  notificationAvailabilityRefs: S.optionalKey(S.Array(S.String)),
  promptAvailabilityRefs: S.optionalKey(S.Array(S.String)),
  providerMutationCaveatRefs: S.optionalKey(S.Array(S.String)),
  pushCaveatRefs: S.optionalKey(S.Array(S.String)),
  reducedMotionRefs: S.optionalKey(S.Array(S.String)),
  remoteBridgeAvailabilityRefs: S.optionalKey(S.Array(S.String)),
  schemaRefs: S.optionalKey(S.Array(S.String)),
  screenReaderStatusRefs: S.optionalKey(S.Array(S.String)),
  spendCaveatRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkAccessibilityNonInteractiveStatus,
  statusLabelRefs: S.optionalKey(S.Array(S.String)),
  structuredOutputRefs: S.optionalKey(S.Array(S.String)),
  terminalCapabilityRefs: S.optionalKey(S.Array(S.String)),
  typedPromptBlockerRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkAccessibilityNonInteractiveEntry =
  typeof AutopilotWorkAccessibilityNonInteractiveEntry.Type

export const AutopilotWorkAccessibilityNonInteractiveEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkAccessibilityNonInteractiveEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkAccessibilityNonInteractiveEvidence =
  typeof AutopilotWorkAccessibilityNonInteractiveEvidence.Type

export const AutopilotWorkLocalizationScope = S.Literals([
  'command',
  'error',
  'help',
  'json_schema',
  'payment',
  'permission_prompt',
  'public_receipt',
  'release_note',
  'status',
  'tip',
  'ui',
  'unknown',
])
export type AutopilotWorkLocalizationScope =
  typeof AutopilotWorkLocalizationScope.Type

export const AutopilotWorkLocalizationStatus = S.Literals([
  'blocked',
  'ready',
  'stale',
  'unknown',
])
export type AutopilotWorkLocalizationStatus =
  typeof AutopilotWorkLocalizationStatus.Type

export const AutopilotWorkLocalizationFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkLocalizationFreshness =
  typeof AutopilotWorkLocalizationFreshness.Type

export const AutopilotWorkLocalizationEntry = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  catalogRefs: S.optionalKey(S.Array(S.String)),
  catalogValidationRefs: S.optionalKey(S.Array(S.String)),
  commandIdStabilityRefs: S.optionalKey(S.Array(S.String)),
  fallbackRefs: S.optionalKey(S.Array(S.String)),
  formatterRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkLocalizationFreshness),
  jsonSchemaStabilityRefs: S.optionalKey(S.Array(S.String)),
  localePreferenceRefs: S.optionalKey(S.Array(S.String)),
  localeRefs: S.optionalKey(S.Array(S.String)),
  localizationRef: S.String,
  missingTranslationRefs: S.optionalKey(S.Array(S.String)),
  paymentLanguageReviewRefs: S.optionalKey(S.Array(S.String)),
  permissionActionRefs: S.optionalKey(S.Array(S.String)),
  permissionIdStabilityRefs: S.optionalKey(S.Array(S.String)),
  permissionPolicyRefs: S.optionalKey(S.Array(S.String)),
  publicReceiptStabilityRefs: S.optionalKey(S.Array(S.String)),
  scope: AutopilotWorkLocalizationScope,
  stableIdBoundaryRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkLocalizationStatus,
  toolIdStabilityRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkLocalizationEntry =
  typeof AutopilotWorkLocalizationEntry.Type

export const AutopilotWorkLocalizationBoundaryEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkLocalizationEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkLocalizationBoundaryEvidence =
  typeof AutopilotWorkLocalizationBoundaryEvidence.Type

export const AutopilotWorkEnterpriseManagedPolicyDecision = S.Literals([
  'allow',
  'ask',
  'deny',
  'restrict',
  'unknown',
])
export type AutopilotWorkEnterpriseManagedPolicyDecision =
  typeof AutopilotWorkEnterpriseManagedPolicyDecision.Type

export const AutopilotWorkEnterpriseManagedPolicyStatus = S.Literals([
  'blocked',
  'ready',
  'stale',
  'unknown',
])
export type AutopilotWorkEnterpriseManagedPolicyStatus =
  typeof AutopilotWorkEnterpriseManagedPolicyStatus.Type

export const AutopilotWorkEnterpriseManagedPolicyFreshness = S.Literals([
  'fresh',
  'stale',
  'unknown',
])
export type AutopilotWorkEnterpriseManagedPolicyFreshness =
  typeof AutopilotWorkEnterpriseManagedPolicyFreshness.Type

export const AutopilotWorkEnterpriseManagedPolicyEntry = S.Struct({
  allowRefs: S.optionalKey(S.Array(S.String)),
  askRefs: S.optionalKey(S.Array(S.String)),
  auditRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  budgetPolicyRefs: S.optionalKey(S.Array(S.String)),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  changeRefs: S.optionalKey(S.Array(S.String)),
  conflictPriorityRefs: S.optionalKey(S.Array(S.String)),
  conflictRefs: S.optionalKey(S.Array(S.String)),
  conflictResolutionRefs: S.optionalKey(S.Array(S.String)),
  decision: AutopilotWorkEnterpriseManagedPolicyDecision,
  denialRefs: S.optionalKey(S.Array(S.String)),
  devicePolicyRefs: S.optionalKey(S.Array(S.String)),
  effectiveAtRefs: S.optionalKey(S.Array(S.String)),
  effectivePolicyRefs: S.optionalKey(S.Array(S.String)),
  emergencyOverrideReceiptRefs: S.optionalKey(S.Array(S.String)),
  enforcementModeRefs: S.optionalKey(S.Array(S.String)),
  expirationRefs: S.optionalKey(S.Array(S.String)),
  freshness: S.optionalKey(AutopilotWorkEnterpriseManagedPolicyFreshness),
  hookPolicyRefs: S.optionalKey(S.Array(S.String)),
  mcpPolicyRefs: S.optionalKey(S.Array(S.String)),
  organizationPolicyRefs: S.optionalKey(S.Array(S.String)),
  ownerAdminRefs: S.optionalKey(S.Array(S.String)),
  pluginPolicyRefs: S.optionalKey(S.Array(S.String)),
  policyRef: S.String,
  projectPolicyRefs: S.optionalKey(S.Array(S.String)),
  providerPolicyRefs: S.optionalKey(S.Array(S.String)),
  publicSummaryRefs: S.optionalKey(S.Array(S.String)),
  remoteBridgePolicyRefs: S.optionalKey(S.Array(S.String)),
  repositoryPolicyRefs: S.optionalKey(S.Array(S.String)),
  restrictRefs: S.optionalKey(S.Array(S.String)),
  retentionPolicyRefs: S.optionalKey(S.Array(S.String)),
  ruleKindRefs: S.optionalKey(S.Array(S.String)),
  runtimeCapabilityBoundaryRefs: S.optionalKey(S.Array(S.String)),
  scopeRefs: S.optionalKey(S.Array(S.String)),
  sessionPolicyRefs: S.optionalKey(S.Array(S.String)),
  status: AutopilotWorkEnterpriseManagedPolicyStatus,
  teamPolicyRefs: S.optionalKey(S.Array(S.String)),
  telemetryPolicyRefs: S.optionalKey(S.Array(S.String)),
  updatePolicyRefs: S.optionalKey(S.Array(S.String)),
  userPolicyRefs: S.optionalKey(S.Array(S.String)),
  userSafeReasonRefs: S.optionalKey(S.Array(S.String)),
  versionRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkEnterpriseManagedPolicyEntry =
  typeof AutopilotWorkEnterpriseManagedPolicyEntry.Type

export const AutopilotWorkEnterpriseManagedPolicyEvidence = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(AutopilotWorkEnterpriseManagedPolicyEntry)),
  generatedAt: S.optionalKey(S.String),
  snapshotRef: S.String,
  versionRef: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkEnterpriseManagedPolicyEvidence =
  typeof AutopilotWorkEnterpriseManagedPolicyEvidence.Type

export const AutopilotWorkReviewDecision = S.Struct({
  acceptedWorkAuthority: S.Boolean,
  action: AutopilotWorkReviewAction,
  actorAgentCredentialId: S.String,
  actorAgentUserId: S.String,
  decisionRefs: S.Array(S.String),
  deployAuthority: S.Boolean,
  forumAutoPublishAllowed: S.Boolean,
  idempotencyKeyHash: S.String,
  publicSafe: S.Boolean,
  recordedAt: S.String,
  rejectionRefs: S.Array(S.String),
  revisionRequestRefs: S.Array(S.String),
  settlementAuthority: S.Boolean,
  workerPayoutAuthority: S.Boolean,
})
export type AutopilotWorkReviewDecision =
  typeof AutopilotWorkReviewDecision.Type

export const AutopilotWorkProjection = S.Struct({
  accessRequestRefs: S.Array(S.String),
  accessRequirements: S.Array(S.Unknown),
  assignmentIntents: S.Array(S.Unknown),
  artifactReceiptIndex: S.optionalKey(AutopilotWorkArtifactReceiptIndex),
  browserDesktopIntegration: S.optionalKey(AutopilotWorkBrowserDesktopIntegration),
  buyerPaymentProofRef: S.NullOr(S.String),
  clientRequestRef: S.String,
  commandSystem: S.optionalKey(AutopilotWorkCommandSystem),
  companionSurface: S.optionalKey(AutopilotWorkCompanionSurface),
  createdAt: S.String,
  eventStreamRef: S.String,
  executionCloseout: S.NullOr(AutopilotWorkExecutionCloseout),
  fallbackLeaseIntents: S.Array(S.Unknown),
  funding: S.Unknown,
  generatedAt: S.String,
  credentialStorage: S.optionalKey(AutopilotWorkCredentialStorage),
  idempotent: S.Boolean,
  nextAction: S.Struct({
    callerActionRefs: S.Array(S.String),
    reasonRefs: S.Array(S.String),
    retryAfterSeconds: S.NullOr(S.Number),
    state: S.String,
  }),
  paymentChallenge: S.NullOr(S.Unknown),
  paymentChallengeRef: S.NullOr(S.String),
  placementDecision: S.Unknown,
  placementPolicy: S.Unknown,
  promiseRef: S.NullOr(AutopilotWorkPromiseRef),
  pylonAssignmentIntents: S.Array(S.Unknown),
  quote: S.Unknown,
  repositoryAuthorities: S.Array(S.Unknown),
  reviewDecision: S.NullOr(AutopilotWorkReviewDecision),
  compaction: S.optionalKey(AutopilotWorkCompaction),
  contextSnapshot: S.optionalKey(AutopilotWorkContextSnapshot),
  diagnostics: S.optionalKey(AutopilotWorkDiagnostics),
  editorIntegration: S.optionalKey(AutopilotWorkEditorIntegration),
  errorRecovery: S.optionalKey(AutopilotWorkErrorRecovery),
  evaluationRegressionEvidence: S.optionalKey(
    AutopilotWorkEvaluationRegressionEvidence,
  ),
  extensibility: S.optionalKey(AutopilotWorkExtensibility),
  externalWorkIntake: S.optionalKey(AutopilotWorkExternalWorkIntake),
  gitWorkflow: S.optionalKey(AutopilotWorkGitWorkflow),
  helpDoctorDebug: S.optionalKey(AutopilotWorkHelpDoctorDebug),
  instructionLayering: S.optionalKey(AutopilotWorkInstructionLayering),
  inputKeybinding: S.optionalKey(AutopilotWorkInputKeybinding),
  mcpServerExport: S.optionalKey(AutopilotWorkMcpServerExport),
  modelProvider: S.optionalKey(AutopilotWorkModelProviderResolution),
  migrationEvidence: S.optionalKey(AutopilotWorkMigrationEvidence),
  multimodalInput: S.optionalKey(AutopilotWorkMultimodalInput),
  multiAgentCoordination: S.optionalKey(AutopilotWorkMultiAgentCoordination),
  notificationAttention: S.optionalKey(AutopilotWorkNotificationAttention),
  planMutationReceipts: S.optionalKey(S.Array(AutopilotWorkPlanMutationReceipt)),
  planMutationRequests: S.optionalKey(S.Array(AutopilotWorkPlanMutationRequest)),
  performanceDiagnostics: S.optionalKey(AutopilotWorkPerformanceDiagnostics),
  remoteSessionBridge: S.optionalKey(AutopilotWorkRemoteSessionBridge),
  retrievalPlan: S.optionalKey(AutopilotWorkRetrievalPlan),
  schedulingCron: S.optionalKey(AutopilotWorkSchedulingCron),
  securityReviewEvidence: S.optionalKey(AutopilotWorkSecurityReviewEvidence),
  dataRetentionDeletionEvidence: S.optionalKey(
    AutopilotWorkDataRetentionDeletionEvidence,
  ),
  onboardingCapabilityEvidence: S.optionalKey(
    AutopilotWorkOnboardingCapabilityEvidence,
  ),
  outputStylePersonaEvidence: S.optionalKey(
    AutopilotWorkOutputStylePersonaEvidence,
  ),
  promptSuggestionsEvidence: S.optionalKey(AutopilotWorkPromptSuggestionsEvidence),
  tipsEducationEvidence: S.optionalKey(AutopilotWorkTipsEducationEvidence),
  themeVisualEvidence: S.optionalKey(AutopilotWorkThemeVisualEvidence),
  accessibilityNonInteractiveEvidence: S.optionalKey(
    AutopilotWorkAccessibilityNonInteractiveEvidence,
  ),
  localizationBoundaryEvidence: S.optionalKey(
    AutopilotWorkLocalizationBoundaryEvidence,
  ),
  enterpriseManagedPolicyEvidence: S.optionalKey(
    AutopilotWorkEnterpriseManagedPolicyEvidence,
  ),
  sessionNavigation: S.optionalKey(AutopilotWorkSessionNavigation),
  supportDiagnostics: S.optionalKey(AutopilotWorkSupportDiagnostics),
  sessionMemory: S.optionalKey(AutopilotWorkSessionMemory),
  settingsConfiguration: S.optionalKey(AutopilotWorkSettingsConfiguration),
  structuredEventLog: S.optionalKey(AutopilotWorkStructuredEventLog),
  telemetryPrivacy: S.optionalKey(AutopilotWorkTelemetryPrivacy),
  teamSharedMemory: S.optionalKey(AutopilotWorkTeamSharedMemory),
  terminalUiShell: S.optionalKey(AutopilotWorkTerminalUiShell),
  testingSmokeEvidence: S.optionalKey(AutopilotWorkTestingSmokeEvidence),
  updateRelease: S.optionalKey(AutopilotWorkUpdateRelease),
  usageBudget: S.optionalKey(AutopilotWorkUsageBudget),
  state: AutopilotWorkState,
  statusUrlRef: S.String,
  taskRefs: S.Array(S.String),
  tasks: S.Array(S.Unknown),
  updatedAt: S.String,
  workOrderRef: S.String,
})
export type AutopilotWorkProjection = typeof AutopilotWorkProjection.Type

export const AutopilotWorkResponse = S.Struct({
  generatedAt: S.String,
  work: AutopilotWorkProjection,
})
export type AutopilotWorkResponse = typeof AutopilotWorkResponse.Type

export const AutopilotWorkEvent = S.Struct({
  eventKind: AutopilotWorkEventKind,
  eventRef: S.String,
  occurredAt: S.String,
  publicSafe: S.Boolean,
  sequence: S.Number,
  state: AutopilotWorkState,
  taskRefs: S.Array(S.String),
  workOrderRef: S.String,
})
export type AutopilotWorkEvent = typeof AutopilotWorkEvent.Type

export const AutopilotWorkEventsResponse = S.Struct({
  events: S.Array(AutopilotWorkEvent),
  generatedAt: S.String,
  nextAfter: S.Number,
  workOrderRef: S.String,
})
export type AutopilotWorkEventsResponse =
  typeof AutopilotWorkEventsResponse.Type

export const AutopilotMissionBriefing = S.Struct({
  briefingRef: S.String,
  costs: S.Unknown,
  decisionsWaiting: S.Struct({
    callerActionRefs: S.Array(S.String),
    nextActionState: S.String,
    reasonRefs: S.Array(S.String),
    reviewAction: S.NullOr(S.String),
    reviewRecordedAt: S.NullOr(S.String),
  }),
  drilldown: S.Array(
    S.Struct({
      kind: S.String,
      refs: S.Array(S.String),
    }),
  ),
  generatedAt: S.String,
  kind: S.String,
  promiseRef: S.NullOr(AutopilotWorkPromiseRef),
  publicSafe: S.Boolean,
  state: AutopilotWorkState,
  whatChanged: S.Struct({
    artifactRefs: S.Array(S.String),
    resultRefs: S.Array(S.String),
    runnerKind: S.NullOr(S.String),
    summaryRefs: S.Array(S.String),
  }),
  whatHappened: S.Array(
    S.Struct({
      eventKind: AutopilotWorkEventKind,
      eventRef: S.String,
      occurredAt: S.String,
      sequence: S.Number,
    }),
  ),
  whatIsBlocked: S.Struct({
    accessRequirementRefs: S.Array(S.String),
    blockerRefs: S.Array(S.String),
    placementRefusalReasonRefs: S.Array(S.String),
  }),
  whatIsRunning: S.Struct({
    pylonAssignmentIntentRefs: S.Array(S.String),
    running: S.Boolean,
    selectedRunnerKind: S.NullOr(S.String),
    taskRefs: S.Array(S.String),
  }),
  workOrderRef: S.String,
})
export type AutopilotMissionBriefing = typeof AutopilotMissionBriefing.Type

export const AutopilotWorkBriefingResponse = S.Struct({
  briefing: AutopilotMissionBriefing,
})
export type AutopilotWorkBriefingResponse =
  typeof AutopilotWorkBriefingResponse.Type

export const AutopilotWorkListIdle = ts('AutopilotWorkListIdle', {})
export const AutopilotWorkListLoading = ts('AutopilotWorkListLoading', {})
export const AutopilotWorkListLoaded = ts('AutopilotWorkListLoaded', {
  response: AutopilotWorkListResponse,
})
export const AutopilotWorkListFailed = ts('AutopilotWorkListFailed', {
  error: S.String,
})
export const AutopilotWorkListState = S.Union([
  AutopilotWorkListIdle,
  AutopilotWorkListLoading,
  AutopilotWorkListLoaded,
  AutopilotWorkListFailed,
])
export type AutopilotWorkListState = typeof AutopilotWorkListState.Type

export const CustomerOneCohortRowState = S.Literals([
  'candidate',
  'invited',
  'workspace_seeded',
  'first_run_started',
  'delivery_reviewed',
  'loop_completed',
  'blocked',
  'deferred',
])
export type CustomerOneCohortRowState = typeof CustomerOneCohortRowState.Type

export const CustomerOneCohortCounts = S.Struct({
  blocked: S.Number,
  candidate: S.Number,
  deferred: S.Number,
  delivery_reviewed: S.Number,
  first_run_started: S.Number,
  invited: S.Number,
  loop_completed: S.Number,
  workspace_seeded: S.Number,
})
export type CustomerOneCohortCounts = typeof CustomerOneCohortCounts.Type

export const CustomerOneCohortProjectionRow = S.Struct({
  artifactRef: S.optionalKey(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  completionBundleRef: S.optionalKey(S.String),
  countsTowardD3Completion: S.Boolean,
  displayLabel: S.String,
  privacyReviewRef: S.optionalKey(S.String),
  reviewRef: S.optionalKey(S.String),
  routingRef: S.optionalKey(S.String),
  runRef: S.optionalKey(S.String),
  state: CustomerOneCohortRowState,
  teamCohortRef: S.String,
  templateRef: S.optionalKey(S.String),
  verificationRef: S.optionalKey(S.String),
  verticalRef: S.optionalKey(S.String),
  workspaceRef: S.optionalKey(S.String),
})
export type CustomerOneCohortProjectionRow =
  typeof CustomerOneCohortProjectionRow.Type

export const CustomerOneCohortGate = S.Struct({
  reasonRefs: S.Array(S.String),
  state: S.Literals(['blocked', 'ready']),
})
export type CustomerOneCohortGate = typeof CustomerOneCohortGate.Type

export const PublicProjectionStalenessContract = S.Struct({
  composition: S.Literals([
    'live_at_read',
    'rebuilt_on_transition',
    'stored_snapshot',
  ]),
  contractVersion: S.Literal('projection_staleness.v1'),
  maxStalenessSeconds: S.Number,
  rebuildsOn: S.Array(S.String),
})
export type PublicProjectionStalenessContract =
  typeof PublicProjectionStalenessContract.Type

export const CustomerOneCohortProjection = S.Struct({
  authority: S.Literal('evidence_only'),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  cohortProjectionVersion: S.Literal('customer-one-cohort-projection:v1'),
  counts: CustomerOneCohortCounts,
  gate: CustomerOneCohortGate,
  generatedAt: S.String,
  rows: S.Array(CustomerOneCohortProjectionRow),
  staleness: PublicProjectionStalenessContract,
  target: S.Struct({
    maximumTargetTeams: S.Number,
    minimumCompletedTeams: S.Number,
  }),
})
export type CustomerOneCohortProjection =
  typeof CustomerOneCohortProjection.Type

export const CustomerOneCohortIdle = ts('CustomerOneCohortIdle', {})
export const CustomerOneCohortLoading = ts('CustomerOneCohortLoading', {})
export const CustomerOneCohortLoaded = ts('CustomerOneCohortLoaded', {
  response: CustomerOneCohortProjection,
})
export const CustomerOneCohortFailed = ts('CustomerOneCohortFailed', {
  error: S.String,
})
export const CustomerOneCohortState = S.Union([
  CustomerOneCohortIdle,
  CustomerOneCohortLoading,
  CustomerOneCohortLoaded,
  CustomerOneCohortFailed,
])
export type CustomerOneCohortState = typeof CustomerOneCohortState.Type

export const AutopilotWorkComposerField = S.Literals([
  'branch',
  'maxSpendCents',
  'objective',
  'repositoryFullName',
  'verificationCommand',
])
export type AutopilotWorkComposerField = typeof AutopilotWorkComposerField.Type

export const AutopilotWorkComposerDraft = S.Struct({
  branch: S.String,
  maxSpendCents: S.String,
  objective: S.String,
  repositoryFullName: S.String,
  verificationCommand: S.String,
})
export type AutopilotWorkComposerDraft = typeof AutopilotWorkComposerDraft.Type

export const AutopilotWorkComposerIdle = ts('AutopilotWorkComposerIdle', {})
export const AutopilotWorkComposerSubmitting = ts(
  'AutopilotWorkComposerSubmitting',
  {},
)
export const AutopilotWorkComposerSucceeded = ts(
  'AutopilotWorkComposerSucceeded',
  {
    response: AutopilotWorkResponse,
  },
)
export const AutopilotWorkComposerFailed = ts('AutopilotWorkComposerFailed', {
  error: S.String,
})
export const AutopilotWorkComposerState = S.Union([
  AutopilotWorkComposerIdle,
  AutopilotWorkComposerSubmitting,
  AutopilotWorkComposerSucceeded,
  AutopilotWorkComposerFailed,
])
export type AutopilotWorkComposerState = typeof AutopilotWorkComposerState.Type

export const AutopilotWorkDetailIdle = ts('AutopilotWorkDetailIdle', {})
export const AutopilotWorkDetailLoading = ts('AutopilotWorkDetailLoading', {})
export const AutopilotWorkDetailLoaded = ts('AutopilotWorkDetailLoaded', {
  response: AutopilotWorkResponse,
})
export const AutopilotWorkDetailFailed = ts('AutopilotWorkDetailFailed', {
  error: S.String,
})
export const AutopilotWorkDetailState = S.Union([
  AutopilotWorkDetailIdle,
  AutopilotWorkDetailLoading,
  AutopilotWorkDetailLoaded,
  AutopilotWorkDetailFailed,
])
export type AutopilotWorkDetailState = typeof AutopilotWorkDetailState.Type

export const AutopilotWorkEventsIdle = ts('AutopilotWorkEventsIdle', {})
export const AutopilotWorkEventsLoading = ts('AutopilotWorkEventsLoading', {})
export const AutopilotWorkEventsLoaded = ts('AutopilotWorkEventsLoaded', {
  response: AutopilotWorkEventsResponse,
})
export const AutopilotWorkEventsFailed = ts('AutopilotWorkEventsFailed', {
  error: S.String,
})
export const AutopilotWorkEventsState = S.Union([
  AutopilotWorkEventsIdle,
  AutopilotWorkEventsLoading,
  AutopilotWorkEventsLoaded,
  AutopilotWorkEventsFailed,
])
export type AutopilotWorkEventsState = typeof AutopilotWorkEventsState.Type

export const AutopilotWorkBriefingIdle = ts('AutopilotWorkBriefingIdle', {})
export const AutopilotWorkBriefingLoading = ts(
  'AutopilotWorkBriefingLoading',
  {},
)
export const AutopilotWorkBriefingLoaded = ts('AutopilotWorkBriefingLoaded', {
  response: AutopilotWorkBriefingResponse,
})
export const AutopilotWorkBriefingFailed = ts('AutopilotWorkBriefingFailed', {
  error: S.String,
})
export const AutopilotWorkBriefingState = S.Union([
  AutopilotWorkBriefingIdle,
  AutopilotWorkBriefingLoading,
  AutopilotWorkBriefingLoaded,
  AutopilotWorkBriefingFailed,
])
export type AutopilotWorkBriefingState = typeof AutopilotWorkBriefingState.Type

export const AutopilotWorkReviewIdle = ts('AutopilotWorkReviewIdle', {})
export const AutopilotWorkReviewSubmitting = ts(
  'AutopilotWorkReviewSubmitting',
  {
    action: AutopilotWorkReviewAction,
  },
)
export const AutopilotWorkReviewSucceeded = ts('AutopilotWorkReviewSucceeded', {
  response: AutopilotWorkResponse,
})
export const AutopilotWorkReviewFailed = ts('AutopilotWorkReviewFailed', {
  error: S.String,
})
export const AutopilotWorkReviewState = S.Union([
  AutopilotWorkReviewIdle,
  AutopilotWorkReviewSubmitting,
  AutopilotWorkReviewSucceeded,
  AutopilotWorkReviewFailed,
])
export type AutopilotWorkReviewState = typeof AutopilotWorkReviewState.Type

export const AutopilotDecisionProjection = S.Struct({
  accountLeaseRefs: S.Array(S.String),
  actionKind: S.String,
  actionLabel: S.String,
  actionRef: S.String,
  actionSubmissionRefs: S.Array(S.String),
  actionSubmissionRequired: S.Boolean,
  assignmentRefs: S.Array(S.String),
  audience: S.String,
  blockedReasonRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  customerNextActionRef: S.String,
  directEffectPermitted: S.Boolean,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  missionRef: S.String,
  prerequisiteRefs: S.Array(S.String),
  programRunRef: S.NullOr(S.String),
  receiptRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  safeSummaryRef: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  status: S.String,
  statusLabel: S.String,
  updatedAtDisplay: S.String,
  workroomRefs: S.Array(S.String),
})
export type AutopilotDecisionProjection =
  typeof AutopilotDecisionProjection.Type

export const AutopilotDecisionWorkContext = S.Struct({
  createdAt: S.String,
  state: AutopilotWorkState,
  taskRefs: S.Array(S.String),
  updatedAt: S.String,
  workOrderRef: S.String,
})
export type AutopilotDecisionWorkContext =
  typeof AutopilotDecisionWorkContext.Type

export const AutopilotDecisionQueueItem = S.Struct({
  decision: AutopilotDecisionProjection,
  work: AutopilotDecisionWorkContext,
})
export type AutopilotDecisionQueueItem = typeof AutopilotDecisionQueueItem.Type

export const AutopilotDecisionListResponse = S.Struct({
  decisions: S.Array(AutopilotDecisionQueueItem),
  directEffectPermitted: S.Boolean,
  generatedAt: S.String,
  pendingCount: S.Number,
})
export type AutopilotDecisionListResponse =
  typeof AutopilotDecisionListResponse.Type

export const AutopilotDecisionActionResponse = S.Struct({
  decision: S.NullOr(AutopilotDecisionProjection),
  directEffectPermitted: S.Boolean,
  generatedAt: S.String,
  idempotent: S.Boolean,
  work: AutopilotDecisionWorkContext,
})
export type AutopilotDecisionActionResponse =
  typeof AutopilotDecisionActionResponse.Type

export const AutopilotDecisionsIdle = ts('AutopilotDecisionsIdle', {})
export const AutopilotDecisionsLoading = ts('AutopilotDecisionsLoading', {})
export const AutopilotDecisionsLoaded = ts('AutopilotDecisionsLoaded', {
  response: AutopilotDecisionListResponse,
})
export const AutopilotDecisionsFailed = ts('AutopilotDecisionsFailed', {
  error: S.String,
})
export const AutopilotDecisionsState = S.Union([
  AutopilotDecisionsIdle,
  AutopilotDecisionsLoading,
  AutopilotDecisionsLoaded,
  AutopilotDecisionsFailed,
])
export type AutopilotDecisionsState = typeof AutopilotDecisionsState.Type

export const AutopilotDecisionActIdle = ts('AutopilotDecisionActIdle', {})
export const AutopilotDecisionActSubmitting = ts(
  'AutopilotDecisionActSubmitting',
  {
    action: AutopilotWorkReviewAction,
    decisionRef: S.String,
  },
)
export const AutopilotDecisionActSucceeded = ts(
  'AutopilotDecisionActSucceeded',
  {
    response: AutopilotDecisionActionResponse,
  },
)
export const AutopilotDecisionActFailed = ts('AutopilotDecisionActFailed', {
  error: S.String,
})
export const AutopilotDecisionActState = S.Union([
  AutopilotDecisionActIdle,
  AutopilotDecisionActSubmitting,
  AutopilotDecisionActSucceeded,
  AutopilotDecisionActFailed,
])
export type AutopilotDecisionActState = typeof AutopilotDecisionActState.Type

export const CustomerSiteRevisionsIdle = ts('CustomerSiteRevisionsIdle', {})
export const CustomerSiteRevisionsLoading = ts(
  'CustomerSiteRevisionsLoading',
  {},
)
export const CustomerSiteRevisionsLoaded = ts('CustomerSiteRevisionsLoaded', {
  revisions: S.Array(CustomerSiteRevision),
})
export const CustomerSiteRevisionsFailed = ts('CustomerSiteRevisionsFailed', {
  error: S.String,
})
export const CustomerSiteRevisionsState = S.Union([
  CustomerSiteRevisionsIdle,
  CustomerSiteRevisionsLoading,
  CustomerSiteRevisionsLoaded,
  CustomerSiteRevisionsFailed,
])
export type CustomerSiteRevisionsState = typeof CustomerSiteRevisionsState.Type

export const CustomerSiteFeedbackIdle = ts('CustomerSiteFeedbackIdle', {})
export const CustomerSiteFeedbackLoading = ts('CustomerSiteFeedbackLoading', {})
export const CustomerSiteFeedbackLoaded = ts('CustomerSiteFeedbackLoaded', {
  feedback: S.Array(CustomerSiteFeedback),
})
export const CustomerSiteFeedbackFailed = ts('CustomerSiteFeedbackFailed', {
  error: S.String,
})
export const CustomerSiteFeedbackState = S.Union([
  CustomerSiteFeedbackIdle,
  CustomerSiteFeedbackLoading,
  CustomerSiteFeedbackLoaded,
  CustomerSiteFeedbackFailed,
])
export type CustomerSiteFeedbackState = typeof CustomerSiteFeedbackState.Type

export const CustomerSiteFeedbackSubmitIdle = ts(
  'CustomerSiteFeedbackSubmitIdle',
  {},
)
export const CustomerSiteFeedbackSubmitting = ts(
  'CustomerSiteFeedbackSubmitting',
  {},
)
export const CustomerSiteFeedbackSubmitSucceeded = ts(
  'CustomerSiteFeedbackSubmitSucceeded',
  {
    feedback: CustomerSiteFeedback,
  },
)
export const CustomerSiteFeedbackSubmitFailed = ts(
  'CustomerSiteFeedbackSubmitFailed',
  {
    error: S.String,
  },
)
export const CustomerSiteFeedbackSubmitState = S.Union([
  CustomerSiteFeedbackSubmitIdle,
  CustomerSiteFeedbackSubmitting,
  CustomerSiteFeedbackSubmitSucceeded,
  CustomerSiteFeedbackSubmitFailed,
])
export type CustomerSiteFeedbackSubmitState =
  typeof CustomerSiteFeedbackSubmitState.Type

export const CustomerSiteBuilderSessionIdle = ts(
  'CustomerSiteBuilderSessionIdle',
  {},
)
export const CustomerSiteBuilderSessionLoading = ts(
  'CustomerSiteBuilderSessionLoading',
  {},
)
export const CustomerSiteBuilderSessionLoaded = ts(
  'CustomerSiteBuilderSessionLoaded',
  {
    session: CustomerSiteBuilderSession,
  },
)
export const CustomerSiteBuilderSessionFailed = ts(
  'CustomerSiteBuilderSessionFailed',
  {
    error: S.String,
  },
)
export const CustomerSiteBuilderSessionState = S.Union([
  CustomerSiteBuilderSessionIdle,
  CustomerSiteBuilderSessionLoading,
  CustomerSiteBuilderSessionLoaded,
  CustomerSiteBuilderSessionFailed,
])
export type CustomerSiteBuilderSessionState =
  typeof CustomerSiteBuilderSessionState.Type

export const CustomerSiteBuilderFilesIdle = ts(
  'CustomerSiteBuilderFilesIdle',
  {},
)
export const CustomerSiteBuilderFilesLoading = ts(
  'CustomerSiteBuilderFilesLoading',
  {},
)
export const CustomerSiteBuilderFilesLoaded = ts(
  'CustomerSiteBuilderFilesLoaded',
  {
    files: S.Array(CustomerSiteBuilderFile),
    fileTree: S.Array(CustomerSiteBuilderFileTreeItem),
  },
)
export const CustomerSiteBuilderFilesFailed = ts(
  'CustomerSiteBuilderFilesFailed',
  {
    error: S.String,
  },
)
export const CustomerSiteBuilderFilesState = S.Union([
  CustomerSiteBuilderFilesIdle,
  CustomerSiteBuilderFilesLoading,
  CustomerSiteBuilderFilesLoaded,
  CustomerSiteBuilderFilesFailed,
])
export type CustomerSiteBuilderFilesState =
  typeof CustomerSiteBuilderFilesState.Type

export const CustomerSiteBuilderFileReadIdle = ts(
  'CustomerSiteBuilderFileReadIdle',
  {},
)
export const CustomerSiteBuilderFileReadLoading = ts(
  'CustomerSiteBuilderFileReadLoading',
  {
    path: S.String,
  },
)
export const CustomerSiteBuilderFileReadLoaded = ts(
  'CustomerSiteBuilderFileReadLoaded',
  {
    file: CustomerSiteBuilderFileRead,
  },
)
export const CustomerSiteBuilderFileReadFailed = ts(
  'CustomerSiteBuilderFileReadFailed',
  {
    error: S.String,
    path: S.String,
  },
)
export const CustomerSiteBuilderFileReadState = S.Union([
  CustomerSiteBuilderFileReadIdle,
  CustomerSiteBuilderFileReadLoading,
  CustomerSiteBuilderFileReadLoaded,
  CustomerSiteBuilderFileReadFailed,
])
export type CustomerSiteBuilderFileReadState =
  typeof CustomerSiteBuilderFileReadState.Type

export const CustomerSiteBuilderEventsIdle = ts(
  'CustomerSiteBuilderEventsIdle',
  {},
)
export const CustomerSiteBuilderEventsLoading = ts(
  'CustomerSiteBuilderEventsLoading',
  {},
)
export const CustomerSiteBuilderEventsLoaded = ts(
  'CustomerSiteBuilderEventsLoaded',
  {
    events: S.Array(CustomerSiteBuilderEvent),
  },
)
export const CustomerSiteBuilderEventsFailed = ts(
  'CustomerSiteBuilderEventsFailed',
  {
    error: S.String,
  },
)
export const CustomerSiteBuilderEventsState = S.Union([
  CustomerSiteBuilderEventsIdle,
  CustomerSiteBuilderEventsLoading,
  CustomerSiteBuilderEventsLoaded,
  CustomerSiteBuilderEventsFailed,
])
export type CustomerSiteBuilderEventsState =
  typeof CustomerSiteBuilderEventsState.Type

export const AdminOverviewUser = S.Struct({
  userId: S.String,
  kind: S.Literals(['human', 'agent']),
  displayName: S.String,
  email: S.NullOr(S.String),
  githubUsername: S.NullOr(S.String),
  status: S.String,
  onboardingStep: S.String,
  onboardingCompletedAt: S.NullOr(S.String),
  softwareOrderCount: S.Number,
  createdAt: S.String,
  updatedAt: S.String,
})
export type AdminOverviewUser = typeof AdminOverviewUser.Type

export const AdminOverviewSoftwareOrder = S.Struct({
  id: S.String,
  userId: S.String,
  userDisplayName: S.NullOr(S.String),
  userEmail: S.NullOr(S.String),
  status: S.String,
  visibility: S.String,
  request: S.String,
  repositoryFullName: S.NullOr(S.String),
  currentRunId: S.NullOr(S.String),
  siteProjectId: S.NullOr(S.String),
  siteTitle: S.NullOr(S.String),
  siteSlug: S.NullOr(S.String),
  siteStatus: S.NullOr(S.String),
  siteAccessMode: S.NullOr(S.String),
  siteVisibility: S.NullOr(S.String),
  siteActiveVersionId: S.NullOr(S.String),
  siteActiveDeploymentId: S.NullOr(S.String),
  siteActiveUrl: S.NullOr(S.String),
  siteVersionCount: S.Number,
  siteLatestVersionId: S.NullOr(S.String),
  siteLatestVersionStatus: S.NullOr(S.String),
  siteLatestVersionSourceKind: S.NullOr(S.String),
  siteLatestVersionCreatedAt: S.NullOr(S.String),
  siteDeploymentCount: S.Number,
  siteLatestDeploymentId: S.NullOr(S.String),
  siteLatestDeploymentStatus: S.NullOr(S.String),
  siteLatestDeploymentRuntimeKind: S.NullOr(S.String),
  siteLatestDeploymentUpdatedAt: S.NullOr(S.String),
  siteStorageBindingCount: S.Number,
  siteStorageBindingSummary: S.NullOr(S.String),
  siteEnvironmentValueCount: S.Number,
  siteEnvironmentKeySummary: S.NullOr(S.String),
  siteAccessGrantCount: S.Number,
  siteLatestEventType: S.NullOr(S.String),
  siteLatestEventSummary: S.NullOr(S.String),
  siteLatestEventCreatedAt: S.NullOr(S.String),
  siteLatestCompatibilityId: S.NullOr(S.String),
  siteLatestCompatibilityStatus: S.NullOr(S.String),
  siteLatestCompatibilityCustomerSafeStatus: S.NullOr(S.String),
  siteLatestCompatibilityCustomerSafeNextAction: S.NullOr(S.String),
  siteLatestCompatibilityBlockerCount: S.Number,
  siteLatestCompatibilityWarningCount: S.Number,
  siteLatestCompatibilityCreatedAt: S.NullOr(S.String),
  siteLatestBuildValidationId: S.NullOr(S.String),
  siteLatestBuildValidationStatus: S.NullOr(S.String),
  siteLatestBuildValidationSourceHash: S.NullOr(S.String),
  siteLatestBuildValidationCustomerSafeStatus: S.NullOr(S.String),
  siteLatestBuildValidationCustomerSafeNextAction: S.NullOr(S.String),
  siteLatestBuildValidationBlockerCount: S.Number,
  siteLatestBuildValidationWarningCount: S.Number,
  siteLatestBuildValidationCreatedAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type AdminOverviewSoftwareOrder = typeof AdminOverviewSoftwareOrder.Type

export const AdminSiteGenerationResponse = S.Struct({
  generation: S.Struct({
    siteId: S.String,
    publicUrl: S.String,
    createdAt: S.String,
  }),
})
export type AdminSiteGenerationResponse =
  typeof AdminSiteGenerationResponse.Type

export const AdminOverviewResponse = S.Struct({
  users: S.Array(AdminOverviewUser),
  softwareOrders: S.Array(AdminOverviewSoftwareOrder),
})
export type AdminOverviewResponse = typeof AdminOverviewResponse.Type

export const AdminOverviewIdle = ts('AdminOverviewIdle', {})
export const AdminOverviewLoading = ts('AdminOverviewLoading', {})
export const AdminOverviewLoaded = ts('AdminOverviewLoaded', {
  users: S.Array(AdminOverviewUser),
  softwareOrders: S.Array(AdminOverviewSoftwareOrder),
})
export const AdminOverviewFailed = ts('AdminOverviewFailed', {
  error: S.String,
})
export const AdminOverviewState = S.Union([
  AdminOverviewIdle,
  AdminOverviewLoading,
  AdminOverviewLoaded,
  AdminOverviewFailed,
])
export type AdminOverviewState = typeof AdminOverviewState.Type

export const AdminAdjutantAssignment = S.Struct({
  id: S.String,
  softwareOrderId: S.NullOr(S.String),
  siteId: S.NullOr(S.String),
  goalId: S.NullOr(S.String),
  currentRunId: S.NullOr(S.String),
  teamId: S.NullOr(S.String),
  projectId: S.NullOr(S.String),
  agentId: S.String,
  assignedByUserId: S.NullOr(S.String),
  assignmentKind: S.String,
  status: S.String,
  visibility: S.String,
  taskSpecPath: S.NullOr(S.String),
  commitSha: S.NullOr(S.String),
  objective: S.String,
  createdAt: S.String,
  updatedAt: S.String,
  completedAt: S.NullOr(S.String),
  blockedAt: S.NullOr(S.String),
  archivedAt: S.NullOr(S.String),
})
export type AdminAdjutantAssignment = typeof AdminAdjutantAssignment.Type

export const AdminAdjutantAssignmentsResponse = S.Struct({
  assignments: S.Array(AdminAdjutantAssignment),
})
export type AdminAdjutantAssignmentsResponse =
  typeof AdminAdjutantAssignmentsResponse.Type

export const AdminAdjutantReviewOrder = S.Struct({
  id: S.String,
  status: S.String,
  visibility: S.String,
  request: S.String,
  repositoryFullName: S.NullOr(S.String),
  currentRunId: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type AdminAdjutantReviewOrder = typeof AdminAdjutantReviewOrder.Type

export const AdminAdjutantReviewSite = S.Struct({
  id: S.String,
  slug: S.String,
  title: S.String,
  status: S.String,
  accessMode: S.String,
  visibility: S.String,
  activeVersionId: S.NullOr(S.String),
  activeDeploymentId: S.NullOr(S.String),
})
export type AdminAdjutantReviewSite = typeof AdminAdjutantReviewSite.Type

export const AdminAdjutantReviewGoal = S.Struct({
  id: S.String,
  agentId: S.String,
  status: S.String,
  visibility: S.String,
  currentRunId: S.NullOr(S.String),
  tokensUsed: S.Number,
  tokenBudget: S.NullOr(S.Number),
  timeUsedSeconds: S.Number,
  updatedAt: S.String,
})
export type AdminAdjutantReviewGoal = typeof AdminAdjutantReviewGoal.Type

export const AdminAdjutantReviewRun = S.Struct({
  id: S.String,
  runtime: S.String,
  backend: S.String,
  status: S.String,
  eventCursor: S.Number,
  externalRunId: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type AdminAdjutantReviewRun = typeof AdminAdjutantReviewRun.Type

export const AdminAdjutantReviewVersion = S.Struct({
  id: S.String,
  sourceKind: S.String,
  sourceCommitSha: S.NullOr(S.String),
  buildStatus: S.String,
  buildCommand: S.NullOr(S.String),
  workerModuleR2Key: S.NullOr(S.String),
  createdByRunId: S.NullOr(S.String),
  createdAt: S.String,
  savedAt: S.NullOr(S.String),
  rejectedAt: S.NullOr(S.String),
})
export type AdminAdjutantReviewVersion = typeof AdminAdjutantReviewVersion.Type

export const AdminAdjutantReviewDeployment = S.Struct({
  id: S.String,
  versionId: S.String,
  url: S.String,
  runtimeKind: S.String,
  status: S.String,
  externalDeploymentId: S.NullOr(S.String),
  activatedAt: S.NullOr(S.String),
  disabledAt: S.NullOr(S.String),
  rolledBackAt: S.NullOr(S.String),
  updatedAt: S.String,
})
export type AdminAdjutantReviewDeployment =
  typeof AdminAdjutantReviewDeployment.Type

export const AdminAdjutantReviewEvent = S.Struct({
  id: S.String,
  type: S.String,
  summary: S.String,
  runId: S.NullOr(S.String),
  createdAt: S.String,
})
export type AdminAdjutantReviewEvent = typeof AdminAdjutantReviewEvent.Type

export const AdminAdjutantReviewUsageReceipt = S.Struct({
  adjustmentId: S.NullOr(S.String),
  assignmentId: S.String,
  billingLedgerEntryId: S.NullOr(S.String),
  billingMode: AdjutantUsageReceiptBillingMode,
  category: AdjutantUsageReceiptCategory,
  createdAt: S.String,
  creditsChargedCents: S.Number,
  creditsChargedFormatted: S.String,
  currency: S.String,
  id: S.String,
  publicDetails: S.Record(S.String, S.Unknown),
  quantity: S.Number,
  runId: S.NullOr(S.String),
  siteId: S.NullOr(S.String),
  softwareOrderId: S.NullOr(S.String),
  summary: S.String,
  teamDetails: S.Record(S.String, S.Unknown),
  unit: S.String,
  visibility: S.Literals(['private', 'team', 'public']),
})
export type AdminAdjutantReviewUsageReceipt =
  typeof AdminAdjutantReviewUsageReceipt.Type

export const AdminAdjutantReviewResearchBriefSource = S.Struct({
  id: S.String,
  title: S.String,
  url: S.String,
  domain: S.String,
  highlightText: S.NullOr(S.String),
})
export type AdminAdjutantReviewResearchBriefSource =
  typeof AdminAdjutantReviewResearchBriefSource.Type

export const AdminAdjutantReviewResearchBrief = S.Struct({
  approvedAt: S.NullOr(S.String),
  claimsNeedingReview: S.optionalKey(S.Array(S.String)),
  createdAt: S.optionalKey(S.String),
  enrichmentRunId: S.NullOr(S.String),
  groundedFacts: S.optionalKey(S.Array(S.String)),
  id: S.String,
  rejectedAt: S.optionalKey(S.NullOr(S.String)),
  reviewReason: S.optionalKey(S.NullOr(S.String)),
  sourceCards: S.optionalKey(S.Array(AdminAdjutantReviewResearchBriefSource)),
  sourceCount: S.optionalKey(S.Number),
  status: S.String,
  suggestedSections: S.optionalKey(S.Array(S.String)),
  summary: S.String,
  unknowns: S.optionalKey(S.Array(S.String)),
  updatedAt: S.String,
})
export type AdminAdjutantReviewResearchBrief =
  typeof AdminAdjutantReviewResearchBrief.Type

export const AdminAdjutantReviewEnrichmentRun = S.Struct({
  id: S.String,
  assignmentId: S.String,
  softwareOrderId: S.NullOr(S.String),
  siteId: S.NullOr(S.String),
  planId: S.String,
  subject: S.String,
  status: S.String,
  requestBudget: S.Number,
  requestCount: S.Number,
  cacheHitCount: S.Number,
  sourceCount: S.Number,
  approvedSourceCount: S.Number,
  costDollars: S.NullOr(S.Number),
  errorCode: S.NullOr(S.String),
  errorSummary: S.NullOr(S.String),
  startedAt: S.NullOr(S.String),
  completedAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type AdminAdjutantReviewEnrichmentRun =
  typeof AdminAdjutantReviewEnrichmentRun.Type

export const AdminAdjutantReviewEnrichmentQuery = S.Struct({
  id: S.String,
  runId: S.String,
  assignmentId: S.String,
  queryHash: S.String,
  queryText: S.String,
  sourceCategory: S.String,
  searchType: S.String,
  freshnessMaxAgeHours: S.Number,
  status: S.String,
  resultCount: S.Number,
  latencyMs: S.NullOr(S.Number),
  costDollars: S.NullOr(S.Number),
  errorCode: S.NullOr(S.String),
  errorSummary: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type AdminAdjutantReviewEnrichmentQuery =
  typeof AdminAdjutantReviewEnrichmentQuery.Type

export const AdminAdjutantReviewEnrichmentSourceCard = S.Struct({
  approvedAt: S.NullOr(S.String),
  domain: S.String,
  highlightText: S.NullOr(S.String),
  id: S.String,
  publicSafe: S.Boolean,
  publishedDate: S.NullOr(S.String),
  rejectedAt: S.NullOr(S.String),
  rejectedReason: S.NullOr(S.String),
  reviewStatus: S.String,
  runId: S.String,
  sourceCategory: S.String,
  title: S.String,
  updatedAt: S.String,
  url: S.String,
})
export type AdminAdjutantReviewEnrichmentSourceCard =
  typeof AdminAdjutantReviewEnrichmentSourceCard.Type

export const AdminAdjutantReviewPublicSourceRef = S.Struct({
  approvedAt: S.NullOr(S.String),
  id: S.String,
  kind: S.String,
  label: S.NullOr(S.String),
  normalizedDomain: S.String,
  publicSafe: S.Boolean,
  rejectedAt: S.NullOr(S.String),
  reviewReason: S.NullOr(S.String),
  status: S.String,
  updatedAt: S.String,
  url: S.NullOr(S.String),
})
export type AdminAdjutantReviewPublicSourceRef =
  typeof AdminAdjutantReviewPublicSourceRef.Type

export const AdminAdjutantReviewEnrichment = S.Struct({
  exaConfigured: S.Boolean,
  latestRun: S.NullOr(AdminAdjutantReviewEnrichmentRun),
  nextAction: S.String,
  queries: S.Array(AdminAdjutantReviewEnrichmentQuery),
  researchBrief: S.NullOr(AdminAdjutantReviewResearchBrief),
  sourceCards: S.Array(AdminAdjutantReviewEnrichmentSourceCard),
  sourceRefs: S.Array(AdminAdjutantReviewPublicSourceRef),
  status: S.String,
})
export type AdminAdjutantReviewEnrichment =
  typeof AdminAdjutantReviewEnrichment.Type

export const AdminAdjutantAssignmentReview = S.Struct({
  assignmentEvents: S.Array(AdminAdjutantReviewEvent),
  currentRun: S.NullOr(AdminAdjutantReviewRun),
  deployments: S.Array(AdminAdjutantReviewDeployment),
  enrichment: AdminAdjutantReviewEnrichment,
  goal: S.NullOr(AdminAdjutantReviewGoal),
  nextAction: S.String,
  order: S.NullOr(AdminAdjutantReviewOrder),
  researchBrief: S.NullOr(AdminAdjutantReviewResearchBrief),
  site: S.NullOr(AdminAdjutantReviewSite),
  siteEvents: S.Array(AdminAdjutantReviewEvent),
  usageReceipts: S.Array(AdminAdjutantReviewUsageReceipt),
  usageSummary: AdjutantUsageReceiptSummary,
  versions: S.Array(AdminAdjutantReviewVersion),
})
export type AdminAdjutantAssignmentReview =
  typeof AdminAdjutantAssignmentReview.Type

export const AdminAdjutantAssignmentReviewResponse = S.Struct({
  assignment: AdminAdjutantAssignment,
  review: AdminAdjutantAssignmentReview,
})
export type AdminAdjutantAssignmentReviewResponse =
  typeof AdminAdjutantAssignmentReviewResponse.Type

export const AdminSiteDeploymentActionResponse = S.Struct({
  deployment: AdminAdjutantReviewDeployment,
})
export type AdminSiteDeploymentActionResponse =
  typeof AdminSiteDeploymentActionResponse.Type

export const AdminAdjutantEnrichmentActionResponse = S.Struct({
  enrichment: AdminAdjutantReviewEnrichment,
})
export type AdminAdjutantEnrichmentActionResponse =
  typeof AdminAdjutantEnrichmentActionResponse.Type

export const AdminAdjutantAssignmentsIdle = ts(
  'AdminAdjutantAssignmentsIdle',
  {},
)
export const AdminAdjutantAssignmentsLoading = ts(
  'AdminAdjutantAssignmentsLoading',
  {},
)
export const AdminAdjutantAssignmentsLoaded = ts(
  'AdminAdjutantAssignmentsLoaded',
  {
    assignments: S.Array(AdminAdjutantAssignment),
  },
)
export const AdminAdjutantAssignmentsFailed = ts(
  'AdminAdjutantAssignmentsFailed',
  {
    error: S.String,
  },
)
export const AdminAdjutantAssignmentsState = S.Union([
  AdminAdjutantAssignmentsIdle,
  AdminAdjutantAssignmentsLoading,
  AdminAdjutantAssignmentsLoaded,
  AdminAdjutantAssignmentsFailed,
])
export type AdminAdjutantAssignmentsState =
  typeof AdminAdjutantAssignmentsState.Type

export const AdminAdjutantReviewIdle = ts('AdminAdjutantReviewIdle', {})
export const AdminAdjutantReviewLoading = ts('AdminAdjutantReviewLoading', {
  assignmentId: S.String,
})
export const AdminAdjutantReviewLoaded = ts('AdminAdjutantReviewLoaded', {
  assignment: AdminAdjutantAssignment,
  review: AdminAdjutantAssignmentReview,
})
export const AdminAdjutantReviewFailed = ts('AdminAdjutantReviewFailed', {
  assignmentId: S.String,
  error: S.String,
})
export const AdminAdjutantReviewState = S.Union([
  AdminAdjutantReviewIdle,
  AdminAdjutantReviewLoading,
  AdminAdjutantReviewLoaded,
  AdminAdjutantReviewFailed,
])
export type AdminAdjutantReviewState = typeof AdminAdjutantReviewState.Type

export const AdminAdjutantEnrichmentActionIdle = ts(
  'AdminAdjutantEnrichmentActionIdle',
  {},
)
export const AdminAdjutantEnrichmentActionPending = ts(
  'AdminAdjutantEnrichmentActionPending',
  {
    action: S.String,
    assignmentId: S.String,
  },
)
export const AdminAdjutantEnrichmentActionSucceeded = ts(
  'AdminAdjutantEnrichmentActionSucceeded',
  {
    message: S.String,
  },
)
export const AdminAdjutantEnrichmentActionFailed = ts(
  'AdminAdjutantEnrichmentActionFailed',
  {
    error: S.String,
  },
)
export const AdminAdjutantEnrichmentActionState = S.Union([
  AdminAdjutantEnrichmentActionIdle,
  AdminAdjutantEnrichmentActionPending,
  AdminAdjutantEnrichmentActionSucceeded,
  AdminAdjutantEnrichmentActionFailed,
])
export type AdminAdjutantEnrichmentActionState =
  typeof AdminAdjutantEnrichmentActionState.Type

export const AdminSiteDeploymentActionIdle = ts(
  'AdminSiteDeploymentActionIdle',
  {},
)
export const AdminSiteDeploymentActionPending = ts(
  'AdminSiteDeploymentActionPending',
  {
    action: S.String,
    assignmentId: S.String,
  },
)
export const AdminSiteDeploymentActionSucceeded = ts(
  'AdminSiteDeploymentActionSucceeded',
  {
    message: S.String,
  },
)
export const AdminSiteDeploymentActionFailed = ts(
  'AdminSiteDeploymentActionFailed',
  {
    error: S.String,
  },
)
export const AdminSiteDeploymentActionState = S.Union([
  AdminSiteDeploymentActionIdle,
  AdminSiteDeploymentActionPending,
  AdminSiteDeploymentActionSucceeded,
  AdminSiteDeploymentActionFailed,
])
export type AdminSiteDeploymentActionState =
  typeof AdminSiteDeploymentActionState.Type

export const ImageGenerationProvider = S.Literals([
  'google-gemini',
  'google-imagen',
])
export type ImageGenerationProvider = typeof ImageGenerationProvider.Type

export const ImageGenerationModelId = S.Literals([
  'gemini-2.5-flash-image',
  'imagen-4.0-generate-001',
  'imagen-4.0-fast-generate-001',
  'imagen-4.0-ultra-generate-001',
])
export type ImageGenerationModelId = typeof ImageGenerationModelId.Type

export const ImageGenerationAspectRatio = S.Literals([
  '1:1',
  '3:4',
  '4:3',
  '9:16',
  '16:9',
])
export type ImageGenerationAspectRatio = typeof ImageGenerationAspectRatio.Type

export const ImageGenerationImageSize = S.Literals(['512', '1K', '2K', '4K'])
export type ImageGenerationImageSize = typeof ImageGenerationImageSize.Type

export const GeneratedImage = S.Struct({
  byteLength: S.Number,
  createdAt: S.String,
  key: S.String,
  mimeType: S.String,
  model: S.String,
  prompt: S.String,
  provider: ImageGenerationProvider,
  url: S.String,
})
export type GeneratedImage = typeof GeneratedImage.Type

export const GenerateImageResponse = S.Struct({
  images: S.Array(GeneratedImage),
})
export type GenerateImageResponse = typeof GenerateImageResponse.Type

export const ImageGenerationIdle = ts('ImageGenerationIdle', {})
export const ImageGenerationSubmitting = ts('ImageGenerationSubmitting', {})
export const ImageGenerationSucceeded = ts('ImageGenerationSucceeded', {
  images: S.Array(GeneratedImage),
})
export const ImageGenerationFailed = ts('ImageGenerationFailed', {
  error: S.String,
})
export const ImageGenerationState = S.Union([
  ImageGenerationIdle,
  ImageGenerationSubmitting,
  ImageGenerationSucceeded,
  ImageGenerationFailed,
])
export type ImageGenerationState = typeof ImageGenerationState.Type

export const IdleInviteCodeAction = ts('InviteCodeActionIdle', {})
export const FailedInviteCodeAction = ts('InviteCodeActionFailed', {
  error: S.String,
})
export const InviteCodeAction = S.Union([
  IdleInviteCodeAction,
  FailedInviteCodeAction,
])
export type InviteCodeAction = typeof InviteCodeAction.Type

export const Model = ts('LoggedIn', {
  adminAdjutantAssignments: AdminAdjutantAssignmentsState,
  adminAdjutantEnrichmentAction: AdminAdjutantEnrichmentActionState,
  adminAdjutantReview: AdminAdjutantReviewState,
  adminOverview: AdminOverviewState,
  adminSiteDeploymentAction: AdminSiteDeploymentActionState,
  auth: AuthBootstrap,
  autopilotMorningReport: AutopilotMorningReportState,
  autopilotWorkBriefing: AutopilotWorkBriefingState,
  autopilotWorkDetail: AutopilotWorkDetailState,
  autopilotWorkEvents: AutopilotWorkEventsState,
  autopilotWorkList: AutopilotWorkListState,
  autopilotWorkComposer: AutopilotWorkComposerState,
  autopilotWorkComposerDraft: AutopilotWorkComposerDraft,
  autopilotWorkReview: AutopilotWorkReviewState,
  autopilotDecisions: AutopilotDecisionsState,
  autopilotDecisionAct: AutopilotDecisionActState,
  workroom: WorkroomModel,
  billingAction: BillingAction,
  billingCouponCode: S.String,
  chatComposerValue: S.String,
  chatMessages: S.Array(ChatMessage),
  chatRun: ChatRun,
  customerOrderCreate: CustomerOrderCreateState,
  customerOrderDraft: S.String,
  customerOrder: CustomerOrderState,
  customerOrders: CustomerOrdersState,
  customerOneCohort: CustomerOneCohortState,
  customerFulfillmentArtifacts: CustomerFulfillmentArtifactsState,
  customerSiteFeedback: CustomerSiteFeedbackState,
  customerSiteFeedbackDraft: S.String,
  customerSiteElementContext: S.NullOr(SiteElementContext),
  customerSiteFeedbackSubmit: CustomerSiteFeedbackSubmitState,
  customerSiteRevisions: CustomerSiteRevisionsState,
  customerSiteBuilderEvents: CustomerSiteBuilderEventsState,
  customerSiteBuilderFileRead: CustomerSiteBuilderFileReadState,
  customerSiteBuilderFiles: CustomerSiteBuilderFilesState,
  customerSiteBuilderSelectedFilePath: S.NullOr(S.String),
  customerSiteBuilderSession: CustomerSiteBuilderSessionState,
  agentGoalPanel: AgentGoalPanelModel,
  artanisOperatorConsole: ArtanisOperatorConsoleModel,
  artanisOperatorDashboard: ArtanisOperatorDashboardModel,
  artanisOperatorDashboardCallerIdFilter: S.String,
  artanisOperatorGoalPanel: ArtanisOperatorGoalPanelModel,
  imageGeneration: ImageGenerationState,
  imageGenerationAspectRatio: ImageGenerationAspectRatio,
  imageGenerationCount: S.Number,
  imageGenerationImageSize: ImageGenerationImageSize,
  imageGenerationModel: ImageGenerationModelId,
  imageGenerationPrompt: S.String,
  imageGenerationProvider: ImageGenerationProvider,
  inviteCodeAction: InviteCodeAction,
  inviteCodeValue: S.String,
  mullet: MulletModel,
  notifications: NotificationsModel,
  onboarding: OnboardingFlowModel,
  providerAccountPool: ProviderAccountPoolState,
  providerConnectionAction: ProviderConnectionAction,
  prefilledWorkspace: PrefilledWorkspaceState,
  runMetadataDialog: RunMetadataDialog,
  route: LoggedInRoute,
  session: Session,
  sidebar: SidebarModel,
  sync: SyncClientModel,
  tokenUsageStats: TokenUsageStatsState,
  teamChatMessagesByTeam: S.Record(S.String, S.Array(TeamChatMessageRecord)),
  threadRoute: ThreadRouteState,
  threadFileDetailErrorsById: S.Record(S.String, S.String),
  threadFileDetailsById: S.Record(S.String, ThreadFileDetailRecord),
  threadFileDownloadErrorsById: S.Record(S.String, S.String),
  threadFileUpload: ThreadFileUpload,
  threadFilesByScope: S.Record(S.String, S.Array(ThreadFileRecord)),
})

export type Model = typeof Model.Type

// INIT

const LEGACY_THREAD_SECTION_TITLE = 'Threads'
const MY_THREAD_SECTION_TITLE = 'My threads'
const TEAM_THREAD_SECTION_TITLE = 'Team threads'

export const authWithBilling = (
  auth: AuthBootstrap,
  billing: BillingSummary,
): AuthBootstrap => ({
  ...auth,
  billing,
})

export const authWithProviderAccounts = (
  auth: AuthBootstrap,
  providerAccounts: NonNullable<AuthBootstrap['providerAccounts']>,
): AuthBootstrap => ({
  ...auth,
  providerAccounts,
})

export const authWithOnboarding = (
  auth: AuthBootstrap,
  onboarding: OnboardingStatus,
): AuthBootstrap => ({
  ...auth,
  onboarding,
})

export const providerAccountBundleFromAuth = (
  auth: AuthBootstrap,
): NonNullable<AuthBootstrap['providerAccounts']> =>
  auth.providerAccounts ?? emptyProviderAccountBundle()

export const onboardingWithRepositories = (
  onboarding: OnboardingFlowModel,
  response: OnboardingRepositoriesResponse,
): OnboardingFlowModel =>
  OnboardingFlowModel({
    ...onboarding,
    action: IdleOnboardingAction(),
    repositories: LoadedOnboardingRepositories({
      repositories: response.repositories,
      tokenStatus: response.tokenStatus,
    }),
    repositoryPageIndex: 0,
    selectedRepositoryId:
      onboarding.selectedRepositoryId.trim() === '' &&
      response.repositories[0] !== undefined
        ? response.repositories[0].id
        : onboarding.selectedRepositoryId,
  })

export const initOnboardingFlow = (
  onboarding: OnboardingStatus,
): OnboardingFlowModel =>
  OnboardingFlowModel({
    action: IdleOnboardingAction(),
    goalValue: onboarding.goal ?? '',
    manualRepositoryName: '',
    manualRepositoryOwner: '',
    repositoryPageIndex: 0,
    repositorySearch: '',
    repositories: IdleOnboardingRepositories(),
    selectedRepositoryId:
      onboarding.repository._tag === 'RepositorySelected'
        ? onboarding.repository.repository.id
        : '',
  })

export const teamRouteRef = (team: Team): string => team.slug ?? team.id

export const teamProjectRouteRef = (
  project: NonNullable<Team['projects']>[number],
): string => project.slug ?? project.id

const sidebarMissionDetail = (detail: string): string =>
  detail.replace(/^([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)(\s+-\s+.+)$/, '$2$3')

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const missionStatusFromUnknown = (value: unknown): SidebarSessionStatus =>
  value === 'active' ||
  value === 'complete' ||
  value === 'failed' ||
  value === 'queued'
    ? value
    : 'queued'

const missionOwnerFromUnknown = (
  owner: unknown,
  teamId: string | undefined,
  projectId: string | undefined,
): SidebarSessionOwner => {
  if (owner === 'personal' || owner === 'team' || owner === 'project') {
    return owner
  }

  if (projectId !== undefined) {
    return 'project'
  }

  return teamId === undefined ? 'personal' : 'team'
}

const optionalTextFromUnknown = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value : undefined

const missionItemFromUnknown = (
  value: unknown,
  activeHref: string | undefined = undefined,
): SidebarSessionItem | undefined => {
  const record = isRecord(value) ? value : undefined

  if (record === undefined) {
    return undefined
  }

  const title = typeof record.title === 'string' ? record.title : undefined
  const detail =
    typeof record.detail === 'string' ? sidebarMissionDetail(record.detail) : ''
  const href = typeof record.href === 'string' ? record.href : undefined
  const ownerUserId = optionalTextFromUnknown(record.ownerUserId)
  const projectId = optionalTextFromUnknown(record.projectId)
  const teamId = optionalTextFromUnknown(record.teamId)
  const updatedAt = optionalTextFromUnknown(record.updatedAt)

  if (title === undefined || href === undefined) {
    return undefined
  }

  if (!projectMissionVisible({ projectId, title }) && href !== activeHref) {
    return undefined
  }

  const status = missionStatusFromUnknown(record.status)
  const owner = missionOwnerFromUnknown(record.owner, teamId, projectId)

  return {
    active: false,
    attention: status === 'active',
    detail,
    href,
    owner,
    ...(ownerUserId === undefined ? {} : { ownerUserId }),
    ...(projectId === undefined ? {} : { projectId }),
    status,
    ...(teamId === undefined ? {} : { teamId }),
    title,
    ...(updatedAt === undefined ? {} : { updatedAt }),
  }
}

const missionSortKey = (item: SidebarSessionItem): string =>
  item.updatedAt ?? ''

const sortMissionItems = (
  items: ReadonlyArray<SidebarSessionItem>,
): ReadonlyArray<SidebarSessionItem> =>
  items
    .map((item, index) => ({ index, item }))
    .sort(
      (left, right) =>
        missionSortKey(right.item).localeCompare(missionSortKey(left.item)) ||
        left.index - right.index,
    )
    .map(entry => entry.item)

const visibleMissionItems = (
  items: ReadonlyArray<SidebarSessionItem>,
  activeHref: string | undefined = undefined,
): ReadonlyArray<SidebarSessionItem> =>
  projectWorkroomsEnabled()
    ? items
    : items.filter(
        item => item.projectId === undefined || item.href === activeHref,
      )

const isThreadSectionTitle = (title: string): boolean =>
  title === LEGACY_THREAD_SECTION_TITLE ||
  title === MY_THREAD_SECTION_TITLE ||
  title === TEAM_THREAD_SECTION_TITLE

const missionSections = (
  items: ReadonlyArray<SidebarSessionItem>,
): ReadonlyArray<SidebarSessionSection> => {
  const personalItems = sortMissionItems(
    items.filter(item => item.owner === 'personal'),
  )
  const teamItems = sortMissionItems(
    items.filter(item => item.owner === 'team' || item.owner === 'project'),
  )

  return [
    ...(personalItems.length === 0
      ? []
      : [
          {
            title: MY_THREAD_SECTION_TITLE,
            items: [...personalItems],
          },
        ]),
    ...(teamItems.length === 0
      ? []
      : [
          {
            title: TEAM_THREAD_SECTION_TITLE,
            items: [...teamItems],
          },
        ]),
  ]
}

export type SidebarMissionReplacementGroup = 'personal' | 'team-owned'

const missionReplacementGroup = (
  item: SidebarSessionItem,
): SidebarMissionReplacementGroup =>
  item.owner === 'personal' ? 'personal' : 'team-owned'

const sidebarMissionItems = (
  sidebar: SidebarModel,
): ReadonlyArray<SidebarSessionItem> =>
  sidebar.sessionSections
    .filter(section => isThreadSectionTitle(section.title))
    .flatMap(section => section.items)

export const sidebarWithMissionItems = (
  sidebar: SidebarModel,
  items: ReadonlyArray<SidebarSessionItem>,
  activeHref: string | undefined = undefined,
  replacementGroups: ReadonlyArray<SidebarMissionReplacementGroup> = [
    'personal',
    'team-owned',
  ],
): SidebarModel => {
  const withoutMissions = sidebar.sessionSections.filter(
    section => !isThreadSectionTitle(section.title),
  )
  const groupsToReplace = new Set<SidebarMissionReplacementGroup>([
    ...replacementGroups,
    ...items.map(missionReplacementGroup),
  ])
  const preservedItems = sidebarMissionItems(sidebar).filter(
    item => !groupsToReplace.has(missionReplacementGroup(item)),
  )
  const visibleItems = visibleMissionItems(
    [...preservedItems, ...items],
    activeHref,
  )

  if (visibleItems.length === 0) {
    return SidebarModel({
      ...sidebar,
      sessionSections: withoutMissions,
    })
  }

  return SidebarModel({
    ...sidebar,
    sessionSections: [...withoutMissions, ...missionSections(visibleItems)],
  })
}

export const sidebarWithMissionItem = (
  sidebar: SidebarModel,
  item: SidebarSessionItem,
  activeHref: string | undefined = undefined,
): SidebarModel => {
  const currentMissions = sidebarMissionItems(sidebar)
  const existingIndex = currentMissions.findIndex(
    existing => existing.href === item.href,
  )

  return sidebarWithMissionItems(
    sidebar,
    existingIndex < 0
      ? [...currentMissions, item]
      : currentMissions.map(existing =>
          existing.href === item.href ? item : existing,
        ),
    activeHref,
  )
}

export const sidebarWithMissionPatch = (
  sidebar: SidebarModel,
  patch: SyncPatch,
  activeHref: string | undefined = undefined,
): SidebarModel => {
  if (patch.collection !== 'missions') {
    return sidebar
  }

  const currentMissions = sidebarMissionItems(sidebar)

  if (patch.op === 'delete' || patch.op === 'invalidate') {
    return sidebarWithMissionItems(
      sidebar,
      currentMissions.filter(item => item.href !== `/t/${patch.id}`),
      activeHref,
    )
  }

  const item = missionItemFromUnknown(patch.value, activeHref)

  if (item === undefined) {
    return sidebar
  }

  return sidebarWithMissionItem(sidebar, item, activeHref)
}

export const sidebarWithBilling = (
  sidebar: SidebarModel,
  _billing: BillingSummary,
): SidebarModel => sidebar

export const sidebarWithTokenLeaderboards = (
  sidebar: SidebarModel,
  _tokenLeaderboards: TokenLeaderboards | undefined,
): SidebarModel => sidebar

export const missionItemsFromSnapshot = (
  snapshot: SyncSnapshot,
  activeHref: string | undefined = undefined,
): ReadonlyArray<SidebarSessionItem> => {
  const missions = snapshot.collections['missions'] ?? {}

  return Object.values(missions).flatMap(value => {
    const item = missionItemFromUnknown(value, activeHref)

    return item === undefined ? [] : [item]
  })
}

export const syncWorkspaceScope = (session: Session): string =>
  `workspace:${session.userId}`

export const syncThreadScope = (threadId: string): string =>
  `thread:${threadId}`

export const syncTeamScope = (teamId: string): string => `team:${teamId}`

export const syncAgentRunScope = (runId: string): string => `agent-run:${runId}`

export const initSync = (session: Session): SyncClientModel =>
  SyncClientModel({
    collectionByScope: {},
    connectionByScope: {
      [syncWorkspaceScope(session)]: { status: 'idle' },
    },
    cursors: {},
    pendingMutations: {},
    workspaceScope: syncWorkspaceScope(session),
  })

export const initTokenUsageStatsFilters = (): TokenUsageStatsFilters =>
  TokenUsageStatsFilters({
    actorTeamId: '',
    actorUserId: '',
    leaderboardEligible: '',
    leaderboardWindow: '7d',
    model: '',
    producerSystem: '',
    provider: '',
    since: '',
    sourceRoute: '',
    until: '',
    usageTruth: '',
  })

export const initSidebar = (auth: AuthBootstrap): SidebarModel =>
  SidebarModel({
    footerRows: [],
    primaryItems: [
      {
        href: autopilotWorkRouter(),
        label: 'Work',
      },
      {
        href: forgeRouter(),
        label: 'Factory',
      },
      {
        href: decisionsRouter(),
        label: 'Decisions',
      },
      ...(auth.isAdmin
        ? [
            {
              href: statsRouter(),
              label: 'Stats',
            },
            {
              href: adminRouter(),
              label: 'Admin',
            },
          ]
        : []),
      ...(loggedInMulletAccessAllowed(auth)
        ? [
            {
              href: mulletRouter(),
              label: 'Mullet',
            },
          ]
        : []),
    ],
    sessionSections: (() => {
      const projectItems: ReadonlyArray<SidebarSessionItem> =
        projectWorkroomsEnabled()
          ? auth.teams.flatMap(team =>
              (team.projects ?? []).map(project => ({
                active: false,
                attention: false,
                detail: project.agent?.repository ?? '',
                href: teamProjectChatRouter({
                  projectRef: teamProjectRouteRef(project),
                  teamRef: teamRouteRef(team),
                }),
                owner: 'project',
                projectId: project.id,
                status: sidebarStatusForProject(project),
                teamId: team.id,
                title: project.name,
              })),
            )
          : []

      return auth.teams.length === 0
        ? []
        : [
            {
              title: 'Team rooms',
              items: auth.teams.map((team, index) => ({
                active: index === 0,
                attention: false,
                detail: '',
                href: teamChatRouter({ teamRef: teamRouteRef(team) }),
                owner: 'team',
                status: 'active',
                teamId: team.id,
                title: team.name,
              })),
            },
            ...(projectItems.length === 0
              ? []
              : [
                  {
                    title: 'Projects',
                    items: projectItems,
                  },
                ]),
          ]
    })(),
  })

export const init = (route: LoggedInRoute, auth: AuthBootstrap): Model =>
  Model({
    adminAdjutantAssignments: AdminAdjutantAssignmentsIdle(),
    adminAdjutantEnrichmentAction: AdminAdjutantEnrichmentActionIdle(),
    adminAdjutantReview: AdminAdjutantReviewIdle(),
    adminOverview: AdminOverviewIdle(),
    adminSiteDeploymentAction: AdminSiteDeploymentActionIdle(),
    agentGoalPanel: AgentGoalPanelModel({
      budgetDraft: '',
      error: Option.none(),
      goal: Option.none(),
      isEditing: false,
      objectiveDraft: auth.onboarding.goal ?? '',
      pendingAction: Option.none(),
      scopeKey: '',
    }),
    artanisOperatorConsole: ArtanisOperatorConsoleIdle(),
    artanisOperatorDashboard: ArtanisOperatorDashboardIdle(),
    artanisOperatorDashboardCallerIdFilter: '',
    artanisOperatorGoalPanel: ArtanisOperatorGoalPanelModel({
      error: Option.none(),
      goal: Option.none(),
      objectiveDraft:
        'Maintain the Pylon v0.2 launch, Forum coordination, and Model Lab improvement loop.',
      pendingAction: Option.none(),
      scopeKey: '',
    }),
    auth,
    autopilotMorningReport: AutopilotMorningReportIdle(),
    autopilotWorkBriefing: AutopilotWorkBriefingIdle(),
    autopilotWorkDetail: AutopilotWorkDetailIdle(),
    autopilotWorkEvents: AutopilotWorkEventsIdle(),
    autopilotWorkList: AutopilotWorkListIdle(),
    autopilotWorkComposer: AutopilotWorkComposerIdle(),
    autopilotWorkComposerDraft: {
      branch: 'main',
      maxSpendCents: '0',
      objective: '',
      repositoryFullName: 'OpenAgentsInc/openagents',
      verificationCommand: 'bun test',
    },
    autopilotWorkReview: AutopilotWorkReviewIdle(),
    autopilotDecisions: AutopilotDecisionsIdle(),
    autopilotDecisionAct: AutopilotDecisionActIdle(),
    workroom: initWorkroom('', WorkroomOverviewTab),
    billingAction: IdleBillingAction(),
    billingCouponCode: '',
    chatComposerValue: '',
    chatMessages: [],
    chatRun: IdleChatRun(),
    customerOrderCreate: CustomerOrderCreateIdle(),
    customerOrderDraft: '',
    customerOrder: CustomerOrderIdle(),
    customerOrders: CustomerOrdersIdle(),
    customerOneCohort: CustomerOneCohortIdle(),
    customerFulfillmentArtifacts: CustomerFulfillmentArtifactsIdle(),
    customerSiteFeedback: CustomerSiteFeedbackIdle(),
    customerSiteFeedbackDraft: '',
    customerSiteElementContext: null,
    customerSiteFeedbackSubmit: CustomerSiteFeedbackSubmitIdle(),
    customerSiteRevisions: CustomerSiteRevisionsIdle(),
    customerSiteBuilderEvents: CustomerSiteBuilderEventsIdle(),
    customerSiteBuilderFileRead: CustomerSiteBuilderFileReadIdle(),
    customerSiteBuilderFiles: CustomerSiteBuilderFilesIdle(),
    customerSiteBuilderSelectedFilePath: null,
    customerSiteBuilderSession: CustomerSiteBuilderSessionIdle(),
    inviteCodeAction: IdleInviteCodeAction(),
    imageGeneration: ImageGenerationIdle(),
    imageGenerationAspectRatio: '1:1',
    imageGenerationCount: 1,
    imageGenerationImageSize: '1K',
    imageGenerationModel: 'gemini-2.5-flash-image',
    imageGenerationPrompt: '',
    imageGenerationProvider: 'google-gemini',
    inviteCodeValue: '',
    mullet: initMullet(),
    notifications: initNotifications(),
    onboarding: initOnboardingFlow(auth.onboarding),
    prefilledWorkspace: PrefilledWorkspaceIdle(),
    providerAccountPool: ProviderAccountPoolIdle(),
    providerConnectionAction: IdleProviderConnectionAction(),
    runMetadataDialog: ClosedRunMetadataDialog(),
    route,
    session: auth.session,
    sidebar: initSidebar(auth),
    sync: initSync(auth.session),
    tokenUsageStats: TokenUsageStatsIdle({
      filters: initTokenUsageStatsFilters(),
    }),
    teamChatMessagesByTeam: {},
    threadRoute: ThreadRouteIdle(),
    threadFileDetailErrorsById: {},
    threadFileDetailsById: {},
    threadFileDownloadErrorsById: {},
    threadFileUpload: IdleThreadFileUpload(),
    threadFilesByScope: {},
  })

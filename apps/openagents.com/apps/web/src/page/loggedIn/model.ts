import {
  SyncPatch,
  SyncSnapshot,
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

export const AutopilotWorkSessionSummary = S.Struct({
  artifactRefs: S.optionalKey(S.Array(S.String)),
  bridgeRefs: S.optionalKey(S.Array(S.String)),
  checkpointRefs: S.optionalKey(S.Array(S.String)),
  eventRefs: S.optionalKey(S.Array(S.String)),
  observedAt: S.optionalKey(S.NullOr(S.String)),
  sessionRef: S.String,
  state: S.optionalKey(AutopilotWorkSessionState),
  title: S.optionalKey(S.NullOr(S.String)),
})
export type AutopilotWorkSessionSummary =
  typeof AutopilotWorkSessionSummary.Type

export const AutopilotWorkSessionNavigation = S.Struct({
  bridgeSessions: S.optionalKey(S.Array(AutopilotWorkSessionSummary)),
  claudeSessions: S.optionalKey(S.Array(AutopilotWorkSessionSummary)),
  codexSessions: S.optionalKey(S.Array(AutopilotWorkSessionSummary)),
  localPylonSessions: S.optionalKey(S.Array(AutopilotWorkSessionSummary)),
})
export type AutopilotWorkSessionNavigation =
  typeof AutopilotWorkSessionNavigation.Type

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

export const AutopilotWorkContextSnapshot = S.Struct({
  adapters: S.optionalKey(AutopilotWorkContextAdapters),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  currentJob: S.optionalKey(AutopilotWorkContextCurrentJob),
  devDoctor: S.optionalKey(AutopilotWorkContextRefGroup),
  freshness: S.optionalKey(AutopilotWorkContextFreshness),
  instructions: S.optionalKey(AutopilotWorkContextInstructions),
  observedAt: S.optionalKey(S.NullOr(S.String)),
  repo: S.optionalKey(AutopilotWorkContextRepo),
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

export const AutopilotWorkRetrievalPlan = S.Struct({
  blockerRefs: S.optionalKey(S.Array(S.String)),
  candidates: S.optionalKey(S.Array(AutopilotWorkRetrievalCandidate)),
  freshness: S.optionalKey(AutopilotWorkRetrievalFreshness),
  generatedAt: S.optionalKey(S.String),
  mode: S.optionalKey(AutopilotWorkRetrievalMode),
  planRef: S.optionalKey(S.String),
  queryRefs: S.optionalKey(S.Array(S.String)),
  requestRef: S.optionalKey(S.String),
  skippedCandidates: S.optionalKey(S.Array(AutopilotWorkRetrievalSkippedCandidate)),
  sourceRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotWorkRetrievalPlan =
  typeof AutopilotWorkRetrievalPlan.Type

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
  buyerPaymentProofRef: S.NullOr(S.String),
  clientRequestRef: S.String,
  createdAt: S.String,
  eventStreamRef: S.String,
  executionCloseout: S.NullOr(AutopilotWorkExecutionCloseout),
  fallbackLeaseIntents: S.Array(S.Unknown),
  funding: S.Unknown,
  generatedAt: S.String,
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
  contextSnapshot: S.optionalKey(AutopilotWorkContextSnapshot),
  planMutationReceipts: S.optionalKey(S.Array(AutopilotWorkPlanMutationReceipt)),
  planMutationRequests: S.optionalKey(S.Array(AutopilotWorkPlanMutationRequest)),
  retrievalPlan: S.optionalKey(AutopilotWorkRetrievalPlan),
  sessionNavigation: S.optionalKey(AutopilotWorkSessionNavigation),
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

import { Schema as S } from 'effect'

export {
  extractAutopilotTokenUsage,
  extractAutopilotTokenUsageFromJson,
  type AutopilotTokenUsage,
} from './token-usage'
export * from './token-usage-ledger'
export {
  normalizeOmniRunnerEventPayload,
  type NormalizedOmniRunnerEventPayload,
} from './runner-event'
export * from './share'

export const SyncScope = S.String.pipe(S.brand('SyncScope'))
export type SyncScope = typeof SyncScope.Type

export const SyncSequence = S.Int.pipe(S.brand('SyncSequence'))
export type SyncSequence = typeof SyncSequence.Type

export const MutationId = S.String.pipe(S.brand('MutationId'))
export type MutationId = typeof MutationId.Type

export const CollectionName = S.String.pipe(S.brand('CollectionName'))
export type CollectionName = typeof CollectionName.Type

export const EntityId = S.String.pipe(S.brand('EntityId'))
export type EntityId = typeof EntityId.Type

export const IsoTimestamp = S.String.pipe(S.brand('IsoTimestamp'))
export type IsoTimestamp = typeof IsoTimestamp.Type

export const SyncOperation = S.Literals([
  'put',
  'patch',
  'delete',
  'invalidate',
])
export type SyncOperation = typeof SyncOperation.Type

export class SyncPatch extends S.Class<SyncPatch>('SyncPatch')({
  scope: SyncScope,
  seq: SyncSequence,
  collection: CollectionName,
  op: SyncOperation,
  id: EntityId,
  value: S.optionalKey(S.Unknown),
  patch: S.optionalKey(S.Unknown),
  serverTime: IsoTimestamp,
  mutationId: S.optionalKey(MutationId),
}) {}

export class SyncCommand extends S.Class<SyncCommand>('SyncCommand')({
  mutationId: MutationId,
  scope: SyncScope,
  command: S.String,
  payload: S.Unknown,
  expectedVersion: S.optionalKey(S.Int),
}) {}

export class SyncMutationAccepted extends S.Class<SyncMutationAccepted>(
  'SyncMutationAccepted',
)({
  status: S.Literal('accepted'),
  mutationId: MutationId,
}) {}

export class SyncMutationRejected extends S.Class<SyncMutationRejected>(
  'SyncMutationRejected',
)({
  status: S.Literal('rejected'),
  mutationId: MutationId,
  reason: S.String,
}) {}

export const SyncMutationResult = S.Union([
  SyncMutationAccepted,
  SyncMutationRejected,
])
export type SyncMutationResult = typeof SyncMutationResult.Type

export class SyncSnapshot extends S.Class<SyncSnapshot>('SyncSnapshot')({
  scope: SyncScope,
  cursor: SyncSequence,
  collections: S.Record(S.String, S.Record(S.String, S.Unknown)),
}) {}

export class CursorGap extends S.Class<CursorGap>('CursorGap')({
  scope: SyncScope,
  expectedSeq: SyncSequence,
  receivedSeq: SyncSequence,
}) {}

export const RunnerRuntime = S.Literals(['opencode_codex', 'codex'])
export type RunnerRuntime = typeof RunnerRuntime.Type

export const RunnerBackend = S.Literals(['shc_vm', 'gcloud_vm'])
export type RunnerBackend = typeof RunnerBackend.Type

export const ArtifactPolicy = S.Literals([
  'redacted_logs',
  'drill_down_private',
  'metadata_only',
])
export type ArtifactPolicy = typeof ArtifactPolicy.Type

export const RetentionMode = S.Literals(['openagents_durable', 'local_only'])
export type RetentionMode = typeof RetentionMode.Type

export const TrainingUse = S.Literals(['allowed', 'org_only', 'denied'])
export type TrainingUse = typeof TrainingUse.Type

export const SandboxMode = S.Literals(['workspace_write', 'danger_full_access'])
export type SandboxMode = typeof SandboxMode.Type

export const NetworkMode = S.Literals(['restricted', 'enabled'])
export type NetworkMode = typeof NetworkMode.Type

export class RepositoryRef extends S.Class<RepositoryRef>('RepositoryRef')({
  provider: S.Literal('github'),
  owner: S.String,
  repo: S.String,
  ref: S.String,
}) {}

export class GitHubWritebackPlan extends S.Class<GitHubWritebackPlan>(
  'GitHubWritebackPlan',
)({
  commentOnIssue: S.Boolean,
  openPullRequest: S.Boolean,
  pushBranch: S.Boolean,
}) {}

export class GitHubWorkOrder extends S.Class<GitHubWorkOrder>(
  'GitHubWorkOrder',
)({
  provider: S.Literal('github'),
  repository: RepositoryRef,
  baseRef: S.String,
  branchName: S.String,
  commitMessage: S.String,
  issueComment: S.optionalKey(S.String),
  issueNumber: S.optionalKey(S.Int),
  issueUrl: S.optionalKey(S.String),
  pullRequestBody: S.optionalKey(S.String),
  pullRequestTitle: S.optionalKey(S.String),
  writeback: GitHubWritebackPlan,
}) {}

export class ModelProfile extends S.Class<ModelProfile>('ModelProfile')({
  kind: S.Literal('codex'),
  provider: S.Literal('openai'),
  model: S.String,
}) {}

export class SandboxPolicy extends S.Class<SandboxPolicy>('SandboxPolicy')({
  mode: SandboxMode,
  network: NetworkMode,
  timeoutMs: S.Int,
}) {}

export class RunnerCallback extends S.Class<RunnerCallback>('RunnerCallback')({
  url: S.String,
  tokenRef: S.String,
}) {}

export class BlueprintAssignmentScope extends S.Class<BlueprintAssignmentScope>(
  'BlueprintAssignmentScope',
)({
  actionSubmissionPolicyRef: S.optionalKey(S.String),
  backendCapabilityRefs: S.optionalKey(S.Array(S.String)),
  contextPackRefs: S.optionalKey(S.Array(S.String)),
  contractExport: S.optionalKey(S.Unknown),
  moduleVersionRefs: S.optionalKey(S.Array(S.String)),
  programRunPurposeRef: S.optionalKey(S.String),
  programSignatureRefs: S.optionalKey(S.Array(S.String)),
  programTypeRefs: S.optionalKey(S.Array(S.String)),
  registry: S.optionalKey(S.Unknown),
  registryVersionRef: S.String,
  releaseGateRefs: S.optionalKey(S.Array(S.String)),
  sourceAuthorityRefs: S.optionalKey(S.Array(S.String)),
  toolScopeRefs: S.optionalKey(S.Array(S.String)),
}) {}

export const AgentGoalToolName = S.Literals([
  'get_goal',
  'create_goal',
  'update_goal',
])
export type AgentGoalToolName = typeof AgentGoalToolName.Type

export class AgentGoalToolEndpoint extends S.Class<AgentGoalToolEndpoint>(
  'AgentGoalToolEndpoint',
)({
  method: S.Literals(['GET', 'POST']),
  pathTemplate: S.String,
}) {}

export class AgentGoalToolSpec extends S.Class<AgentGoalToolSpec>(
  'AgentGoalToolSpec',
)({
  name: AgentGoalToolName,
  description: S.String,
  inputSchema: S.Record(S.String, S.Unknown),
  endpoint: AgentGoalToolEndpoint,
}) {}

export class AgentGoalToolContract extends S.Class<AgentGoalToolContract>(
  'AgentGoalToolContract',
)({
  schemaVersion: S.Literal('openagents.agent_goal_tools.v1'),
  tools: S.Array(AgentGoalToolSpec),
}) {}

export class AgentGoalHiddenSteering extends S.Class<AgentGoalHiddenSteering>(
  'AgentGoalHiddenSteering',
)({
  continuation: S.String,
  budgetLimit: S.String,
  objectiveUpdated: S.String,
  publicVisibility: S.String,
}) {}

export class AgentGoalAssignmentContext extends S.Class<AgentGoalAssignmentContext>(
  'AgentGoalAssignmentContext',
)({
  schemaVersion: S.Literal('openagents.agent_goal_context.v1'),
  goalId: S.NullOr(S.String),
  objective: S.String,
  status: S.NullOr(S.String),
  visibility: S.Literals(['private', 'team', 'public']),
  tokenBudget: S.NullOr(S.Int),
  tokensUsed: S.Int,
  timeUsedSeconds: S.Int,
  remainingTokens: S.NullOr(S.Int),
  toolContract: AgentGoalToolContract,
  hiddenSteering: AgentGoalHiddenSteering,
}) {}

export class AgentRunAssignment extends S.Class<AgentRunAssignment>(
  'AgentRunAssignment',
)({
  schemaVersion: S.Literal('openagents.agent_run_assignment.v1'),
  runId: S.String,
  runtime: RunnerRuntime,
  backend: RunnerBackend,
  assignmentKind: S.Literal('workroom_agent'),
  goal: S.String,
  repository: RepositoryRef,
  providerAccountRef: S.optionalKey(S.String),
  authGrantRef: S.optionalKey(S.String),
  githubWriteConnectionRef: S.optionalKey(S.String),
  githubWriteGrantRef: S.optionalKey(S.String),
  githubWorkOrder: S.optionalKey(GitHubWorkOrder),
  modelProfile: ModelProfile,
  sandbox: SandboxPolicy,
  artifactPolicy: ArtifactPolicy,
  retentionMode: RetentionMode,
  trainingUse: TrainingUse,
  callback: RunnerCallback,
  blueprint: S.optionalKey(BlueprintAssignmentScope),
  goalContext: S.optionalKey(AgentGoalAssignmentContext),
}) {}

export class DeployCommands extends S.Class<DeployCommands>('DeployCommands')({
  install: S.String,
  typecheck: S.String,
  test: S.String,
  build: S.String,
  smoke: S.String,
}) {}

export class DeployRollbackPolicy extends S.Class<DeployRollbackPolicy>(
  'DeployRollbackPolicy',
)({
  retainPreviousRelease: S.Boolean,
  healthCheckUrl: S.String,
}) {}

export class AppDeployAssignment extends S.Class<AppDeployAssignment>(
  'AppDeployAssignment',
)({
  schemaVersion: S.Literal('openagents.app_deploy_assignment.v1'),
  deployId: S.String,
  runtime: RunnerRuntime,
  primaryBackend: RunnerBackend,
  fallbackBackend: RunnerBackend,
  service: S.String,
  repository: RepositoryRef,
  commands: DeployCommands,
  callback: RunnerCallback,
  rollback: DeployRollbackPolicy,
}) {}

export class OmniRunnerEvent extends S.Class<OmniRunnerEvent>(
  'OmniRunnerEvent',
)({
  sequence: S.Int,
  type: S.String,
  summary: S.String,
  status: S.optionalKey(S.String),
  source: S.String,
  payload: S.optionalKey(S.Unknown),
  externalEventId: S.optionalKey(S.String),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  createdAt: IsoTimestamp,
}) {}

export const ServerMessage = S.Union([SyncPatch, CursorGap])
export type ServerMessage = typeof ServerMessage.Type

export const decodeSyncCommand = S.decodeUnknownEffect(SyncCommand)
export const decodeSyncPatch = S.decodeUnknownEffect(SyncPatch)
export const decodeAgentRunAssignment =
  S.decodeUnknownEffect(AgentRunAssignment)
export const decodeAppDeployAssignment =
  S.decodeUnknownEffect(AppDeployAssignment)
export const decodeOmniRunnerEvent = S.decodeUnknownEffect(OmniRunnerEvent)

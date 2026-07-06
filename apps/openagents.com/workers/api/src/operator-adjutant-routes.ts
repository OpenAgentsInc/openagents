import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
// KS-8.11 (#8322): enrichment ledger/operations ride the CRM/email
// dual-write seam; non-domain services keep the authoritative D1.
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
  makeCrmEmailDatabaseForEnv,
} from './crm-email-domain-store'
import { Effect, Match as M, Schema as S } from 'effect'

import {
  type AdjutantAdjustmentContinuationMode,
  type AdjutantAdjustmentError,
  type AdjutantAdjustmentRequest,
  makeAdjutantAdjustmentService,
} from './adjutant-adjustments'
import {
  type AdjutantAssignment,
  type AdjutantAssignmentError,
  type AdjutantAssignmentKind,
  AdjutantAssignmentNotFound,
  AdjutantAssignmentService,
  AdjutantAssignmentSiteNotFound,
  type AdjutantAssignmentVisibility,
  type CreateAdjutantAssignmentInput,
  makeAdjutantAssignmentService,
  systemAdjutantAssignmentRuntime,
} from './adjutant-assignments'
import { makeAgentGoalRepositoryForEnv } from './agent-runtime-store'
import {
  type AdjutantEnrichmentJob,
  type AdjutantEnrichmentJobError,
  AdjutantEnrichmentQueueMessage,
  makeAdjutantEnrichmentJobService,
} from './adjutant-enrichment-jobs'
import {
  type AdjutantEnrichmentLedgerError,
  type ExaEnrichmentQuery,
  type ExaEnrichmentRun,
  type ExaEnrichmentSourceCard,
  ExaSourceReviewStatus,
  type ExaSourceReviewStatus as ExaSourceReviewStatusShape,
  type ExaSourceCategory as LedgerExaSourceCategory,
  makeAdjutantEnrichmentLedger,
} from './adjutant-enrichment-ledger'
import {
  type AdjutantEnrichmentOperationsError,
  type CachedExaSourceResult,
  type ExaEnrichmentOperationsPolicy,
  type RecordExaMetricInput,
  exaCacheKey,
  exaEnrichmentOperationsPolicyFromConfig,
  makeAdjutantEnrichmentOperationsService,
  retryExaEffect,
} from './adjutant-enrichment-operations'
import {
  type AdjutantEnrichmentOrderContext,
  type AdjutantEnrichmentPlannerError,
  type AdjutantEnrichmentSiteContext,
  type ExaEnrichmentPlan,
  type ExaEnrichmentPlanTask,
  PublicSourceRefKind,
  PublicSourceRefStatus,
  type PublicSourceRefStatus as PublicSourceRefStatusShape,
  makeAdjutantEnrichmentPlanner,
} from './adjutant-enrichment-planner'
import {
  type AdjutantPublicSourceRef,
  type AdjutantPublicSourceRefError,
  makeAdjutantPublicSourceRefService,
} from './adjutant-public-source-refs'
import {
  type AdjutantResearchBrief,
  type AdjutantResearchBriefError,
  makeAdjutantResearchBriefService,
} from './adjutant-research-briefs'
import {
  type AdjutantResearchPolicy,
  type AdjutantResearchPolicyError,
  AdjutantResearchPolicyMode,
  makeAdjutantResearchPolicyService,
} from './adjutant-research-policies'
import {
  type AdjutantTaskPacketFreshness,
  type AdjutantTaskPacketFreshnessError,
  makeAdjutantTaskPacketFreshnessService,
} from './adjutant-task-packet-freshness'
import {
  ADJUTANT_TASK_PACKET_REPOSITORY,
  type AdjutantTaskPacketError,
  AdjutantTaskPacketRefMissing,
  AdjutantTaskPacketRefValidationFailed,
  type AdjutantTaskPacketRefValidationInput,
  buildAdjutantTaskPacket,
} from './adjutant-task-packets'
import {
  type AdjutantUsageReceipt,
  type AdjutantUsageReceiptError,
  type AdjutantUsageReceiptSummary,
  listAdjutantUsageReceiptsForAssignment,
  recordAdjutantUsageReceipt,
  summarizeAdjutantUsageReceipts,
} from './adjutant-usage-receipts'
import {
  type ExaSearchType,
  type OpenAgentsWorkerConfigEnv,
  getOpenAgentsWorkerConfig,
} from './config'
import {
  type ExaClientShape,
  ExaConfigurationDisabled,
  type ExaContentsResult,
  type ExaError,
  ExaProviderHttpError,
  type ExaSearchCategory,
  type ExaSearchResult,
  makeExaClient,
} from './exa'
import {
  type FirstBatchPaymentPolicy,
  type FirstBatchPaymentPolicyStorageError,
  readFirstBatchPaymentGate,
} from './first-batch-payment-policies'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { type AuthKvStore, authKvStoreForEnv } from './auth/auth-kv'
import { githubIdentityTokenKey } from './onboarding/github'
// KS-8.12 (#8323): sites writes ride the dual-write mirror seam — the
// mirroring database is a passthrough for non-scoped statements and
// degrades to the raw D1 handle when no KHALA_SYNC_DB binding exists.
import { businessDomainDatabaseForEnv } from './business-domain-store'
import { sitesContentDatabaseForEnv as openAgentsDatabase } from './sites-content-store'
import {
  compactRandomId,
  currentEpochMillis,
  currentIsoTimestamp,
} from './runtime-primitives'
import { AutopilotSiteLaunchChecklist } from './sites'
import {
  makeSupervisionLongtailMirrorForEnv,
  type SupervisionLongtailMirror,
} from './supervision-longtail-domain-store'

type OperatorAdjutantEnv = OpenAgentsWorkerConfigEnv &
  Readonly<{
    ADJUTANT_ENRICHMENT_QUEUE: Queue
    AUTH_KV?: AuthKvStore | undefined
    KHALA_SYNC_DB?: Readonly<{ connectionString: string }> | undefined
    OPENAGENTS_DB: D1Database
  }>
type HttpResponse = globalThis.Response

type OperatorAdjutantSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type OperatorAdjutantCheckStatus = 'ok' | 'warning' | 'blocked' | 'unknown'

type OperatorAdjutantCheck = Readonly<{
  details?: Record<string, unknown> | undefined
  message: string
  name: string
  status: OperatorAdjutantCheckStatus
}>

type OperatorAdjutantPreflightPayload = Readonly<{
  checks: ReadonlyArray<OperatorAdjutantCheck>
  nextSafeAction: string
  run?: unknown
  status: OperatorAdjutantCheckStatus
  targetUser?: unknown
}>

type OperatorAdjutantTargetUser = Readonly<{
  displayName: string
  email: string | null
  githubUsername: string | null
  userId: string
}>

type OperatorAdjutantAutopilotLaunch = Readonly<{
  payload: Record<string, unknown>
  runId: string
}>

type OperatorAdjutantAutopilotLaunchResult =
  | Readonly<{ launch: OperatorAdjutantAutopilotLaunch; ok: true }>
  | Readonly<{ ok: false; response: HttpResponse }>

type OperatorAdjutantAutopilotContinuation = Readonly<{
  goalId: string | null
  mode: 'follow_up_turn'
  payload: Record<string, unknown>
  runId: string
}>

type OperatorAdjutantAutopilotContinuationResult =
  | Readonly<{ continuation: OperatorAdjutantAutopilotContinuation; ok: true }>
  | Readonly<{ ok: false; response: HttpResponse }>

type OperatorAdjutantRouteDependencies<
  Session extends OperatorAdjutantSession,
  Bindings extends OperatorAdjutantEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  buildOperatorAutopilotPreflightPayload: (
    request: Request,
    env: Bindings,
    selector: Record<string, unknown>,
    targetUser: OperatorAdjutantTargetUser,
  ) => Promise<OperatorAdjutantPreflightPayload>
  continueUserAutopilotRun: (
    env: Bindings,
    ctx: ExecutionContext,
    input: Readonly<{
      prompt: string
      runId: string
      userId: string
    }>,
  ) => Promise<OperatorAdjutantAutopilotContinuationResult>
  isOpenAgentsAdminEmail: (email: string) => boolean
  launchUserAutopilotMission: (
    env: Bindings,
    ctx: ExecutionContext,
    input: Readonly<{
      selector: Record<string, unknown>
      userId: string
    }>,
  ) => Promise<OperatorAdjutantAutopilotLaunchResult>
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  validateAdjutantTaskPacketRef: (
    input: AdjutantTaskPacketRefValidationInput,
  ) => Promise<boolean>
}>

class OperatorAdjutantUnauthorized extends S.TaggedErrorClass<OperatorAdjutantUnauthorized>()(
  'OperatorAdjutantUnauthorized',
  {},
) {}

class OperatorAdjutantForbidden extends S.TaggedErrorClass<OperatorAdjutantForbidden>()(
  'OperatorAdjutantForbidden',
  {},
) {}

class OperatorAdjutantBadRequest extends S.TaggedErrorClass<OperatorAdjutantBadRequest>()(
  'OperatorAdjutantBadRequest',
  {
    reason: S.String,
  },
) {}

class OperatorAdjutantConflict extends S.TaggedErrorClass<OperatorAdjutantConflict>()(
  'OperatorAdjutantConflict',
  {
    error: S.String,
    reason: S.String,
  },
) {}

class OperatorAdjutantInvalidVisibility extends S.TaggedErrorClass<OperatorAdjutantInvalidVisibility>()(
  'OperatorAdjutantInvalidVisibility',
  {},
) {}

class OperatorAdjutantSessionError extends S.TaggedErrorClass<OperatorAdjutantSessionError>()(
  'OperatorAdjutantSessionError',
  {
    error: S.Defect,
  },
) {}

class OperatorAdjutantPreflightError extends S.TaggedErrorClass<OperatorAdjutantPreflightError>()(
  'OperatorAdjutantPreflightError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

class OperatorAdjutantLaunchBlocked extends S.TaggedErrorClass<OperatorAdjutantLaunchBlocked>()(
  'OperatorAdjutantLaunchBlocked',
  {
    checks: S.Array(S.Unknown),
    nextSafeAction: S.String,
    status: S.String,
  },
) {}

class OperatorAdjutantLaunchError extends S.TaggedErrorClass<OperatorAdjutantLaunchError>()(
  'OperatorAdjutantLaunchError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

class OperatorAdjutantStorageError extends S.TaggedErrorClass<OperatorAdjutantStorageError>()(
  'OperatorAdjutantStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

type OperatorAdjutantRouteError =
  | AdjutantAdjustmentError
  | AdjutantAssignmentError
  | AdjutantEnrichmentOperationsError
  | AdjutantEnrichmentLedgerError
  | AdjutantEnrichmentJobError
  | AdjutantEnrichmentPlannerError
  | AdjutantPublicSourceRefError
  | AdjutantResearchBriefError
  | AdjutantResearchPolicyError
  | AdjutantTaskPacketFreshnessError
  | AdjutantTaskPacketError
  | AdjutantUsageReceiptError
  | ExaError
  | OperatorAdjutantBadRequest
  | OperatorAdjutantConflict
  | OperatorAdjutantForbidden
  | OperatorAdjutantInvalidVisibility
  | OperatorAdjutantLaunchBlocked
  | OperatorAdjutantLaunchError
  | OperatorAdjutantPreflightError
  | OperatorAdjutantSessionError
  | OperatorAdjutantStorageError
  | OperatorAdjutantUnauthorized

export class CreateOperatorAdjutantAssignmentRequest extends S.Class<CreateOperatorAdjutantAssignmentRequest>(
  'CreateOperatorAdjutantAssignmentRequest',
)({
  agentId: S.optionalKey(S.String),
  assignmentKind: S.optionalKey(S.String),
  commitSha: S.optionalKey(S.String),
  currentRunId: S.optionalKey(S.String),
  goalId: S.optionalKey(S.String),
  objective: S.optionalKey(S.String),
  projectId: S.optionalKey(S.String),
  status: S.optionalKey(S.String),
  taskSpecPath: S.optionalKey(S.String),
  teamId: S.optionalKey(S.String),
  visibility: S.optionalKey(S.String),
}) {}

export class PreflightOperatorAdjutantAssignmentRequest extends S.Class<PreflightOperatorAdjutantAssignmentRequest>(
  'PreflightOperatorAdjutantAssignmentRequest',
)({
  includeCallbackLag: S.optionalKey(S.Boolean),
  launchChecklist: S.optionalKey(AutopilotSiteLaunchChecklist),
}) {}

export class GenerateOperatorAdjutantTaskPacketRequest extends S.Class<GenerateOperatorAdjutantTaskPacketRequest>(
  'GenerateOperatorAdjutantTaskPacketRequest',
)({
  commitSha: S.String,
  operatorNotes: S.optionalKey(S.String),
  taskSpecPath: S.optionalKey(S.String),
}) {}

export class KeepCurrentOperatorAdjutantTaskPacketRequest extends S.Class<KeepCurrentOperatorAdjutantTaskPacketRequest>(
  'KeepCurrentOperatorAdjutantTaskPacketRequest',
)({
  customerSafeSummary: S.String,
  reason: S.String,
}) {}

export class ClearOperatorAdjutantCurrentRunRequest extends S.Class<ClearOperatorAdjutantCurrentRunRequest>(
  'ClearOperatorAdjutantCurrentRunRequest',
)({
  reason: S.String,
  runId: S.String,
}) {}

export class LaunchOperatorAdjutantAssignmentRequest extends S.Class<LaunchOperatorAdjutantAssignmentRequest>(
  'LaunchOperatorAdjutantAssignmentRequest',
)({
  includeCallbackLag: S.optionalKey(S.Boolean),
  launchChecklist: S.optionalKey(AutopilotSiteLaunchChecklist),
  providerAccountId: S.optionalKey(S.String),
  providerAccountRef: S.optionalKey(S.String),
  runnerBackend: S.optionalKey(S.String),
  timeoutMs: S.optionalKey(S.Number),
}) {}

export class CreateOperatorAdjutantAdjustmentRequest extends S.Class<CreateOperatorAdjutantAdjustmentRequest>(
  'CreateOperatorAdjutantAdjustmentRequest',
)({
  instruction: S.String,
  providerAccountId: S.optionalKey(S.String),
  providerAccountRef: S.optionalKey(S.String),
  runnerBackend: S.optionalKey(S.String),
  timeoutMs: S.optionalKey(S.Number),
}) {}

export class PlanOperatorAdjutantEnrichmentRequest extends S.Class<PlanOperatorAdjutantEnrichmentRequest>(
  'PlanOperatorAdjutantEnrichmentRequest',
)({
  freshnessMaxAgeHours: S.optionalKey(S.Number),
  numResults: S.optionalKey(S.Number),
  operatorNotes: S.optionalKey(S.String),
}) {}

export class RunOperatorAdjutantEnrichmentRequest extends S.Class<RunOperatorAdjutantEnrichmentRequest>(
  'RunOperatorAdjutantEnrichmentRequest',
)({
  freshnessMaxAgeHours: S.optionalKey(S.Number),
  numResults: S.optionalKey(S.Number),
  operatorNotes: S.optionalKey(S.String),
  requestBudget: S.optionalKey(S.Number),
}) {}

export class EnqueueOperatorAdjutantEnrichmentRequest extends S.Class<EnqueueOperatorAdjutantEnrichmentRequest>(
  'EnqueueOperatorAdjutantEnrichmentRequest',
)({
  freshnessMaxAgeHours: S.optionalKey(S.Number),
  numResults: S.optionalKey(S.Number),
  operatorNotes: S.optionalKey(S.String),
  refresh: S.optionalKey(S.Boolean),
  requestBudget: S.optionalKey(S.Number),
  triggerKind: S.optionalKey(
    S.Literals(['research_required', 'operator_requested', 'operator_refresh']),
  ),
}) {}

export class CreateOperatorAdjutantPublicSourceRefRequest extends S.Class<CreateOperatorAdjutantPublicSourceRefRequest>(
  'CreateOperatorAdjutantPublicSourceRefRequest',
)({
  kind: PublicSourceRefKind,
  label: S.optionalKey(S.NullOr(S.String)),
  status: S.optionalKey(PublicSourceRefStatus),
  url: S.String,
}) {}

export class ReviewOperatorAdjutantPublicSourceRefRequest extends S.Class<ReviewOperatorAdjutantPublicSourceRefRequest>(
  'ReviewOperatorAdjutantPublicSourceRefRequest',
)({
  publicSafe: S.optionalKey(S.Boolean),
  reviewReason: S.optionalKey(S.NullOr(S.String)),
  status: PublicSourceRefStatus,
}) {}

export class ReviewOperatorAdjutantSourceCardRequest extends S.Class<ReviewOperatorAdjutantSourceCardRequest>(
  'ReviewOperatorAdjutantSourceCardRequest',
)({
  publicSafe: S.optionalKey(S.Boolean),
  rejectedReason: S.optionalKey(S.NullOr(S.String)),
  reviewStatus: ExaSourceReviewStatus,
}) {}

export class ReviewOperatorAdjutantResearchBriefRequest extends S.Class<ReviewOperatorAdjutantResearchBriefRequest>(
  'ReviewOperatorAdjutantResearchBriefRequest',
)({
  reviewReason: S.optionalKey(S.NullOr(S.String)),
  status: S.Literals(['approved', 'rejected', 'stale']),
}) {}

export class SetOperatorAdjutantResearchPolicyRequest extends S.Class<SetOperatorAdjutantResearchPolicyRequest>(
  'SetOperatorAdjutantResearchPolicyRequest',
)({
  customerSafeSummary: S.String,
  policyMode: AdjutantResearchPolicyMode,
  reason: S.String,
  sourceAuthorityRef: S.optionalKey(S.NullOr(S.String)),
}) {}

type PreflightSoftwareOrderRow = Readonly<{
  id: string
  repository_default_branch: string | null
  repository_name: string | null
  repository_owner: string | null
  repository_provider: 'github' | null
}>

type PreflightSiteRow = Readonly<{
  access_mode: string
  active_deployment_id: string | null
  active_version_id: string | null
  id: string
  source_repository_name: string | null
  source_repository_owner: string | null
  source_repository_provider: 'github' | null
  source_repository_ref: string | null
  slug: string
  title: string
  visibility: string
}>

type EnrichmentSoftwareOrderRow = Readonly<{
  id: string
  repository_default_branch: string | null
  repository_full_name: string | null
  repository_html_url: string | null
  repository_name: string | null
  repository_owner: string | null
  repository_private: number | null
  request: string
}>

type EnrichmentSiteRow = Readonly<{
  id: string
  slug: string
  source_repository_name: string | null
  source_repository_owner: string | null
  source_repository_provider: 'github' | null
  source_repository_ref: string | null
  title: string
}>

type OperatorAdjutantReviewSoftwareOrder = Readonly<{
  id: string
  status: string
  visibility: string
  request: string
  repositoryFullName: string | null
  currentRunId: string | null
  createdAt: string
  updatedAt: string
}>

type OperatorAdjutantReviewSite = Readonly<{
  id: string
  slug: string
  title: string
  status: string
  accessMode: string
  visibility: string
  activeVersionId: string | null
  activeDeploymentId: string | null
}>

type OperatorAdjutantReviewGoal = Readonly<{
  id: string
  agentId: string
  status: string
  visibility: string
  currentRunId: string | null
  tokensUsed: number
  tokenBudget: number | null
  timeUsedSeconds: number
  updatedAt: string
}>

type OperatorAdjutantReviewRun = Readonly<{
  id: string
  runtime: string
  backend: string
  status: string
  eventCursor: number
  externalRunId: string | null
  createdAt: string
  updatedAt: string
}>

type OperatorAdjutantReviewVersion = Readonly<{
  id: string
  sourceKind: string
  sourceCommitSha: string | null
  buildStatus: string
  buildCommand: string | null
  workerModuleR2Key: string | null
  createdByRunId: string | null
  createdAt: string
  savedAt: string | null
  rejectedAt: string | null
}>

type OperatorAdjutantReviewDeployment = Readonly<{
  id: string
  versionId: string
  url: string
  runtimeKind: string
  status: string
  externalDeploymentId: string | null
  activatedAt: string | null
  disabledAt: string | null
  rolledBackAt: string | null
  updatedAt: string
}>

type OperatorAdjutantReviewAdjustment = Readonly<{
  id: string
  instruction: string
  status: string
  continuationMode: string | null
  sourceRunId: string | null
  continuationRunId: string | null
  resultingVersionId: string | null
  requestedByUserId: string | null
  createdAt: string
  updatedAt: string
}>

type OperatorAdjutantReviewResearchBrief = Readonly<{
  approvedAt: string | null
  enrichmentRunId: string | null
  id: string
  sourceCount: number
  status: string
  summary: string
  updatedAt: string
}>

type OperatorAdjutantReviewEnrichmentSourceCard = Readonly<{
  approvedAt: string | null
  domain: string
  highlightText: string | null
  id: string
  publicSafe: boolean
  publishedDate: string | null
  rejectedAt: string | null
  rejectedReason: string | null
  reviewStatus: ExaSourceReviewStatusShape
  runId: string
  sourceCategory: string
  title: string
  updatedAt: string
  url: string
}>

type OperatorAdjutantReviewPublicSourceRef = Readonly<{
  approvedAt: string | null
  id: string
  kind: string
  label: string | null
  normalizedDomain: string
  publicSafe: boolean
  rejectedAt: string | null
  reviewReason: string | null
  status: PublicSourceRefStatusShape
  updatedAt: string
  url: string | null
}>

type OperatorAdjutantReviewEnrichment = Readonly<{
  exaConfigured: boolean
  latestJob: AdjutantEnrichmentJob | null
  latestRun: ExaEnrichmentRun | null
  nextAction: string
  queries: ReadonlyArray<ExaEnrichmentQuery>
  researchBrief: AdjutantResearchBrief | null
  sourceCards: ReadonlyArray<OperatorAdjutantReviewEnrichmentSourceCard>
  sourceRefs: ReadonlyArray<OperatorAdjutantReviewPublicSourceRef>
  status:
    | 'not_configured'
    | 'not_planned'
    | 'planned'
    | 'queued'
    | 'running'
    | 'succeeded'
    | 'partial_failure'
    | 'failed'
    | 'needs_review'
    | 'approved'
    | 'rejected'
    | 'stale'
}>

type OperatorAdjutantReviewEvent = Readonly<{
  id: string
  type: string
  summary: string
  runId: string | null
  createdAt: string
}>

type OperatorAdjutantAssignmentReview = Readonly<{
  adjustments: ReadonlyArray<OperatorAdjutantReviewAdjustment>
  assignmentEvents: ReadonlyArray<OperatorAdjutantReviewEvent>
  currentRun: OperatorAdjutantReviewRun | null
  deployments: ReadonlyArray<OperatorAdjutantReviewDeployment>
  enrichment: OperatorAdjutantReviewEnrichment
  goal: OperatorAdjutantReviewGoal | null
  nextAction: string
  order: OperatorAdjutantReviewSoftwareOrder | null
  researchPolicy: AdjutantResearchPolicy
  researchBrief: OperatorAdjutantReviewResearchBrief | null
  site: OperatorAdjutantReviewSite | null
  siteEvents: ReadonlyArray<OperatorAdjutantReviewEvent>
  taskPacketFreshness: AdjutantTaskPacketFreshness
  usageReceipts: ReadonlyArray<AdjutantUsageReceipt>
  usageSummary: AdjutantUsageReceiptSummary
  versions: ReadonlyArray<OperatorAdjutantReviewVersion>
}>

type ReviewSoftwareOrderRow = Readonly<{
  created_at: string
  current_run_id: string | null
  id: string
  repository_full_name: string | null
  request: string
  status: string
  updated_at: string
  visibility: string
}>

type ReviewSiteRow = Readonly<{
  access_mode: string
  active_deployment_id: string | null
  active_version_id: string | null
  id: string
  slug: string
  status: string
  title: string
  visibility: string
}>

type ReviewGoalRow = Readonly<{
  agent_id: string
  current_run_id: string | null
  id: string
  status: string
  time_used_seconds: number
  token_budget: number | null
  tokens_used: number
  updated_at: string
  visibility: string
}>

type ReviewRunRow = Readonly<{
  backend: string
  created_at: string
  event_cursor: number
  external_run_id: string | null
  id: string
  runtime: string
  status: string
  updated_at: string
}>

type ReviewVersionRow = Readonly<{
  build_command: string | null
  build_status: string
  created_at: string
  created_by_run_id: string | null
  id: string
  rejected_at: string | null
  saved_at: string | null
  source_commit_sha: string | null
  source_kind: string
  worker_module_r2_key: string | null
}>

type ReviewDeploymentRow = Readonly<{
  activated_at: string | null
  disabled_at: string | null
  external_deployment_id: string | null
  id: string
  rolled_back_at: string | null
  runtime_kind: string
  status: string
  updated_at: string
  url: string
  version_id: string
}>

type ReviewAssignmentEventRow = Readonly<{
  created_at: string
  event_type: string
  id: string
  run_id: string | null
  summary: string
}>

type ReviewSiteEventRow = Readonly<{
  actor_run_id: string | null
  created_at: string
  id: string
  summary: string
  type: string
}>

type ReviewAdjustmentRow = Readonly<{
  continuation_mode: string | null
  continuation_run_id: string | null
  created_at: string
  id: string
  instruction: string
  requested_by_user_id: string | null
  resulting_version_id: string | null
  source_run_id: string | null
  status: string
  updated_at: string
}>

const assignmentKinds = new Set<AdjutantAssignmentKind>([
  'site_generation',
  'site_adjustment',
  'site_review',
  'site_deployment',
  'general_order_fulfillment',
])
const assignmentStatuses = new Set<CreateAdjutantAssignmentInput['status']>([
  'draft',
  'preflight_pending',
  'blocked',
  'queued',
  'running',
  'review_needed',
  'deployed',
  'delivered',
  'complete',
  'canceled',
])
const assignmentVisibilities = new Set<AdjutantAssignmentVisibility>([
  'private',
  'team',
  'public',
])

const routeErrorResponse = (error: OperatorAdjutantRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      AdjutantAdjustmentNotFound: ({ adjustmentId }) =>
        noStoreJsonResponse(
          { adjustmentId, error: 'adjustment_not_found' },
          { status: 404 },
        ),
      AdjutantAdjustmentStorageError: () =>
        noStoreJsonResponse(
          { error: 'adjustment_storage_error' },
          { status: 500 },
        ),
      AdjutantAdjustmentUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_adjustment_payload', reason },
          { status: 400 },
        ),
      AdjutantAdjustmentValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'adjustment_validation_error', reason },
          { status: 400 },
        ),
      AdjutantAssignmentActiveExists: ({
        assignmentId,
        siteId,
        softwareOrderId,
      }) =>
        noStoreJsonResponse(
          {
            assignmentId,
            error: 'active_assignment_exists',
            siteId,
            softwareOrderId,
          },
          { status: 409 },
        ),
      AdjutantAssignmentNotFound: ({ assignmentId }) =>
        noStoreJsonResponse(
          { assignmentId, error: 'assignment_not_found' },
          { status: 404 },
        ),
      AdjutantAssignmentGoalNotFound: ({ goalId }) =>
        noStoreJsonResponse(
          { error: 'goal_not_found', goalId },
          { status: 404 },
        ),
      AdjutantAssignmentGoalStorageError: () =>
        noStoreJsonResponse({ error: 'goal_storage_error' }, { status: 500 }),
      AdjutantAssignmentRunGoalRequired: ({
        assignmentId,
        currentRunId,
        reason,
      }) =>
        noStoreJsonResponse(
          {
            assignmentId,
            currentRunId,
            error: 'run_goal_required',
            reason,
          },
          { status: 409 },
        ),
      AdjutantAssignmentSiteNotFound: ({ siteId }) =>
        noStoreJsonResponse(
          { error: 'site_not_found', siteId },
          { status: 404 },
        ),
      AdjutantAssignmentSoftwareOrderNotFound: ({ softwareOrderId }) =>
        noStoreJsonResponse(
          { error: 'software_order_not_found', softwareOrderId },
          { status: 404 },
        ),
      AdjutantAssignmentStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
      AdjutantAssignmentUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_assignment_payload', reason },
          { status: 400 },
        ),
      AdjutantAssignmentValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'assignment_validation_error', reason },
          { status: 400 },
        ),
      AdjutantEnrichmentLedgerStorageError: () =>
        noStoreJsonResponse(
          { error: 'enrichment_storage_error' },
          { status: 500 },
        ),
      AdjutantEnrichmentLedgerUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_enrichment_payload', reason },
          { status: 400 },
        ),
      AdjutantEnrichmentLedgerValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'enrichment_validation_error', reason },
          { status: 400 },
        ),
      AdjutantEnrichmentJobActiveExists: ({ assignmentId, jobId }) =>
        noStoreJsonResponse(
          { assignmentId, error: 'adjutant_enrichment_job_active', jobId },
          { status: 409 },
        ),
      AdjutantEnrichmentJobNotFound: ({ jobId }) =>
        noStoreJsonResponse(
          { error: 'adjutant_enrichment_job_not_found', jobId },
          { status: 404 },
        ),
      AdjutantEnrichmentJobStorageError: () =>
        noStoreJsonResponse(
          { error: 'adjutant_enrichment_job_storage_error' },
          { status: 500 },
        ),
      AdjutantEnrichmentJobUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_adjutant_enrichment_job_payload', reason },
          { status: 400 },
        ),
      AdjutantEnrichmentPlannerValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'enrichment_plan_validation_error', reason },
          { status: 400 },
        ),
      ExaEnrichmentBudgetExhausted: ({
        assignmentBudget,
        assignmentUsed,
        dailyBudget,
        dailyUsed,
        message,
        requested,
        scope,
      }) =>
        noStoreJsonResponse(
          {
            assignmentBudget,
            assignmentUsed,
            dailyBudget,
            dailyUsed,
            error: 'exa_budget_exhausted',
            message,
            requested,
            scope,
          },
          { status: 409 },
        ),
      ExaEnrichmentMetricUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_enrichment_operations_payload', reason },
          { status: 400 },
        ),
      ExaEnrichmentOperationsStorageError: () =>
        noStoreJsonResponse(
          { error: 'enrichment_operations_storage_error' },
          { status: 500 },
        ),
      AdjutantPublicSourceRefStorageError: () =>
        noStoreJsonResponse(
          { error: 'source_ref_storage_error' },
          { status: 500 },
        ),
      AdjutantPublicSourceRefUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_source_ref_payload', reason },
          { status: 400 },
        ),
      AdjutantPublicSourceRefValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'source_ref_validation_error', reason },
          { status: 400 },
        ),
      AdjutantResearchBriefStorageError: () =>
        noStoreJsonResponse(
          { error: 'research_brief_storage_error' },
          { status: 500 },
        ),
      AdjutantResearchBriefUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_research_brief_payload', reason },
          { status: 400 },
        ),
      AdjutantResearchBriefValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'research_brief_validation_error', reason },
          { status: 400 },
        ),
      AdjutantResearchPolicyStorageError: () =>
        noStoreJsonResponse(
          { error: 'research_policy_storage_error' },
          { status: 500 },
        ),
      AdjutantResearchPolicyUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_research_policy_payload', reason },
          { status: 400 },
        ),
      AdjutantResearchPolicyValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'research_policy_validation_error', reason },
          { status: 400 },
        ),
      AdjutantTaskPacketUnsafe: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_task_packet', reason },
          { status: 400 },
        ),
      AdjutantTaskPacketRefMissing: ({ commitSha, path, reason }) =>
        noStoreJsonResponse(
          { commitSha, error: 'task_packet_ref_missing', path, reason },
          { status: 409 },
        ),
      AdjutantTaskPacketRefValidationFailed: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'task_packet_ref_validation_failed', reason },
          { status: 502 },
        ),
      AdjutantTaskPacketFreshnessStorageError: () =>
        noStoreJsonResponse(
          { error: 'task_packet_freshness_storage_error' },
          { status: 500 },
        ),
      AdjutantTaskPacketFreshnessUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_task_packet_freshness_payload', reason },
          { status: 400 },
        ),
      AdjutantTaskPacketFreshnessValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'task_packet_freshness_validation_error', reason },
          { status: 400 },
        ),
      AdjutantTaskPacketValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'task_packet_validation_error', reason },
          { status: 400 },
        ),
      AdjutantUsageReceiptStorageError: () =>
        noStoreJsonResponse(
          { error: 'usage_receipt_storage_error' },
          { status: 500 },
        ),
      AdjutantUsageReceiptUnsafe: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_usage_receipt', reason },
          { status: 400 },
        ),
      ExaConfigurationDisabled: ({ reason }) =>
        noStoreJsonResponse(
          { configured: false, error: 'exa_unconfigured', reason },
          { status: 409 },
        ),
      ExaProviderFetchError: () =>
        noStoreJsonResponse({ error: 'exa_fetch_error' }, { status: 502 }),
      ExaProviderHttpError: ({ endpoint, message, status }) =>
        noStoreJsonResponse(
          {
            endpoint,
            error: 'exa_http_error',
            providerStatus: status,
            summary: message,
          },
          { status: status === 429 ? 429 : 502 },
        ),
      ExaProviderInvalidJson: ({ endpoint }) =>
        noStoreJsonResponse(
          { endpoint, error: 'exa_invalid_json' },
          { status: 502 },
        ),
      ExaProviderSchemaError: ({ endpoint }) =>
        noStoreJsonResponse(
          { endpoint, error: 'exa_schema_error' },
          { status: 502 },
        ),
      ExaProviderTimeout: ({ endpoint, timeoutMs }) =>
        noStoreJsonResponse(
          { endpoint, error: 'exa_timeout', timeoutMs },
          { status: 504 },
        ),
      OperatorAdjutantBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      OperatorAdjutantConflict: ({ error, reason }) =>
        noStoreJsonResponse({ error, reason }, { status: 409 }),
      OperatorAdjutantForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      OperatorAdjutantInvalidVisibility: () =>
        noStoreJsonResponse({ error: 'invalid_visibility' }, { status: 400 }),
      OperatorAdjutantLaunchBlocked: ({ checks, nextSafeAction, status }) =>
        noStoreJsonResponse(
          {
            checks,
            error: 'adjutant_launch_blocked',
            nextSafeAction,
            status,
          },
          { status: 409 },
        ),
      OperatorAdjutantLaunchError: () =>
        noStoreJsonResponse({ error: 'launch_error' }, { status: 500 }),
      OperatorAdjutantPreflightError: () =>
        noStoreJsonResponse({ error: 'preflight_error' }, { status: 500 }),
      OperatorAdjutantSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      OperatorAdjutantStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
      OperatorAdjutantUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
    }),
    M.exhaustive,
  )

const decodeJsonBody = <Schema extends S.Top>(
  request: Request,
  schema: Schema,
) =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      catch: error =>
        new OperatorAdjutantBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
      try: () => request.json(),
    })

    return yield* S.decodeUnknownEffect(schema)(payload)
  }).pipe(
    Effect.mapError(error =>
      error instanceof OperatorAdjutantBadRequest
        ? error
        : new OperatorAdjutantBadRequest({ reason: 'invalid request body' }),
    ),
  )

type AdjutantAssignmentServiceShape = ReturnType<
  typeof makeAdjutantAssignmentService
>

const readRequiredAdjutantAssignment = (
  assignments: AdjutantAssignmentServiceShape,
  assignmentId: string,
) =>
  Effect.gen(function* () {
    const assignment = yield* assignments.readAssignmentById(assignmentId)

    if (assignment === null) {
      return yield* new AdjutantAssignmentNotFound({ assignmentId })
    }

    return assignment
  })

const requireAdminSession = <
  Session extends OperatorAdjutantSession,
  Bindings extends OperatorAdjutantEnv,
>(
  dependencies: OperatorAdjutantRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const requireAdminApiToken = dependencies.requireAdminApiToken

    if (requireAdminApiToken !== undefined) {
      const hasAdminApiToken = yield* Effect.tryPromise({
        catch: error => new OperatorAdjutantSessionError({ error }),
        try: () => requireAdminApiToken(request, env),
      })

      if (hasAdminApiToken === true) {
        return {
          user: {
            email: 'chris@openagents.com',
            userId: 'github:14167547',
          },
        } as Session
      }
    }

    const session = yield* Effect.tryPromise({
      catch: error => new OperatorAdjutantSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new OperatorAdjutantUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new OperatorAdjutantForbidden({})
    }

    return session
  })

const readOperatorGitHubIdentityToken = async (
  env: OperatorAdjutantEnv,
  userId: string,
): Promise<string | undefined> => {
  // Fail-soft on storage errors: enrichment simply proceeds without the
  // operator's GitHub identity token (CFG-3 #8518: Postgres KvStore, not KV).
  try {
    return (
      (await authKvStoreForEnv(env).get(githubIdentityTokenKey(userId))) ??
      undefined
    )
  } catch {
    return undefined
  }
}

const rejectUnlessMethod = (
  request: Request,
  allowedMethods: readonly string[],
): HttpResponse | null =>
  allowedMethods.includes(request.method)
    ? null
    : methodNotAllowed([...allowedMethods])

const runRoute = (
  env: OperatorAdjutantEnv,
  request: Request,
  allowedMethods: readonly string[],
  effect: Effect.Effect<
    HttpResponse,
    OperatorAdjutantRouteError,
    AdjutantAssignmentService
  >,
): Effect.Effect<HttpResponse> => {
  const methodRejection = rejectUnlessMethod(request, allowedMethods)

  return methodRejection === null
    ? effect.pipe(
        Effect.provide(AdjutantAssignmentService.layer(env)),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    : Effect.succeed(methodRejection)
}

const preflightD1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OperatorAdjutantPreflightError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new OperatorAdjutantPreflightError({ error, operation }),
  })

const storageD1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OperatorAdjutantStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new OperatorAdjutantStorageError({ error, operation }),
  })

const mapResearchBriefStorageError =
  (operation: string) =>
  (error: AdjutantResearchBriefError): OperatorAdjutantStorageError =>
    new OperatorAdjutantStorageError({ error, operation })

const mapResearchBriefPreflightError =
  (operation: string) =>
  (error: AdjutantResearchBriefError): OperatorAdjutantPreflightError =>
    new OperatorAdjutantPreflightError({ error, operation })

const mapResearchPolicyStorageError =
  (operation: string) =>
  (error: AdjutantResearchPolicyError): OperatorAdjutantStorageError =>
    new OperatorAdjutantStorageError({ error, operation })

const mapResearchPolicyPreflightError =
  (operation: string) =>
  (error: AdjutantResearchPolicyError): OperatorAdjutantPreflightError =>
    new OperatorAdjutantPreflightError({ error, operation })

const mapEnrichmentJobPreflightError =
  (operation: string) =>
  (error: AdjutantEnrichmentJobError): OperatorAdjutantPreflightError =>
    new OperatorAdjutantPreflightError({ error, operation })

const researchBriefReviewSummary = (
  brief: AdjutantResearchBrief | null,
): OperatorAdjutantReviewResearchBrief | null =>
  brief === null
    ? null
    : {
        approvedAt: brief.approvedAt,
        enrichmentRunId: brief.enrichmentRunId,
        id: brief.id,
        sourceCount: brief.sourceCards.length,
        status: brief.status,
        summary: brief.summary,
        updatedAt: brief.updatedAt,
      }

const latestApprovedResearchBrief = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<AdjutantResearchBrief | null, OperatorAdjutantStorageError> =>
  makeAdjutantResearchBriefService(crmEmailAuthorityDb(db))
    .latestApprovedBriefForAssignment(assignmentId)
    .pipe(
      Effect.mapError(
        mapResearchBriefStorageError(
          'operatorAdjutant.researchBrief.approved.read',
        ),
      ),
    )

const latestResearchBriefForPreflight = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<
  AdjutantResearchBrief | null,
  OperatorAdjutantPreflightError
> =>
  makeAdjutantResearchBriefService(crmEmailAuthorityDb(db))
    .latestBriefForAssignment(assignmentId)
    .pipe(
      Effect.mapError(
        mapResearchBriefPreflightError(
          'operatorAdjutant.preflight.researchBrief.read',
        ),
      ),
    )

const latestEnrichmentRunForPreflight = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<ExaEnrichmentRun | null, OperatorAdjutantPreflightError> =>
  makeAdjutantEnrichmentLedger(db)
    .latestRunForAssignment(assignmentId)
    .pipe(
      Effect.mapError(
        error =>
          new OperatorAdjutantPreflightError({
            error,
            operation: 'operatorAdjutant.preflight.enrichmentRun.read',
          }),
      ),
    )

const latestEnrichmentJobForPreflight = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<
  AdjutantEnrichmentJob | null,
  OperatorAdjutantPreflightError
> =>
  makeAdjutantEnrichmentJobService(crmEmailAuthorityDb(db))
    .latestJobForAssignment(assignmentId)
    .pipe(
      Effect.mapError(
        mapEnrichmentJobPreflightError(
          'operatorAdjutant.preflight.enrichmentJob.read',
        ),
      ),
    )

const effectiveResearchPolicyForPreflight = (
  db: CrmEmailDatabase,
  assignment: AdjutantAssignment,
): Effect.Effect<AdjutantResearchPolicy, OperatorAdjutantPreflightError> =>
  makeAdjutantResearchPolicyService(crmEmailAuthorityDb(db))
    .readEffectivePolicy(assignment)
    .pipe(
      Effect.mapError(
        mapResearchPolicyPreflightError(
          'operatorAdjutant.preflight.researchPolicy.read',
        ),
      ),
    )

const latestResearchBriefForReview = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<AdjutantResearchBrief | null, OperatorAdjutantStorageError> =>
  makeAdjutantResearchBriefService(crmEmailAuthorityDb(db))
    .latestBriefForAssignment(assignmentId)
    .pipe(
      Effect.mapError(
        mapResearchBriefStorageError(
          'operatorAdjutant.review.researchBrief.read',
        ),
      ),
    )

const effectiveResearchPolicyForReview = (
  db: CrmEmailDatabase,
  assignment: AdjutantAssignment,
): Effect.Effect<AdjutantResearchPolicy, OperatorAdjutantStorageError> =>
  makeAdjutantResearchPolicyService(crmEmailAuthorityDb(db))
    .readEffectivePolicy(assignment)
    .pipe(
      Effect.mapError(
        mapResearchPolicyStorageError(
          'operatorAdjutant.review.researchPolicy.read',
        ),
      ),
    )

const sourceCardReviewSummary = (
  sourceCard: ExaEnrichmentSourceCard,
): OperatorAdjutantReviewEnrichmentSourceCard => {
  const hiddenSourceText =
    sourceCard.reviewStatus === 'rejected' ||
    sourceCard.reviewStatus === 'internal_only'

  return {
    approvedAt: sourceCard.approvedAt,
    domain: sourceCard.domain,
    highlightText: hiddenSourceText ? null : sourceCard.highlightText,
    id: sourceCard.id,
    publicSafe: sourceCard.publicSafe,
    publishedDate: sourceCard.publishedDate,
    rejectedAt: sourceCard.rejectedAt,
    rejectedReason: sourceCard.rejectedReason,
    reviewStatus: sourceCard.reviewStatus,
    runId: sourceCard.runId,
    sourceCategory: sourceCard.sourceCategory,
    title: sourceCard.title,
    updatedAt: sourceCard.updatedAt,
    url: sourceCard.url,
  }
}

const sourceRefReviewSummary = (
  sourceRef: AdjutantPublicSourceRef,
): OperatorAdjutantReviewPublicSourceRef => {
  const hiddenUrl =
    sourceRef.status === 'rejected' || sourceRef.status === 'internal_only'

  return {
    approvedAt: sourceRef.approvedAt,
    id: sourceRef.id,
    kind: sourceRef.kind,
    label: sourceRef.label,
    normalizedDomain: sourceRef.normalizedDomain,
    publicSafe: sourceRef.publicSafe,
    rejectedAt: sourceRef.rejectedAt,
    reviewReason: sourceRef.reviewReason,
    status: sourceRef.status,
    updatedAt: sourceRef.updatedAt,
    url: hiddenUrl ? null : sourceRef.url,
  }
}

const enrichmentReviewStatus = (
  exaConfigured: boolean,
  latestRun: ExaEnrichmentRun | null,
  researchBrief: AdjutantResearchBrief | null,
): OperatorAdjutantReviewEnrichment['status'] => {
  if (!exaConfigured) {
    return 'not_configured'
  }

  if (researchBrief?.status === 'approved') {
    return 'approved'
  }

  if (researchBrief?.status === 'rejected') {
    return 'rejected'
  }

  if (researchBrief?.status === 'stale') {
    return 'stale'
  }

  if (researchBrief?.status === 'needs_review') {
    return 'needs_review'
  }

  return latestRun?.status ?? 'not_planned'
}

const enrichmentNextAction = (
  status: OperatorAdjutantReviewEnrichment['status'],
  sourceCards: ReadonlyArray<OperatorAdjutantReviewEnrichmentSourceCard>,
  researchBrief: AdjutantResearchBrief | null,
): string => {
  if (status === 'not_configured') {
    return 'Configure EXA_API_KEY before running enrichment.'
  }

  if (status === 'not_planned') {
    return 'Plan and run enrichment before launching Autopilot.'
  }

  if (status === 'running' || status === 'queued' || status === 'planned') {
    return 'Wait for enrichment to finish before reviewing sources.'
  }

  if (status === 'failed') {
    return 'Refresh enrichment or inspect provider status before launch.'
  }

  if (status === 'partial_failure') {
    return 'Review available source cards and refresh if more evidence is needed.'
  }

  const proposedSource = sourceCards.find(
    sourceCard => sourceCard.reviewStatus === 'proposed',
  )

  if (proposedSource !== undefined) {
    return 'Review proposed source cards before approving the brief.'
  }

  if (researchBrief?.status === 'needs_review') {
    return 'Approve or reject the research brief for task packet context.'
  }

  if (status === 'approved') {
    return 'Research is approved for Autopilot launch context.'
  }

  if (status === 'rejected' || status === 'stale') {
    return 'Refresh enrichment before launch.'
  }

  return 'Review enrichment state before launch.'
}

const readEnrichmentReview = (
  db: CrmEmailDatabase,
  assignmentId: string,
  exaConfigured: boolean,
): Effect.Effect<
  OperatorAdjutantReviewEnrichment,
  OperatorAdjutantRouteError
> =>
  Effect.gen(function* () {
    const ledger = makeAdjutantEnrichmentLedger(db)
    const jobs = makeAdjutantEnrichmentJobService(crmEmailAuthorityDb(db))
    const sourceRefs = makeAdjutantPublicSourceRefService(crmEmailAuthorityDb(db))
    const briefService = makeAdjutantResearchBriefService(crmEmailAuthorityDb(db))
    const latestJob = yield* jobs.latestJobForAssignment(assignmentId)
    const latestRun = yield* ledger.latestRunForAssignment(assignmentId)
    const queries =
      latestRun === null ? [] : yield* ledger.queriesForRun(latestRun.id)
    const sourceCards = yield* ledger.sourceCardsForAssignment(assignmentId)
    const refs = yield* sourceRefs.listForAssignment(assignmentId)
    const researchBrief =
      yield* briefService.latestBriefForAssignment(assignmentId)
    const summarizedCards = sourceCards.map(sourceCardReviewSummary)
    const status = enrichmentReviewStatus(
      exaConfigured,
      latestRun,
      researchBrief,
    )

    return {
      exaConfigured,
      latestJob,
      latestRun,
      nextAction: enrichmentNextAction(status, summarizedCards, researchBrief),
      queries,
      researchBrief,
      sourceCards: summarizedCards,
      sourceRefs: refs.map(sourceRefReviewSummary),
      status,
    }
  })

const readEnrichmentSoftwareOrder = (
  db: CrmEmailDatabase,
  softwareOrderId: string,
): Effect.Effect<
  AdjutantEnrichmentOrderContext | null,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.enrichment.softwareOrder.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                request,
                repository_full_name,
                repository_owner,
                repository_name,
                repository_private,
                repository_default_branch,
                repository_html_url
           FROM software_orders
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<EnrichmentSoftwareOrderRow>(),
  ).pipe(
    Effect.map(row =>
      row === null
        ? null
        : {
            id: row.id,
            repositoryDefaultBranch: row.repository_default_branch,
            repositoryFullName: row.repository_full_name,
            repositoryHtmlUrl: row.repository_html_url,
            repositoryName: row.repository_name,
            repositoryOwner: row.repository_owner,
            repositoryPrivate:
              row.repository_private === null
                ? null
                : row.repository_private === 1,
            request: row.request,
          },
    ),
  )

const readEnrichmentSite = (
  db: CrmEmailDatabase,
  siteId: string,
): Effect.Effect<
  AdjutantEnrichmentSiteContext | null,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.enrichment.site.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                slug,
                title,
                source_repository_provider,
                source_repository_owner,
                source_repository_name,
                source_repository_ref
           FROM site_projects
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(siteId)
      .first<EnrichmentSiteRow>(),
  ).pipe(
    Effect.map(row =>
      row === null
        ? null
        : {
            id: row.id,
            slug: row.slug,
            sourceRepositoryName: row.source_repository_name,
            sourceRepositoryOwner: row.source_repository_owner,
            sourceRepositoryProvider: row.source_repository_provider,
            sourceRepositoryRef: row.source_repository_ref,
            title: row.title,
          },
    ),
  )

const buildEnrichmentPlan = (
  db: CrmEmailDatabase,
  assignment: AdjutantAssignment,
  body:
    | PlanOperatorAdjutantEnrichmentRequest
    | RunOperatorAdjutantEnrichmentRequest,
): Effect.Effect<ExaEnrichmentPlan, OperatorAdjutantRouteError> =>
  Effect.gen(function* () {
    const order =
      assignment.softwareOrderId === null
        ? null
        : yield* readEnrichmentSoftwareOrder(db, assignment.softwareOrderId)
    const site =
      assignment.siteId === null
        ? null
        : yield* readEnrichmentSite(db, assignment.siteId)
    const explicitSourceRefs = yield* makeAdjutantPublicSourceRefService(
      crmEmailAuthorityDb(db),
    ).plannerSourceRefsForAssignment(assignment.id)

    return yield* makeAdjutantEnrichmentPlanner().buildPlan({
      assignment,
      explicitSourceRefs,
      freshnessMaxAgeHours: body.freshnessMaxAgeHours,
      numResults: body.numResults,
      operatorNotes: body.operatorNotes,
      order,
      site,
    })
  })

const exaSearchCategories = new Set<ExaSearchCategory>([
  'company',
  'github',
  'linkedin profile',
  'news',
  'pdf',
  'people',
  'personal site',
  'research paper',
  'tweet',
])
const exaSearchTypes = new Set<ExaSearchType>([
  'auto',
  'deep',
  'deep-lite',
  'deep-reasoning',
  'fast',
  'instant',
])
const ledgerSourceCategories = new Set<LedgerExaSourceCategory>([
  'generic_url',
  'github_profile',
  'linkedin_profile',
  'order_request',
  'people_profile',
  'personal_site',
  'repository',
  'topic_web',
  'x_profile',
])

const exaCategoryForTask = (
  task: ExaEnrichmentPlanTask,
): ExaSearchCategory | undefined =>
  task.category !== null &&
  exaSearchCategories.has(task.category as ExaSearchCategory)
    ? (task.category as ExaSearchCategory)
    : undefined

const exaSearchTypeForTask = (task: ExaEnrichmentPlanTask): ExaSearchType =>
  exaSearchTypes.has(task.searchType as ExaSearchType)
    ? (task.searchType as ExaSearchType)
    : 'auto'

const ledgerSourceCategoryForTask = (
  task: ExaEnrichmentPlanTask,
): LedgerExaSourceCategory =>
  ledgerSourceCategories.has(task.sourceCategory as LedgerExaSourceCategory)
    ? (task.sourceCategory as LedgerExaSourceCategory)
    : 'generic_url'

const exaErrorSummary = (error: ExaError): string =>
  M.value(error).pipe(
    M.tags({
      ExaConfigurationDisabled: ({ reason }) => reason,
      ExaProviderFetchError: () => 'Exa fetch failed.',
      ExaProviderHttpError: ({ message, status }) =>
        `Exa HTTP ${status}: ${message}`,
      ExaProviderInvalidJson: () => 'Exa returned invalid JSON.',
      ExaProviderSchemaError: () => 'Exa response did not match schema.',
      ExaProviderTimeout: ({ timeoutMs }) =>
        `Exa request timed out after ${timeoutMs}ms.`,
    }),
    M.exhaustive,
  )

type EnrichmentTaskExecution = Readonly<{
  error: ExaError | null
  failed: boolean
  query: ExaEnrichmentQuery
  sourceCards: ReadonlyArray<ExaEnrichmentSourceCard>
}>

type AdjutantEnrichmentOperations = ReturnType<
  typeof makeAdjutantEnrichmentOperationsService
>

const MAX_EXA_SOURCE_HIGHLIGHT_CHARS = 1200

const titleForSearchResult = (result: ExaSearchResult): string =>
  result.title?.trim() === '' || result.title === undefined
    ? result.url
    : result.title

const titleForContentsResult = (result: ExaContentsResult): string =>
  result.title?.trim() === '' || result.title === undefined
    ? result.url
    : result.title

const highlightForSearchResult = (result: ExaSearchResult): string | null =>
  boundedExaSourceHighlight(
    result.highlights?.[0] ??
      result.summary ??
      result.text ??
      result.contents?.highlights?.[0] ??
      result.contents?.summary ??
      result.contents?.text ??
      null,
  )

const highlightForContentsResult = (result: ExaContentsResult): string | null =>
  boundedExaSourceHighlight(
    result.highlights?.[0] ?? result.summary ?? result.text ?? null,
  )

const boundedExaSourceHighlight = (value: string | null): string | null => {
  if (value === null) {
    return null
  }

  return value.length <= MAX_EXA_SOURCE_HIGHLIGHT_CHARS
    ? value
    : value.slice(0, MAX_EXA_SOURCE_HIGHLIGHT_CHARS)
}

const sourceDomainForUrl = (url: string, fallback: string | null): string => {
  const normalizedFallback = fallback?.trim()

  try {
    return new URL(url).hostname
  } catch {
    return normalizedFallback === undefined || normalizedFallback === ''
      ? 'unknown.invalid'
      : normalizedFallback
  }
}

const cachedSearchResult = (
  result: ExaSearchResult,
): CachedExaSourceResult => ({
  domain: sourceDomainForUrl(result.url, result.domain ?? null),
  highlightText: highlightForSearchResult(result),
  publishedDate: result.publishedDate ?? null,
  title: titleForSearchResult(result),
  url: result.url,
})

const cachedContentsResult = (
  result: ExaContentsResult,
): CachedExaSourceResult => ({
  domain: sourceDomainForUrl(result.url, null),
  highlightText: highlightForContentsResult(result),
  publishedDate: result.publishedDate ?? null,
  title: titleForContentsResult(result),
  url: result.url,
})

const taskCacheKey = (
  task: ExaEnrichmentPlanTask,
): Effect.Effect<string, OperatorAdjutantRouteError> =>
  exaCacheKey({
    freshnessMaxAgeHours: task.contentsMaxAgeHours,
    includeDomains: task.includeDomains,
    query: task.query,
    searchType: task.searchType,
    sourceCategory: task.sourceCategory,
    urls: task.kind === 'contents' ? task.urls : [],
  })

const taskMetricBase = (
  assignment: AdjutantAssignment,
  run: ExaEnrichmentRun,
  task: ExaEnrichmentPlanTask,
): Pick<
  RecordExaMetricInput,
  'assignmentId' | 'runId' | 'searchType' | 'sourceCategory'
> => ({
  assignmentId: assignment.id,
  runId: run.id,
  searchType: task.searchType,
  sourceCategory: ledgerSourceCategoryForTask(task),
})

const errorMetricCode = (error: ExaError): string =>
  error instanceof ExaProviderHttpError
    ? `${error._tag}:${error.status}`
    : error._tag

const storeCachedSourceCards = (
  ledger: ReturnType<typeof makeAdjutantEnrichmentLedger>,
  assignment: AdjutantAssignment,
  run: ExaEnrichmentRun,
  task: ExaEnrichmentPlanTask,
  query: ExaEnrichmentQuery,
  results: ReadonlyArray<CachedExaSourceResult>,
): Effect.Effect<
  ReadonlyArray<ExaEnrichmentSourceCard>,
  OperatorAdjutantRouteError
> =>
  Effect.forEach(
    results.slice(0, task.kind === 'search' ? task.numResults : results.length),
    result =>
      ledger.storeSourceCard({
        assignmentId: assignment.id,
        domain: result.domain,
        highlightText: result.highlightText,
        publishedDate: result.publishedDate,
        queryId: query.id,
        runId: run.id,
        searchType: task.searchType,
        selectedText: result.highlightText,
        siteId: assignment.siteId,
        softwareOrderId: assignment.softwareOrderId,
        sourceCategory: ledgerSourceCategoryForTask(task),
        title: result.title,
        url: result.url,
      }),
    { concurrency: 1 },
  )

const executeSearchTask = (
  client: ExaClientShape,
  ledger: ReturnType<typeof makeAdjutantEnrichmentLedger>,
  operations: AdjutantEnrichmentOperations,
  policy: ExaEnrichmentOperationsPolicy,
  assignment: AdjutantAssignment,
  run: ExaEnrichmentRun,
  task: ExaEnrichmentPlanTask,
): Effect.Effect<EnrichmentTaskExecution, OperatorAdjutantRouteError> => {
  const startedAt = currentEpochMillis()

  return Effect.gen(function* () {
    const cacheKey = yield* taskCacheKey(task)
    const sourceCategory = ledgerSourceCategoryForTask(task)
    const searchType = exaSearchTypeForTask(task)
    const cached = yield* operations.readFreshCache({
      cacheKey,
      freshnessMaxAgeHours: task.contentsMaxAgeHours,
    })

    if (cached !== null) {
      const query = yield* ledger.recordQuery({
        assignmentId: assignment.id,
        freshnessMaxAgeHours: task.contentsMaxAgeHours,
        latencyMs: currentEpochMillis() - startedAt,
        queryText: task.query,
        resultCount: cached.length,
        runId: run.id,
        searchType: task.searchType,
        sourceCategory,
        status: 'cached',
      })
      const sourceCards = yield* storeCachedSourceCards(
        ledger,
        assignment,
        run,
        task,
        query,
        cached,
      )

      yield* operations.recordMetric({
        ...taskMetricBase(assignment, run, task),
        cacheStatus: 'hit',
        eventName: 'exa.enrichment.cache.hit',
        latencyMs: currentEpochMillis() - startedAt,
        queryId: query.id,
        resultCount: cached.length,
        sourceCardCount: sourceCards.length,
        status: 'cached',
      })

      return {
        error: null,
        failed: false,
        query,
        sourceCards,
      }
    }

    yield* operations.recordMetric({
      ...taskMetricBase(assignment, run, task),
      cacheStatus: 'miss',
      eventName: 'exa.enrichment.cache.miss',
      latencyMs: currentEpochMillis() - startedAt,
      resultCount: 0,
      sourceCardCount: 0,
      status: 'miss',
    })

    const category = exaCategoryForTask(task)
    const response = yield* retryExaEffect(
      policy,
      client.search({
        ...(category === undefined ? {} : { category }),
        contents: {
          highlights: true,
          maxAgeHours: task.contentsMaxAgeHours,
        },
        ...(task.includeDomains.length === 0
          ? {}
          : { includeDomains: [...task.includeDomains] }),
        numResults: task.numResults,
        query: task.query,
        type: searchType,
      }),
    )
    const normalizedResults = response.results
      .slice(0, task.numResults)
      .map(cachedSearchResult)
    const query = yield* ledger.recordQuery({
      assignmentId: assignment.id,
      ...(response.costDollars === undefined
        ? {}
        : { costDollars: response.costDollars }),
      freshnessMaxAgeHours: task.contentsMaxAgeHours,
      latencyMs: currentEpochMillis() - startedAt,
      queryText: task.query,
      resultCount: response.results.length,
      runId: run.id,
      searchType: response.searchType ?? task.searchType,
      sourceCategory,
      status: 'succeeded',
    })
    const sourceCards = yield* storeCachedSourceCards(
      ledger,
      assignment,
      run,
      task,
      query,
      normalizedResults,
    )

    yield* operations.storeCache({
      cacheKey,
      costDollars: response.costDollars ?? null,
      freshnessMaxAgeHours: task.contentsMaxAgeHours,
      policy,
      results: normalizedResults,
      searchType: response.searchType ?? task.searchType,
      sourceCategory,
    })
    yield* operations.recordMetric({
      ...taskMetricBase(assignment, run, task),
      cacheStatus: 'miss',
      costDollars: response.costDollars ?? null,
      eventName: 'exa.enrichment.search.succeeded',
      latencyMs: currentEpochMillis() - startedAt,
      queryId: query.id,
      resultCount: response.results.length,
      sourceCardCount: sourceCards.length,
      status: 'succeeded',
    })

    return {
      error: null,
      failed: false,
      query,
      sourceCards,
    }
  }).pipe(
    Effect.catchTag('ExaConfigurationDisabled', error => Effect.fail(error)),
    Effect.catchTags({
      ExaProviderFetchError: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
      ExaProviderHttpError: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
      ExaProviderInvalidJson: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
      ExaProviderSchemaError: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
      ExaProviderTimeout: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
    }),
  )
}

const executeContentsTask = (
  client: ExaClientShape,
  ledger: ReturnType<typeof makeAdjutantEnrichmentLedger>,
  operations: AdjutantEnrichmentOperations,
  policy: ExaEnrichmentOperationsPolicy,
  assignment: AdjutantAssignment,
  run: ExaEnrichmentRun,
  task: ExaEnrichmentPlanTask,
): Effect.Effect<EnrichmentTaskExecution, OperatorAdjutantRouteError> => {
  const startedAt = currentEpochMillis()

  return Effect.gen(function* () {
    const cacheKey = yield* taskCacheKey(task)
    const sourceCategory = ledgerSourceCategoryForTask(task)
    const cached = yield* operations.readFreshCache({
      cacheKey,
      freshnessMaxAgeHours: task.contentsMaxAgeHours,
    })

    if (cached !== null) {
      const query = yield* ledger.recordQuery({
        assignmentId: assignment.id,
        freshnessMaxAgeHours: task.contentsMaxAgeHours,
        latencyMs: currentEpochMillis() - startedAt,
        queryText: task.query,
        resultCount: cached.length,
        runId: run.id,
        searchType: task.searchType,
        sourceCategory,
        status: 'cached',
      })
      const sourceCards = yield* storeCachedSourceCards(
        ledger,
        assignment,
        run,
        task,
        query,
        cached,
      )

      yield* operations.recordMetric({
        ...taskMetricBase(assignment, run, task),
        cacheStatus: 'hit',
        eventName: 'exa.enrichment.cache.hit',
        latencyMs: currentEpochMillis() - startedAt,
        queryId: query.id,
        resultCount: cached.length,
        sourceCardCount: sourceCards.length,
        status: 'cached',
      })

      return {
        error: null,
        failed: false,
        query,
        sourceCards,
      }
    }

    yield* operations.recordMetric({
      ...taskMetricBase(assignment, run, task),
      cacheStatus: 'miss',
      eventName: 'exa.enrichment.cache.miss',
      latencyMs: currentEpochMillis() - startedAt,
      resultCount: 0,
      sourceCardCount: 0,
      status: 'miss',
    })

    const response = yield* retryExaEffect(
      policy,
      client.getContents({
        contents: {
          highlights: true,
          maxAgeHours: task.contentsMaxAgeHours,
          text: true,
        },
        urls: [...task.urls],
      }),
    )
    const normalizedResults = response.results.map(cachedContentsResult)
    const query = yield* ledger.recordQuery({
      assignmentId: assignment.id,
      ...(response.costDollars === undefined
        ? {}
        : { costDollars: response.costDollars }),
      freshnessMaxAgeHours: task.contentsMaxAgeHours,
      latencyMs: currentEpochMillis() - startedAt,
      queryText: task.query,
      resultCount: response.results.length,
      runId: run.id,
      searchType: task.searchType,
      sourceCategory,
      status: 'succeeded',
    })
    const sourceCards = yield* storeCachedSourceCards(
      ledger,
      assignment,
      run,
      task,
      query,
      normalizedResults,
    )

    yield* operations.storeCache({
      cacheKey,
      costDollars: response.costDollars ?? null,
      freshnessMaxAgeHours: task.contentsMaxAgeHours,
      policy,
      results: normalizedResults,
      searchType: task.searchType,
      sourceCategory,
    })
    yield* operations.recordMetric({
      ...taskMetricBase(assignment, run, task),
      cacheStatus: 'miss',
      costDollars: response.costDollars ?? null,
      eventName: 'exa.enrichment.contents.succeeded',
      latencyMs: currentEpochMillis() - startedAt,
      queryId: query.id,
      resultCount: response.results.length,
      sourceCardCount: sourceCards.length,
      status: 'succeeded',
    })

    return {
      error: null,
      failed: false,
      query,
      sourceCards,
    }
  }).pipe(
    Effect.catchTag('ExaConfigurationDisabled', error => Effect.fail(error)),
    Effect.catchTags({
      ExaProviderFetchError: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
      ExaProviderHttpError: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
      ExaProviderInvalidJson: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
      ExaProviderSchemaError: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
      ExaProviderTimeout: error =>
        recordFailedExaTask(
          operations,
          ledger,
          assignment,
          run,
          task,
          error,
          startedAt,
        ),
    }),
  )
}

const recordFailedExaTask = (
  operations: AdjutantEnrichmentOperations,
  ledger: ReturnType<typeof makeAdjutantEnrichmentLedger>,
  assignment: AdjutantAssignment,
  run: ExaEnrichmentRun,
  task: ExaEnrichmentPlanTask,
  error: ExaError,
  startedAt: number,
): Effect.Effect<EnrichmentTaskExecution, OperatorAdjutantRouteError> =>
  Effect.gen(function* () {
    const query = yield* ledger.recordQuery({
      assignmentId: assignment.id,
      errorCode: error._tag,
      errorSummary: exaErrorSummary(error),
      freshnessMaxAgeHours: task.contentsMaxAgeHours,
      queryText: task.query,
      resultCount: 0,
      runId: run.id,
      searchType: task.searchType,
      sourceCategory: ledgerSourceCategoryForTask(task),
      status: 'failed',
    })

    yield* operations.recordMetric({
      ...taskMetricBase(assignment, run, task),
      cacheStatus: 'miss',
      errorCode: errorMetricCode(error),
      eventName:
        task.kind === 'contents'
          ? 'exa.enrichment.contents.failed'
          : 'exa.enrichment.search.failed',
      latencyMs: currentEpochMillis() - startedAt,
      queryId: query.id,
      resultCount: 0,
      sourceCardCount: 0,
      status: 'failed',
    })

    return {
      error,
      failed: true,
      query,
      sourceCards: [],
    }
  })

const executeEnrichmentTask = (
  client: ExaClientShape,
  ledger: ReturnType<typeof makeAdjutantEnrichmentLedger>,
  operations: AdjutantEnrichmentOperations,
  policy: ExaEnrichmentOperationsPolicy,
  assignment: AdjutantAssignment,
  run: ExaEnrichmentRun,
  task: ExaEnrichmentPlanTask,
): Effect.Effect<EnrichmentTaskExecution, OperatorAdjutantRouteError> =>
  task.kind === 'contents'
    ? executeContentsTask(
        client,
        ledger,
        operations,
        policy,
        assignment,
        run,
        task,
      )
    : executeSearchTask(
        client,
        ledger,
        operations,
        policy,
        assignment,
        run,
        task,
      )

const finalRunStatus = (
  results: ReadonlyArray<EnrichmentTaskExecution>,
): ExaEnrichmentRun['status'] => {
  const sourceCount = results.reduce(
    (count, result) => count + result.sourceCards.length,
    0,
  )
  const failureCount = results.filter(result => result.failed).length

  if (failureCount > 0 && sourceCount === 0) {
    return 'failed'
  }

  if (failureCount > 0) {
    return 'partial_failure'
  }

  return 'needs_review'
}

const runErrorSummary = (
  results: ReadonlyArray<EnrichmentTaskExecution>,
): string | null => {
  const failed = results.find(result => result.error !== null)

  return failed?.error === null || failed === undefined
    ? null
    : exaErrorSummary(failed.error)
}

const optionalNumberFromRecord = (
  record: Record<string, unknown>,
  key: string,
): number | undefined =>
  typeof record[key] === 'number' && Number.isFinite(record[key])
    ? record[key]
    : undefined

const optionalStringFromRecord = (
  record: Record<string, unknown>,
  key: string,
): string | undefined =>
  typeof record[key] === 'string' ? record[key] : undefined

const failQueuedEnrichmentJob = (
  db: CrmEmailDatabase,
  jobId: string,
  runId: string | null,
  errorCode: string,
  errorSummary: string,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<void, OperatorAdjutantRouteError> =>
  Effect.gen(function* () {
    const jobs = makeAdjutantEnrichmentJobService(
      crmEmailAuthorityDb(db),
      undefined,
      mirror,
    )
    const ledger = makeAdjutantEnrichmentLedger(db, undefined, mirror)

    if (runId !== null) {
      yield* ledger.updateRunStatus({
        completedAt: currentIsoTimestamp(),
        errorCode,
        errorSummary,
        runId,
        status: 'failed',
      })
    }

    yield* jobs.updateJobStatus({
      completed: true,
      errorCode,
      errorSummary,
      jobId,
      status: 'failed',
    })
  }).pipe(Effect.asVoid)

export const executeQueuedAdjutantEnrichmentJob = (
  env: OpenAgentsWorkerConfigEnv & Readonly<{ OPENAGENTS_DB: D1Database }>,
  message: AdjutantEnrichmentQueueMessage,
): Effect.Effect<void, OperatorAdjutantRouteError> =>
  Effect.gen(function* () {
    const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
    const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
    const assignments = makeAdjutantAssignmentService(
      crmEmailAuthorityDb(db),
      systemAdjutantAssignmentRuntime,
      // KS-8.5 (#8316): goal mutations ride the agent-runtime dual-write
      // seam.
      makeAgentGoalRepositoryForEnv(env),
      supervisionMirror,
    )
    const jobs = makeAdjutantEnrichmentJobService(
      crmEmailAuthorityDb(db),
      undefined,
      supervisionMirror,
    )
    const ledger = makeAdjutantEnrichmentLedger(db, undefined, supervisionMirror)
    const briefService = makeAdjutantResearchBriefService(
      crmEmailAuthorityDb(db),
      undefined,
      supervisionMirror,
    )
    const operations = makeAdjutantEnrichmentOperationsService(db)
    const job = yield* jobs.readJobById(message.jobId)

    if (job.status !== 'queued') {
      return
    }

    const assignment = yield* assignments.readAssignmentById(job.assignmentId)

    if (assignment === null) {
      yield* failQueuedEnrichmentJob(
        db,
        job.id,
        job.enrichmentRunId,
        'assignment_not_found',
        'Queued enrichment assignment was not found.',
        supervisionMirror,
      )

      return
    }

    const config = getOpenAgentsWorkerConfig(env)

    if (!config.exa.enabled) {
      yield* failQueuedEnrichmentJob(
        db,
        job.id,
        job.enrichmentRunId,
        'exa_unconfigured',
        'EXA_API_KEY is not configured.',
        supervisionMirror,
      )

      return
    }

    yield* jobs.updateJobStatus({
      jobId: job.id,
      started: true,
      status: 'running',
    })

    const latestRun = yield* ledger.latestRunForAssignment(assignment.id)

    if (
      latestRun === null ||
      latestRun.id !== job.enrichmentRunId ||
      latestRun.status !== 'queued'
    ) {
      yield* failQueuedEnrichmentJob(
        db,
        job.id,
        job.enrichmentRunId,
        'queued_run_not_found',
        'Queued enrichment run was not found or is no longer queued.',
        supervisionMirror,
      )

      return
    }

    const request = job.request
    const freshnessMaxAgeHours = optionalNumberFromRecord(
      request,
      'freshnessMaxAgeHours',
    )
    const numResults = optionalNumberFromRecord(request, 'numResults')
    const operatorNotes = optionalStringFromRecord(request, 'operatorNotes')
    const plan = yield* buildEnrichmentPlan(db, assignment, {
      ...(freshnessMaxAgeHours === undefined ? {} : { freshnessMaxAgeHours }),
      ...(numResults === undefined ? {} : { numResults }),
      ...(operatorNotes === undefined ? {} : { operatorNotes }),
    })
    const policy = exaEnrichmentOperationsPolicyFromConfig(config.exa)
    const allTasks = [...plan.searchTasks, ...plan.contentsTasks]
    const requestedBudget = Math.max(
      1,
      Math.trunc(
        optionalNumberFromRecord(request, 'requestBudget') ??
          Math.min(6, policy.assignmentRequestBudget),
      ),
    )
    const requestBudget =
      allTasks.length === 0 ? 0 : Math.min(allTasks.length, requestedBudget)
    const selectedTasks = allTasks.slice(0, requestBudget)

    yield* operations.reserveBudget({
      assignmentId: assignment.id,
      policy,
      reason: job.refresh ? 'queued_refresh' : 'queued_run',
      requestUnits: requestBudget,
    })

    if (job.refresh) {
      const latestBrief = yield* briefService.latestBriefForAssignment(
        assignment.id,
      )

      if (latestBrief !== null) {
        yield* briefService.reviewBrief({
          briefId: latestBrief.id,
          reviewReason: 'Marked stale before queued enrichment refresh.',
          reviewedByUserId: job.requestedByUserId,
          status: 'stale',
        })
      }
    }

    yield* ledger.updateRunStatus({
      runId: latestRun.id,
      status: 'running',
    })
    yield* ledger.linkAssignmentRun({
      assignmentId: assignment.id,
      enrichmentRunId: latestRun.id,
      requiredForLaunch: true,
      status: 'running',
    })

    const client = makeExaClient(config.exa)
    const results = yield* Effect.forEach(
      selectedTasks,
      task =>
        executeEnrichmentTask(
          client,
          ledger,
          operations,
          policy,
          assignment,
          latestRun,
          task,
        ),
      { concurrency: 1 },
    )
    const status = finalRunStatus(results)
    const errorSummary = runErrorSummary(results)
    const sourceCards = yield* ledger.sourceCardsForAssignment(assignment.id)
    const runSourceCards = sourceCards.filter(
      sourceCard => sourceCard.runId === latestRun.id,
    )
    const brief =
      status === 'failed'
        ? null
        : yield* briefService.createBrief({
            assignmentId: assignment.id,
            createdByUserId: job.requestedByUserId,
            customerRequest: plan.subjectSummary,
            enrichmentRunId: latestRun.id,
            sourceCards: runSourceCards,
            status: 'needs_review',
          })

    yield* ledger.updateRunStatus({
      completedAt: currentIsoTimestamp(),
      errorCode: errorSummary === null ? null : 'exa_task_failure',
      errorSummary,
      runId: latestRun.id,
      status,
    })
    yield* ledger.linkAssignmentRun({
      assignmentId: assignment.id,
      enrichmentRunId: latestRun.id,
      requiredForLaunch: true,
      researchBriefId: brief?.id ?? null,
      status: status === 'failed' ? 'failed' : 'needs_review',
    })
    yield* operations.recordMetric({
      assignmentId: assignment.id,
      eventName: 'exa.enrichment.queued.completed',
      resultCount: results.length,
      runId: latestRun.id,
      sourceCardCount: runSourceCards.length,
      status,
    })
    yield* assignments.recordEvent({
      actorUserId: job.requestedByUserId,
      assignmentId: assignment.id,
      eventType: 'adjutant.enrichment_job_completed',
      payload: {
        enrichmentJobId: job.id,
        enrichmentRunId: latestRun.id,
        researchBriefId: brief?.id ?? null,
        status,
      },
      summary: 'Queued Autopilot enrichment completed.',
    })
    yield* jobs.updateJobStatus({
      completed: true,
      errorCode: errorSummary === null ? null : 'exa_task_failure',
      errorSummary,
      jobId: job.id,
      status: status === 'failed' ? 'failed' : 'succeeded',
    })
  })

const siteEventPayloadJson = (
  payload: unknown,
): Effect.Effect<string | null, OperatorAdjutantStorageError> =>
  Effect.gen(function* () {
    const json = yield* Effect.try({
      catch: error =>
        new OperatorAdjutantStorageError({
          error,
          operation: 'operatorAdjutant.siteEvent.payload',
        }),
      try: () => JSON.stringify(payload),
    })

    if (json === undefined) {
      return null
    }

    if (json.length > 4096) {
      return yield* new OperatorAdjutantStorageError({
        error: 'Site event payload is too large.',
        operation: 'operatorAdjutant.siteEvent.payload',
      })
    }

    if (containsProviderSecretMaterial(json)) {
      return yield* new OperatorAdjutantStorageError({
        error: 'Site event payload contains secret-shaped material.',
        operation: 'operatorAdjutant.siteEvent.payload',
      })
    }

    return json
  })

const updateSoftwareOrderLaunchState = (
  db: CrmEmailDatabase,
  input: Readonly<{
    runId: string
    softwareOrderId: string | null
    status: 'agent_queued' | 'agent_running' | 'unavailable'
    updatedAt: string
  }>,
): Effect.Effect<void, OperatorAdjutantStorageError> =>
  input.softwareOrderId === null
    ? Effect.void
    : storageD1Effect('operatorAdjutant.softwareOrder.launch.update', () =>
        crmEmailAuthorityDb(db)
          .prepare(
            `UPDATE software_orders
                SET current_run_id = ?,
                    status = ?,
                    agent_started_at = COALESCE(agent_started_at, ?),
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(
            input.runId,
            input.status,
            input.updatedAt,
            input.updatedAt,
            input.softwareOrderId,
          )
          .run(),
      ).pipe(Effect.asVoid)

const updateSiteAdjustmentState = (
  db: CrmEmailDatabase,
  input: Readonly<{
    siteId: string
    status: 'generating' | 'needs_review'
    updatedAt: string
  }>,
): Effect.Effect<void, OperatorAdjutantStorageError> =>
  storageD1Effect('operatorAdjutant.site.adjustment.update', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE site_projects
            SET status = ?,
                updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind(input.status, input.updatedAt, input.siteId)
      .run(),
  ).pipe(Effect.asVoid)

const recordSiteLaunchEvent = (
  db: CrmEmailDatabase,
  input: Readonly<{
    actorUserId: string
    eventType:
      | 'adjutant.adjustment_requested'
      | 'adjutant.adjustment_running'
      | 'adjutant.dispatch_failed'
      | 'adjutant.run_queued'
    payload: Record<string, unknown>
    runId: string
    siteId: string | null
    summary: string
  }>,
): Effect.Effect<void, OperatorAdjutantStorageError> =>
  input.siteId === null
    ? Effect.void
    : Effect.gen(function* () {
        const payloadJson = yield* siteEventPayloadJson(input.payload)
        const now = currentIsoTimestamp()

        yield* storageD1Effect('operatorAdjutant.siteEvent.launch.insert', () =>
          crmEmailAuthorityDb(db)
            .prepare(
              `INSERT INTO site_events
                 (id,
                  site_id,
                  version_id,
                  deployment_id,
                  type,
                  summary,
                  actor_user_id,
                  actor_run_id,
                  payload_json,
                  created_at)
               VALUES (?, ?, NULL, NULL, ?, ?, ?, ?, ?, ?)`,
            )
            .bind(
              compactRandomId('site_event'),
              input.siteId,
              input.eventType,
              input.summary,
              input.actorUserId,
              input.runId,
              payloadJson,
              now,
            )
            .run(),
        )
      })

const generationReceiptEligible = (assignment: AdjutantAssignment): boolean =>
  assignment.assignmentKind === 'site_generation' ||
  assignment.assignmentKind === 'general_order_fulfillment'

const recordGenerationLaunchUsageReceipt = (
  db: CrmEmailDatabase,
  input: Readonly<{
    assignment: AdjutantAssignment
    paymentPolicy?: FirstBatchPaymentPolicy | null | undefined
    runId: string
    site: PreflightSiteRow | null
  }>,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<void, AdjutantUsageReceiptError> =>
  generationReceiptEligible(input.assignment)
    ? recordAdjutantUsageReceipt(
        crmEmailAuthorityDb(db),
        {
          assignmentId: input.assignment.id,
          billingMode: 'public_beta_free',
          category: 'generation',
          idempotencyKey: [
            'adjutant_usage',
            input.assignment.id,
            input.runId,
            'generation',
          ].join(':'),
          publicDetails: {
            billingNote:
              input.paymentPolicy?.customerSafeSummary ??
              'Public beta Site generation is free.',
            firstBatchPaymentPolicyMode:
              input.paymentPolicy?.policyMode ?? null,
            siteTitle: input.site?.title ?? null,
          },
          quantity: 1,
          runId: input.runId,
          siteId: input.assignment.siteId,
          softwareOrderId: input.assignment.softwareOrderId,
          summary: 'Autopilot Site generation run was queued.',
          teamDetails: {
            assignmentKind: input.assignment.assignmentKind,
            billingPolicy: 'public_beta_free',
            firstBatchPaymentPolicyId: input.paymentPolicy?.id ?? null,
            firstBatchPaymentPolicyMode:
              input.paymentPolicy?.policyMode ?? null,
            runId: input.runId,
            siteId: input.assignment.siteId,
            softwareOrderId: input.assignment.softwareOrderId,
          },
          unit: 'run',
          visibility: input.assignment.visibility,
        },
        undefined,
        mirror,
      ).pipe(Effect.asVoid)
    : Effect.void

const recordAdjustmentLaunchUsageReceipt = (
  db: CrmEmailDatabase,
  input: Readonly<{
    adjustment: AdjutantAdjustmentRequest
    assignment: AdjutantAssignment
    mode: AdjutantAdjustmentContinuationMode
    runId: string
  }>,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<void, AdjutantUsageReceiptError> =>
  recordAdjutantUsageReceipt(
    crmEmailAuthorityDb(db),
    {
      adjustmentId: input.adjustment.id,
      assignmentId: input.assignment.id,
      billingMode: 'public_beta_free',
      category: 'adjustment',
      idempotencyKey: [
        'adjutant_usage',
        input.assignment.id,
        input.adjustment.id,
        input.runId,
        'adjustment',
      ].join(':'),
      publicDetails: {
        billingNote: 'Public beta Site adjustments are free.',
      },
      quantity: 1,
      runId: input.runId,
      siteId: input.assignment.siteId,
      softwareOrderId: input.assignment.softwareOrderId,
      summary: 'Autopilot Site adjustment was accepted for runner work.',
      teamDetails: {
        adjustmentId: input.adjustment.id,
        billingPolicy: 'public_beta_free',
        mode: input.mode,
        runId: input.runId,
        siteId: input.assignment.siteId,
        softwareOrderId: input.assignment.softwareOrderId,
      },
      unit: 'adjustment',
      visibility: input.assignment.visibility,
    },
    undefined,
    mirror,
  ).pipe(Effect.asVoid)

const operatorCheck = (
  name: string,
  status: OperatorAdjutantCheckStatus,
  message: string,
  details?: Record<string, unknown>,
): OperatorAdjutantCheck => ({
  ...(details === undefined ? {} : { details }),
  message,
  name,
  status,
})

const operatorCheckRollup = (
  checks: ReadonlyArray<OperatorAdjutantCheck>,
): OperatorAdjutantCheckStatus =>
  checks.some(check => check.status === 'blocked')
    ? 'blocked'
    : checks.some(check => check.status === 'warning')
      ? 'warning'
      : checks.some(check => check.status === 'unknown')
        ? 'unknown'
        : 'ok'

const readPreflightSoftwareOrder = (
  db: CrmEmailDatabase,
  softwareOrderId: string,
): Effect.Effect<
  PreflightSoftwareOrderRow | null,
  OperatorAdjutantPreflightError
> =>
  preflightD1Effect('operatorAdjutant.preflight.softwareOrder.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                repository_provider,
                repository_owner,
                repository_name,
                repository_default_branch
           FROM software_orders
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<PreflightSoftwareOrderRow>(),
  )

const readPreflightSite = (
  db: CrmEmailDatabase,
  siteId: string,
): Effect.Effect<PreflightSiteRow | null, OperatorAdjutantPreflightError> =>
  preflightD1Effect('operatorAdjutant.preflight.site.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                slug,
                title,
                access_mode,
                visibility,
                active_version_id,
                active_deployment_id,
                source_repository_provider,
                source_repository_owner,
                source_repository_name,
                source_repository_ref
           FROM site_projects
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(siteId)
      .first<PreflightSiteRow>(),
  )

const reviewSoftwareOrderFromRow = (
  row: ReviewSoftwareOrderRow,
): OperatorAdjutantReviewSoftwareOrder => ({
  id: row.id,
  status: row.status,
  visibility: row.visibility,
  request: row.request,
  repositoryFullName: row.repository_full_name,
  currentRunId: row.current_run_id,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const reviewSiteFromRow = (row: ReviewSiteRow): OperatorAdjutantReviewSite => ({
  id: row.id,
  slug: row.slug,
  title: row.title,
  status: row.status,
  accessMode: row.access_mode,
  visibility: row.visibility,
  activeVersionId: row.active_version_id,
  activeDeploymentId: row.active_deployment_id,
})

const reviewGoalFromRow = (row: ReviewGoalRow): OperatorAdjutantReviewGoal => ({
  id: row.id,
  agentId: row.agent_id,
  status: row.status,
  visibility: row.visibility,
  currentRunId: row.current_run_id,
  tokensUsed: row.tokens_used,
  tokenBudget: row.token_budget,
  timeUsedSeconds: row.time_used_seconds,
  updatedAt: row.updated_at,
})

const reviewRunFromRow = (row: ReviewRunRow): OperatorAdjutantReviewRun => ({
  id: row.id,
  runtime: row.runtime,
  backend: row.backend,
  status: row.status,
  eventCursor: row.event_cursor,
  externalRunId: row.external_run_id,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const reviewVersionFromRow = (
  row: ReviewVersionRow,
): OperatorAdjutantReviewVersion => ({
  id: row.id,
  sourceKind: row.source_kind,
  sourceCommitSha: row.source_commit_sha,
  buildStatus: row.build_status,
  buildCommand: row.build_command,
  workerModuleR2Key: row.worker_module_r2_key,
  createdByRunId: row.created_by_run_id,
  createdAt: row.created_at,
  savedAt: row.saved_at,
  rejectedAt: row.rejected_at,
})

const reviewDeploymentFromRow = (
  row: ReviewDeploymentRow,
): OperatorAdjutantReviewDeployment => ({
  id: row.id,
  versionId: row.version_id,
  url: row.url,
  runtimeKind: row.runtime_kind,
  status: row.status,
  externalDeploymentId: row.external_deployment_id,
  activatedAt: row.activated_at,
  disabledAt: row.disabled_at,
  rolledBackAt: row.rolled_back_at,
  updatedAt: row.updated_at,
})

const reviewAssignmentEventFromRow = (
  row: ReviewAssignmentEventRow,
): OperatorAdjutantReviewEvent => ({
  id: row.id,
  type: row.event_type,
  summary: row.summary,
  runId: row.run_id,
  createdAt: row.created_at,
})

const reviewSiteEventFromRow = (
  row: ReviewSiteEventRow,
): OperatorAdjutantReviewEvent => ({
  id: row.id,
  type: row.type,
  summary: row.summary,
  runId: row.actor_run_id,
  createdAt: row.created_at,
})

const reviewAdjustmentFromRow = (
  row: ReviewAdjustmentRow,
): OperatorAdjutantReviewAdjustment => ({
  id: row.id,
  instruction: row.instruction,
  status: row.status,
  continuationMode: row.continuation_mode,
  sourceRunId: row.source_run_id,
  continuationRunId: row.continuation_run_id,
  resultingVersionId: row.resulting_version_id,
  requestedByUserId: row.requested_by_user_id,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const readReviewSoftwareOrder = (
  db: CrmEmailDatabase,
  softwareOrderId: string,
): Effect.Effect<
  OperatorAdjutantReviewSoftwareOrder | null,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.review.softwareOrder.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                status,
                visibility,
                request,
                repository_full_name,
                current_run_id,
                created_at,
                updated_at
           FROM software_orders
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<ReviewSoftwareOrderRow>(),
  ).pipe(
    Effect.map(row => (row === null ? null : reviewSoftwareOrderFromRow(row))),
  )

const readReviewSite = (
  db: CrmEmailDatabase,
  siteId: string,
): Effect.Effect<
  OperatorAdjutantReviewSite | null,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.review.site.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                slug,
                title,
                status,
                access_mode,
                visibility,
                active_version_id,
                active_deployment_id
           FROM site_projects
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(siteId)
      .first<ReviewSiteRow>(),
  ).pipe(Effect.map(row => (row === null ? null : reviewSiteFromRow(row))))

const readReviewGoal = (
  db: CrmEmailDatabase,
  goalId: string,
): Effect.Effect<
  OperatorAdjutantReviewGoal | null,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.review.goal.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                agent_id,
                status,
                visibility,
                current_run_id,
                tokens_used,
                token_budget,
                time_used_seconds,
                updated_at
           FROM agent_goals
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(goalId)
      .first<ReviewGoalRow>(),
  ).pipe(Effect.map(row => (row === null ? null : reviewGoalFromRow(row))))

const readReviewRun = (
  db: CrmEmailDatabase,
  runId: string,
): Effect.Effect<
  OperatorAdjutantReviewRun | null,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.review.run.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                runtime,
                backend,
                status,
                event_cursor,
                external_run_id,
                created_at,
                updated_at
           FROM agent_runs
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(runId)
      .first<ReviewRunRow>(),
  ).pipe(Effect.map(row => (row === null ? null : reviewRunFromRow(row))))

const listReviewVersions = (
  db: CrmEmailDatabase,
  siteId: string,
): Effect.Effect<
  ReadonlyArray<OperatorAdjutantReviewVersion>,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.review.versions.list', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                source_kind,
                source_commit_sha,
                build_status,
                build_command,
                worker_module_r2_key,
                created_by_run_id,
                created_at,
                saved_at,
                rejected_at
           FROM site_versions
          WHERE site_id = ?
          ORDER BY created_at DESC
          LIMIT 20`,
      )
      .bind(siteId)
      .all<ReviewVersionRow>(),
  ).pipe(Effect.map(result => result.results.map(reviewVersionFromRow)))

const listReviewDeployments = (
  db: CrmEmailDatabase,
  siteId: string,
): Effect.Effect<
  ReadonlyArray<OperatorAdjutantReviewDeployment>,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.review.deployments.list', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                version_id,
                url,
                runtime_kind,
                status,
                external_deployment_id,
                activated_at,
                disabled_at,
                rolled_back_at,
                updated_at
           FROM site_deployments
          WHERE site_id = ?
          ORDER BY updated_at DESC
          LIMIT 20`,
      )
      .bind(siteId)
      .all<ReviewDeploymentRow>(),
  ).pipe(Effect.map(result => result.results.map(reviewDeploymentFromRow)))

const listReviewAssignmentEvents = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<
  ReadonlyArray<OperatorAdjutantReviewEvent>,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.review.assignmentEvents.list', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                event_type,
                summary,
                run_id,
                created_at
           FROM adjutant_assignment_events
          WHERE assignment_id = ?
          ORDER BY created_at DESC
          LIMIT 20`,
      )
      .bind(assignmentId)
      .all<ReviewAssignmentEventRow>(),
  ).pipe(Effect.map(result => result.results.map(reviewAssignmentEventFromRow)))

const listReviewSiteEvents = (
  db: CrmEmailDatabase,
  siteId: string,
): Effect.Effect<
  ReadonlyArray<OperatorAdjutantReviewEvent>,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.review.siteEvents.list', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                type,
                summary,
                actor_run_id,
                created_at
           FROM site_events
          WHERE site_id = ?
          ORDER BY created_at DESC
          LIMIT 20`,
      )
      .bind(siteId)
      .all<ReviewSiteEventRow>(),
  ).pipe(Effect.map(result => result.results.map(reviewSiteEventFromRow)))

const listReviewAdjustments = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<
  ReadonlyArray<OperatorAdjutantReviewAdjustment>,
  OperatorAdjutantStorageError
> =>
  storageD1Effect('operatorAdjutant.review.adjustments.list', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                instruction,
                status,
                continuation_mode,
                source_run_id,
                continuation_run_id,
                resulting_version_id,
                requested_by_user_id,
                created_at,
                updated_at
           FROM adjutant_adjustment_requests
          WHERE assignment_id = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT 20`,
      )
      .bind(assignmentId)
      .all<ReviewAdjustmentRow>(),
  ).pipe(Effect.map(result => result.results.map(reviewAdjustmentFromRow)))

const listReviewUsageReceipts = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<
  ReadonlyArray<AdjutantUsageReceipt>,
  OperatorAdjutantStorageError
> =>
  listAdjutantUsageReceiptsForAssignment(crmEmailAuthorityDb(db), assignmentId, 50).pipe(
    Effect.mapError(
      error =>
        new OperatorAdjutantStorageError({
          error,
          operation: 'operatorAdjutant.review.usageReceipts.list',
        }),
    ),
  )

const reviewNextAction = (
  assignment: AdjutantAssignment,
  adjustments: ReadonlyArray<OperatorAdjutantReviewAdjustment>,
  site: OperatorAdjutantReviewSite | null,
  versions: ReadonlyArray<OperatorAdjutantReviewVersion>,
  deployments: ReadonlyArray<OperatorAdjutantReviewDeployment>,
): string => {
  const activeAdjustment = adjustments.find(
    adjustment =>
      adjustment.status === 'requested' ||
      adjustment.status === 'queued' ||
      adjustment.status === 'running',
  )

  if (activeAdjustment !== undefined) {
    return 'Wait for Autopilot to save the adjusted Site version for review.'
  }

  if (assignment.status === 'draft') {
    return 'Generate the tracked task packet, run preflight, then launch Autopilot.'
  }

  if (assignment.currentRunId !== null && versions.length === 0) {
    return 'Wait for the runner artifact receipt or inspect the current run.'
  }

  const savedVersion = versions.find(version => version.buildStatus === 'saved')

  if (savedVersion !== undefined && site?.activeVersionId !== savedVersion.id) {
    return 'Review the saved generated version and deploy it after the launch checklist.'
  }

  const activeDeployment = deployments.find(
    deployment => deployment.status === 'active',
  )

  if (activeDeployment !== undefined) {
    return 'Monitor the active deployment or disable/rollback if the release is unsafe.'
  }

  if (versions.length > 0) {
    return 'Review build results and request an adjustment if the saved version is not deployable.'
  }

  return 'Continue Autopilot fulfillment from the assignment controls.'
}

const readAssignmentReview = (
  db: CrmEmailDatabase,
  assignment: AdjutantAssignment,
  exaConfigured: boolean,
): Effect.Effect<
  OperatorAdjutantAssignmentReview,
  OperatorAdjutantStorageError
> =>
  Effect.gen(function* () {
    const order =
      assignment.softwareOrderId === null
        ? null
        : yield* readReviewSoftwareOrder(db, assignment.softwareOrderId)
    const site =
      assignment.siteId === null
        ? null
        : yield* readReviewSite(db, assignment.siteId)
    const goal =
      assignment.goalId === null
        ? null
        : yield* readReviewGoal(db, assignment.goalId)
    const runId =
      assignment.currentRunId ??
      goal?.currentRunId ??
      order?.currentRunId ??
      null
    const currentRun = runId === null ? null : yield* readReviewRun(db, runId)
    const versions =
      assignment.siteId === null
        ? []
        : yield* listReviewVersions(db, assignment.siteId)
    const deployments =
      assignment.siteId === null
        ? []
        : yield* listReviewDeployments(db, assignment.siteId)
    const assignmentEvents = yield* listReviewAssignmentEvents(
      db,
      assignment.id,
    )
    const siteEvents =
      assignment.siteId === null
        ? []
        : yield* listReviewSiteEvents(db, assignment.siteId)
    const adjustments = yield* listReviewAdjustments(db, assignment.id)
    const usageReceipts = yield* listReviewUsageReceipts(db, assignment.id)
    const researchPolicy = yield* effectiveResearchPolicyForReview(
      db,
      assignment,
    )
    const researchBrief = yield* latestResearchBriefForReview(db, assignment.id)
    const latestApprovedBrief = yield* latestApprovedResearchBrief(
      db,
      assignment.id,
    )
    const taskPacketFreshness = yield* makeAdjutantTaskPacketFreshnessService(
      crmEmailAuthorityDb(db),
    )
      .readFreshness(assignment, latestApprovedBrief)
      .pipe(
        Effect.mapError(
          error =>
            new OperatorAdjutantStorageError({
              error,
              operation: 'operatorAdjutant.review.taskPacketFreshness.read',
            }),
        ),
      )
    const enrichment = yield* readEnrichmentReview(
      db,
      assignment.id,
      exaConfigured,
    ).pipe(
      Effect.mapError(
        error =>
          new OperatorAdjutantStorageError({
            error,
            operation: 'operatorAdjutant.review.enrichment.read',
          }),
      ),
    )

    return {
      adjustments,
      assignmentEvents,
      currentRun,
      deployments,
      enrichment,
      goal,
      nextAction: reviewNextAction(
        assignment,
        adjustments,
        site,
        versions,
        deployments,
      ),
      order,
      researchPolicy,
      researchBrief: researchBriefReviewSummary(researchBrief),
      site,
      siteEvents,
      taskPacketFreshness,
      usageReceipts,
      usageSummary: summarizeAdjutantUsageReceipts(usageReceipts),
      versions,
    }
  })

const launchChecklistComplete = (
  checklist: PreflightOperatorAdjutantAssignmentRequest['launchChecklist'],
): boolean =>
  checklist?.sourceReviewed === true &&
  checklist.buildReviewed === true &&
  checklist.audienceReviewed === true &&
  checklist.secretsReviewed === true &&
  checklist.urlReviewed === true

const sourceRepositoryDetails = (
  order: PreflightSoftwareOrderRow | null,
  site: PreflightSiteRow | null,
): Record<string, unknown> | null => {
  if (
    site?.source_repository_provider === 'github' &&
    site.source_repository_owner !== null &&
    site.source_repository_name !== null
  ) {
    return {
      provider: site.source_repository_provider,
      owner: site.source_repository_owner,
      name: site.source_repository_name,
      ref: site.source_repository_ref,
      source: 'site',
    }
  }

  if (
    order?.repository_provider === 'github' &&
    order.repository_owner !== null &&
    order.repository_name !== null
  ) {
    return {
      provider: order.repository_provider,
      owner: order.repository_owner,
      name: order.repository_name,
      ref: order.repository_default_branch,
      source: 'software_order',
    }
  }

  return null
}

const sourceRepositorySelector = (
  order: PreflightSoftwareOrderRow | null,
  site: PreflightSiteRow | null,
): Readonly<{ name: string; owner: string; ref: string }> => {
  if (
    site?.source_repository_provider === 'github' &&
    site.source_repository_owner !== null &&
    site.source_repository_name !== null
  ) {
    return {
      name: site.source_repository_name,
      owner: site.source_repository_owner,
      ref: site.source_repository_ref ?? 'main',
    }
  }

  if (
    order?.repository_provider === 'github' &&
    order.repository_owner !== null &&
    order.repository_name !== null
  ) {
    return {
      name: order.repository_name,
      owner: order.repository_owner,
      ref: order.repository_default_branch ?? 'main',
    }
  }

  return {
    name: 'autopilot-omega',
    owner: 'OpenAgentsInc',
    ref: 'main',
  }
}

const siteTargetUrl = (site: PreflightSiteRow | null): string =>
  site === null
    ? 'pending Site URL'
    : `https://sites.openagents.com/${site.slug}`

const launchPayloadRecord = (
  payload: Record<string, unknown>,
  key: string,
): Record<string, unknown> | undefined => {
  const value = payload[key]

  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined
}

const launchPayloadStatus = (
  payload: Record<string, unknown>,
): string | undefined => {
  const missionStatus = launchPayloadRecord(payload, 'mission')?.status
  const runStatus = launchPayloadRecord(payload, 'run')?.status

  return typeof missionStatus === 'string'
    ? missionStatus
    : typeof runStatus === 'string'
      ? runStatus
      : undefined
}

const preflightSummary = (
  checks: ReadonlyArray<OperatorAdjutantCheck>,
): string => checks.map(check => `${check.name}: ${check.status}`).join(', ')

const compactOperatorChecksForEvent = (
  checks: ReadonlyArray<OperatorAdjutantCheck>,
): ReadonlyArray<Readonly<{ name: string; status: string }>> =>
  checks.map(check => ({
    name: check.name,
    status: check.status,
  }))

const launchSelectorForAssignment = (
  assignment: AdjutantAssignment,
  order: PreflightSoftwareOrderRow | null,
  site: PreflightSiteRow | null,
  researchBrief: AdjutantResearchBrief | null,
  checks: ReadonlyArray<OperatorAdjutantCheck>,
  body: LaunchOperatorAdjutantAssignmentRequest,
): Record<string, unknown> => {
  const repository = sourceRepositorySelector(order, site)
  const orderId = assignment.softwareOrderId ?? 'none'
  const siteId = assignment.siteId ?? 'none'
  const taskSpecPath = assignment.taskSpecPath ?? 'none'
  const commitSha = assignment.commitSha ?? 'none'
  const targetUrl = siteTargetUrl(site)
  const dispatchGoal = [
    `Autopilot assignment ${assignment.id}`,
    `Assignment kind: ${assignment.assignmentKind}`,
    `Software order ID: ${orderId}`,
    `Site ID: ${siteId}`,
    `Goal ID: ${assignment.goalId ?? 'none'}`,
    `Task packet: ${taskSpecPath}`,
    `Task packet commit SHA: ${commitSha}`,
    `Research brief ID: ${researchBrief?.id ?? 'none'}`,
    `Target URL: ${targetUrl}`,
    `Operator preflight: ${preflightSummary(checks)}`,
    '',
    assignment.objective,
  ].join('\n')

  return {
    baseRef: repository.ref,
    branchName: `adjutant/${assignment.id}`,
    commitMessage: `Complete Autopilot assignment ${assignment.id}`,
    dispatchGoal,
    goal: assignment.objective,
    ...(assignment.goalId === null ? {} : { goalId: assignment.goalId }),
    ...(researchBrief === null
      ? {}
      : {
          researchBrief: {
            approvedAt: researchBrief.approvedAt,
            id: researchBrief.id,
            sourceCount: researchBrief.sourceCards.length,
            status: researchBrief.status,
            summary: researchBrief.summary,
          },
          researchBriefId: researchBrief.id,
        }),
    openPullRequest: true,
    prompt: assignment.objective,
    pullRequestBody: [
      `Autopilot assignment: ${assignment.id}`,
      `Software order: ${orderId}`,
      `Site: ${siteId}`,
      'Specification: linked in the assignment metadata',
      `Specification commit: ${commitSha}`,
      `Research brief: ${researchBrief?.id ?? 'none'}`,
      `Target URL: ${targetUrl}`,
    ].join('\n'),
    pullRequestTitle:
      site === null
        ? `Autopilot ${assignment.assignmentKind} for ${orderId}`
        : `Autopilot ${assignment.assignmentKind} for ${site.title}`,
    repository: `${repository.owner}/${repository.name}@${repository.ref}`,
    repositoryRef: repository.ref,
    ...(assignment.projectId === null
      ? {}
      : { projectId: assignment.projectId }),
    ...(assignment.teamId === null ? {} : { teamId: assignment.teamId }),
    ...(body.providerAccountId === undefined
      ? {}
      : { providerAccountId: body.providerAccountId }),
    ...(body.providerAccountRef === undefined
      ? {}
      : { providerAccountRef: body.providerAccountRef }),
    ...(body.runnerBackend === undefined
      ? {}
      : { runnerBackend: body.runnerBackend }),
    ...(body.timeoutMs === undefined ? {} : { timeoutMs: body.timeoutMs }),
  }
}

const continuableRunStatuses = new Set([
  'queued',
  'running',
  'waiting_for_input',
])

const terminalRunStatuses = new Set(['completed', 'failed', 'canceled'])

const canContinueCurrentRun = (
  run: OperatorAdjutantReviewRun | null,
): run is OperatorAdjutantReviewRun =>
  run !== null && continuableRunStatuses.has(run.status)

const adjustmentPrompt = (
  assignment: AdjutantAssignment,
  adjustment: AdjutantAdjustmentRequest,
  site: PreflightSiteRow,
): string =>
  [
    `Autopilot adjustment request ${adjustment.id}`,
    `Assignment ID: ${assignment.id}`,
    `Software order ID: ${assignment.softwareOrderId ?? 'none'}`,
    `Site ID: ${site.id}`,
    `Goal ID: ${assignment.goalId ?? 'none'}`,
    `Current public URL: ${siteTargetUrl(site)}`,
    '',
    'Apply the requested Site adjustment through the existing Sites version lifecycle.',
    'Save the adjusted output as a new Site version for operator review before deployment.',
    'Keep the same Autopilot assignment and durable goal identity.',
    '',
    'Requested adjustment:',
    adjustment.instruction,
  ].join('\n')

const adjustmentLaunchSelectorForAssignment = (
  assignment: AdjutantAssignment,
  order: PreflightSoftwareOrderRow | null,
  site: PreflightSiteRow,
  adjustment: AdjutantAdjustmentRequest,
  body: CreateOperatorAdjutantAdjustmentRequest,
): Record<string, unknown> => {
  const repository = sourceRepositorySelector(order, site)
  const prompt = adjustmentPrompt(assignment, adjustment, site)
  const branchName = `adjutant/${assignment.id}/adjustment-${adjustment.id}`

  return {
    baseRef: repository.ref,
    branchName,
    commitMessage: `Apply Autopilot adjustment ${adjustment.id}`,
    dispatchGoal: prompt,
    goal: assignment.objective,
    ...(assignment.goalId === null ? {} : { goalId: assignment.goalId }),
    openPullRequest: true,
    prompt,
    pullRequestBody: [
      `Autopilot assignment: ${assignment.id}`,
      `Autopilot adjustment: ${adjustment.id}`,
      `Software order: ${assignment.softwareOrderId ?? 'none'}`,
      `Site: ${site.id}`,
      'Specification: linked in the assignment metadata',
      `Target URL: ${siteTargetUrl(site)}`,
    ].join('\n'),
    pullRequestTitle: `Autopilot adjustment for ${site.title}`,
    repository: `${repository.owner}/${repository.name}@${repository.ref}`,
    repositoryRef: repository.ref,
    ...(assignment.projectId === null
      ? {}
      : { projectId: assignment.projectId }),
    ...(assignment.teamId === null ? {} : { teamId: assignment.teamId }),
    adjutantAdjustmentId: adjustment.id,
    continuationOfRunId: adjustment.sourceRunId,
    ...(body.providerAccountId === undefined
      ? {}
      : { providerAccountId: body.providerAccountId }),
    ...(body.providerAccountRef === undefined
      ? {}
      : { providerAccountRef: body.providerAccountRef }),
    ...(body.runnerBackend === undefined
      ? {}
      : { runnerBackend: body.runnerBackend }),
    ...(body.timeoutMs === undefined ? {} : { timeoutMs: body.timeoutMs }),
  }
}

const siteRequiredForAssignment = (assignment: AdjutantAssignment): boolean =>
  assignment.assignmentKind === 'site_adjustment' ||
  assignment.assignmentKind === 'site_review' ||
  assignment.assignmentKind === 'site_deployment'

const publicLaunchChecklistRequired = (
  assignment: AdjutantAssignment,
  site: PreflightSiteRow | null,
): boolean =>
  assignment.assignmentKind === 'site_deployment' &&
  site !== null &&
  (site.access_mode === 'public' || site.visibility === 'public')

const researchGateStatus = (
  researchPolicy: AdjutantResearchPolicy,
  researchBrief: AdjutantResearchBrief | null,
  latestApprovedBrief: AdjutantResearchBrief | null,
  latestEnrichmentRun: ExaEnrichmentRun | null,
  latestEnrichmentJob: AdjutantEnrichmentJob | null,
): OperatorAdjutantCheck => {
  if (researchPolicy.effectiveMode === 'research_bypassed_by_operator') {
    return researchPolicy.actorUserId === null ||
      researchPolicy.reason === null ||
      researchPolicy.reason.trim() === '' ||
      researchPolicy.updatedAt.trim() === ''
      ? operatorCheck(
          'research_required_gate',
          'blocked',
          'Research bypass is missing its operator receipt fields.',
          {
            customerSafeStatus: researchPolicy.customerSafeStatus,
            effectiveMode: researchPolicy.effectiveMode,
            source: researchPolicy.source,
          },
        )
      : operatorCheck(
          'research_required_gate',
          'ok',
          'Required research was explicitly bypassed by an operator receipt.',
          {
            actorUserId: researchPolicy.actorUserId,
            customerSafeStatus: researchPolicy.customerSafeStatus,
            effectiveMode: researchPolicy.effectiveMode,
            source: researchPolicy.source,
            sourceAuthorityRef: researchPolicy.sourceAuthorityRef,
            updatedAt: researchPolicy.updatedAt,
          },
        )
  }

  if (researchPolicy.effectiveMode !== 'research_required') {
    return operatorCheck(
      'research_required_gate',
      'ok',
      'Research is not required before this assignment launches.',
      {
        customerSafeStatus: researchPolicy.customerSafeStatus,
        effectiveMode: researchPolicy.effectiveMode,
      },
    )
  }

  if (latestApprovedBrief !== null) {
    return operatorCheck(
      'research_required_gate',
      'ok',
      'Required research is approved for launch.',
      {
        approvedAt: latestApprovedBrief.approvedAt,
        researchBriefId: latestApprovedBrief.id,
        sourceCount: latestApprovedBrief.sourceCards.length,
      },
    )
  }

  if (
    latestEnrichmentJob !== null &&
    (latestEnrichmentJob.status === 'queued' ||
      latestEnrichmentJob.status === 'running')
  ) {
    return operatorCheck(
      'research_required_gate',
      'blocked',
      'Required research is still running before launch.',
      {
        enrichmentJobId: latestEnrichmentJob.id,
        enrichmentJobStatus: latestEnrichmentJob.status,
        nextAction:
          'Wait for the enrichment job to finish, then review and approve the research brief.',
      },
    )
  }

  if (
    latestEnrichmentJob !== null &&
    (latestEnrichmentJob.status === 'failed' ||
      latestEnrichmentJob.status === 'canceled' ||
      latestEnrichmentJob.status === 'skipped')
  ) {
    return operatorCheck(
      'research_required_gate',
      'blocked',
      'Required research is unavailable and must be refreshed before launch.',
      {
        enrichmentJobId: latestEnrichmentJob.id,
        enrichmentJobStatus: latestEnrichmentJob.status,
        redactedReason: latestEnrichmentJob.errorCode ?? 'research_unavailable',
        nextAction: 'Refresh required research and review the new brief.',
      },
    )
  }

  if (
    latestEnrichmentRun !== null &&
    (latestEnrichmentRun.status === 'planned' ||
      latestEnrichmentRun.status === 'queued' ||
      latestEnrichmentRun.status === 'running')
  ) {
    return operatorCheck(
      'research_required_gate',
      'blocked',
      'Required research is still active before launch.',
      {
        enrichmentRunId: latestEnrichmentRun.id,
        enrichmentRunStatus: latestEnrichmentRun.status,
        nextAction:
          'Wait for the enrichment run to finish, then review and approve the research brief.',
      },
    )
  }

  if (
    latestEnrichmentRun !== null &&
    (latestEnrichmentRun.status === 'failed' ||
      latestEnrichmentRun.status === 'rejected' ||
      latestEnrichmentRun.status === 'stale')
  ) {
    return operatorCheck(
      'research_required_gate',
      'blocked',
      'Required research is not launch-ready and must be refreshed.',
      {
        enrichmentRunId: latestEnrichmentRun.id,
        enrichmentRunStatus: latestEnrichmentRun.status,
        nextAction: 'Refresh required research and review the new brief.',
      },
    )
  }

  if (researchBrief !== null) {
    return operatorCheck(
      'research_required_gate',
      'blocked',
      'Required research brief must be approved before launch.',
      {
        nextAction: 'Review and approve the research brief or record a bypass.',
        researchBriefId: researchBrief.id,
        researchBriefStatus: researchBrief.status,
      },
    )
  }

  return operatorCheck(
    'research_required_gate',
    'blocked',
    'Required research has not been started for this assignment.',
    {
      nextAction:
        'Queue Exa enrichment, review source cards, and approve the research brief.',
    },
  )
}

const assignmentSourceChecks = (
  db: CrmEmailDatabase,
  assignment: AdjutantAssignment,
  body: PreflightOperatorAdjutantAssignmentRequest,
  exaConfigured: boolean,
): Effect.Effect<
  ReadonlyArray<OperatorAdjutantCheck>,
  OperatorAdjutantPreflightError
> =>
  Effect.gen(function* () {
    const order =
      assignment.softwareOrderId === null
        ? null
        : yield* readPreflightSoftwareOrder(db, assignment.softwareOrderId)
    const site =
      assignment.siteId === null
        ? null
        : yield* readPreflightSite(db, assignment.siteId)
    const sourceRepository = sourceRepositoryDetails(order, site)
    const researchBrief = yield* latestResearchBriefForPreflight(
      db,
      assignment.id,
    )
    const latestApprovedBrief = yield* latestApprovedResearchBrief(
      db,
      assignment.id,
    ).pipe(
      Effect.mapError(
        error =>
          new OperatorAdjutantPreflightError({
            error,
            operation: 'operatorAdjutant.preflight.approvedResearchBrief.read',
          }),
      ),
    )
    const taskPacketFreshness = yield* makeAdjutantTaskPacketFreshnessService(
      crmEmailAuthorityDb(db),
    )
      .readFreshness(assignment, latestApprovedBrief)
      .pipe(
        Effect.mapError(
          error =>
            new OperatorAdjutantPreflightError({
              error,
              operation: 'operatorAdjutant.preflight.taskPacketFreshness.read',
            }),
        ),
      )
    const latestEnrichmentRun = yield* latestEnrichmentRunForPreflight(
      db,
      assignment.id,
    )
    const latestEnrichmentJob = yield* latestEnrichmentJobForPreflight(
      db,
      assignment.id,
    )
    const researchPolicy = yield* effectiveResearchPolicyForPreflight(
      db,
      assignment,
    )
    const siteRequired = siteRequiredForAssignment(assignment)
    const requiresLaunchChecklist = publicLaunchChecklistRequired(
      assignment,
      site,
    )
    const checks: Array<OperatorAdjutantCheck> = []

    checks.push(
      assignment.goalId === null
        ? operatorCheck(
            'adjutant_goal',
            'blocked',
            'Assignment is not linked to a durable Autopilot goal.',
          )
        : operatorCheck(
            'adjutant_goal',
            'ok',
            'Assignment is linked to a durable Autopilot goal.',
            { goalId: assignment.goalId },
          ),
    )

    checks.push(
      assignment.status === 'complete' || assignment.status === 'canceled'
        ? operatorCheck(
            'assignment_state',
            'blocked',
            'Completed or canceled assignments cannot be launched.',
            { status: assignment.status },
          )
        : operatorCheck(
            'assignment_state',
            'ok',
            'Assignment is eligible for preflight.',
            { status: assignment.status },
          ),
    )

    checks.push(
      assignment.currentRunId === null
        ? operatorCheck(
            'current_run',
            'ok',
            'Assignment is not linked to an existing run.',
          )
        : operatorCheck(
            'current_run',
            'blocked',
            'Assignment is already linked to an Autopilot run.',
            { currentRunId: assignment.currentRunId },
          ),
    )

    checks.push(
      assignment.softwareOrderId === null
        ? operatorCheck(
            'software_order',
            'warning',
            'Assignment is not linked to a software order.',
          )
        : order === null
          ? operatorCheck(
              'software_order',
              'blocked',
              'Linked software order was not found or is archived.',
              { softwareOrderId: assignment.softwareOrderId },
            )
          : operatorCheck(
              'software_order',
              'ok',
              'Linked software order exists.',
              { softwareOrderId: order.id },
            ),
    )

    checks.push(
      assignment.siteId === null
        ? siteRequired
          ? operatorCheck(
              'site_project',
              'blocked',
              'This assignment kind requires a linked Site.',
              { assignmentKind: assignment.assignmentKind },
            )
          : operatorCheck(
              'site_project',
              'warning',
              'No Site is linked yet; order-level work can continue.',
            )
        : site === null
          ? operatorCheck(
              'site_project',
              'blocked',
              'Linked Site was not found or is archived.',
              { siteId: assignment.siteId },
            )
          : operatorCheck('site_project', 'ok', 'Linked Site exists.', {
              accessMode: site.access_mode,
              siteId: site.id,
              visibility: site.visibility,
            }),
    )

    checks.push(
      sourceRepository === null
        ? operatorCheck(
            'source_repository',
            'warning',
            'No source repository is linked to the order or Site.',
          )
        : operatorCheck(
            'source_repository',
            'ok',
            'Source repository context is available.',
            sourceRepository,
          ),
    )

    checks.push(
      researchPolicy.effectiveMode === 'research_required'
        ? operatorCheck(
            'research_policy',
            'ok',
            'This assignment requires approved public-source research before launch.',
            {
              customerSafeStatus: researchPolicy.customerSafeStatus,
              defaultMode: researchPolicy.defaultMode,
              effectiveMode: researchPolicy.effectiveMode,
              source: researchPolicy.source,
            },
          )
        : researchPolicy.effectiveMode === 'research_bypassed_by_operator'
          ? operatorCheck(
              'research_policy',
              'ok',
              'Research requirement was bypassed by an operator policy record.',
              {
                customerSafeStatus: researchPolicy.customerSafeStatus,
                effectiveMode: researchPolicy.effectiveMode,
                source: researchPolicy.source,
                sourceAuthorityRef: researchPolicy.sourceAuthorityRef,
              },
            )
          : operatorCheck(
              'research_policy',
              'ok',
              'Research policy does not require approved research before this assignment can proceed.',
              {
                customerSafeStatus: researchPolicy.customerSafeStatus,
                defaultMode: researchPolicy.defaultMode,
                effectiveMode: researchPolicy.effectiveMode,
                source: researchPolicy.source,
              },
            ),
    )

    checks.push(
      researchGateStatus(
        researchPolicy,
        researchBrief,
        latestApprovedBrief,
        latestEnrichmentRun,
        latestEnrichmentJob,
      ),
    )

    checks.push(
      !exaConfigured
        ? researchPolicy.effectiveMode === 'research_required' &&
          latestApprovedBrief === null
          ? operatorCheck(
              'exa_enrichment',
              'blocked',
              'Exa is not configured and required research cannot be refreshed.',
              {
                nextAction:
                  'Configure Exa or record an explicit operator research bypass.',
              },
            )
          : operatorCheck(
              'exa_enrichment',
              'warning',
              'Exa is not configured; enrichment cannot be refreshed.',
            )
        : latestEnrichmentRun === null
          ? operatorCheck(
              'exa_enrichment',
              researchPolicy.effectiveMode === 'research_required' &&
                latestApprovedBrief === null
                ? 'blocked'
                : 'warning',
              'No Exa enrichment run is recorded for this assignment.',
            )
          : latestEnrichmentRun.status === 'failed' ||
              latestEnrichmentRun.status === 'rejected' ||
              latestEnrichmentRun.status === 'stale'
            ? operatorCheck(
                'exa_enrichment',
                researchPolicy.effectiveMode === 'research_required' &&
                  latestApprovedBrief === null
                  ? 'blocked'
                  : 'warning',
                'Latest Exa enrichment run is not launch-ready.',
                {
                  enrichmentRunId: latestEnrichmentRun.id,
                  status: latestEnrichmentRun.status,
                },
              )
            : latestEnrichmentRun.status === 'queued' ||
                latestEnrichmentRun.status === 'running' ||
                latestEnrichmentRun.status === 'planned'
              ? operatorCheck(
                  'exa_enrichment',
                  researchPolicy.effectiveMode === 'research_required'
                    ? 'blocked'
                    : 'warning',
                  'Latest Exa enrichment run is still active.',
                  {
                    enrichmentRunId: latestEnrichmentRun.id,
                    status: latestEnrichmentRun.status,
                  },
                )
              : operatorCheck(
                  'exa_enrichment',
                  'ok',
                  'Latest Exa enrichment run is available.',
                  {
                    enrichmentRunId: latestEnrichmentRun.id,
                    status: latestEnrichmentRun.status,
                  },
                ),
    )

    checks.push(
      researchBrief === null
        ? operatorCheck(
            'research_brief',
            'warning',
            'No Autopilot research brief is attached yet.',
          )
        : operatorCheck(
            'research_brief',
            'ok',
            'Autopilot research brief is available.',
            {
              researchBriefId: researchBrief.id,
              status: researchBrief.status,
            },
          ),
    )

    checks.push(
      researchBrief === null
        ? operatorCheck(
            'research_review',
            'warning',
            'Research brief has not been reviewed for launch.',
          )
        : researchBrief.status === 'approved'
          ? operatorCheck(
              'research_review',
              'ok',
              'Research brief is approved for task packet context.',
              {
                approvedAt: researchBrief.approvedAt,
                researchBriefId: researchBrief.id,
                sourceCount: researchBrief.sourceCards.length,
              },
            )
          : operatorCheck(
              'research_review',
              'warning',
              'Research brief is not approved for task packet context.',
              {
                researchBriefId: researchBrief.id,
                status: researchBrief.status,
              },
            ),
    )

    checks.push(
      assignment.taskSpecPath === null
        ? operatorCheck(
            'task_packet',
            'blocked',
            'Assignment does not yet reference a tracked task packet.',
          )
        : operatorCheck(
            'task_packet',
            'ok',
            'Assignment references a tracked task packet.',
            { taskSpecPath: assignment.taskSpecPath },
          ),
    )

    checks.push(
      taskPacketFreshness.status === 'missing'
        ? operatorCheck(
            'task_packet_freshness',
            'blocked',
            'No task packet freshness record is available because no packet is linked.',
          )
        : taskPacketFreshness.status === 'stale'
          ? operatorCheck(
              'task_packet_freshness',
              'warning',
              'Task packet was generated before the latest approved research.',
              {
                latestApprovedResearchBriefId:
                  taskPacketFreshness.latestApprovedResearchBriefId,
                researchBriefId: taskPacketFreshness.researchBriefId,
                taskSpecPath: taskPacketFreshness.taskSpecPath,
              },
            )
          : taskPacketFreshness.status === 'kept_current'
            ? operatorCheck(
                'task_packet_freshness',
                'ok',
                'Operator kept the current task packet after reviewing approved research.',
                {
                  latestApprovedResearchBriefId:
                    taskPacketFreshness.latestApprovedResearchBriefId,
                  taskSpecPath: taskPacketFreshness.taskSpecPath,
                },
              )
            : operatorCheck(
                'task_packet_freshness',
                'ok',
                'Task packet is current for the latest approved research context.',
                {
                  researchBriefId: taskPacketFreshness.researchBriefId,
                  taskSpecPath: taskPacketFreshness.taskSpecPath,
                },
              ),
    )

    checks.push(
      assignment.commitSha === null
        ? operatorCheck(
            'commit_sha',
            'blocked',
            'Assignment does not yet record the pushed commit SHA for its packet.',
          )
        : operatorCheck(
            'commit_sha',
            'ok',
            'Assignment records a pushed commit SHA.',
            { commitSha: assignment.commitSha },
          ),
    )

    checks.push(
      requiresLaunchChecklist && !launchChecklistComplete(body.launchChecklist)
        ? operatorCheck(
            'sites_launch_checklist',
            'blocked',
            'Public Site deployment work requires the Sites launch checklist.',
            { siteId: assignment.siteId },
          )
        : operatorCheck(
            'sites_launch_checklist',
            'ok',
            requiresLaunchChecklist
              ? 'Public Site launch checklist is complete.'
              : 'Sites launch checklist is not required for this assignment.',
            { required: requiresLaunchChecklist },
          ),
    )

    checks.push(
      site === null
        ? operatorCheck(
            'active_deployment_state',
            'unknown',
            'No Site deployment state is available.',
          )
        : site.active_deployment_id === null
          ? operatorCheck(
              'active_deployment_state',
              'warning',
              'Site has no active deployment yet.',
              {
                activeDeploymentId: null,
                activeVersionId: site.active_version_id,
              },
            )
          : operatorCheck(
              'active_deployment_state',
              'ok',
              'Site active deployment state is available.',
              {
                activeDeploymentId: site.active_deployment_id,
                activeVersionId: site.active_version_id,
              },
            ),
    )

    return checks
  })

const adjutantNextSafeAction = (
  checks: ReadonlyArray<OperatorAdjutantCheck>,
  autopilotNextSafeAction: string,
): string => {
  const blocked = checks.find(check => check.status === 'blocked')

  if (blocked !== undefined) {
    return `Resolve ${blocked.name}: ${blocked.message}`
  }

  return autopilotNextSafeAction
}

const defaultOrderObjective = (softwareOrderId: string): string =>
  `Build and supervise Site fulfillment for software order ${softwareOrderId}.`

const defaultSiteObjective = (siteId: string): string =>
  `Build and supervise the Site fulfillment for ${siteId}.`

const assignmentInput = (
  session: OperatorAdjutantSession,
  source:
    | Readonly<{ kind: 'order'; softwareOrderId: string }>
    | Readonly<{ kind: 'site'; siteId: string }>,
  body: CreateOperatorAdjutantAssignmentRequest,
): Effect.Effect<
  CreateAdjutantAssignmentInput,
  OperatorAdjutantBadRequest | OperatorAdjutantInvalidVisibility
> =>
  Effect.gen(function* () {
    let assignmentKind: AdjutantAssignmentKind = 'site_generation'

    if (body.assignmentKind !== undefined) {
      if (!assignmentKinds.has(body.assignmentKind as AdjutantAssignmentKind)) {
        return yield* new OperatorAdjutantBadRequest({
          reason: 'invalid assignment kind',
        })
      }

      assignmentKind = body.assignmentKind as AdjutantAssignmentKind
    }

    let status: CreateAdjutantAssignmentInput['status'] = undefined

    if (body.status !== undefined) {
      if (
        !assignmentStatuses.has(
          body.status as CreateAdjutantAssignmentInput['status'],
        )
      ) {
        return yield* new OperatorAdjutantBadRequest({
          reason: 'invalid status',
        })
      }

      status = body.status as CreateAdjutantAssignmentInput['status']
    }

    let visibility: AdjutantAssignmentVisibility | undefined = undefined

    if (body.visibility !== undefined) {
      if (
        !assignmentVisibilities.has(
          body.visibility as AdjutantAssignmentVisibility,
        )
      ) {
        return yield* new OperatorAdjutantInvalidVisibility({})
      }

      visibility = body.visibility as AdjutantAssignmentVisibility
    }

    const bodyObjective = body.objective?.trim()
    const objective =
      bodyObjective === undefined || bodyObjective === ''
        ? source.kind === 'order'
          ? defaultOrderObjective(source.softwareOrderId)
          : defaultSiteObjective(source.siteId)
        : bodyObjective

    return {
      assignmentKind,
      assignedByUserId: session.user.userId,
      objective,
      ...(body.agentId === undefined ? {} : { agentId: body.agentId }),
      ...(body.commitSha === undefined ? {} : { commitSha: body.commitSha }),
      ...(body.currentRunId === undefined
        ? {}
        : { currentRunId: body.currentRunId }),
      ...(body.goalId === undefined ? {} : { goalId: body.goalId }),
      ...(body.projectId === undefined ? {} : { projectId: body.projectId }),
      ...(source.kind === 'order'
        ? { softwareOrderId: source.softwareOrderId }
        : { siteId: source.siteId }),
      ...(status === undefined ? {} : { status }),
      ...(body.taskSpecPath === undefined
        ? {}
        : { taskSpecPath: body.taskSpecPath }),
      ...(body.teamId === undefined ? {} : { teamId: body.teamId }),
      ...(visibility === undefined ? {} : { visibility }),
    }
  })

export const makeOperatorAdjutantRoutes = <
  Session extends OperatorAdjutantSession,
  Bindings extends OperatorAdjutantEnv,
>(
  dependencies: OperatorAdjutantRouteDependencies<Session, Bindings>,
) => {
  const assignOrder = (
    softwareOrderId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          CreateOperatorAdjutantAssignmentRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* assignments.createAssignment(
          yield* assignmentInput(
            session,
            { kind: 'order', softwareOrderId },
            body,
          ),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ assignment }, { status: 201 }),
          session,
        )
      }),
    )

  const assignSite = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          CreateOperatorAdjutantAssignmentRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* assignments.createAssignment(
          yield* assignmentInput(session, { kind: 'site', siteId }, body),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ assignment }, { status: 201 }),
          session,
        )
      }),
    )

  const listAssignments = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['GET'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const assignments = yield* AdjutantAssignmentService
        const list = yield* assignments.listAssignments(100)

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ assignments: list }),
          session,
        )
      }),
    )

  const readAssignment = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['GET'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const review = yield* readAssignmentReview(
          makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) }),
          assignment,
          getOpenAgentsWorkerConfig(env).exa.enabled,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ assignment, review }),
          session,
        )
      }),
    )

  const readEnrichment = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['GET'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const enrichment = yield* readEnrichmentReview(
          makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) }),
          assignment.id,
          getOpenAgentsWorkerConfig(env).exa.enabled,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ enrichment }),
          session,
        )
      }),
    )

  const enqueueEnrichment = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          EnqueueOperatorAdjutantEnrichmentRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
        const jobs = makeAdjutantEnrichmentJobService(
          crmEmailAuthorityDb(db),
          undefined,
          supervisionMirror,
        )
        const triggerKind =
          body.triggerKind ??
          (body.refresh === true ? 'operator_refresh' : 'operator_requested')
        const enqueued = yield* jobs.enqueueJob({
          assignment,
          freshnessMaxAgeHours: body.freshnessMaxAgeHours,
          numResults: body.numResults,
          operatorNotes: body.operatorNotes,
          refresh: body.refresh,
          requestBudget: body.requestBudget,
          requestedByUserId: session.user.userId,
          triggerKind,
        })

        if (!enqueued.duplicate) {
          yield* Effect.tryPromise({
            catch: error =>
              new OperatorAdjutantStorageError({
                error,
                operation: 'operatorAdjutant.enrichment.queue.send',
              }),
            try: () =>
              env.ADJUTANT_ENRICHMENT_QUEUE.send(
                new AdjutantEnrichmentQueueMessage({
                  assignmentId: assignment.id,
                  jobId: enqueued.job.id,
                  schemaVersion: 'openagents.adjutant_enrichment_job.v1',
                }),
              ),
          })
          yield* assignments.recordEvent({
            actorUserId: session.user.userId,
            assignmentId: assignment.id,
            eventType: 'adjutant.enrichment_job_queued',
            payload: {
              enrichmentJobId: enqueued.job.id,
              enrichmentRunId: enqueued.job.enrichmentRunId,
              triggerKind: enqueued.job.triggerKind,
            },
            summary: 'Autopilot enrichment job was queued.',
          })
        }

        const enrichment = yield* readEnrichmentReview(
          db,
          assignment.id,
          getOpenAgentsWorkerConfig(env).exa.enabled,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(
            {
              duplicate: enqueued.duplicate,
              enrichment,
              job: enqueued.job,
              plan: enqueued.plan,
            },
            { status: enqueued.duplicate ? 200 : 202 },
          ),
          session,
        )
      }),
    )

  const researchPolicy = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['GET', 'POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
        const service = makeAdjutantResearchPolicyService(
          crmEmailAuthorityDb(db),
          undefined,
          supervisionMirror,
        )

        if (request.method === 'GET') {
          const policy = yield* service.readEffectivePolicy(assignment)

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse({ policy }),
            session,
          )
        }

        const body = yield* decodeJsonBody(
          request,
          SetOperatorAdjutantResearchPolicyRequest,
        )
        const policy = yield* service.setPolicyOverride(assignment, {
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          customerSafeSummary: body.customerSafeSummary,
          policyMode: body.policyMode,
          reason: body.reason,
          ...(body.sourceAuthorityRef === undefined
            ? {}
            : { sourceAuthorityRef: body.sourceAuthorityRef }),
        })

        yield* assignments.recordEvent({
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          eventType: 'adjutant.research_policy_set',
          payload: {
            customerSafeStatus: policy.customerSafeStatus,
            effectiveMode: policy.effectiveMode,
            sourceAuthorityRef: policy.sourceAuthorityRef,
          },
          summary:
            policy.effectiveMode === 'research_bypassed_by_operator'
              ? 'Research policy bypass was recorded.'
              : 'Research policy override was recorded.',
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ policy }),
          session,
        )
      }),
    )

  const planEnrichment = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          PlanOperatorAdjutantEnrichmentRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const config = getOpenAgentsWorkerConfig(env)
        const plan = yield* buildEnrichmentPlan(
          makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) }),
          assignment,
          body,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({
            exaConfigured: config.exa.enabled,
            plan,
          }),
          session,
        )
      }),
    )

  const runEnrichment = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
    refresh: boolean,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          RunOperatorAdjutantEnrichmentRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const config = getOpenAgentsWorkerConfig(env)

        if (!config.exa.enabled) {
          return yield* new ExaConfigurationDisabled({
            reason: 'EXA_API_KEY is not configured.',
          })
        }

        const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
        const briefService = makeAdjutantResearchBriefService(
          crmEmailAuthorityDb(db),
          undefined,
          supervisionMirror,
        )
        const ledger = makeAdjutantEnrichmentLedger(db, undefined, supervisionMirror)
        const operations = makeAdjutantEnrichmentOperationsService(db)
        const policy = exaEnrichmentOperationsPolicyFromConfig(config.exa)
        const latestRun = yield* ledger.latestRunForAssignment(assignment.id)

        if (
          latestRun !== null &&
          (latestRun.status === 'queued' || latestRun.status === 'running')
        ) {
          return yield* new OperatorAdjutantConflict({
            error: 'adjutant_enrichment_already_running',
            reason:
              'An Exa enrichment run is already active for this assignment.',
          })
        }

        const plan = yield* buildEnrichmentPlan(db, assignment, body)
        const allTasks = [...plan.searchTasks, ...plan.contentsTasks]
        const requestedBudget = Math.max(
          1,
          Math.trunc(
            body.requestBudget ?? Math.min(6, policy.assignmentRequestBudget),
          ),
        )
        const requestBudget =
          allTasks.length === 0 ? 0 : Math.min(allTasks.length, requestedBudget)
        const selectedTasks = allTasks.slice(0, requestBudget)

        yield* operations.reserveBudget({
          assignmentId: assignment.id,
          policy,
          reason: refresh ? 'refresh' : 'run',
          requestUnits: requestBudget,
        })

        if (refresh) {
          const latestBrief = yield* briefService.latestBriefForAssignment(
            assignment.id,
          )

          if (latestBrief !== null) {
            yield* briefService.reviewBrief({
              briefId: latestBrief.id,
              reviewReason: 'Marked stale before enrichment refresh.',
              reviewedByUserId: session.user.userId,
              status: 'stale',
            })
          }
        }

        const run = yield* ledger.createRun({
          assignmentId: assignment.id,
          planId: plan.planId,
          requestBudget,
          siteId: assignment.siteId,
          softwareOrderId: assignment.softwareOrderId,
          startedAt: currentIsoTimestamp(),
          status: 'running',
          subject: plan.subjectSummary,
        })
        yield* ledger.linkAssignmentRun({
          assignmentId: assignment.id,
          enrichmentRunId: run.id,
          requiredForLaunch: true,
          status: 'running',
        })

        const client = makeExaClient(config.exa)
        const results = yield* Effect.forEach(
          selectedTasks,
          task =>
            executeEnrichmentTask(
              client,
              ledger,
              operations,
              policy,
              assignment,
              run,
              task,
            ),
          { concurrency: 1 },
        )
        const status = finalRunStatus(results)
        const errorSummary = runErrorSummary(results)
        const sourceCards = yield* ledger.sourceCardsForAssignment(
          assignment.id,
        )
        const runSourceCards = sourceCards.filter(
          sourceCard => sourceCard.runId === run.id,
        )
        const brief =
          status === 'failed'
            ? null
            : yield* briefService.createBrief({
                assignmentId: assignment.id,
                createdByUserId: session.user.userId,
                customerRequest: plan.subjectSummary,
                enrichmentRunId: run.id,
                sourceCards: runSourceCards,
                status: 'needs_review',
              })

        yield* ledger.updateRunStatus({
          completedAt: currentIsoTimestamp(),
          errorCode: errorSummary === null ? null : 'exa_task_failure',
          errorSummary,
          runId: run.id,
          status,
        })
        yield* ledger.linkAssignmentRun({
          assignmentId: assignment.id,
          enrichmentRunId: run.id,
          requiredForLaunch: true,
          researchBriefId: brief?.id ?? null,
          status: status === 'failed' ? 'failed' : 'needs_review',
        })
        yield* operations.recordMetric({
          assignmentId: assignment.id,
          eventName: 'exa.enrichment.run.completed',
          resultCount: results.length,
          runId: run.id,
          sourceCardCount: runSourceCards.length,
          status,
        })
        yield* assignments.recordEvent({
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          eventType: refresh
            ? 'adjutant.enrichment_refreshed'
            : 'adjutant.enrichment_ran',
          payload: {
            enrichmentRunId: run.id,
            planId: plan.planId,
            researchBriefId: brief?.id ?? null,
            requestBudget,
            status,
          },
          summary: refresh
            ? 'Autopilot enrichment was refreshed.'
            : 'Autopilot enrichment completed.',
        })

        const enrichment = yield* readEnrichmentReview(
          db,
          assignment.id,
          config.exa.enabled,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(
            {
              enrichment,
              plan,
              researchBrief: brief,
              runId: run.id,
            },
            { status: 202 },
          ),
          session,
        )
      }),
    )

  const createPublicSourceRef = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          CreateOperatorAdjutantPublicSourceRefRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
        const order =
          assignment.softwareOrderId === null
            ? null
            : yield* readEnrichmentSoftwareOrder(db, assignment.softwareOrderId)
        const service = makeAdjutantPublicSourceRefService(
          crmEmailAuthorityDb(db),
          undefined,
          supervisionMirror,
        )
        const sourceRef = yield* service.createSourceRef({
          assignmentId: assignment.id,
          kind: body.kind,
          proposedByUserId: session.user.userId,
          repositoryPrivate: order?.repositoryPrivate === true,
          siteId: assignment.siteId,
          softwareOrderId: assignment.softwareOrderId,
          url: body.url,
          ...(body.label === undefined ? {} : { label: body.label }),
          ...(body.status === undefined ? {} : { status: body.status }),
        })
        const enrichment = yield* readEnrichmentReview(
          db,
          assignment.id,
          getOpenAgentsWorkerConfig(env).exa.enabled,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ enrichment, sourceRef }, { status: 201 }),
          session,
        )
      }),
    )

  const reviewPublicSourceRef = (
    assignmentId: string,
    sourceRefId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          ReviewOperatorAdjutantPublicSourceRefRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)

        yield* makeAdjutantPublicSourceRefService(
          crmEmailAuthorityDb(db),
          undefined,
          supervisionMirror,
        ).reviewSourceRef({
          reviewedByUserId: session.user.userId,
          sourceRefId,
          status: body.status,
          ...(body.publicSafe === undefined
            ? {}
            : { publicSafe: body.publicSafe }),
          ...(body.reviewReason === undefined
            ? {}
            : { reviewReason: body.reviewReason }),
        })

        const enrichment = yield* readEnrichmentReview(
          db,
          assignment.id,
          getOpenAgentsWorkerConfig(env).exa.enabled,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ enrichment }),
          session,
        )
      }),
    )

  const reviewSourceCard = (
    assignmentId: string,
    sourceId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          ReviewOperatorAdjutantSourceCardRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
        const ledger = makeAdjutantEnrichmentLedger(db, undefined, supervisionMirror)
        const briefService = makeAdjutantResearchBriefService(
          crmEmailAuthorityDb(db),
          undefined,
          supervisionMirror,
        )

        yield* ledger.reviewSourceCard({
          reviewStatus: body.reviewStatus,
          sourceId,
          ...(body.publicSafe === undefined
            ? {}
            : { publicSafe: body.publicSafe }),
          ...(body.rejectedReason === undefined
            ? {}
            : { rejectedReason: body.rejectedReason }),
        })

        const sourceCards = yield* ledger.sourceCardsForAssignment(
          assignment.id,
        )
        const latestRun = yield* ledger.latestRunForAssignment(assignment.id)
        const shouldRefreshBrief =
          body.reviewStatus === 'approved' ||
          body.reviewStatus === 'public_safe'
        const refreshedBrief = shouldRefreshBrief
          ? yield* briefService.createBrief({
              assignmentId: assignment.id,
              createdByUserId: session.user.userId,
              customerRequest: assignment.objective,
              enrichmentRunId: latestRun?.id ?? null,
              sourceCards,
              status: 'needs_review',
            })
          : null

        if (latestRun !== null && refreshedBrief !== null) {
          yield* ledger.linkAssignmentRun({
            assignmentId: assignment.id,
            enrichmentRunId: latestRun.id,
            requiredForLaunch: true,
            researchBriefId: refreshedBrief.id,
            status: 'needs_review',
          })
          yield* ledger.updateRunStatus({
            completedAt: latestRun.completedAt,
            runId: latestRun.id,
            status: 'needs_review',
          })
        }

        const enrichment = yield* readEnrichmentReview(
          db,
          assignment.id,
          getOpenAgentsWorkerConfig(env).exa.enabled,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ enrichment, researchBrief: refreshedBrief }),
          session,
        )
      }),
    )

  const reviewResearchBrief = (
    assignmentId: string,
    briefId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          ReviewOperatorAdjutantResearchBriefRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
        const briefService = makeAdjutantResearchBriefService(
          crmEmailAuthorityDb(db),
          undefined,
          supervisionMirror,
        )
        const ledger = makeAdjutantEnrichmentLedger(db, undefined, supervisionMirror)

        yield* briefService.reviewBrief({
          briefId,
          reviewedByUserId: session.user.userId,
          status: body.status,
          ...(body.reviewReason === undefined
            ? {}
            : { reviewReason: body.reviewReason }),
        })

        const latestBrief = yield* briefService.latestBriefForAssignment(
          assignment.id,
        )

        if (latestBrief !== null && latestBrief.id === briefId) {
          yield* makeAdjutantEnrichmentOperationsService(db).recordMetric({
            assignmentId: assignment.id,
            eventName: 'exa.enrichment.brief.reviewed',
            runId: latestBrief.enrichmentRunId,
            sourceCardCount: latestBrief.sourceCards.length,
            status: body.status,
          })
        }

        if (
          latestBrief !== null &&
          latestBrief.id === briefId &&
          latestBrief.enrichmentRunId !== null
        ) {
          const status =
            body.status === 'approved'
              ? 'approved'
              : body.status === 'rejected'
                ? 'rejected'
                : 'stale'

          yield* ledger.linkAssignmentRun({
            assignmentId: assignment.id,
            enrichmentRunId: latestBrief.enrichmentRunId,
            requiredForLaunch: true,
            researchBriefId: latestBrief.id,
            status,
          })
          yield* ledger.updateRunStatus({
            completedAt: currentIsoTimestamp(),
            runId: latestBrief.enrichmentRunId,
            status,
          })
        }

        const taskPacketFreshness =
          latestBrief !== null &&
          latestBrief.id === briefId &&
          body.status === 'approved'
            ? yield* makeAdjutantTaskPacketFreshnessService(
                crmEmailAuthorityDb(db),
                undefined,
                supervisionMirror,
              ).markStaleForApprovedResearch({
                assignment,
                researchBrief: latestBrief,
              })
            : null

        if (
          taskPacketFreshness !== null &&
          taskPacketFreshness.status === 'stale'
        ) {
          yield* assignments.recordEvent({
            actorUserId: session.user.userId,
            assignmentId: assignment.id,
            eventType: 'adjutant.task_packet_stale',
            payload: {
              latestApprovedResearchBriefId:
                taskPacketFreshness.latestApprovedResearchBriefId,
              researchBriefId: taskPacketFreshness.researchBriefId,
              taskSpecPath: taskPacketFreshness.taskSpecPath,
            },
            summary:
              'Autopilot task packet was marked stale after research approval.',
          })
        }

        const enrichment = yield* readEnrichmentReview(
          db,
          assignment.id,
          getOpenAgentsWorkerConfig(env).exa.enabled,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ enrichment, taskPacketFreshness }),
          session,
        )
      }),
    )

  const preflightAssignment = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          PreflightOperatorAdjutantAssignmentRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const targetUser: OperatorAdjutantTargetUser = {
          displayName: session.user.email,
          email: session.user.email,
          githubUsername: null,
          userId: session.user.userId,
        }
        const selector = {
          ...(assignment.agentId === '' ? {} : { agentId: assignment.agentId }),
          ...(assignment.currentRunId === null
            ? {}
            : { runId: assignment.currentRunId }),
          ...(assignment.projectId === null
            ? {}
            : { projectId: assignment.projectId }),
          ...(assignment.teamId === null ? {} : { teamId: assignment.teamId }),
          email: session.user.email,
          includeCallbackLag: body.includeCallbackLag ?? false,
          userId: session.user.userId,
        }
        const autopilotPreflight = yield* Effect.tryPromise({
          catch: error =>
            new OperatorAdjutantPreflightError({
              error,
              operation: 'operatorAdjutant.preflight.autopilot',
            }),
          try: () =>
            dependencies.buildOperatorAutopilotPreflightPayload(
              request,
              env,
              selector,
              targetUser,
            ),
        })
        const sourceChecks = yield* assignmentSourceChecks(
          makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) }),
          assignment,
          body,
          getOpenAgentsWorkerConfig(env).exa.enabled,
        )
        const checks = [...autopilotPreflight.checks, ...sourceChecks]
        const status = operatorCheckRollup(checks)
        const nextStatus =
          status === 'blocked' ? 'blocked' : 'preflight_pending'
        const updatedAssignment =
          assignment.status === 'complete' || assignment.status === 'canceled'
            ? assignment
            : yield* assignments.updateAssignment({
                assignmentId: assignment.id,
                status: nextStatus,
              })
        yield* assignments.recordEvent({
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          eventType:
            status === 'blocked'
              ? 'adjutant.preflight_blocked'
              : 'adjutant.preflight_ready',
          payload: {
            checks: compactOperatorChecksForEvent(checks),
            nextSafeAction: adjutantNextSafeAction(
              checks,
              autopilotPreflight.nextSafeAction,
            ),
            status,
          },
          summary:
            status === 'blocked'
              ? 'Autopilot preflight found launch blockers.'
              : 'Autopilot preflight completed without launch blockers.',
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({
            assignment: updatedAssignment,
            autopilotPreflight,
            checks,
            nextSafeAction: adjutantNextSafeAction(
              checks,
              autopilotPreflight.nextSafeAction,
            ),
            status,
          }),
          session,
        )
      }),
    )

  const generateTaskPacket = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          GenerateOperatorAdjutantTaskPacketRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const site =
          assignment.siteId === null
            ? null
            : yield* readPreflightSite(
                makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) }),
                assignment.siteId,
              )

        if (assignment.siteId !== null && site === null) {
          return yield* new AdjutantAssignmentSiteNotFound({
            siteId: assignment.siteId,
          })
        }

        const researchBrief = yield* latestApprovedResearchBrief(
          makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) }),
          assignment.id,
        )
        const packet = yield* buildAdjutantTaskPacket({
          assignment,
          commitSha: body.commitSha,
          operatorNotes: body.operatorNotes,
          researchBrief,
          site:
            site === null
              ? null
              : {
                  id: site.id,
                  slug: site.slug,
                  title: site.title,
                },
          taskSpecPath: body.taskSpecPath,
        })
        const githubAccessToken = yield* Effect.promise(() =>
          readOperatorGitHubIdentityToken(env, session.user.userId),
        )
        const packetExists = yield* Effect.tryPromise({
          catch: error =>
            new AdjutantTaskPacketRefValidationFailed({
              reason:
                error instanceof Error
                  ? error.message
                  : 'Unable to validate the task packet ref.',
            }),
          try: () =>
            dependencies.validateAdjutantTaskPacketRef({
              commitSha: packet.commitSha,
              ...(githubAccessToken === undefined ? {} : { githubAccessToken }),
              path: packet.path,
              repositoryName: ADJUTANT_TASK_PACKET_REPOSITORY.name,
              repositoryOwner: ADJUTANT_TASK_PACKET_REPOSITORY.owner,
            }),
        })

        if (!packetExists) {
          return yield* new AdjutantTaskPacketRefMissing({
            commitSha: packet.commitSha,
            path: packet.path,
            reason: 'Task packet was not found at the pushed commit SHA.',
          })
        }

        const updatedAssignment = yield* assignments.updateAssignment({
          assignmentId: assignment.id,
          commitSha: packet.commitSha,
          taskSpecPath: packet.path,
        })
        const taskPacketFreshness =
          yield* makeAdjutantTaskPacketFreshnessService(
            openAgentsDatabase(env),
            undefined,
            makeSupervisionLongtailMirrorForEnv(env),
          ).recordGenerated({
            assignment: updatedAssignment,
            researchBrief,
          })
        yield* assignments.recordEvent({
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          eventType: 'adjutant.task_packet_generated',
          payload: {
            researchBriefId: researchBrief?.id ?? null,
            status: taskPacketFreshness.status,
            taskSpecPath: packet.path,
          },
          summary: 'Autopilot task packet was generated.',
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({
            assignment: updatedAssignment,
            packet,
            taskPacketFreshness,
          }),
          session,
        )
      }),
    )

  const keepCurrentTaskPacket = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          KeepCurrentOperatorAdjutantTaskPacketRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
        const latestApprovedBrief = yield* latestApprovedResearchBrief(
          db,
          assignment.id,
        )
        const taskPacketFreshness =
          yield* makeAdjutantTaskPacketFreshnessService(
            crmEmailAuthorityDb(db),
            undefined,
            supervisionMirror,
          ).keepCurrent({
            actorUserId: session.user.userId,
            assignment,
            customerSafeSummary: body.customerSafeSummary,
            latestApprovedBrief,
            reason: body.reason,
          })
        yield* assignments.recordEvent({
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          eventType: 'adjutant.task_packet_kept_current',
          payload: {
            latestApprovedResearchBriefId:
              taskPacketFreshness.latestApprovedResearchBriefId,
            status: taskPacketFreshness.status,
            taskSpecPath: taskPacketFreshness.taskSpecPath,
          },
          summary:
            'Operator kept the current Autopilot task packet after research review.',
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ taskPacketFreshness }),
          session,
        )
      }),
    )

  const clearCurrentRun = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          ClearOperatorAdjutantCurrentRunRequest,
        )
        const reason = body.reason.trim()

        if (reason === '' || reason.length > 1000) {
          return yield* new OperatorAdjutantBadRequest({
            reason:
              'reason must be a non-empty operator note of 1000 characters or less',
          })
        }

        if (containsProviderSecretMaterial(reason)) {
          return yield* new OperatorAdjutantBadRequest({
            reason: 'reason contains secret-shaped material',
          })
        }

        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        if (
          assignment.status === 'complete' ||
          assignment.status === 'canceled'
        ) {
          return yield* new OperatorAdjutantConflict({
            error: 'assignment_terminal',
            reason: 'Cannot clear the current run on a terminal assignment.',
          })
        }

        if (assignment.currentRunId === null) {
          return yield* new OperatorAdjutantConflict({
            error: 'current_run_missing',
            reason: 'Assignment does not have a current run to clear.',
          })
        }

        if (assignment.currentRunId !== body.runId) {
          return yield* new OperatorAdjutantConflict({
            error: 'current_run_mismatch',
            reason:
              'runId must match the assignment currentRunId before it can be cleared.',
          })
        }

        const run = yield* readReviewRun(makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) }), body.runId)

        if (run === null) {
          return yield* new OperatorAdjutantConflict({
            error: 'current_run_not_found',
            reason: 'The linked current run was not found.',
          })
        }

        if (!terminalRunStatuses.has(run.status)) {
          return yield* new OperatorAdjutantConflict({
            error: 'current_run_not_terminal',
            reason:
              'Only completed, failed, or canceled current runs can be cleared.',
          })
        }

        const updatedAssignment = yield* assignments.updateAssignment({
          assignmentId: assignment.id,
          currentRunId: null,
          status: 'preflight_pending',
        })
        yield* assignments.recordEvent({
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          eventType: 'adjutant.current_run_cleared',
          payload: {
            reason,
            runId: run.id,
            runStatus: run.status,
          },
          summary:
            'Operator cleared a terminal current run so the assignment can be retried.',
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({
            assignment: updatedAssignment,
            clearedRun: {
              id: run.id,
              status: run.status,
            },
          }),
          session,
        )
      }),
    )

  const launchAssignment = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          LaunchOperatorAdjutantAssignmentRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        const targetUser: OperatorAdjutantTargetUser = {
          displayName: session.user.email,
          email: session.user.email,
          githubUsername: null,
          userId: session.user.userId,
        }
        const preflightSelector = {
          ...(assignment.agentId === '' ? {} : { agentId: assignment.agentId }),
          ...(assignment.currentRunId === null
            ? {}
            : { runId: assignment.currentRunId }),
          ...(assignment.projectId === null
            ? {}
            : { projectId: assignment.projectId }),
          ...(assignment.teamId === null ? {} : { teamId: assignment.teamId }),
          email: session.user.email,
          includeCallbackLag: body.includeCallbackLag ?? false,
          userId: session.user.userId,
        }
        const autopilotPreflight = yield* Effect.tryPromise({
          catch: error =>
            new OperatorAdjutantPreflightError({
              error,
              operation: 'operatorAdjutant.launch.preflight.autopilot',
            }),
          try: () =>
            dependencies.buildOperatorAutopilotPreflightPayload(
              request,
              env,
              preflightSelector,
              targetUser,
            ),
        })
        // KS-8.14 (#8359): this handler flips software_orders launch
        // state; compose the business funnel mirror UNDER the CRM seam and
        // OVER the sites proxy so order writes mirror to the business twin.
        const db = makeCrmEmailDatabaseForEnv(env, {
          d1: businessDomainDatabaseForEnv(env, { d1: openAgentsDatabase(env) }),
        })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
        const config = getOpenAgentsWorkerConfig(env)
        const sourceChecks = yield* assignmentSourceChecks(
          db,
          assignment,
          {
            ...(body.includeCallbackLag === undefined
              ? {}
              : { includeCallbackLag: body.includeCallbackLag }),
            ...(body.launchChecklist === undefined
              ? {}
              : { launchChecklist: body.launchChecklist }),
          },
          config.exa.enabled,
        )
        const paymentGate = yield* readFirstBatchPaymentGate(
          crmEmailAuthorityDb(db),
          assignment.softwareOrderId,
        ).pipe(
          Effect.mapError(
            (error: FirstBatchPaymentPolicyStorageError) =>
              new OperatorAdjutantStorageError({
                error,
                operation: 'operatorAdjutant.launch.firstBatchPaymentGate',
              }),
          ),
        )
        const paymentCheck = paymentGate.required
          ? paymentGate.policy === null
            ? operatorCheck(
                'first_batch_payment_policy',
                'blocked',
                'First-batch no-payment policy is required before launch.',
                {
                  requiredPolicyModes: ['public_beta_free', 'operator_grant'],
                  softwareOrderId: assignment.softwareOrderId,
                },
              )
            : operatorCheck(
                'first_batch_payment_policy',
                'ok',
                paymentGate.policy.customerSafeSummary,
                {
                  policyId: paymentGate.policy.id,
                  policyMode: paymentGate.policy.policyMode,
                  softwareOrderId: assignment.softwareOrderId,
                },
              )
          : operatorCheck(
              'first_batch_payment_policy',
              'ok',
              'No first-batch no-payment policy is required for this assignment.',
              { required: false },
            )
        const checks = [
          ...autopilotPreflight.checks,
          ...sourceChecks,
          paymentCheck,
        ]
        const status = operatorCheckRollup(checks)
        const nextSafeAction = adjutantNextSafeAction(
          checks,
          autopilotPreflight.nextSafeAction,
        )

        if (status === 'blocked') {
          if (
            assignment.status !== 'complete' &&
            assignment.status !== 'canceled' &&
            assignment.currentRunId === null
          ) {
            yield* assignments.updateAssignment({
              assignmentId: assignment.id,
              status: 'blocked',
            })
          }

          yield* assignments.recordEvent({
            actorUserId: session.user.userId,
            assignmentId: assignment.id,
            eventType: 'adjutant.launch_blocked',
            payload: {
              checks: compactOperatorChecksForEvent(checks),
              nextSafeAction,
              status,
            },
            summary: 'Autopilot launch was blocked by preflight.',
          })

          return yield* new OperatorAdjutantLaunchBlocked({
            checks,
            nextSafeAction,
            status,
          })
        }

        const order =
          assignment.softwareOrderId === null
            ? null
            : yield* readPreflightSoftwareOrder(db, assignment.softwareOrderId)
        const site =
          assignment.siteId === null
            ? null
            : yield* readPreflightSite(db, assignment.siteId)
        const researchBrief = yield* latestApprovedResearchBrief(
          db,
          assignment.id,
        )
        const selector = launchSelectorForAssignment(
          assignment,
          order,
          site,
          researchBrief,
          checks,
          body,
        )
        const launch = yield* Effect.tryPromise({
          catch: error =>
            new OperatorAdjutantLaunchError({
              error,
              operation: 'operatorAdjutant.launch.omni',
            }),
          try: () =>
            dependencies.launchUserAutopilotMission(env, ctx, {
              selector,
              userId: session.user.userId,
            }),
        })

        if (!launch.ok) {
          if (
            assignment.status !== 'complete' &&
            assignment.status !== 'canceled' &&
            assignment.currentRunId === null
          ) {
            yield* assignments.updateAssignment({
              assignmentId: assignment.id,
              status: 'blocked',
            })
          }

          yield* assignments.recordEvent({
            actorUserId: session.user.userId,
            assignmentId: assignment.id,
            eventType: 'adjutant.launch_blocked',
            payload: {
              httpStatus: launch.response.status,
            },
            summary: 'Autopilot launch was blocked before dispatch.',
          })

          return dependencies.appendRefreshedSessionCookies(
            launch.response,
            session,
          )
        }

        const launchStatus = launchPayloadStatus(launch.launch.payload)
        const dispatchFailed = launchStatus === 'failed'
        const updatedAssignment = yield* assignments.updateAssignment({
          assignmentId: assignment.id,
          currentRunId: launch.launch.runId,
          status: dispatchFailed ? 'blocked' : 'queued',
        })
        yield* updateSoftwareOrderLaunchState(db, {
          runId: launch.launch.runId,
          softwareOrderId: assignment.softwareOrderId,
          status: dispatchFailed ? 'unavailable' : 'agent_queued',
          updatedAt: updatedAssignment.updatedAt,
        })
        yield* recordSiteLaunchEvent(db, {
          actorUserId: session.user.userId,
          eventType: dispatchFailed
            ? 'adjutant.dispatch_failed'
            : 'adjutant.run_queued',
          payload: {
            assignmentId: assignment.id,
            commitSha: assignment.commitSha,
            researchBriefId: researchBrief?.id ?? null,
            runId: launch.launch.runId,
            status: launchStatus ?? 'queued',
            taskSpecPath: assignment.taskSpecPath,
          },
          runId: launch.launch.runId,
          siteId: assignment.siteId,
          summary: dispatchFailed
            ? 'Autopilot dispatch failed before runner execution.'
            : 'Autopilot launched a Site run.',
        })
        if (!dispatchFailed) {
          yield* recordGenerationLaunchUsageReceipt(
            db,
            {
              assignment,
              paymentPolicy: paymentGate.policy,
              runId: launch.launch.runId,
              site,
            },
            supervisionMirror,
          )
        }
        yield* assignments.recordEvent({
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          eventType: dispatchFailed
            ? 'adjutant.dispatch_failed'
            : 'adjutant.run_queued',
          payload: {
            commitSha: assignment.commitSha,
            researchBriefId: researchBrief?.id ?? null,
            runId: launch.launch.runId,
            status: launchStatus ?? 'queued',
            taskSpecPath: assignment.taskSpecPath,
          },
          runId: launch.launch.runId,
          summary: dispatchFailed
            ? 'Autopilot dispatch failed before runner execution.'
            : 'Autopilot launched a run.',
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(
            {
              accepted: true,
              assignment: updatedAssignment,
              launch: launch.launch.payload,
              preflight: {
                checks,
                nextSafeAction,
                status,
              },
              runId: launch.launch.runId,
            },
            { status: 202 },
          ),
          session,
        )
      }),
    )

  const requestAdjustment = (
    assignmentId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      request,
      ['POST'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          CreateOperatorAdjutantAdjustmentRequest,
        )
        const assignments = yield* AdjutantAssignmentService
        const assignment = yield* readRequiredAdjutantAssignment(assignments, assignmentId)

        if (assignment.siteId === null) {
          return yield* new OperatorAdjutantBadRequest({
            reason: 'Autopilot adjustments require a linked Site.',
          })
        }

        if (assignment.status === 'canceled') {
          return yield* new OperatorAdjutantBadRequest({
            reason: 'Canceled Autopilot assignments cannot be adjusted.',
          })
        }

        // KS-8.14 (#8359): this handler flips software_orders launch
        // state; compose the business funnel mirror UNDER the CRM seam and
        // OVER the sites proxy so order writes mirror to the business twin.
        const db = makeCrmEmailDatabaseForEnv(env, {
          d1: businessDomainDatabaseForEnv(env, { d1: openAgentsDatabase(env) }),
        })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)
        const order =
          assignment.softwareOrderId === null
            ? null
            : yield* readPreflightSoftwareOrder(db, assignment.softwareOrderId)
        const site = yield* readPreflightSite(db, assignment.siteId)

        if (site === null) {
          return yield* new AdjutantAssignmentSiteNotFound({
            siteId: assignment.siteId,
          })
        }

        const adjustments = makeAdjutantAdjustmentService(
          crmEmailAuthorityDb(db),
          undefined,
          supervisionMirror,
        )
        const adjustment = yield* adjustments.createAdjustment({
          assignmentId: assignment.id,
          goalId: assignment.goalId,
          instruction: body.instruction,
          requestedByUserId: session.user.userId,
          siteId: assignment.siteId,
          softwareOrderId: assignment.softwareOrderId,
          sourceRunId: assignment.currentRunId,
          visibility: assignment.visibility,
        })
        const prompt = adjustmentPrompt(assignment, adjustment, site)

        yield* assignments.recordEvent({
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          eventType: 'adjutant.adjustment_requested',
          payload: {
            adjustmentId: adjustment.id,
            siteId: assignment.siteId,
            sourceRunId: assignment.currentRunId,
          },
          runId: assignment.currentRunId,
          summary: 'Autopilot Site adjustment was requested.',
        })
        yield* recordSiteLaunchEvent(db, {
          actorUserId: session.user.userId,
          eventType: 'adjutant.adjustment_requested',
          payload: {
            adjustmentId: adjustment.id,
            assignmentId: assignment.id,
            sourceRunId: assignment.currentRunId,
          },
          runId: assignment.currentRunId ?? adjustment.id,
          siteId: assignment.siteId,
          summary: 'Autopilot Site adjustment was requested.',
        })

        const currentRun =
          assignment.currentRunId === null
            ? null
            : yield* readReviewRun(db, assignment.currentRunId)

        if (canContinueCurrentRun(currentRun)) {
          const continuation = yield* Effect.tryPromise({
            catch: error =>
              new OperatorAdjutantLaunchError({
                error,
                operation: 'operatorAdjutant.adjustment.continue',
              }),
            try: () =>
              dependencies.continueUserAutopilotRun(env, ctx, {
                prompt,
                runId: currentRun.id,
                userId: session.user.userId,
              }),
          })

          if (!continuation.ok) {
            yield* adjustments.updateAdjustment({
              adjustmentId: adjustment.id,
              status: 'failed',
            })
            yield* assignments.recordEvent({
              actorUserId: session.user.userId,
              assignmentId: assignment.id,
              eventType: 'adjutant.adjustment_failed',
              payload: {
                adjustmentId: adjustment.id,
                httpStatus: continuation.response.status,
                runId: currentRun.id,
              },
              runId: currentRun.id,
              summary: 'Autopilot Site adjustment continuation failed.',
            })

            return dependencies.appendRefreshedSessionCookies(
              continuation.response,
              session,
            )
          }

          const updatedAdjustment = yield* adjustments.updateAdjustment({
            adjustmentId: adjustment.id,
            continuationMode: 'follow_up_turn',
            continuationRunId: continuation.continuation.runId,
            status: 'running',
          })
          const updatedAssignment = yield* assignments.updateAssignment({
            assignmentId: assignment.id,
            currentRunId: continuation.continuation.runId,
            status: 'running',
          })
          yield* updateSoftwareOrderLaunchState(db, {
            runId: continuation.continuation.runId,
            softwareOrderId: assignment.softwareOrderId,
            status: 'agent_running',
            updatedAt: updatedAssignment.updatedAt,
          })
          yield* updateSiteAdjustmentState(db, {
            siteId: assignment.siteId,
            status: 'generating',
            updatedAt: updatedAssignment.updatedAt,
          })
          yield* recordAdjustmentLaunchUsageReceipt(
            db,
            {
              adjustment: updatedAdjustment,
              assignment,
              mode: 'follow_up_turn',
              runId: continuation.continuation.runId,
            },
            supervisionMirror,
          )
          yield* assignments.recordEvent({
            actorUserId: session.user.userId,
            assignmentId: assignment.id,
            eventType: 'adjutant.adjustment_running',
            payload: {
              adjustmentId: adjustment.id,
              mode: 'follow_up_turn',
              runId: continuation.continuation.runId,
            },
            runId: continuation.continuation.runId,
            summary:
              'Autopilot continued the current run for a Site adjustment.',
          })
          yield* recordSiteLaunchEvent(db, {
            actorUserId: session.user.userId,
            eventType: 'adjutant.adjustment_running',
            payload: {
              adjustmentId: adjustment.id,
              assignmentId: assignment.id,
              mode: 'follow_up_turn',
              runId: continuation.continuation.runId,
            },
            runId: continuation.continuation.runId,
            siteId: assignment.siteId,
            summary:
              'Autopilot continued the current run for a Site adjustment.',
          })

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse(
              {
                accepted: true,
                adjustment: updatedAdjustment,
                assignment: updatedAssignment,
                continuation: continuation.continuation,
                runId: continuation.continuation.runId,
              },
              { status: 202 },
            ),
            session,
          )
        }

        const selector = adjustmentLaunchSelectorForAssignment(
          assignment,
          order,
          site,
          adjustment,
          body,
        )
        const launch = yield* Effect.tryPromise({
          catch: error =>
            new OperatorAdjutantLaunchError({
              error,
              operation: 'operatorAdjutant.adjustment.launch',
            }),
          try: () =>
            dependencies.launchUserAutopilotMission(env, ctx, {
              selector,
              userId: session.user.userId,
            }),
        })

        if (!launch.ok) {
          yield* adjustments.updateAdjustment({
            adjustmentId: adjustment.id,
            status: 'failed',
          })
          yield* assignments.recordEvent({
            actorUserId: session.user.userId,
            assignmentId: assignment.id,
            eventType: 'adjutant.adjustment_failed',
            payload: {
              adjustmentId: adjustment.id,
              httpStatus: launch.response.status,
            },
            runId: assignment.currentRunId,
            summary: 'Autopilot Site adjustment launch failed.',
          })

          return dependencies.appendRefreshedSessionCookies(
            launch.response,
            session,
          )
        }

        const launchStatus = launchPayloadStatus(launch.launch.payload)
        const dispatchFailed = launchStatus === 'failed'
        const continuationMode: AdjutantAdjustmentContinuationMode =
          'new_goal_run'
        const updatedAdjustment = yield* adjustments.updateAdjustment({
          adjustmentId: adjustment.id,
          continuationMode,
          continuationRunId: launch.launch.runId,
          status: dispatchFailed ? 'failed' : 'queued',
        })
        const updatedAssignment = yield* assignments.updateAssignment({
          assignmentId: assignment.id,
          currentRunId: launch.launch.runId,
          status: dispatchFailed ? 'blocked' : 'queued',
        })
        yield* updateSoftwareOrderLaunchState(db, {
          runId: launch.launch.runId,
          softwareOrderId: assignment.softwareOrderId,
          status: dispatchFailed ? 'unavailable' : 'agent_queued',
          updatedAt: updatedAssignment.updatedAt,
        })

        if (!dispatchFailed) {
          yield* updateSiteAdjustmentState(db, {
            siteId: assignment.siteId,
            status: 'generating',
            updatedAt: updatedAssignment.updatedAt,
          })
          yield* recordAdjustmentLaunchUsageReceipt(
            db,
            {
              adjustment: updatedAdjustment,
              assignment,
              mode: continuationMode,
              runId: launch.launch.runId,
            },
            supervisionMirror,
          )
        }

        yield* assignments.recordEvent({
          actorUserId: session.user.userId,
          assignmentId: assignment.id,
          eventType: dispatchFailed
            ? 'adjutant.adjustment_failed'
            : 'adjutant.adjustment_running',
          payload: {
            adjustmentId: adjustment.id,
            mode: continuationMode,
            runId: launch.launch.runId,
            status: launchStatus ?? 'queued',
          },
          runId: launch.launch.runId,
          summary: dispatchFailed
            ? 'Autopilot Site adjustment dispatch failed.'
            : 'Autopilot launched a new run for a Site adjustment.',
        })
        yield* recordSiteLaunchEvent(db, {
          actorUserId: session.user.userId,
          eventType: dispatchFailed
            ? 'adjutant.dispatch_failed'
            : 'adjutant.adjustment_running',
          payload: {
            adjustmentId: adjustment.id,
            assignmentId: assignment.id,
            mode: continuationMode,
            runId: launch.launch.runId,
            status: launchStatus ?? 'queued',
          },
          runId: launch.launch.runId,
          siteId: assignment.siteId,
          summary: dispatchFailed
            ? 'Autopilot Site adjustment dispatch failed.'
            : 'Autopilot launched a new run for a Site adjustment.',
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(
            {
              accepted: true,
              adjustment: updatedAdjustment,
              assignment: updatedAssignment,
              launch: launch.launch.payload,
              runId: launch.launch.runId,
            },
            { status: 202 },
          ),
          session,
        )
      }),
    )

  return {
    routeOperatorAdjutantRequest: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)

      if (url.pathname === '/api/operator/adjutant/assignments') {
        return listAssignments(request, env, ctx)
      }

      const preflightMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/preflight$/.exec(
          url.pathname,
        )

      if (preflightMatch !== null) {
        return preflightAssignment(
          decodeURIComponent(preflightMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const clearCurrentRunMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/current-run\/clear$/.exec(
          url.pathname,
        )

      if (clearCurrentRunMatch !== null) {
        return clearCurrentRun(
          decodeURIComponent(clearCurrentRunMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const taskPacketKeepCurrentMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/task-packet\/keep-current$/.exec(
          url.pathname,
        )

      if (taskPacketKeepCurrentMatch !== null) {
        return keepCurrentTaskPacket(
          decodeURIComponent(taskPacketKeepCurrentMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const taskPacketMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/task-packet$/.exec(
          url.pathname,
        )

      if (taskPacketMatch !== null) {
        return generateTaskPacket(
          decodeURIComponent(taskPacketMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const launchMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/launch$/.exec(
          url.pathname,
        )

      if (launchMatch !== null) {
        return launchAssignment(
          decodeURIComponent(launchMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const adjustmentMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/adjustments$/.exec(
          url.pathname,
        )

      if (adjustmentMatch !== null) {
        return requestAdjustment(
          decodeURIComponent(adjustmentMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const enrichmentPlanMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/enrichment\/plan$/.exec(
          url.pathname,
        )

      if (enrichmentPlanMatch !== null) {
        return planEnrichment(
          decodeURIComponent(enrichmentPlanMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const enrichmentRunMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/enrichment\/run$/.exec(
          url.pathname,
        )

      if (enrichmentRunMatch !== null) {
        return runEnrichment(
          decodeURIComponent(enrichmentRunMatch[1] ?? ''),
          request,
          env,
          ctx,
          false,
        )
      }

      const enrichmentRefreshMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/enrichment\/refresh$/.exec(
          url.pathname,
        )

      if (enrichmentRefreshMatch !== null) {
        return runEnrichment(
          decodeURIComponent(enrichmentRefreshMatch[1] ?? ''),
          request,
          env,
          ctx,
          true,
        )
      }

      const enrichmentEnqueueMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/enrichment\/enqueue$/.exec(
          url.pathname,
        )

      if (enrichmentEnqueueMatch !== null) {
        return enqueueEnrichment(
          decodeURIComponent(enrichmentEnqueueMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const sourceRefsMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/enrichment\/source-refs$/.exec(
          url.pathname,
        )

      if (sourceRefsMatch !== null) {
        return createPublicSourceRef(
          decodeURIComponent(sourceRefsMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const sourceRefReviewMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/enrichment\/source-refs\/([^/]+)\/review$/.exec(
          url.pathname,
        )

      if (sourceRefReviewMatch !== null) {
        return reviewPublicSourceRef(
          decodeURIComponent(sourceRefReviewMatch[1] ?? ''),
          decodeURIComponent(sourceRefReviewMatch[2] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const sourceCardReviewMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/enrichment\/source-cards\/([^/]+)\/review$/.exec(
          url.pathname,
        )

      if (sourceCardReviewMatch !== null) {
        return reviewSourceCard(
          decodeURIComponent(sourceCardReviewMatch[1] ?? ''),
          decodeURIComponent(sourceCardReviewMatch[2] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const researchBriefReviewMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/enrichment\/briefs\/([^/]+)\/review$/.exec(
          url.pathname,
        )

      if (researchBriefReviewMatch !== null) {
        return reviewResearchBrief(
          decodeURIComponent(researchBriefReviewMatch[1] ?? ''),
          decodeURIComponent(researchBriefReviewMatch[2] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const enrichmentMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/enrichment$/.exec(
          url.pathname,
        )

      if (enrichmentMatch !== null) {
        return readEnrichment(
          decodeURIComponent(enrichmentMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const researchPolicyMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)\/research-policy$/.exec(
          url.pathname,
        )

      if (researchPolicyMatch !== null) {
        return researchPolicy(
          decodeURIComponent(researchPolicyMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const assignmentMatch =
        /^\/api\/operator\/adjutant\/assignments\/([^/]+)$/.exec(url.pathname)

      if (assignmentMatch !== null) {
        return readAssignment(
          decodeURIComponent(assignmentMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const orderAssignMatch =
        /^\/api\/operator\/adjutant\/orders\/([^/]+)\/assign$/.exec(
          url.pathname,
        )

      if (orderAssignMatch !== null) {
        return assignOrder(
          decodeURIComponent(orderAssignMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      const siteAssignMatch =
        /^\/api\/operator\/adjutant\/sites\/([^/]+)\/assign$/.exec(url.pathname)

      if (siteAssignMatch !== null) {
        return assignSite(
          decodeURIComponent(siteAssignMatch[1] ?? ''),
          request,
          env,
          ctx,
        )
      }

      return undefined
    },
  }
}

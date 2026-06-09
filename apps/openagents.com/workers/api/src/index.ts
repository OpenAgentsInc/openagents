import { Container, getContainer } from '@cloudflare/containers'
import {
  type WorkerBindings,
  badRequest,
  cursorGap,
  jsonResponse,
  makeD1SyncOutboxRepository,
  notFound,
} from '@openagents/sync-worker'
import { issuer } from '@openauthjs/openauth'
import { type Tokens, createClient } from '@openauthjs/openauth/client'
import { GithubProvider } from '@openauthjs/openauth/provider/github'
import { createSubjects } from '@openauthjs/openauth/subject'
import { THEME_OPENAUTH } from '@openauthjs/openauth/ui/theme'
import { Cause, Effect, Layer, Option, Schema as S } from 'effect'
import { Exit } from 'effect'
import { WorkerEnvironment } from 'effect-cf'

import { AdjutantEnrichmentQueueMessage } from './adjutant-enrichment-jobs'
import type { AdjutantTaskPacketRefValidationInput } from './adjutant-task-packets'
import { recordAdjutantUsageReceipt } from './adjutant-usage-receipts'
import { makeAdminOverviewHandlers } from './admin-overview-routes'
import { makeAgentGoalRoutes } from './agent-goal-routes'
import { handleProgrammaticAgentHome } from './agent-home-routes'
import {
  makeAgentOwnerClaimRoutes,
  makeD1AgentOwnerClaimStore,
} from './agent-owner-claim-routes'
import {
  makeAgentProposalRoutes,
  makeD1AgentProposalStore,
} from './agent-proposal-routes'
import { withAgentRateLimitHeaders } from './agent-rate-limit-policy'
import { makeD1AgentRateLimitRecoveryStore } from './agent-rate-limit-recovery'
import {
  type AgentRegistrationStore,
  ProgrammaticAgentRegistrationRequest,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
  createProgrammaticAgentRegistration,
  makeD1AgentRegistrationStore,
  timingSafeEqual,
} from './agent-registration'
import {
  makeAgentScopedGrantRoutes,
  makeD1AgentScopedGrantStore,
} from './agent-scoped-grant-routes'
import { makeAgentSearchRoutes } from './agent-search-routes'
import { makeAgentSiteRoutes } from './agent-site-routes'
import { makeOperatorArtanisConsoleRoutes } from './artanis-operator-console-routes'
import { handlePublicArtanisReportApi } from './artanis-public-report-routes'
import { runArtanisScheduledTickForWorker } from './artanis-scheduled-runner'
import {
  ACCESS_COOKIE,
  AUTH_STATE_COOKIE,
  AUTH_STATE_MAX_AGE_SECONDS,
  REFRESH_COOKIE,
  SESSION_MAX_AGE_SECONDS,
  appendClearSessionCookies,
  appendSessionCookies,
  expiredCookie,
  parseCookies,
  serializeCookie,
} from './auth-cookies'
import { makeD1Storage } from './auth/openauth-storage'
import {
  type VerifiedSession as VerifiedAuthSession,
  makeBrowserSessionBoundary,
} from './auth/session'
import {
  type BillingSummary,
  markOutOfCreditsNotificationFailed,
  markOutOfCreditsNotificationSent,
  readBillingSummary,
  recordContainerUsageDebitForRun,
  reserveOutOfCreditsNotification,
  suspendBillingAccountIfOutOfCredits,
} from './billing'
import { makeBillingApiHandlers } from './billing-routes'
import { OpenAgentsDatabase, ThreadFileArtifacts } from './bindings'
import { makeBlueprintProbeContributionRoutes } from './blueprint-probe-contribution-routes'
import { makeBlueprintRoutes } from './blueprint-routes'
import {
  listBlueprintActionSubmissions,
  recordBlueprintActionSubmissionProposal,
} from './blueprint/repositories/action-submissions'
import {
  listBlueprintProbeContributions,
  recordBlueprintProbeContribution,
} from './blueprint/repositories/probe-contributions'
import {
  listBlueprintProgramRuns,
  recordBlueprintProgramRun,
} from './blueprint/repositories/program-runs'
import { makeD1BuyerPaymentLedgerStore } from './buyer-payment-ledger'
import {
  type OpenAgentsWorkerConfigEnv,
  getOpenAgentsWorkerConfig,
  redactedValue,
} from './config'
import {
  OrderSitesTransactionalEmailInput,
  buildOrderSitesTransactionalEmailIdempotencyKey,
  sendOrderSitesTransactionalEmailWithLedger,
  sendOutOfCreditsEmailWithLedger,
} from './email'
import {
  type EmailCampaignDispatcherResult,
  dispatchDueEmailCampaignSends,
} from './email-campaign-dispatcher'
import type { OnboardingDripOrderState } from './email-onboarding-drip'
import { makeForumRoutes } from './forum-routes'
import {
  GITHUB_WRITE_REQUIRED_SCOPES,
  GitHubWriteApiFailure,
  type GitHubWriteConnectionAttemptRecord,
  type GitHubWriteConnectionBundle,
  GitHubWriteTokenStorageFailure,
  gitHubWriteConnectionMetadataJson,
  githubWriteConnectionRef,
  githubWriteSecretKey,
  githubWriteSecretRef,
  listGitHubWriteConnectionsForUser,
  makeD1GitHubWriteRepository,
  recordGitHubWriteConnectionConnected,
  requireGitHubWriteCallbackAccount,
  requireGitHubWritePermissions,
  resolveGitHubWriteGrant,
  startGitHubWriteConnectionAttempt,
} from './github-write-connections'
import {
  gitHubWriteRouteErrorMessage,
  gitHubWriteRouteErrorName,
  gitHubWriteRouteErrorStatus,
} from './github-write-route-errors'
import {
  makeMissingOpenAgentsHostedMdkClient,
  makeOpenAgentsHostedMdkRouteClient,
} from './hosted-mdk-client'
import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  redirectResponse,
  serverError,
  unauthorized,
} from './http/responses'
import { routeAccessResponse } from './http/route-access-response'
import { routeEffect, routeEffectOrResponse } from './http/route-effects'
import type { ExactRoute } from './http/router'
import { makeImageGenerationRoutes } from './image-generation-routes'
import {
  decodeUnknownWithSchema,
  isRecord,
  nestedUnknown,
  optionalInteger,
  optionalNestedString,
  optionalString,
  readJsonObject,
  safeJsonRecord,
  stringArrayFromUnknown,
} from './json-boundary'
import { makeOpenAgentsL402HmacSigningBoundary } from './l402-credential-service'
import { makeMulletRoutes } from './mullet/routes'
import { makeNexusPylonVisibilityRoutes } from './nexus-pylon-visibility-routes'
import { makeD1NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import {
  logWorkerRouteError,
  logWorkerRouteWarning,
  observedEffect,
  observedPromise,
} from './observability'
import { handleOmniApiSdkSeedApi } from './omni-api-sdk-seed-routes'
import { makeOmniHandlers } from './omni-handlers'
import { makeOmniRoutes } from './omni-routes'
import {
  type AgentRunBundle,
  type AgentRunRecord,
  type OmniEventRecord,
  cancelActiveAgentRunsForBillingExhaustion,
  cancelAgentRunOnShc,
  listActiveAgentRunsForBilling,
  makeD1OmniRunStore,
} from './omni-runs'
import { githubIdentityTokenKey } from './onboarding/github'
import { readOnboardingStatusForUser } from './onboarding/repository'
import { makeOnboardingRoutes } from './onboarding/routes'
import {
  handleOpenAgentsAgentOnboarding,
  handleOpenAgentsCompanionFile,
} from './openagents-agent-onboarding-routes'
import { handleOpenAgentsCapabilityManifestApi } from './openagents-capability-manifest-routes'
import { handleOpenAgentsOpenApi } from './openagents-openapi-routes'
import {
  executeQueuedAdjutantEnrichmentJob,
  makeOperatorAdjutantRoutes,
} from './operator-adjutant-routes'
import { makeOperatorBillingHandlers } from './operator-billing-routes'
import { makeOperatorEmailInspectionRoutes } from './operator-email-inspection-routes'
import { makeOperatorOrderTriageRoutes } from './operator-order-triage-routes'
import { makeOperatorProviderAccountRoutes } from './operator-provider-account-routes'
import { makeOperatorPylonMarketplaceRoutes } from './operator-pylon-marketplace-routes'
import { makeOperatorSitesRoutes } from './operator-sites-routes'
import {
  type OperatorTargetUser,
  readOperatorTargetUser,
} from './operator-targets'
import { publicProductPromisesDocument } from './product-promises'
import { makeProviderAccountBrowserHandlers } from './provider-account-browser-routes'
import { makeProviderAccountRoutes } from './provider-account-routes'
import { makeProviderAccountServiceHandlers } from './provider-account-service-routes'
import {
  type CodexOAuthAuth,
  type ProviderAccountBundle,
  listProviderAccountsForUser,
  makeD1ProviderAccountRepository,
} from './provider-accounts'
import { handlePublicAdjutantActivityApi } from './public-adjutant-activity-routes'
import { handlePublicLaunchDashboardApi } from './public-launch-dashboard-routes'
import { handlePublicOtecProofApi } from './public-otec-proof-routes'
import { handlePublicPylonStatsApi } from './public-pylon-stats-routes'
import { makeD1PylonApiStore } from './pylon-api'
import { makePylonApiRoutes } from './pylon-api-routes'
import { makeD1PylonMarketplaceJobStore } from './pylon-marketplace-service'
import { handleResendWebhook } from './resend-webhooks'
import {
  OpenAgentsWorkerRequest,
  WorkerRequestLayer,
  openAgentsDatabase,
  scheduleBackgroundWork,
} from './runtime'
import {
  compactRandomId,
  currentDate,
  currentIsoTimestamp,
  isoTimestampAfter,
  randomUuid,
} from './runtime-primitives'
import { makeShareRoutes } from './share-routes'
import { handleSignaturePackageValidationApi } from './signature-package-validation-routes'
import { makeD1SiteCommerceReviewStore } from './site-commerce-review'
import { makeSiteCommerceRoutes } from './site-commerce-routes'
import { makeD1SiteMdkAccountBindingStore } from './site-mdk-account-bindings'
import { makeD1SiteMdkCheckoutIntentStore } from './site-mdk-checkout-intents'
import { omegaMdkDemoSitePaymentCatalog } from './site-mdk-demo-product'
import {
  type ReferralConsumptionResult,
  consumePendingReferralForUser,
} from './site-referral-attribution-consumption'
import { makeSiteReferralInspectionRoutes } from './site-referral-inspection-routes'
import { sendSiteReferralOnboardingForConsumption } from './site-referral-onboarding'
import { makeSiteReferralRoutes } from './site-referral-routes'
import { PENDING_REFERRAL_COOKIE } from './site-referrals'
import { makeSiteRuntimeRoutes } from './site-runtime-routes'
import {
  type SyncNotificationContext,
  notifyAgentRunSyncScopes,
  notifySyncScopes,
  publishTeamChatMessageSync,
  publishTeamThreadFileSync,
} from './sync-notifier'
import { type ParsedSyncPath, makeSyncRoutes } from './sync-routes'
import {
  type TeamChatMessage,
  type TeamChatRunSummary,
  insertTeamChatMessage,
  listTeamChatMessages,
  makeTeamChatMessageId,
  makeTeamChatThreadId,
  readTeamChatMessageByAgentRunId,
  readTeamChatMessageById,
  teamChatLaunchErrorFromResponse,
  updateTeamChatMessageRunSummary,
} from './team-chat'
import { makeTeamChatRoutes } from './team-chat-routes'
import {
  type UserTeam,
  type UserTeamProject,
  readActiveTeamMembershipRole,
  readActiveTeamProject,
  readTeamsForUser,
} from './team-repository'
import {
  type RouteAccessError,
  RouteAccessForbidden,
  RouteAccessNotFound,
  ThreadAccessService,
} from './thread-access'
import { makeThreadFileRoutes } from './thread-file-routes'
import {
  type PublicThreadFile,
  type ThreadFileRow,
  insertThreadFileMessageReferences,
  listTeamThreadFiles,
  readThreadFileById,
} from './thread-files'
import {
  type AutopilotTokenLeaderboards,
  TokenUsageLeaderboards,
} from './token-usage'
import { makeTokenUsageLedgerRoutes } from './token-usage-ledger-routes'
import { makeTreasuryPaymentAuthority } from './treasury-payment-authority'
import { makeHostedMdkPayoutAdapter } from './treasury-payment-hosted-mdk-payout-adapter'
import {
  type ViralAgentFunnelEventKind,
  recordViralAgentFunnelEvent,
} from './viral-agent-funnel'
import { makeWorkerRouteRequest } from './worker-routes'

export type Env = WorkerBindings & OpenAgentsWorkerConfigEnv

type EmailCampaignDispatcherBindings = WorkerBindings &
  OpenAgentsWorkerConfigEnv
export {
  SESSION_MAX_AGE_SECONDS,
  appendClearSessionCookies,
  appendSessionCookies,
} from './auth-cookies'
export {
  publishTeamChatMessageSync,
  publishTeamThreadFileSync,
  teamChatMessageSyncValue,
  threadFileSyncValue,
} from './sync-notifier'

export const OPENAGENTS_ADMIN_EMAILS = ['chris@openagents.com'] as const
const OPENAGENTS_CORE_TEAM_ID = 'team_openagents_core'
const MDK_SIDECAR_INSTANCE_NAME = 'openagents-mdk-sidecar-20260607-4'
const SIMPLE_EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
const workerRuntime = {
  makeUuid: randomUuid,
  now: currentDate,
  nowIso: currentIsoTimestamp,
} as const

class MdkSidecarUnavailable extends S.TaggedErrorClass<MdkSidecarUnavailable>()(
  'MdkSidecarUnavailable',
  {
    error: S.Defect,
  },
) {}
class UnsupportedAuthProvider extends S.TaggedErrorClass<UnsupportedAuthProvider>()(
  'UnsupportedAuthProvider',
  {
    provider: S.String,
  },
) {}

const optionalMdkContainerSecret = (
  value: string | undefined,
): string | undefined => {
  const trimmed = value?.trim()

  return trimmed === undefined || trimmed === '' ? undefined : trimmed
}

const mdkContainerEnvVars = (
  environment: OpenAgentsWorkerConfigEnv,
): Record<string, string> => {
  const accessToken = optionalMdkContainerSecret(environment.MDK_ACCESS_TOKEN)
  const mnemonic = optionalMdkContainerSecret(environment.MDK_MNEMONIC)
  const webhookSecret = optionalMdkContainerSecret(
    environment.MDK_WEBHOOK_SECRET,
  )

  return {
    ...(accessToken === undefined ? {} : { MDK_ACCESS_TOKEN: accessToken }),
    ...(mnemonic === undefined ? {} : { MDK_MNEMONIC: mnemonic }),
    ...(webhookSecret === undefined
      ? {}
      : { MDK_WEBHOOK_SECRET: webhookSecret }),
  }
}

export class MdkSidecarContainer extends Container<Env> {
  override defaultPort = 8080
  override sleepAfter = '30m'
  override pingEndpoint = 'localhost:8080/healthz'

  constructor(ctx: DurableObjectState<{}>, environment: Env) {
    super(ctx, environment)
    this.envVars = mdkContainerEnvVars(environment)
    this.labels = {
      service: 'openagents-mdk-sidecar',
    }
  }
}

const fetchMdkSidecarRequest = async (request: Request, environment: Env) => {
  if (environment.MDK_SIDECAR === undefined) {
    return noStoreJsonResponse(
      { error: 'mdk_sidecar_unconfigured' },
      { status: 503 },
    )
  }

  try {
    return await getContainer(
      environment.MDK_SIDECAR as DurableObjectNamespace<MdkSidecarContainer>,
      MDK_SIDECAR_INSTANCE_NAME,
    ).fetch(request)
  } catch {
    return noStoreJsonResponse(
      { error: 'mdk_sidecar_unavailable' },
      { status: 503 },
    )
  }
}

const routeMdkSidecarRequest = (request: Request, environment: Env) =>
  Effect.tryPromise({
    catch: error => new MdkSidecarUnavailable({ error }),
    try: () => fetchMdkSidecarRequest(request, environment),
  }).pipe(
    Effect.catchTag('MdkSidecarUnavailable', () =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'mdk_sidecar_unavailable' },
          { status: 503 },
        ),
      ),
    ),
  )
const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const EmailString = NonEmptyTrimmedString.check(
  S.isPattern(SIMPLE_EMAIL_PATTERN),
)

const UserSubject = S.Struct({
  userId: NonEmptyTrimmedString,
  provider: S.Literal('github'),
  githubId: NonEmptyTrimmedString,
  login: NonEmptyTrimmedString,
  email: EmailString,
  name: NonEmptyTrimmedString,
  avatarUrl: S.String,
})

type UserSubject = typeof UserSubject.Type

const subjects = createSubjects({
  user: S.toStandardSchemaV1(UserSubject),
})

export const isOpenAgentsAdminEmail = (email: string): boolean =>
  OPENAGENTS_ADMIN_EMAILS.some(
    adminEmail => adminEmail === email.trim().toLowerCase(),
  )

const GitHubUser = S.Struct({
  id: S.Union([S.Number, S.String]),
  login: NonEmptyTrimmedString,
  name: S.optionalKey(S.NullOr(S.String)),
  avatar_url: S.optionalKey(S.NullOr(S.String)),
})

const GitHubEmail = S.Struct({
  email: EmailString,
  primary: S.Boolean,
  verified: S.Boolean,
})

const GitHubEmails = S.Array(GitHubEmail)

const GitHubOAuthToken = S.Struct({
  access_token: NonEmptyTrimmedString,
  scope: S.optionalKey(TrimmedString),
  token_type: S.optionalKey(TrimmedString),
})

const GITHUB_LOGIN_SCOPES = ['read:user', 'user:email', 'repo'] as const
const LOGIN_ORIGIN_COOKIE = 'oa_login_origin'
const LOGIN_RETURN_TO_COOKIE = 'oa_login_return_to'

type GitHubUser = typeof GitHubUser.Type
type GitHubEmail = typeof GitHubEmail.Type
type GitHubOAuthToken = typeof GitHubOAuthToken.Type

type UserKindTotals = Readonly<{
  admins: number
  adminEmails: ReadonlyArray<string>
  humans: number
  agents: number
  total: number
}>

type TeamAutopilotContextMessage = Readonly<{
  authorName: string
  authorUserId: string
  body: string
  createdAt: string
}>

type TeamAutopilotContextFile = Pick<
  PublicThreadFile,
  'contentType' | 'createdAt' | 'filename' | 'id' | 'sizeBytes'
>

type TeamAutopilotSelectedFile = TeamAutopilotContextFile &
  Readonly<{ excerpt?: string }>

export type TeamAutopilotContextBundle = Readonly<{
  normalizedPrompt: string
  parentProjectId?: string
  parentProjectName?: string
  parentTeamChatMessageId: string
  parentTeamId: string
  recentMessages: ReadonlyArray<TeamAutopilotContextMessage>
  selectedFiles: ReadonlyArray<TeamAutopilotSelectedFile>
  selectedTeamFileIds: ReadonlyArray<string>
}>

const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

const optionalUuid = (value: unknown): string | undefined => {
  const text = optionalString(value)

  return text !== undefined && uuidPattern.test(text)
    ? text.toLowerCase()
    : undefined
}

type AuthenticatedActor =
  | Readonly<{
      kind: 'human'
      user: UserSubject
      tokens?: Tokens
    }>
  | Readonly<{
      kind: 'agent'
      agent: ProgrammaticAgentSession
    }>

const teamChatThreadId = (teamId: string): string => `team:${teamId}:chat`

const teamProjectChatThreadId = (teamId: string, projectId: string): string =>
  `team:${teamId}:project:${projectId}:chat`

const compactTeamContextText = (value: string, maxLength: number): string => {
  const compact = value.replace(/\s+/g, ' ').trim()

  return compact.length <= maxLength
    ? compact
    : `${compact.slice(0, Math.max(0, maxLength - 3))}...`
}

export const selectedTeamFileIdsForAutopilotPrompt = (
  input: Readonly<{
    files: ReadonlyArray<TeamAutopilotContextFile>
    prompt: string
    requestedFileIds?: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => {
  const teamFileIds = new Set(input.files.map(file => file.id))
  const requested = (input.requestedFileIds ?? []).filter(fileId =>
    teamFileIds.has(fileId),
  )

  return [...new Set(requested)].slice(0, 8)
}

export const teamAutopilotContextBundle = (
  input: Readonly<{
    files: ReadonlyArray<TeamAutopilotContextFile>
    messages: ReadonlyArray<TeamChatMessage>
    project?: UserTeamProject
    parentTeamChatMessageId: string
    prompt: string
    requestedFileIds?: ReadonlyArray<string>
    teamId: string
  }>,
): TeamAutopilotContextBundle => {
  const selectedTeamFileIds = selectedTeamFileIdsForAutopilotPrompt({
    files: input.files,
    prompt: input.prompt,
    ...(input.requestedFileIds === undefined
      ? {}
      : { requestedFileIds: input.requestedFileIds }),
  })
  const fileById = new Map(input.files.map(file => [file.id, file]))

  return {
    normalizedPrompt: compactTeamContextText(input.prompt, 4_000),
    ...(input.project === undefined
      ? {}
      : {
          parentProjectId: input.project.id,
          parentProjectName: input.project.name,
        }),
    parentTeamChatMessageId: input.parentTeamChatMessageId,
    parentTeamId: input.teamId,
    recentMessages: input.messages.slice(-8).map(message => ({
      authorName: message.author.name,
      authorUserId: message.author.userId,
      body: compactTeamContextText(message.body, 320),
      createdAt: message.createdAt,
    })),
    selectedFiles: selectedTeamFileIds.flatMap(fileId => {
      const file = fileById.get(fileId)

      return file === undefined ? [] : [file]
    }),
    selectedTeamFileIds,
  }
}

const teamAutopilotContextLine = (
  message: TeamAutopilotContextMessage,
): string =>
  `- ${message.createdAt} ${message.authorName} (${message.authorUserId}): ${message.body}`

const teamAutopilotSelectedFileLines = (
  file: TeamAutopilotSelectedFile,
): ReadonlyArray<string> => [
  `- ${file.filename} (${file.contentType}, ${file.sizeBytes} bytes, id ${file.id})`,
  ...(file.excerpt === undefined ? [] : [`  excerpt: ${file.excerpt}`]),
]

export const teamAutopilotChildRunGoal = (
  bundle: TeamAutopilotContextBundle,
): string =>
  [
    bundle.normalizedPrompt,
    '',
    'Team room context for this Autopilot run:',
    `parentTeamId: ${bundle.parentTeamId}`,
    ...(bundle.parentProjectId === undefined
      ? []
      : [
          `parentProjectId: ${bundle.parentProjectId}`,
          `parentProjectName: ${bundle.parentProjectName ?? 'unknown project'}`,
        ]),
    `parentTeamChatMessageId: ${bundle.parentTeamChatMessageId}`,
    `normalizedPrompt: ${bundle.normalizedPrompt}`,
    `selectedTeamFileIds: ${
      bundle.selectedTeamFileIds.length === 0
        ? 'none'
        : bundle.selectedTeamFileIds.join(', ')
    }`,
    'selectedTeamFiles:',
    ...(bundle.selectedFiles.length === 0
      ? ['- none']
      : bundle.selectedFiles.flatMap(teamAutopilotSelectedFileLines)),
    'recentTeamConversation:',
    ...(bundle.recentMessages.length === 0
      ? ['- none']
      : bundle.recentMessages.map(teamAutopilotContextLine)),
    '',
    'Answer the team room directly. Use the context above only to orient the run; do not narrate internal dispatch or writeback mechanics to the user.',
  ].join('\n')

const getAppOrigin = (env: OpenAgentsWorkerConfigEnv): string =>
  getOpenAgentsWorkerConfig(env).app.origin

const getResendEmailConfig = (env: EmailCampaignDispatcherBindings) =>
  getOpenAgentsWorkerConfig(env).email.resend

const getRunnerBackendConfig = (env: OpenAgentsWorkerConfigEnv) =>
  getOpenAgentsWorkerConfig(env).runnerBackends

const getIssuerOrigin = (env: Env): string =>
  getOpenAgentsWorkerConfig(env).openauth.issuerOrigin

const readBearerToken = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')

  if (scheme?.toLowerCase() !== 'bearer' || token === undefined) {
    return undefined
  }

  return token
}

const getAdminApiToken = (env: Env): string | undefined => {
  const token = redactedValue(getOpenAgentsWorkerConfig(env).adminApiToken)

  if (token === undefined || token.trim() === '') {
    return undefined
  }

  return token
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const errorName = (error: unknown): string =>
  error instanceof Error ? error.name : typeof error

class GitHubFetchFailure extends Error {
  override name = 'GitHubFetchFailure'
}

const isUniqueConstraintError = (error: unknown): boolean =>
  errorMessage(error).includes('UNIQUE constraint failed')

const fetchGitHubJson = async <T>(
  schema: S.Decoder<T>,
  url: string,
  accessToken: string,
): Promise<T> => {
  const response = await fetch(url, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
      Accept: 'application/vnd.github+json',
      'User-Agent': 'OpenAgents',
      'X-GitHub-Api-Version': '2022-11-28',
    },
  })

  if (!response.ok) {
    throw new GitHubFetchFailure(
      `GitHub request failed with ${response.status}`,
    )
  }

  return decodeUnknownWithSchema(schema, await response.json())
}

const parseGitHubScopeHeader = (
  value: string | undefined,
): ReadonlyArray<string> =>
  value === undefined
    ? []
    : value
        .split(',')
        .map(scope => scope.trim())
        .filter(scope => scope !== '')

const gitHubWriteRedirectUri = (env: Env): string =>
  `${getIssuerOrigin(env)}/github/callback`

const gitHubWriteAuthorizeUrl = (
  env: Env,
  state: string,
  scopes: ReadonlyArray<string>,
): string => {
  const config = getOpenAgentsWorkerConfig(env)
  const url = new URL('https://github.com/login/oauth/authorize')
  url.searchParams.set('client_id', config.github.clientId)
  url.searchParams.set('redirect_uri', gitHubWriteRedirectUri(env))
  url.searchParams.set('response_type', 'code')
  url.searchParams.set('scope', scopes.join(' '))
  url.searchParams.set('state', state)

  return url.toString()
}

const exchangeGitHubOAuthCode = async (
  env: Env,
  code: string,
): Promise<GitHubOAuthToken> => {
  const config = getOpenAgentsWorkerConfig(env)
  const response = await fetch('https://github.com/login/oauth/access_token', {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'content-type': 'application/x-www-form-urlencoded',
      'User-Agent': 'OpenAgents',
    },
    body: new URLSearchParams({
      client_id: config.github.clientId,
      client_secret: redactedValue(config.github.clientSecret) ?? '',
      code,
      redirect_uri: gitHubWriteRedirectUri(env),
    }).toString(),
  })

  if (!response.ok) {
    throw new GitHubWriteApiFailure({
      operation: 'oauth_token_exchange',
      status: response.status,
      message: `GitHub OAuth token exchange failed with ${response.status}`,
    })
  }

  return decodeUnknownWithSchema(GitHubOAuthToken, await response.json())
}

type GitHubWriteTokenStorage = Readonly<{
  AUTH_STORAGE: KVNamespace
}>

const storeGitHubWriteAccessToken = async (
  storage: GitHubWriteTokenStorage,
  connectionRef: string,
  accessToken: string,
): Promise<void> => {
  try {
    await storage.AUTH_STORAGE.put(
      githubWriteSecretKey(connectionRef),
      accessToken,
    )
  } catch (error) {
    throw new GitHubWriteTokenStorageFailure({
      operation: 'put',
      message:
        error instanceof Error
          ? error.message
          : 'GitHub write token storage failed.',
    })
  }
}

const getPrimaryVerifiedEmail = (
  emails: ReadonlyArray<GitHubEmail>,
): GitHubEmail => {
  const primary = emails.find(email => email.primary)

  if (primary === undefined) {
    throw new Error('No primary GitHub email found')
  }

  if (!primary.verified) {
    throw new Error('Primary GitHub email is not verified')
  }

  return primary
}

const githubUserToSubject = (
  user: GitHubUser,
  primaryEmail: GitHubEmail,
): UserSubject => {
  const githubId = String(user.id)
  const login = user.login

  return {
    userId: `github:${githubId}`,
    provider: 'github',
    githubId,
    login,
    email: primaryEmail.email,
    name: user.name ?? login,
    avatarUrl: user.avatar_url ?? '',
  }
}

const upsertGitHubUser = async (
  db: D1Database,
  user: UserSubject,
): Promise<void> => {
  const now = workerRuntime.nowIso()

  await db.batch([
    db
      .prepare(
        `INSERT INTO users
          (id, kind, display_name, primary_email, avatar_url, status, created_at, updated_at)
         VALUES (?, 'human', ?, ?, ?, 'active', ?, ?)
         ON CONFLICT(id) DO UPDATE SET
          display_name = excluded.display_name,
          primary_email = excluded.primary_email,
          avatar_url = excluded.avatar_url,
          status = 'active',
          updated_at = excluded.updated_at,
          deleted_at = NULL`,
      )
      .bind(user.userId, user.name, user.email, user.avatarUrl, now, now),
    db
      .prepare(
        `INSERT INTO auth_identities
          (id, user_id, provider, provider_subject, provider_username, email, created_at, updated_at)
         VALUES (?, ?, 'github', ?, ?, ?, ?, ?)
         ON CONFLICT(provider, provider_subject) DO UPDATE SET
          user_id = excluded.user_id,
          provider_username = excluded.provider_username,
          email = excluded.email,
          updated_at = excluded.updated_at,
          deleted_at = NULL`,
      )
      .bind(
        `auth_identity_github_${user.githubId}`,
        user.userId,
        user.githubId,
        user.login,
        user.email,
        now,
        now,
      ),
  ])
}

const readUserKindTotals = async (db: D1Database): Promise<UserKindTotals> => {
  const rows = await db
    .prepare(
      `SELECT kind, COUNT(*) AS count
       FROM users
       WHERE status = 'active'
         AND deleted_at IS NULL
       GROUP BY kind`,
    )
    .all<Readonly<{ kind: 'human' | 'agent'; count: number }>>()
  const countFor = (kind: 'human' | 'agent') =>
    rows.results.find(row => row.kind === kind)?.count ?? 0
  const humans = countFor('human')
  const agents = countFor('agent')

  return {
    admins: OPENAGENTS_ADMIN_EMAILS.length,
    adminEmails: [...OPENAGENTS_ADMIN_EMAILS],
    humans,
    agents,
    total: humans + agents,
  }
}

const TEAM_AUTOPILOT_COMMAND = '@autopilot'
const TEAM_ADJUTANT_COMMAND = '@adjutant'
const ADJUTANT_PROJECT_ID = 'project_adjutant'
const ADJUTANT_INTENT_SCHEMA_VERSION = 'openagents.team_chat.adjutant_intent.v1'
const softwareOrderIdPattern = /^software_order_[A-Za-z0-9_-]+$/
const siteIdPattern = /^site_project_[A-Za-z0-9_-]+$/
const taskSpecPathPattern =
  /^docs\/autopilot-tasks\/[A-Za-z0-9][A-Za-z0-9._/-]*\.md$/
const softwareOrderTokenPattern =
  /(?:^|[\s,;])(?:softwareOrderId|software_order_id|orderId|order)\s*[:=]\s*(software_order_[A-Za-z0-9_-]+)/gi
const siteTokenPattern =
  /(?:^|[\s,;])(?:siteId|site_id|site)\s*[:=]\s*(site_project_[A-Za-z0-9_-]+)/gi
const taskSpecTokenPattern =
  /(?:^|[\s,;])(?:taskSpecPath|task_spec_path|taskPacketPath|task_packet_path|task)\s*[:=]\s*(docs\/autopilot-tasks\/[A-Za-z0-9][A-Za-z0-9._/-]*\.md)/gi

export type TeamAdjutantIntent = Readonly<{
  schemaVersion: typeof ADJUTANT_INTENT_SCHEMA_VERSION
  prompt: string
  softwareOrderId?: string | undefined
  siteId?: string | undefined
  taskSpecPath?: string | undefined
}>

export const teamAutopilotPromptFromBody = (messageBody: string): string => {
  const trimmed = messageBody.trim()
  const lower = trimmed.toLowerCase()
  const prefix = `${TEAM_AUTOPILOT_COMMAND} `

  if (lower.startsWith(prefix)) {
    const prompt = trimmed.slice(TEAM_AUTOPILOT_COMMAND.length).trim()

    return prompt === '' ? messageBody : prompt
  }

  const lines = messageBody.split(/\r?\n/)
  const commandLineIndex = lines.findIndex(
    line => line.trim().toLowerCase() === TEAM_AUTOPILOT_COMMAND,
  )

  if (commandLineIndex !== -1) {
    const prompt = [
      ...lines.slice(0, commandLineIndex),
      ...lines.slice(commandLineIndex + 1),
    ]
      .join('\n')
      .trim()

    return prompt === '' ? messageBody : prompt
  }

  const suffix = ` ${TEAM_AUTOPILOT_COMMAND}`

  if (lower.endsWith(suffix)) {
    const prompt = trimmed.slice(0, -TEAM_AUTOPILOT_COMMAND.length).trim()

    return prompt === '' ? messageBody : prompt
  }

  return messageBody
}

const exactTeamCommandPromptFromBody = (
  messageBody: string,
  command: string,
): string | undefined => {
  const trimmed = messageBody.trim()
  const lower = trimmed.toLowerCase()
  const prefix = `${command} `

  if (lower.startsWith(prefix)) {
    return trimmed.slice(command.length).trim()
  }

  const lines = messageBody.split(/\r?\n/)
  const commandLineIndex = lines.findIndex(
    line => line.trim().toLowerCase() === command,
  )

  if (commandLineIndex !== -1) {
    return [
      ...lines.slice(0, commandLineIndex),
      ...lines.slice(commandLineIndex + 1),
    ]
      .join('\n')
      .trim()
  }

  const suffix = ` ${command}`

  if (lower.endsWith(suffix)) {
    return trimmed.slice(0, -command.length).trim()
  }

  return undefined
}

const exactTeamAdjutantPromptFromBody = (
  messageBody: string,
): string | undefined =>
  exactTeamCommandPromptFromBody(messageBody, TEAM_AUTOPILOT_COMMAND) ??
  exactTeamCommandPromptFromBody(messageBody, TEAM_ADJUTANT_COMMAND)

const firstRegexCapture = (
  pattern: RegExp,
  value: string,
): string | undefined => {
  pattern.lastIndex = 0
  const match = pattern.exec(value)
  pattern.lastIndex = 0

  return match?.[1]
}

const cleanAdjutantPrompt = (prompt: string): string =>
  prompt
    .replace(softwareOrderTokenPattern, ' ')
    .replace(siteTokenPattern, ' ')
    .replace(taskSpecTokenPattern, ' ')
    .replace(/[ \t]{2,}/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim()

const boundedTaskSpecPath = (value: string | undefined): string | undefined =>
  value !== undefined &&
  taskSpecPathPattern.test(value) &&
  !value.includes('..') &&
  !value.includes('//')
    ? value
    : undefined

const explicitAdjutantContextString = (
  value: unknown,
  pattern: RegExp,
): string | undefined => {
  const text = optionalString(value)

  return text !== undefined && pattern.test(text) ? text : undefined
}

export const teamAdjutantIntentFromBody = (
  messageBody: string,
): TeamAdjutantIntent | undefined => {
  const prompt = exactTeamAdjutantPromptFromBody(messageBody)

  if (prompt === undefined) {
    return undefined
  }

  const softwareOrderId = firstRegexCapture(softwareOrderTokenPattern, prompt)
  const siteId = firstRegexCapture(siteTokenPattern, prompt)
  const taskSpecPath = boundedTaskSpecPath(
    firstRegexCapture(taskSpecTokenPattern, prompt),
  )
  const cleanPrompt = cleanAdjutantPrompt(prompt)

  return {
    schemaVersion: ADJUTANT_INTENT_SCHEMA_VERSION,
    prompt: cleanPrompt === '' ? prompt : cleanPrompt,
    ...(softwareOrderId === undefined ? {} : { softwareOrderId }),
    ...(siteId === undefined ? {} : { siteId }),
    ...(taskSpecPath === undefined ? {} : { taskSpecPath }),
  }
}

const teamAutopilotAnswerBackId = (runId: string): string =>
  `team_chat_answer_${runId}`

const genericAutopilotEventText = new Set([
  'Assistant message completed.',
  'Codex one-shot run completed.',
  'Codex one-shot turn completed.',
  'Codex run resource usage receipt emitted.',
  'Codex workspace removed.',
  'Closeout receipt emitted.',
  'OpenCode run completed and closeout manifest submitted.',
  'OpenCode/Codex one-shot run completed.',
  'OpenCode/Codex one-shot turn completed.',
  'OpenCode/Codex run finished with status completed.',
  'Codex VM artifact captured.',
  'Codex VM receipt emitted.',
  'Runner event received.',
  'stdout JSON event captured.',
])

const internalAutopilotAnswerPhrases = [
  'closeout receipt',
  'closeout manifest',
  'completion artifact',
  'completion artifacts',
  'github-writeback.json',
  'local artifact',
  'local artifacts',
  'record the requested summary',
  'result.md',
  'run artifact',
  'run artifacts',
  'run outcome',
  'usage receipt',
  'workspace removed',
]

const isVisibleAutopilotAnswerText = (text: string): boolean => {
  const compact = compactTeamContextText(text, 3_800)
  const normalized = compact.toLowerCase()

  if (genericAutopilotEventText.has(compact)) {
    return false
  }

  if (
    internalAutopilotAnswerPhrases.some(phrase => normalized.includes(phrase))
  ) {
    return false
  }

  if (
    normalized.includes('artifact') &&
    (normalized.includes('required') ||
      normalized.includes('prepare') ||
      normalized.includes('write') ||
      normalized.includes('adding') ||
      normalized.includes('record'))
  ) {
    return false
  }

  return true
}

const eventPayloadRecord = (
  event: Readonly<{ payloadJson: string | null }>,
): Record<string, unknown> | undefined => safeJsonRecord(event.payloadJson)

const rawEventPayloadRecord = (
  event: Readonly<{ payloadJson: string | null }>,
): Record<string, unknown> | undefined => {
  const payload = eventPayloadRecord(event)
  const dataJson = optionalString(payload?.dataJson)
  const rawPayloadJson =
    optionalString(payload?.rawPayloadJson) ??
    optionalString(payload?.raw_payload_json)

  return safeJsonRecord(dataJson) ?? safeJsonRecord(rawPayloadJson) ?? payload
}

const eventRawPart = (
  event: Readonly<{ payloadJson: string | null }>,
): Record<string, unknown> | undefined => {
  const raw = rawEventPayloadRecord(event)
  const direct = raw?.part
  const propertiesPart = nestedUnknown(raw, ['properties', 'part'])

  return isRecord(direct)
    ? direct
    : isRecord(propertiesPart)
      ? propertiesPart
      : undefined
}

const eventLooksLikeToolCall = (
  event: Readonly<{ payloadJson: string | null; type: string }>,
): boolean => {
  const raw = rawEventPayloadRecord(event)
  const rawType = optionalString(raw?.type)
  const part = eventRawPart(event)

  return (
    event.type.includes('tool') ||
    rawType === 'tool_use' ||
    rawType === 'tool_result' ||
    optionalString(raw?.tool) !== undefined ||
    optionalString(part?.tool) !== undefined
  )
}

const eventTokenTotal = (
  event: Readonly<{ payloadJson: string | null }>,
): number => {
  const raw = rawEventPayloadRecord(event)
  const total =
    optionalInteger(nestedUnknown(raw, ['usage', 'totalTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'total_tokens'])) ??
    optionalInteger(nestedUnknown(raw, ['tokenUsage', 'totalTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['token_usage', 'total_tokens'])) ??
    optionalInteger(nestedUnknown(raw, ['totalTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['total_tokens']))

  if (total !== undefined) {
    return total
  }

  const input =
    optionalInteger(nestedUnknown(raw, ['usage', 'inputTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'input_tokens'])) ??
    0
  const output =
    optionalInteger(nestedUnknown(raw, ['usage', 'outputTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'output_tokens'])) ??
    0
  const reasoning =
    optionalInteger(nestedUnknown(raw, ['usage', 'reasoningTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'reasoning_tokens'])) ??
    0

  return input + output + reasoning
}

const durationSecondsForRun = (
  run: Pick<
    AgentRunRecord,
    | 'canceledAt'
    | 'completedAt'
    | 'createdAt'
    | 'failedAt'
    | 'startedAt'
    | 'updatedAt'
  >,
): number | null => {
  const startedAt = Date.parse(run.startedAt ?? run.createdAt)
  const endedAt = Date.parse(
    run.completedAt ?? run.failedAt ?? run.canceledAt ?? run.updatedAt,
  )

  return Number.isFinite(startedAt) && Number.isFinite(endedAt)
    ? Math.max(0, Math.round((endedAt - startedAt) / 1000))
    : null
}

const teamChatRunSummaryFromBundle = (
  bundle: AgentRunBundle,
): TeamChatRunSummary => ({
  runId: bundle.run.id,
  status: bundle.run.status,
  runtime: bundle.run.runtime,
  backend: bundle.run.backend,
  repository: `${bundle.run.repository.owner}/${bundle.run.repository.repo}@${bundle.run.repository.ref}`,
  eventCount: bundle.events.length,
  toolCallCount: bundle.events.filter(eventLooksLikeToolCall).length,
  tokenTotal: bundle.events.reduce(
    (total, event) => total + eventTokenTotal(event),
    0,
  ),
  durationSeconds: durationSecondsForRun(bundle.run),
  updatedAt: bundle.run.updatedAt,
})

const assistantAnswerTextFromEvent = (
  event: Readonly<{
    payloadJson: string | null
    summary: string
    type: string
  }>,
): string | undefined => {
  const payload = rawEventPayloadRecord(event)
  const rawType =
    optionalString(payload?.type) ??
    optionalString(payload?.event) ??
    event.type

  if (
    event.type !== 'message.completed' &&
    event.type !== 'message.part.updated' &&
    rawType !== 'message.completed' &&
    rawType !== 'message.part.updated' &&
    rawType !== 'text'
  ) {
    return undefined
  }

  const propertiesPart = nestedUnknown(payload, ['properties', 'part'])
  const directPart = nestedUnknown(payload, ['part'])
  const part = isRecord(propertiesPart)
    ? propertiesPart
    : isRecord(directPart)
      ? directPart
      : undefined
  const text =
    (rawType === 'message.part.updated' || rawType === 'text') &&
    (optionalString(part?.type) ?? 'text') === 'text'
      ? (optionalString(part?.text) ?? optionalString(part?.content))
      : (optionalNestedString(payload, [
          ['finalAnswer'],
          ['final_answer'],
          ['answer'],
          ['text'],
          ['detail'],
          ['message'],
          ['content'],
          ['output'],
          ['response', 'output_text'],
          ['properties', 'part', 'text'],
          ['properties', 'part', 'content'],
          ['part', 'text'],
          ['part', 'content'],
          ['item', 'text'],
          ['item', 'message'],
          ['item', 'content', '0', 'text'],
        ]) ?? (event.type === 'message.completed' ? event.summary : undefined))
  const compact =
    text === undefined ? undefined : compactTeamContextText(text, 3_800)

  return compact === undefined || !isVisibleAutopilotAnswerText(compact)
    ? undefined
    : compact
}

export const teamAutopilotAnswerBackDraft = (
  bundle: Pick<AgentRunBundle, 'events'>,
): Readonly<{ body: string; sourceEventId: string | null }> => {
  const chronological = [...bundle.events].sort((left, right) =>
    left.sequence === right.sequence
      ? Date.parse(left.createdAt) - Date.parse(right.createdAt)
      : left.sequence - right.sequence,
  )
  const candidates = chronological
    .map(event => ({
      body: assistantAnswerTextFromEvent(event),
      sourceEventId: event.id,
    }))
    .filter(
      (value): value is Readonly<{ body: string; sourceEventId: string }> =>
        value.body !== undefined,
    )
  const candidate = candidates[candidates.length - 1]

  return candidate === undefined
    ? {
        body: 'Autopilot completed the run. Open the linked thread for the full transcript.',
        sourceEventId: null,
      }
    : candidate
}

const resultArtifactEventId = (
  events: ReadonlyArray<OmniEventRecord>,
): string | undefined => {
  const candidates = [...events]
    .sort((left, right) =>
      left.sequence === right.sequence
        ? Date.parse(left.createdAt) - Date.parse(right.createdAt)
        : left.sequence - right.sequence,
    )
    .filter(event => {
      const raw = rawEventPayloadRecord(event)
      const detail = optionalString(raw?.detail)
      const artifactName = optionalString(raw?.artifactName)
      const filename = optionalString(raw?.filename)

      return (
        event.type === 'artifact.created' &&
        (detail === 'result.md' ||
          artifactName === 'result.md' ||
          filename === 'result.md')
      )
    })

  return candidates[candidates.length - 1]?.id
}

const githubContentsUrl = (
  repository: AgentRunRecord['repository'],
  branchName: string,
  path: string,
): string | undefined => {
  if (repository.provider !== 'github') {
    return undefined
  }

  const owner = encodeURIComponent(repository.owner)
  const repo = encodeURIComponent(repository.repo)
  const encodedPath = path.split('/').map(encodeURIComponent).join('/')
  const ref = encodeURIComponent(branchName)

  return `https://api.github.com/repos/${owner}/${repo}/contents/${encodedPath}?ref=${ref}`
}

const textFromGithubContentsResponse = (
  payload: unknown,
): string | undefined => {
  if (!isRecord(payload)) {
    return undefined
  }

  const encoding = optionalString(payload.encoding)
  const content = optionalString(payload.content)

  if (encoding !== 'base64' || content === undefined) {
    return undefined
  }

  try {
    const decoded = atob(content.replace(/\s+/g, ''))
    const bytes = Uint8Array.from(decoded, char => char.charCodeAt(0))

    return new TextDecoder().decode(bytes)
  } catch {
    return undefined
  }
}

export const teamAutopilotResultArtifactAnswerBackDraft = async (
  bundle: AgentRunBundle,
  options: Readonly<{
    githubAccessToken?: string | undefined
  }> = {},
): Promise<Readonly<{ body: string; sourceEventId: string }> | undefined> => {
  const sourceEventId = resultArtifactEventId(bundle.events)
  const branchName = bundle.run.assignment.githubWorkOrder?.branchName

  if (sourceEventId === undefined || branchName === undefined) {
    return undefined
  }

  const url = githubContentsUrl(bundle.run.repository, branchName, 'result.md')

  if (url === undefined) {
    return undefined
  }

  const response = await fetch(url, {
    headers: {
      accept: 'application/vnd.github+json',
      ...(options.githubAccessToken === undefined
        ? {}
        : { authorization: `Bearer ${options.githubAccessToken}` }),
      'user-agent': 'openagents-autopilot-worker',
    },
  }).catch((): Response | undefined => undefined)

  if (response === undefined || !response.ok) {
    return undefined
  }

  const payload = await response.json().catch((): unknown => undefined)
  const text = textFromGithubContentsResponse(payload)
  const body =
    text === undefined ? undefined : compactTeamContextText(text, 3_800)

  return body === undefined || !isVisibleAutopilotAnswerText(body)
    ? undefined
    : {
        body,
        sourceEventId,
      }
}

export const teamAutopilotAnswerBackDraftForBundle = async (
  bundle: AgentRunBundle,
  options: Readonly<{
    githubAccessToken?: string | undefined
  }> = {},
): Promise<Readonly<{ body: string; sourceEventId: string | null }>> =>
  (await teamAutopilotResultArtifactAnswerBackDraft(bundle, options)) ??
  teamAutopilotAnswerBackDraft(bundle)

const selectedFileIdsFromTeamMessageMetadata = (
  metadataJson: string,
): ReadonlyArray<string> => {
  const metadata = safeJsonRecord(metadataJson)
  const direct = stringArrayFromUnknown(metadata?.selectedTeamFileIds)
  const nestedIds = stringArrayFromUnknown(
    nestedUnknown(metadata, ['context', 'selectedTeamFileIds']),
  )

  return direct.length > 0 ? direct : nestedIds
}

const appendTeamAutopilotAnswerBack = async (
  env: Env,
  ctx: SyncNotificationContext,
  runId: string,
): Promise<void> => {
  const parent = await readTeamChatMessageByAgentRunId(
    openAgentsDatabase(env),
    runId,
  )

  if (parent === undefined) {
    return
  }

  const answerId = teamAutopilotAnswerBackId(runId)
  const existing = await readTeamChatMessageById(
    openAgentsDatabase(env),
    answerId,
  )

  if (existing !== undefined) {
    return
  }

  const bundle = await makeD1OmniRunStore(
    openAgentsDatabase(env),
  ).findAgentRunForUser(parent.message.author.userId, runId)

  if (bundle === undefined || bundle.run.status !== 'completed') {
    return
  }

  const updatedParent = await updateTeamChatMessageRunSummary(
    openAgentsDatabase(env),
    {
      messageId: parent.message.id,
      metadataJson: parent.metadataJson,
      runSummary: teamChatRunSummaryFromBundle(bundle),
    },
  )

  if (updatedParent !== undefined) {
    await publishTeamChatMessageSync(
      env,
      ctx,
      updatedParent,
      parent.message.author.userId,
    )
  }

  const githubAccessToken =
    bundle.run.assignment.githubWriteConnectionRef === undefined
      ? null
      : await env.AUTH_STORAGE.get(
          githubWriteSecretKey(bundle.run.assignment.githubWriteConnectionRef),
        )
  const draft = await teamAutopilotAnswerBackDraftForBundle(bundle, {
    ...(githubAccessToken === null ? {} : { githubAccessToken }),
  })
  const selectedTeamFileIds = selectedFileIdsFromTeamMessageMetadata(
    parent.metadataJson,
  )
  const message = await insertTeamChatMessage(openAgentsDatabase(env), {
    agentRunId: runId,
    authorUserId: parent.message.author.userId,
    body: draft.body,
    id: answerId,
    kind: 'system',
    metadataJson: JSON.stringify({
      agentRunId: runId,
      kind: 'autopilot_answer_back',
      parentTeamChatMessageId: parent.message.id,
      selectedTeamFileIds,
      sourceEventId: draft.sourceEventId,
    }),
    ...(parent.message.autopilotThreadId === null
      ? {}
      : { autopilotThreadId: parent.message.autopilotThreadId }),
    ...(parent.message.projectId === null
      ? {}
      : { projectId: parent.message.projectId }),
    teamId: parent.message.teamId,
  }).catch(async error => {
    const replayed = await readTeamChatMessageById(
      openAgentsDatabase(env),
      answerId,
    )

    if (replayed !== undefined) {
      return undefined
    }

    throw error
  })

  if (message === undefined) {
    return
  }

  await insertThreadFileMessageReferences(openAgentsDatabase(env), {
    fileIds: selectedTeamFileIds,
    messageId: message.id,
    referenceKind: 'autopilot_answer',
    teamId: parent.message.teamId,
    threadId:
      parent.message.autopilotThreadId ??
      (parent.message.projectId === null
        ? teamChatThreadId(parent.message.teamId)
        : teamProjectChatThreadId(
            parent.message.teamId,
            parent.message.projectId,
          )),
  })

  await publishTeamChatMessageSync(
    env,
    ctx,
    message,
    parent.message.author.userId,
  )
}

const TEAM_AUTOPILOT_FILE_CONTEXT_MAX_BYTES = 256 * 1024
const TEAM_AUTOPILOT_FILE_CONTEXT_MAX_CHARS = 24_000

const threadFileLooksTextLike = (row: ThreadFileRow): boolean => {
  const contentType = row.content_type.toLowerCase()
  const filename = row.filename.toLowerCase()

  return (
    contentType.startsWith('text/') ||
    contentType.includes('json') ||
    contentType.includes('markdown') ||
    contentType.includes('xml') ||
    contentType.includes('yaml') ||
    contentType.includes('csv') ||
    /\.(txt|md|markdown|json|jsonl|csv|tsv|xml|yaml|yml|log)$/i.test(filename)
  )
}

const teamAutopilotFileExcerpt = async (
  env: Env,
  row: ThreadFileRow,
): Promise<string | undefined> => {
  if (
    !threadFileLooksTextLike(row) ||
    row.size_bytes > TEAM_AUTOPILOT_FILE_CONTEXT_MAX_BYTES
  ) {
    return undefined
  }

  return Effect.runPromise(
    Effect.gen(function* () {
      const artifacts = yield* ThreadFileArtifacts
      const maybeObject = yield* artifacts.get(row.object_key)
      const object = Option.getOrUndefined(maybeObject)

      if (object === undefined) {
        return undefined
      }

      const text = yield* object.text

      return compactTeamContextText(text, TEAM_AUTOPILOT_FILE_CONTEXT_MAX_CHARS)
    }).pipe(
      Effect.provide(
        ThreadFileArtifacts.layer({ binding: 'ARTIFACTS' }).pipe(
          Layer.provide(Layer.succeed(WorkerEnvironment, env)),
        ),
      ),
      Effect.catchTag('R2OperationError', () =>
        Effect.sync((): undefined => undefined),
      ),
    ),
  )
}

const hydrateTeamAutopilotContextFileExcerpts = async (
  env: Env,
  bundle: TeamAutopilotContextBundle,
): Promise<TeamAutopilotContextBundle> => {
  const selectedFiles = await Promise.all(
    bundle.selectedFiles.map(async file => {
      const row = await readThreadFileById(openAgentsDatabase(env), file.id)
      const excerpt =
        row === undefined
          ? undefined
          : await teamAutopilotFileExcerpt(env, row).catch(() => undefined)

      return excerpt === undefined ? file : { ...file, excerpt }
    }),
  )

  return { ...bundle, selectedFiles }
}

const makeAuthIssuer = (env: Env) => {
  const config = getOpenAgentsWorkerConfig(env)

  return issuer({
    theme: {
      ...THEME_OPENAUTH,
      title: 'OpenAgents',
    },
    providers: {
      github: GithubProvider({
        clientID: config.github.clientId,
        clientSecret: redactedValue(config.github.clientSecret) ?? '',
        scopes: [...GITHUB_LOGIN_SCOPES],
      }),
    },
    storage: makeD1Storage(openAgentsDatabase(env)),
    subjects,
    allow: async ({ redirectURI }) => {
      const hostname = new URL(redirectURI).hostname

      return (
        hostname === 'openagents.com' ||
        hostname === 'auth.openagents.com' ||
        hostname === 'localhost' ||
        hostname === '127.0.0.1'
      )
    },
    success: async (ctx, response) => {
      if (response.provider !== 'github') {
        throw new UnsupportedAuthProvider({ provider: response.provider })
      }

      const [user, emails] = await Promise.all([
        fetchGitHubJson(
          GitHubUser,
          'https://api.github.com/user',
          response.tokenset.access,
        ),
        fetchGitHubJson(
          GitHubEmails,
          'https://api.github.com/user/emails',
          response.tokenset.access,
        ),
      ])

      const subject = githubUserToSubject(user, getPrimaryVerifiedEmail(emails))
      await upsertGitHubUser(openAgentsDatabase(env), subject)
      await env.AUTH_STORAGE.put(
        githubIdentityTokenKey(subject.userId),
        response.tokenset.access,
        { expirationTtl: SESSION_MAX_AGE_SECONDS },
      )

      return ctx.subject('user', subject, {
        subject: subject.userId,
        ttl: {
          access: SESSION_MAX_AGE_SECONDS,
          refresh: SESSION_MAX_AGE_SECONDS,
        },
      })
    },
  })
}

const makeIssuerAwareFetch =
  (env: Env, ctx: ExecutionContext) =>
  async (
    input: Parameters<typeof fetch>[0],
    init?: Parameters<typeof fetch>[1],
  ): Promise<Response> => {
    const request = new Request(input, init)
    const url = new URL(request.url)
    const issuerHost = new URL(getIssuerOrigin(env)).hostname

    if (url.hostname === issuerHost) {
      return makeAuthIssuer(env).fetch(request, env, ctx)
    }

    return fetch(request)
  }

const makeAuthClient = (env: Env, ctx: ExecutionContext) => {
  const config = getOpenAgentsWorkerConfig(env)

  return createClient({
    clientID: config.openauth.clientId,
    issuer: getIssuerOrigin(env),
    fetch: makeIssuerAwareFetch(env, ctx),
  })
}

type VerifiedSession = VerifiedAuthSession<UserSubject>

const verifySession = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<VerifiedSession | undefined> => {
  const cookies = parseCookies(request)
  const access = cookies.get(ACCESS_COOKIE)

  if (access === undefined) {
    return undefined
  }

  const refresh = cookies.get(REFRESH_COOKIE)
  const verified = await observedPromise('Auth.verifySession', () =>
    refresh === undefined
      ? makeAuthClient(env, ctx).verify(subjects, access)
      : makeAuthClient(env, ctx).verify(subjects, access, { refresh }),
  ).catch(error => {
    logWorkerRouteError('auth_session_verify_failed', error)

    return undefined
  })

  if (verified === undefined) {
    return undefined
  }

  if (verified.err !== undefined) {
    return undefined
  }

  if (verified.subject.type !== 'user') {
    return undefined
  }

  if (verified.tokens === undefined) {
    return { user: verified.subject.properties }
  }

  return { user: verified.subject.properties, tokens: verified.tokens }
}

const scheduleSiteReferralOnboardingEmail = (
  ctx: ExecutionContext,
  env: EmailCampaignDispatcherBindings,
  session: VerifiedSession,
  referralResult: ReferralConsumptionResult,
  orderState: OnboardingDripOrderState,
): void => {
  if (referralResult._tag !== 'consumed') {
    return
  }

  scheduleBackgroundWork(
    ctx,
    sendSiteReferralOnboardingForConsumption(openAgentsDatabase(env), {
      appOrigin: getAppOrigin(env),
      displayName: session.user.name,
      email: session.user.email,
      orderState,
      referralResult,
      resend: getResendEmailConfig(env),
      userId: session.user.userId,
    }).catch(error =>
      logWorkerRouteError('site_referral_onboarding_failed', error, {
        userId: session.user.userId,
      }),
    ),
  )
}

const { appendRefreshedSessionCookies, requireBrowserSession } =
  makeBrowserSessionBoundary<UserSubject, Env>({
    persistUser: (env, user) => upsertGitHubUser(openAgentsDatabase(env), user),
    verifySession,
  })

const authenticateRequestActor = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<AuthenticatedActor | undefined> => {
  const bearerToken = readBearerToken(request)

  if (bearerToken !== undefined) {
    const agent = await authenticateProgrammaticAgent(
      makeD1AgentRegistrationStore(openAgentsDatabase(env)),
      bearerToken,
    )

    if (agent !== undefined) {
      return { kind: 'agent', agent }
    }
  }

  const session = await verifySession(request, env, ctx)

  if (session === undefined) {
    return undefined
  }

  await upsertGitHubUser(openAgentsDatabase(env), session.user)

  if (session.tokens === undefined) {
    return {
      kind: 'human',
      user: session.user,
    }
  }

  return {
    kind: 'human',
    user: session.user,
    tokens: session.tokens,
  }
}

const actorJson = (actor: AuthenticatedActor) => {
  if (actor.kind === 'agent') {
    return {
      kind: actor.kind,
      userId: actor.agent.user.id,
      displayName: actor.agent.user.displayName,
      credentialId: actor.agent.credential.id,
      tokenPrefix: actor.agent.credential.tokenPrefix,
    }
  }

  return {
    kind: actor.kind,
    userId: actor.user.userId,
    login: actor.user.login,
    email: actor.user.email,
    name: actor.user.name,
  }
}

const cloneResponseWithHeaders = (
  response: Response,
  mutateHeaders: (headers: Headers) => void,
): Response => {
  const headers = new Headers(response.headers)
  mutateHeaders(headers)

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  })
}

const handleAppShellPage = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  const cookies = parseCookies(request)
  const hadSessionCookie =
    cookies.has(ACCESS_COOKIE) || cookies.has(REFRESH_COOKIE)
  const session = await verifySession(request, env, ctx)
  const tokens = session?.tokens
  const assetResponse = await env.ASSETS.fetch(request)

  if (tokens !== undefined) {
    return cloneResponseWithHeaders(assetResponse, headers => {
      appendSessionCookies(headers, tokens)
    })
  }

  if (hadSessionCookie && session === undefined) {
    return cloneResponseWithHeaders(assetResponse, headers => {
      appendClearSessionCookies(headers, new URL(request.url).hostname)
    })
  }

  return assetResponse
}

const readAuthenticatedPageContext = async (
  env: Env,
  session: VerifiedSession,
): Promise<
  Readonly<{
    totals: UserKindTotals | undefined
    teams: ReadonlyArray<UserTeam>
    providerAccounts: ProviderAccountBundle
    githubWriteConnections: GitHubWriteConnectionBundle
    tokenLeaderboards: AutopilotTokenLeaderboards
    billing: BillingSummary
    onboarding: Awaited<ReturnType<typeof readOnboardingStatusForUser>>
  }>
> => {
  await upsertGitHubUser(openAgentsDatabase(env), session.user)

  const providerAccountRepository = makeD1ProviderAccountRepository(
    openAgentsDatabase(env),
  )
  const githubWriteRepository = makeD1GitHubWriteRepository(
    openAgentsDatabase(env),
  )
  const [
    maybeTotals,
    teams,
    providerAccounts,
    githubWriteConnections,
    tokenLeaderboards,
    billing,
    onboarding,
  ] = await Promise.all([
    isOpenAgentsAdminEmail(session.user.email)
      ? readUserKindTotals(openAgentsDatabase(env))
      : Promise.resolve(undefined),
    readTeamsForUser(openAgentsDatabase(env), session.user.userId),
    listProviderAccountsForUser(providerAccountRepository, session.user.userId),
    listGitHubWriteConnectionsForUser(
      githubWriteRepository,
      session.user.userId,
    ),
    readTokenUsageLeaderboardsForUser(env, session.user.userId),
    readBillingSummary(openAgentsDatabase(env), session.user.userId),
    readOnboardingStatusForUser(env, session.user.userId),
  ])

  return {
    totals: maybeTotals,
    teams,
    providerAccounts,
    githubWriteConnections,
    tokenLeaderboards,
    billing,
    onboarding,
  }
}

const handleHomePage = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  const shellResponse = await handleAppShellPage(request, env, ctx)
  const headers = new Headers(shellResponse.headers)
  headers.append(
    'Link',
    '<https://openagents.com/api/public/home>; rel="alternate"; type="application/json"; title="OpenAgents homepage JSON"',
  )
  headers.append(
    'Link',
    '<https://openagents.com/.well-known/openagents.json>; rel="service-desc"; type="application/json"; title="OpenAgents capability manifest"',
  )
  headers.set(
    'X-OpenAgents-Homepage-Json',
    'https://openagents.com/api/public/home',
  )

  return new Response(shellResponse.body, {
    headers,
    status: shellResponse.status,
    statusText: shellResponse.statusText,
  })
}

const handlePublicHomeApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse({
          schemaVersion: 'openagents.public_home.v1',
          page: {
            canonicalUrl: 'https://openagents.com/',
            htmlUrl: 'https://openagents.com/',
            title: 'OpenAgents',
          },
          agentDiscovery: {
            homepageJson: 'https://openagents.com/api/public/home',
            productPromises:
              'https://openagents.com/api/public/product-promises',
            capabilityManifest:
              'https://openagents.com/.well-known/openagents.json',
            agentInstructions: 'https://openagents.com/AGENTS.md',
            openApi: 'https://openagents.com/api/openapi.json',
          },
          homepageData: [
            {
              id: 'capability_manifest',
              href: 'https://openagents.com/.well-known/openagents.json',
              method: 'GET',
              description:
                'Machine-readable OpenAgents capability manifest for agents and operators.',
            },
            {
              id: 'openapi',
              href: 'https://openagents.com/api/openapi.json',
              method: 'GET',
              description:
                'Machine-readable OpenAPI contract for public-safe API routes.',
            },
            {
              id: 'pylon_stats',
              href: 'https://openagents.com/api/public/pylon-stats',
              method: 'GET',
              description:
                'Pylon heartbeat, readiness, and receipt-gated accepted-work counters shown on the homepage.',
            },
            {
              id: 'product_promises',
              href: 'https://openagents.com/api/public/product-promises',
              method: 'GET',
              description:
                'Versioned OpenAgents product-promise registry for agents and users, including live, scoped, gated, degraded, and planned claim states.',
            },
            {
              id: 'forum_tip_leaderboards',
              href: 'https://openagents.com/api/forum/tip-leaderboards?limit=10',
              method: 'GET',
              description:
                'Public Forum tip paid and settled evidence rows shown on the homepage.',
            },
            {
              id: 'forum_launch_status',
              href: 'https://openagents.com/api/forum/launch-status',
              method: 'GET',
              description:
                'Forum posting, moderation/reporting, and tipping launch gates shown on the homepage.',
            },
            {
              id: 'public_adjutant_activity',
              href: 'https://openagents.com/api/public/adjutant/activity',
              method: 'GET',
              description:
                'Public Autopilot activity projection linked from the homepage.',
            },
          ],
          notes: [
            'This endpoint is public and safe for agents to crawl.',
            'Use the listed hrefs for the live JSON data behind the homepage.',
            'This endpoint is discovery only and grants no write authority.',
          ],
        }),
      )

const handlePublicProductPromisesApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(publicProductPromisesDocument()))

const handleThreadPage = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  threadId: string,
): Promise<Response> => {
  const session = await verifySession(request, env, ctx)

  if (session !== undefined) {
    const accessResult = await threadRouteAccessBundle(
      env,
      session.user.userId,
      threadId,
    )

    if (isRouteAccessError(accessResult)) {
      const response = routeAccessResponse(accessResult, {
        href: '/',
        surface: 'product',
      })

      if (session.tokens !== undefined) {
        appendSessionCookies(response.headers, session.tokens)
      }

      return response
    }
  }

  return handleAppShellPage(request, env, ctx)
}

const isAgentClaimReturnPath = (pathname: string): boolean =>
  /^\/agents\/claims\/[^/]+$/.test(pathname)

const cleanLoginReturnPath = (value: string | null): string | undefined => {
  const raw = value?.trim()

  if (
    raw === undefined ||
    raw === '' ||
    !raw.startsWith('/') ||
    raw.startsWith('//') ||
    raw.includes('\n') ||
    raw.includes('\r')
  ) {
    return undefined
  }

  try {
    const url = new URL(raw, 'https://openagents.local')

    if (
      url.origin !== 'https://openagents.local' ||
      url.pathname === '/auth/callback' ||
      url.pathname.startsWith('/login')
    ) {
      return undefined
    }

    const isAgentClaimReturn = isAgentClaimReturnPath(url.pathname)

    if (
      url.pathname === '/' ||
      url.pathname === '/billing' ||
      url.pathname === '/onboarding' ||
      url.pathname === '/order' ||
      isAgentClaimReturn ||
      url.pathname.startsWith('/orders/') ||
      url.pathname.startsWith('/share/')
    ) {
      return isAgentClaimReturn ? url.pathname : `${url.pathname}${url.search}`
    }

    return undefined
  } catch {
    return undefined
  }
}

const handleGitHubStart = async (request: Request, env: Env) => {
  const config = getOpenAgentsWorkerConfig(env)
  const redirectUri = `${getAppOrigin(env)}/auth/callback`
  const { challenge, url } = await createClient({
    clientID: config.openauth.clientId,
    issuer: getIssuerOrigin(env),
  }).authorize(redirectUri, 'code', {
    provider: 'github',
  })
  const requestUrl = new URL(request.url)
  const maybeReturnTo = cleanLoginReturnPath(
    requestUrl.searchParams.get('returnTo'),
  )
  const cookies = [
    serializeCookie(
      AUTH_STATE_COOKIE,
      challenge.state,
      AUTH_STATE_MAX_AGE_SECONDS,
      '/auth',
    ),
    serializeCookie(
      LOGIN_ORIGIN_COOKIE,
      requestUrl.origin,
      AUTH_STATE_MAX_AGE_SECONDS,
      '/auth',
    ),
    maybeReturnTo === undefined
      ? expiredCookie(LOGIN_RETURN_TO_COOKIE, '/auth')
      : serializeCookie(
          LOGIN_RETURN_TO_COOKIE,
          maybeReturnTo,
          AUTH_STATE_MAX_AGE_SECONDS,
          '/auth',
        ),
  ]

  return redirectResponse(url, cookies)
}

export const githubWriteResultRedirectLocation = (appOrigin: string): string =>
  appOrigin

const githubWriteResultRedirect = (env: Env): Response =>
  redirectResponse(githubWriteResultRedirectLocation(getAppOrigin(env)))

export const cleanProductRouteRedirectLocation = (
  url: URL,
): string | undefined => {
  if (url.search === '') {
    return undefined
  }

  if (url.pathname === '/login') {
    return `${url.origin}/`
  }

  if (
    url.pathname === '/' ||
    url.pathname === '/billing' ||
    url.pathname === '/onboarding' ||
    url.pathname === '/order' ||
    url.pathname.startsWith('/orders/') ||
    url.pathname.startsWith('/share/')
  ) {
    return `${url.origin}${url.pathname}`
  }

  return undefined
}

const handleGitHubWriteStart = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return redirectResponse(getAppOrigin(env))
  }

  const attempt = await startGitHubWriteConnectionAttempt(
    makeD1GitHubWriteRepository(openAgentsDatabase(env)),
    {
      expectedGithubId: session.user.githubId,
      expectedGithubLogin: session.user.login,
      redirectAfter: '/',
      scopes: GITHUB_WRITE_REQUIRED_SCOPES,
      userId: session.user.userId,
    },
  )

  return redirectResponse(
    gitHubWriteAuthorizeUrl(env, attempt.state, attempt.scopes),
  )
}

const handleGitHubWriteCallback = async (
  request: Request,
  env: Env,
  attempt: GitHubWriteConnectionAttemptRecord | undefined,
): Promise<Response> => {
  if (attempt === undefined) {
    return githubWriteResultRedirect(env)
  }

  const repository = makeD1GitHubWriteRepository(openAgentsDatabase(env))
  const url = new URL(request.url)
  const now = workerRuntime.now()
  const nowIso = now.toISOString()
  const fail = async (
    status: 'denied' | 'expired' | 'failed',
    reason: string,
  ): Promise<Response> => {
    await repository
      .markAttemptFailed(attempt, status, reason, nowIso)
      .catch(error => {
        logWorkerRouteError('github_write_attempt_mark_failed', error, {
          attemptId: attempt.id,
          errorName: errorName(error),
          status,
        })
      })

    return githubWriteResultRedirect(env)
  }

  if (attempt.status !== 'pending') {
    return githubWriteResultRedirect(env)
  }

  if (Date.parse(attempt.expiresAt) <= now.getTime()) {
    return fail('expired', 'GitHub write connection state expired.')
  }

  const error = url.searchParams.get('error')

  if (error !== null) {
    return fail(error === 'access_denied' ? 'denied' : 'failed', error)
  }

  const code = url.searchParams.get('code')

  if (code === null) {
    return fail('failed', 'GitHub write OAuth code is missing.')
  }

  try {
    const token = await exchangeGitHubOAuthCode(env, code)
    const scopes = parseGitHubScopeHeader(token.scope)
    const githubUser = await fetchGitHubJson(
      GitHubUser,
      'https://api.github.com/user',
      token.access_token,
    ).catch(error => {
      throw new GitHubWriteApiFailure({
        operation: 'fetch_authenticated_user',
        status: 0,
        message: errorMessage(error),
      })
    })
    const githubId = String(githubUser.id)

    try {
      requireGitHubWriteCallbackAccount(attempt, githubId)
      requireGitHubWritePermissions(scopes)
    } catch (error) {
      return fail('failed', gitHubWriteRouteErrorMessage(error))
    }

    const connectionRef = githubWriteConnectionRef(workerRuntime.makeUuid())
    const secretRef = githubWriteSecretRef(connectionRef)

    await storeGitHubWriteAccessToken(env, connectionRef, token.access_token)

    try {
      await recordGitHubWriteConnectionConnected(repository, {
        attempt,
        connectionRef,
        githubId,
        githubLogin: githubUser.login,
        scopes,
        secretRef,
      })
    } catch (error) {
      await env.AUTH_STORAGE.delete(githubWriteSecretKey(connectionRef))
      throw error
    }

    return githubWriteResultRedirect(env)
  } catch (error) {
    logWorkerRouteError('github_write_callback_failed', error, {
      attemptId: attempt.id,
      errorName: errorName(error),
    })

    return fail('failed', 'GitHub write connection failed.')
  }
}

const handleGitHubWriteConnectionsApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  const githubWriteConnections = await listGitHubWriteConnectionsForUser(
    makeD1GitHubWriteRepository(openAgentsDatabase(env)),
    session.user.userId,
  )

  return appendRefreshedSessionCookies(
    noStoreJsonResponse({ githubWriteConnections }),
    session,
  )
}

const handleGitHubWriteDisconnectApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  connectionRef: string,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const session = await requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  const now = workerRuntime.nowIso()
  const repository = makeD1GitHubWriteRepository(openAgentsDatabase(env))
  const connection = await repository.disconnectConnection({
    connectionRef,
    metadataJson: gitHubWriteConnectionMetadataJson({
      githubLogin: session.user.login,
      scopes: [],
      source: 'browser_disconnect',
      status: 'disconnected',
    }),
    now,
    userId: session.user.userId,
  })

  if (connection === undefined) {
    return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
  }

  await env.AUTH_STORAGE.delete(githubWriteSecretKey(connectionRef))

  return appendRefreshedSessionCookies(
    noStoreJsonResponse({
      connection,
    }),
    session,
  )
}

const runnerResolvedGitHubWriteGrantJson = (
  grant: Awaited<ReturnType<typeof resolveGitHubWriteGrant>>,
  accessToken: string,
) => {
  if (grant === undefined) {
    return undefined
  }

  const expiresAt = Date.parse(grant.expiresAt)

  if (!Number.isFinite(expiresAt)) {
    throw new Error('Resolved GitHub write grant expiry is invalid.')
  }

  return {
    connectionRef: grant.connectionRef,
    credential: {
      accessToken,
      provider: 'github',
      scopes: grant.scopes,
      tokenType: 'oauth',
    },
    expiresAt,
    githubLogin: grant.githubLogin,
    grantRef: grant.grantRef,
    materialization: {
      authRef: grant.secretRef,
      gitCredentialEnv: 'GITHUB_TOKEN',
      provider: 'github',
      remoteUrlMode: 'https_token',
      scrubAfterCloseout: true,
    },
    requestedAction: grant.requestedAction,
    runnerSessionId: grant.runnerSessionId,
    status: 'issued',
  }
}

const redactedGrantErrorMessage = (error: unknown): string =>
  gitHubWriteRouteErrorMessage(error)

const grantResolveErrorStatus = (error: unknown): number => {
  return gitHubWriteRouteErrorStatus(error)
}

const handleGitHubWriteGrantResolveApi = async (
  request: Request,
  env: Env,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const actor = await requireProviderServiceActor(request, env)

  if (actor === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  const body = await readJsonObject(request).catch(
    (): Record<string, unknown> => ({}),
  )
  const grantRef =
    optionalString(body.githubWriteGrantRef) ?? optionalString(body.grantRef)

  if (grantRef === undefined) {
    return noStoreJsonResponse(
      { error: 'bad_request', reason: 'githubWriteGrantRef is required' },
      { status: 400 },
    )
  }

  const runnerSessionId =
    optionalString(body.runnerSessionId) ?? optionalString(body.runId)

  try {
    const grant = await resolveGitHubWriteGrant(
      makeD1GitHubWriteRepository(openAgentsDatabase(env)),
      {
        grantRef,
        ...(runnerSessionId === undefined ? {} : { runnerSessionId }),
      },
    )

    if (grant === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const accessToken = await env.AUTH_STORAGE.get(
      githubWriteSecretKey(grant.connectionRef),
    )

    if (accessToken === null) {
      return noStoreJsonResponse(
        {
          error: 'github_write_secret_missing',
          message: 'GitHub write credential is not available.',
        },
        { status: 409 },
      )
    }

    return noStoreJsonResponse({
      grant: runnerResolvedGitHubWriteGrantJson(grant, accessToken),
    })
  } catch (error) {
    logWorkerRouteError('github_write_grant_resolve_failed', error, {
      errorName: gitHubWriteRouteErrorName(error),
      grantRef,
      runnerSessionId,
    })

    return noStoreJsonResponse(
      {
        error: 'github_write_grant_resolve_failed',
        message: redactedGrantErrorMessage(error),
      },
      { status: grantResolveErrorStatus(error) },
    )
  }
}

const handleAuthCallback = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  const url = new URL(request.url)
  const error = url.searchParams.get('error')
  const cookies = parseCookies(request)
  const cleanupCookies = [
    expiredCookie(AUTH_STATE_COOKIE, '/auth'),
    expiredCookie(LOGIN_ORIGIN_COOKIE, '/auth'),
    expiredCookie(LOGIN_RETURN_TO_COOKIE, '/auth'),
  ]

  if (error !== null) {
    return redirectResponse('/', cleanupCookies)
  }

  const code = url.searchParams.get('code')
  const state = url.searchParams.get('state')
  const expectedState = cookies.get(AUTH_STATE_COOKIE)
  const maybeReturnTo = cleanLoginReturnPath(
    cookies.get(LOGIN_RETURN_TO_COOKIE) ?? null,
  )

  if (
    code === null ||
    state === null ||
    expectedState === undefined ||
    state !== expectedState
  ) {
    return redirectResponse('/', cleanupCookies)
  }

  const redirectUri = `${getAppOrigin(env)}/auth/callback`
  const exchanged = await observedPromise('Auth.exchangeCode', () =>
    makeAuthClient(env, ctx).exchange(code, redirectUri),
  ).catch(error => {
    logWorkerRouteError('auth_code_exchange_failed', error, {
      errorName: errorName(error),
    })

    return undefined
  })

  if (exchanged === undefined) {
    return redirectResponse('/', cleanupCookies)
  }

  if (exchanged.err !== false) {
    return redirectResponse('/', cleanupCookies)
  }

  const response = redirectResponse(maybeReturnTo ?? '/', cleanupCookies)
  appendSessionCookies(response.headers, exchanged.tokens)

  return response
}

const handleLogout = (request: Request): Response => {
  const response = redirectResponse('/')
  appendClearSessionCookies(response.headers, new URL(request.url).hostname)

  return response
}

const handleSessionApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await verifySession(request, env, ctx)

  if (session === undefined) {
    const response = noStoreJsonResponse(
      { authenticated: false },
      { status: 200 },
    )

    const cookies = parseCookies(request)

    if (cookies.has(ACCESS_COOKIE) || cookies.has(REFRESH_COOKIE)) {
      appendClearSessionCookies(response.headers, new URL(request.url).hostname)
    }

    return response
  }

  await upsertGitHubUser(openAgentsDatabase(env), session.user)
  const referralResult = await consumePendingReferralForUser(
    openAgentsDatabase(env),
    workerRuntime,
    {
      pendingAttributionId: parseCookies(request).get(PENDING_REFERRAL_COOKIE),
      userId: session.user.userId,
    },
  ).catch(error => {
    logWorkerRouteError('site_referral_session_consumption_failed', error)

    return { _tag: 'none' as const }
  })
  const accountContext = await readAuthenticatedPageContext(env, session)
  scheduleSiteReferralOnboardingEmail(ctx, env, session, referralResult, 'none')

  const response = noStoreJsonResponse({
    authenticated: true,
    bootstrap: {
      session: {
        userId: session.user.userId,
        email: session.user.email,
        name: session.user.name,
        login: session.user.login,
        avatarUrl: session.user.avatarUrl,
        provider: session.user.provider,
        githubId: session.user.githubId,
      },
      teams: accountContext.teams,
      tokenLeaderboards: accountContext.tokenLeaderboards,
      billing: accountContext.billing,
      onboarding: accountContext.onboarding,
      providerAccounts: accountContext.providerAccounts,
      isAdmin: isOpenAgentsAdminEmail(session.user.email),
    },
  })

  if (session.tokens !== undefined) {
    appendSessionCookies(response.headers, session.tokens)
  }

  if (referralResult._tag !== 'none') {
    response.headers.append(
      'set-cookie',
      expiredCookie(PENDING_REFERRAL_COOKIE),
    )
  }

  return response
}

const handleAuthTotalsApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const actor = await authenticateRequestActor(request, env, ctx)

  if (actor === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  if (actor.kind !== 'human' || !isOpenAgentsAdminEmail(actor.user.email)) {
    return forbidden()
  }

  const response = noStoreJsonResponse({
    authenticated: true,
    actor: actorJson(actor),
    totals: await readUserKindTotals(openAgentsDatabase(env)),
  })

  if (actor.kind === 'human' && actor.tokens !== undefined) {
    appendSessionCookies(response.headers, actor.tokens)
  }

  return response
}

const handleAuthTeamsApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const actor = await authenticateRequestActor(request, env, ctx)

  if (actor === undefined) {
    return unauthorized()
  }

  const teams = await readTeamsForUser(
    openAgentsDatabase(env),
    actor.kind === 'human' ? actor.user.userId : actor.agent.user.id,
  )
  const response = jsonResponse({
    authenticated: true,
    actor: actorJson(actor),
    teams,
  })

  if (actor.kind === 'human' && actor.tokens !== undefined) {
    appendSessionCookies(response.headers, actor.tokens)
  }

  return response
}

type PostedTeamChatMessage = Readonly<{
  payload: Record<string, unknown>
  status: number
}>

const adjutantIntentContextFromRequest = (
  input: Record<string, unknown>,
): Partial<Omit<TeamAdjutantIntent, 'prompt' | 'schemaVersion'>> => {
  const softwareOrderId = explicitAdjutantContextString(
    input.softwareOrderId ?? input.software_order_id ?? input.orderId,
    softwareOrderIdPattern,
  )
  const siteId = explicitAdjutantContextString(
    input.siteId ?? input.site_id,
    siteIdPattern,
  )
  const taskSpecPath = boundedTaskSpecPath(
    optionalString(
      input.taskSpecPath ?? input.task_spec_path ?? input.taskPacketPath,
    ),
  )

  return {
    ...(softwareOrderId === undefined ? {} : { softwareOrderId }),
    ...(siteId === undefined ? {} : { siteId }),
    ...(taskSpecPath === undefined ? {} : { taskSpecPath }),
  }
}

const mergeAdjutantIntentContext = (
  bodyIntent: TeamAdjutantIntent,
  requestContext: Partial<Omit<TeamAdjutantIntent, 'prompt' | 'schemaVersion'>>,
  requestedPrompt: string | undefined,
): TeamAdjutantIntent => ({
  schemaVersion: ADJUTANT_INTENT_SCHEMA_VERSION,
  prompt: requestedPrompt ?? bodyIntent.prompt,
  softwareOrderId: requestContext.softwareOrderId ?? bodyIntent.softwareOrderId,
  siteId: requestContext.siteId ?? bodyIntent.siteId,
  taskSpecPath: requestContext.taskSpecPath ?? bodyIntent.taskSpecPath,
})

const adjutantIntentHasContext = (intent: TeamAdjutantIntent): boolean =>
  intent.softwareOrderId !== undefined ||
  intent.siteId !== undefined ||
  intent.taskSpecPath !== undefined

const postTeamChatMessageForUser = async (
  env: Env,
  ctx: ExecutionContext,
  input: Readonly<{
    body: Record<string, unknown>
    project?: UserTeamProject
    roomThreadId: string
    teamId: string
    userId: string
  }>,
): Promise<PostedTeamChatMessage> => {
  const messageBody = optionalString(input.body.body ?? input.body.message)
  const kind = optionalUserWritableTeamChatKind(input.body.kind) ?? 'message'

  if (messageBody === undefined) {
    return {
      payload: { error: 'bad_request', reason: 'body is required' },
      status: 400,
    }
  }

  if (messageBody.length > 4000) {
    return {
      payload: {
        error: 'bad_request',
        reason: 'body must be 4000 characters or fewer',
      },
      status: 400,
    }
  }

  const bodyAdjutantIntent =
    kind === 'adjutant_intent'
      ? teamAdjutantIntentFromBody(messageBody)
      : undefined
  const adjutantIntent =
    bodyAdjutantIntent === undefined
      ? undefined
      : mergeAdjutantIntentContext(
          bodyAdjutantIntent,
          adjutantIntentContextFromRequest(input.body),
          optionalString(input.body.prompt),
        )

  if (
    kind === 'adjutant_intent' &&
    (input.project === undefined || input.project.id !== ADJUTANT_PROJECT_ID)
  ) {
    return {
      payload: {
        error: 'adjutant_project_context_required',
        reason: '@autopilot requires the Autopilot project room',
      },
      status: 400,
    }
  }

  if (kind === 'adjutant_intent' && adjutantIntent === undefined) {
    return {
      payload: {
        error: 'adjutant_command_required',
        reason: 'Autopilot messages must include an exact @autopilot tag',
      },
      status: 400,
    }
  }

  if (
    kind === 'adjutant_intent' &&
    adjutantIntent !== undefined &&
    !adjutantIntentHasContext(adjutantIntent)
  ) {
    return {
      payload: {
        error: 'adjutant_context_required',
        reason:
          '@autopilot requires softwareOrderId, siteId, or taskSpecPath context',
      },
      status: 400,
    }
  }

  const autopilotThreadId =
    kind === 'autopilot_intent'
      ? (optionalUuid(input.body.threadId) ?? makeTeamChatThreadId())
      : undefined
  const executionPrompt =
    kind === 'autopilot_intent'
      ? (optionalString(input.body.prompt) ??
        teamAutopilotPromptFromBody(messageBody))
      : messageBody
  const requestedFileIds = stringArrayFromUnknown(input.body.fileIds)
  const messageId =
    kind === 'autopilot_intent' ? makeTeamChatMessageId() : undefined
  const teamAutopilotContext =
    kind === 'autopilot_intent' && messageId !== undefined
      ? await hydrateTeamAutopilotContextFileExcerpts(
          env,
          teamAutopilotContextBundle({
            files: await listTeamThreadFiles(openAgentsDatabase(env), {
              teamId: input.teamId,
              threadId: input.roomThreadId,
            }),
            messages: await listTeamChatMessages(
              openAgentsDatabase(env),
              input.teamId,
              12,
              undefined,
              undefined,
              input.project?.id ?? null,
            ),
            ...(input.project === undefined ? {} : { project: input.project }),
            parentTeamChatMessageId: messageId,
            prompt: executionPrompt,
            requestedFileIds,
            teamId: input.teamId,
          }),
        )
      : undefined
  const missionLaunch =
    kind === 'autopilot_intent' && teamAutopilotContext !== undefined
      ? await launchUserAutopilotMission(env, ctx, {
          selector: {
            ...input.body,
            autopilotThreadId,
            dispatchGoal: teamAutopilotChildRunGoal(teamAutopilotContext),
            goal: teamAutopilotContext.normalizedPrompt,
            parentTeamChatMessageId:
              teamAutopilotContext.parentTeamChatMessageId,
            prompt: teamAutopilotContext.normalizedPrompt,
            ...(input.project === undefined
              ? {}
              : { projectId: input.project.id }),
            selectedTeamFileIds: teamAutopilotContext.selectedTeamFileIds,
            teamId: input.teamId,
          },
          userId: input.userId,
        })
      : undefined
  const launchError =
    missionLaunch?.ok === false
      ? await teamChatLaunchErrorFromResponse(missionLaunch.response)
      : undefined

  const message = await insertTeamChatMessage(openAgentsDatabase(env), {
    ...(missionLaunch === undefined || missionLaunch.ok === false
      ? {}
      : {
          agentRunId: missionLaunch.launch.runId,
          ...(autopilotThreadId === undefined ? {} : { autopilotThreadId }),
          metadataJson: JSON.stringify({
            mission: missionLaunch.launch.payload.mission,
            runSummary: missionLaunch.launch.payload.runSummary,
            statusUrl: missionLaunch.launch.payload.statusUrl,
            streamUrl: missionLaunch.launch.payload.streamUrl,
            context: teamAutopilotContext,
            selectedTeamFileIds:
              teamAutopilotContext?.selectedTeamFileIds ?? [],
          }),
        }),
    ...(launchError === undefined
      ? {}
      : {
          ...(autopilotThreadId === undefined ? {} : { autopilotThreadId }),
          metadataJson: JSON.stringify({
            context: teamAutopilotContext,
            launchError,
            launchStatus:
              missionLaunch?.ok === false
                ? missionLaunch.response.status
                : undefined,
            selectedTeamFileIds:
              teamAutopilotContext?.selectedTeamFileIds ?? [],
          }),
        }),
    ...(adjutantIntent === undefined
      ? {}
      : {
          metadataJson: JSON.stringify({
            adjutantIntent: {
              schemaVersion: adjutantIntent.schemaVersion,
              prompt: adjutantIntent.prompt,
              softwareOrderId: adjutantIntent.softwareOrderId ?? null,
              siteId: adjutantIntent.siteId ?? null,
              taskSpecPath: adjutantIntent.taskSpecPath ?? null,
            },
          }),
        }),
    authorUserId: input.userId,
    body: messageBody,
    ...(messageId === undefined ? {} : { id: messageId }),
    kind,
    ...(input.project === undefined ? {} : { projectId: input.project.id }),
    teamId: input.teamId,
  })
  const selectedTeamFileIds =
    kind === 'autopilot_intent'
      ? (teamAutopilotContext?.selectedTeamFileIds ?? [])
      : requestedFileIds

  await insertThreadFileMessageReferences(openAgentsDatabase(env), {
    fileIds: selectedTeamFileIds,
    messageId: message.id,
    referenceKind:
      kind === 'autopilot_intent' ? 'autopilot_input' : 'message_attachment',
    teamId: input.teamId,
    threadId: message.autopilotThreadId ?? input.roomThreadId,
  })

  await publishTeamChatMessageSync(env, ctx, message, input.userId)

  return {
    payload: {
      ...(missionLaunch === undefined || missionLaunch.ok === false
        ? {}
        : missionLaunch.launch.payload),
      ...(launchError === undefined ? {} : { launchError }),
      ...(autopilotThreadId === undefined ||
      missionLaunch === undefined ||
      missionLaunch.ok === false
        ? {}
        : {
            threadId: autopilotThreadId,
            threadUrl: `/t/${missionLaunch.launch.runId}`,
          }),
      message,
      projectId: input.project?.id ?? null,
      teamId: input.teamId,
    },
    status:
      kind === 'autopilot_intent' && launchError === undefined ? 202 : 201,
  }
}

const handleTeamChatMessagesApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  teamId: string,
  projectId?: string,
): Promise<Response> => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }

  const session = await requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  const role = await readActiveTeamMembershipRole(
    openAgentsDatabase(env),
    teamId,
    session.user.userId,
  )

  if (role === undefined) {
    return forbidden()
  }

  const project =
    projectId === undefined
      ? undefined
      : await readActiveTeamProject(openAgentsDatabase(env), teamId, projectId)

  if (projectId !== undefined && project === undefined) {
    return notFound()
  }

  const roomThreadId =
    project === undefined
      ? teamChatThreadId(teamId)
      : teamProjectChatThreadId(teamId, project.id)

  if (request.method === 'GET') {
    const url = new URL(request.url)
    const requestedLimit = optionalInteger(url.searchParams.get('limit')) ?? 50
    const limit = Math.min(Math.max(requestedLimit, 1), 100)
    const kind = optionalUserWritableTeamChatKind(url.searchParams.get('kind'))
    const autopilotThreadId = optionalUuid(url.searchParams.get('threadId'))
    const response = noStoreJsonResponse({
      messages: await listTeamChatMessages(
        openAgentsDatabase(env),
        teamId,
        limit,
        kind,
        autopilotThreadId,
        project?.id ?? null,
      ),
      projectId: project?.id ?? null,
      teamId,
    })

    return appendRefreshedSessionCookies(response, session)
  }

  const body = await readJsonObject(request).catch(
    (): Record<string, unknown> => ({}),
  )
  const posted = await postTeamChatMessageForUser(env, ctx, {
    body,
    ...(project === undefined ? {} : { project }),
    roomThreadId,
    teamId,
    userId: session.user.userId,
  })

  const response = noStoreJsonResponse(posted.payload, {
    status: posted.status,
  })

  return appendRefreshedSessionCookies(response, session)
}

const optionalUserWritableTeamChatKind = (
  value: unknown,
): 'message' | 'autopilot_intent' | 'adjutant_intent' | undefined =>
  value === 'message' ||
  value === 'autopilot_intent' ||
  value === 'adjutant_intent'
    ? value
    : undefined

const requireProviderServiceActor = async (
  request: Request,
  env: Env,
): Promise<ProgrammaticAgentSession | undefined> => {
  const bearerToken = readBearerToken(request)

  if (bearerToken === undefined) {
    return undefined
  }

  return authenticateProgrammaticAgent(
    makeD1AgentRegistrationStore(openAgentsDatabase(env)),
    bearerToken,
  )
}

const requireRunnerCallbackAuth = async (
  request: Request,
  env: Env,
): Promise<boolean> => {
  const expected = redactedValue(
    getOpenAgentsWorkerConfig(env).shc.runnerCallbackToken,
  )
  const actual = readBearerToken(request)

  if (
    expected === undefined ||
    expected.trim() === '' ||
    actual === undefined
  ) {
    return false
  }

  return timingSafeEqual(actual, expected)
}

const requireAdminApiToken = async (
  request: Request,
  env: Env,
): Promise<boolean> => {
  const expected = getAdminApiToken(env)
  const actual = readBearerToken(request)

  if (expected === undefined || actual === undefined) {
    return false
  }

  return timingSafeEqual(actual, expected)
}

const shcDispatchConfig = (env: Env) => {
  const config = getOpenAgentsWorkerConfig(env)

  return {
    controlApiBearerToken: redactedValue(config.shc.controlApiBearerToken),
    controlApiUrl: config.shc.controlApiUrl,
    dispatchMode: config.shc.dispatchMode,
  }
}

const cleanupCanceledAgentRunOnShc = async (
  env: Env,
  run: AgentRunRecord,
): Promise<void> => {
  try {
    const result = await cancelAgentRunOnShc(run, {
      ...shcDispatchConfig(env),
      reason: 'Credits exhausted; OpenAgents stopped the run.',
    })

    if (!result.ok) {
      logWorkerRouteWarning('shc_billing_cleanup_not_acknowledged', {
        error: result.error,
        runId: run.id,
        status: result.status,
        targetPath: result.targetPath,
      })
    }
  } catch (error) {
    logWorkerRouteWarning('shc_billing_cleanup_request_failed', {
      error: errorMessage(error),
      runId: run.id,
    })
  }
}

const sendOutOfCreditsNotificationOnce = async (
  env: Env,
  input: Readonly<{
    balanceCents: number
    balanceFormatted: string
    userId: string
  }>,
): Promise<void> => {
  const reservation = await reserveOutOfCreditsNotification(
    openAgentsDatabase(env),
    input,
  )

  if (!reservation.ok) {
    return
  }

  const resend = getOpenAgentsWorkerConfig(env).email.resend
  const delivery =
    resend === undefined
      ? {
          errorMessage: 'Resend email configuration is not set.',
          errorName: 'resend_config_missing',
          ok: false as const,
        }
      : await observedEffect(
          'Email.sendOutOfCreditsEmailWithLedger',
          sendOutOfCreditsEmailWithLedger(
            openAgentsDatabase(env),
            resend,
            {
              appOrigin: getAppOrigin(env),
              balanceFormatted: input.balanceFormatted,
              displayName: reservation.displayName,
              idempotencyKey: reservation.idempotencyKey,
              to: reservation.email,
            },
            {
              metadata: {
                balanceCents: input.balanceCents,
                existingReservationTable: 'billing_credit_notifications',
              },
              sourceAuthorityRef: 'system.billing_out_of_credits.v1',
              targetUserId: input.userId,
            },
          ),
        )

  if (delivery.ok) {
    await markOutOfCreditsNotificationSent(openAgentsDatabase(env), {
      resendEmailId: delivery.id,
      userId: input.userId,
    })

    return
  }

  await markOutOfCreditsNotificationFailed(openAgentsDatabase(env), {
    errorMessage: delivery.errorMessage,
    userId: input.userId,
  })
  logWorkerRouteWarning('out_of_credits_email_failed', {
    error: delivery.errorMessage,
    userId: input.userId,
  })
}

const handleEmailResendWebhookApi = async (
  request: Request,
  env: Parameters<typeof getResendEmailConfig>[0],
) => {
  try {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const config = getOpenAgentsWorkerConfig(env)
    const result = await handleResendWebhook(openAgentsDatabase(env), {
      body: await request.text(),
      headers: request.headers,
      secret: config.email.resendWebhookSecret,
    })

    return noStoreJsonResponse(
      {
        duplicate: result.duplicate,
        eventType: result.eventType,
        ok: result.status === 'accepted',
        providerEventId: result.providerEventId,
        status: result.status,
      },
      { status: result.status === 'unauthorized' ? 401 : 200 },
    )
  } catch (error) {
    return noStoreJsonResponse(
      {
        error: 'resend_webhook_error',
        message:
          error instanceof Error
            ? error.message
            : 'Resend webhook processing failed.',
      },
      { status: 400 },
    )
  }
}

type SiteCustomerNotificationRow = Readonly<{
  display_name: string | null
  order_id: string | null
  primary_email: string | null
  site_title: string | null
  target_user_id: string | null
}>

type SiteCustomerNotificationOutcome = Readonly<{
  emailMessageId: string | null
  emailStatus: 'accepted' | 'failed' | 'skipped'
  providerMessageId?: string | null | undefined
  skipReason?: string | undefined
}>

type ReviewReadyNotificationRow = Readonly<{
  assignment_current_run_id: string | null
  assignment_goal_id: string | null
  assignment_id: string | null
  assignment_visibility: 'private' | 'team' | 'public' | null
  deployment_id: string | null
  display_name: string | null
  order_id: string | null
  primary_email: string | null
  site_id: string
  site_title: string | null
  site_url: string | null
  target_user_id: string | null
  version_id: string
}>

type ReviewReadyArtifactNotificationRow = Readonly<{
  artifact_id: string
  artifact_title: string
  artifact_url: string | null
  assignment_current_run_id: string | null
  assignment_goal_id: string | null
  assignment_id: string | null
  assignment_visibility: 'private' | 'team' | 'public' | null
  display_name: string | null
  kind: string
  order_id: string
  primary_email: string | null
  repository_full_name: string | null
  target_user_id: string | null
}>

type AdjutantNotificationAssignmentRow = Readonly<{
  current_run_id: string | null
  goal_id: string | null
  id: string
  software_order_id: string | null
  visibility: 'private' | 'team' | 'public'
}>

const notificationPayloadJson = (
  input: Readonly<{
    deploymentId: string
    emailMessageId: string | null
    emailStatus: string
    providerMessageId?: string | null | undefined
    siteId: string
    siteUrl: string
    skipReason?: string | undefined
    softwareOrderId: string | null
    stage: 'deployed'
  }>,
): string =>
  JSON.stringify({
    deploymentId: input.deploymentId,
    emailMessageId: input.emailMessageId,
    emailStatus: input.emailStatus,
    providerMessageId: input.providerMessageId ?? null,
    siteId: input.siteId,
    siteUrl: input.siteUrl,
    skipReason: input.skipReason ?? null,
    softwareOrderId: input.softwareOrderId,
    stage: input.stage,
  })

const reviewReadyNotificationPayloadJson = (
  input: Readonly<{
    deploymentId: string | null
    emailMessageId: string | null
    emailStatus: string
    providerMessageId?: string | null | undefined
    siteId: string
    siteUrl: string | null
    skipReason?: string | undefined
    softwareOrderId: string | null
    stage: 'review_ready'
    versionId: string
  }>,
): string =>
  JSON.stringify({
    deploymentId: input.deploymentId,
    emailMessageId: input.emailMessageId,
    emailStatus: input.emailStatus,
    providerMessageId: input.providerMessageId ?? null,
    siteId: input.siteId,
    siteUrl: input.siteUrl,
    skipReason: input.skipReason ?? null,
    softwareOrderId: input.softwareOrderId,
    stage: input.stage,
    versionId: input.versionId,
  })

const reviewReadyArtifactNotificationPayloadJson = (
  input: Readonly<{
    artifactId: string
    artifactUrl: string | null
    emailMessageId: string | null
    emailStatus: string
    providerMessageId?: string | null | undefined
    skipReason?: string | undefined
    softwareOrderId: string
    stage: 'review_ready'
  }>,
): string =>
  JSON.stringify({
    artifactId: input.artifactId,
    artifactUrl: input.artifactUrl,
    emailMessageId: input.emailMessageId,
    emailStatus: input.emailStatus,
    providerMessageId: input.providerMessageId ?? null,
    skipReason: input.skipReason ?? null,
    softwareOrderId: input.softwareOrderId,
    stage: input.stage,
  })

const siteRevisionUrl = (
  siteUrl: string | null,
  versionId: string | null,
): string | null =>
  siteUrl === null || versionId === null
    ? null
    : `${siteUrl.replace(/\/+$/, '')}/versions/${encodeURIComponent(versionId)}`

const notifyCustomerSiteDeployed = async (
  env: Env,
  input: Readonly<{
    actorUserId: string
    deploymentId: string
    siteId: string
    siteUrl: string
  }>,
): Promise<SiteCustomerNotificationOutcome> => {
  const db = openAgentsDatabase(env)
  const existingSiteEvent = await db
    .prepare(
      `SELECT id
         FROM site_events
        WHERE site_id = ?
          AND deployment_id = ?
          AND type = 'adjutant.notification.deployed'
        LIMIT 1`,
    )
    .bind(input.siteId, input.deploymentId)
    .first<Readonly<{ id: string }>>()

  if (existingSiteEvent !== null) {
    return {
      emailMessageId: null,
      emailStatus: 'skipped',
      skipReason: 'notification_event_already_recorded',
    }
  }

  const target = await db
    .prepare(
      `SELECT software_orders.id AS order_id,
              users.id AS target_user_id,
              users.display_name,
              users.primary_email,
              site_projects.title AS site_title
         FROM site_projects
         LEFT JOIN software_orders
           ON software_orders.id = site_projects.software_order_id
          AND software_orders.archived_at IS NULL
         LEFT JOIN users
           ON users.id = software_orders.user_id
          AND users.deleted_at IS NULL
        WHERE site_projects.id = ?
          AND site_projects.archived_at IS NULL
        LIMIT 1`,
    )
    .bind(input.siteId)
    .first<SiteCustomerNotificationRow>()
  const assignment = await db
    .prepare(
      `SELECT id,
              software_order_id,
              goal_id,
              current_run_id,
              visibility
         FROM adjutant_assignments
        WHERE site_id = ?
          AND archived_at IS NULL
        ORDER BY updated_at DESC
        LIMIT 1`,
    )
    .bind(input.siteId)
    .first<AdjutantNotificationAssignmentRow>()
  const email = target?.primary_email?.trim()
  const resend = getResendEmailConfig(env)
  const notification =
    await (async (): Promise<SiteCustomerNotificationOutcome> => {
      if (
        target === null ||
        target.order_id === null ||
        target.target_user_id === null
      ) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_order_notification_target',
        }
      }

      if (email === undefined || email === '') {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_customer_email',
        }
      }

      if (resend === undefined) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'email_config_missing',
        }
      }

      const notificationInput = new OrderSitesTransactionalEmailInput({
        appOrigin: getAppOrigin(env),
        ...(assignment === null ? {} : { assignmentId: assignment.id }),
        artifactLabel: null,
        artifactUrl: null,
        customerSafeStatus: 'deployed',
        displayName: target.display_name ?? email,
        eventRef: input.deploymentId,
        lifecycleKind: 'site_deployed',
        nextAction:
          'Review the deployed Site and reply with any requested adjustment.',
        orderId: target.order_id,
        revisionUrl: null,
        safeReason: null,
        siteId: input.siteId,
        siteTitle: target.site_title,
        siteUrl: input.siteUrl,
        sourceAuthorityRefs: [
          'docs/2026-06-05-adjutant-sites-supervisor-audit.md#16',
        ],
        targetRefs: [
          target.order_id,
          input.siteId,
          input.deploymentId,
          ...(assignment === null ? [] : [assignment.id]),
        ],
        to: email,
      })

      const result = await observedEffect(
        'Email.sendOrderSitesTransactionalEmailWithLedger',
        sendOrderSitesTransactionalEmailWithLedger(
          db,
          resend,
          new OrderSitesTransactionalEmailInput({
            ...notificationInput,
            idempotencyKey:
              buildOrderSitesTransactionalEmailIdempotencyKey(
                notificationInput,
              ),
          }),
          {
            actorUserId: input.actorUserId,
            metadata: {
              assignmentId: assignment?.id ?? null,
              deploymentId: input.deploymentId,
              eventSource: 'operator_sites_deploy',
              lifecycleKind: 'site_deployed',
              siteId: input.siteId,
              softwareOrderId: target.order_id,
              stage: 'deployed',
            },
            sourceAuthorityRef: 'system.order_sites_lifecycle_email.v1',
            targetUserId: target.target_user_id,
          },
        ),
      )

      return result.ok
        ? {
            emailMessageId: result.emailMessageId,
            emailStatus: 'accepted',
            providerMessageId: result.providerMessageId,
          }
        : {
            emailMessageId: result.emailMessageId,
            emailStatus: 'failed',
            skipReason: result.errorMessage,
          }
    })()
  const now = currentIsoTimestamp()
  const payload = notificationPayloadJson({
    deploymentId: input.deploymentId,
    emailMessageId: notification.emailMessageId,
    emailStatus: notification.emailStatus,
    providerMessageId: notification.providerMessageId,
    siteId: input.siteId,
    siteUrl: input.siteUrl,
    skipReason: notification.skipReason,
    softwareOrderId: assignment?.software_order_id ?? target?.order_id ?? null,
    stage: 'deployed',
  })
  const summary =
    notification.emailStatus === 'accepted'
      ? 'Autopilot customer deployed email notification was accepted.'
      : notification.emailStatus === 'failed'
        ? 'Autopilot customer deployed email notification failed.'
        : 'Autopilot customer deployed email notification is needed.'

  if (assignment !== null) {
    await observedEffect(
      'AdjutantUsageReceipts.recordHosting',
      recordAdjutantUsageReceipt(db, {
        assignmentId: assignment.id,
        billingMode: 'public_beta_free',
        category: 'hosting',
        idempotencyKey: [
          'adjutant_usage',
          assignment.id,
          input.deploymentId,
          'hosting',
        ].join(':'),
        publicDetails: {
          billingNote: 'Public beta Site hosting is free.',
          siteUrl: input.siteUrl,
        },
        quantity: 1,
        runId: assignment.current_run_id,
        siteId: input.siteId,
        softwareOrderId: assignment.software_order_id,
        summary: 'Autopilot activated public Site hosting.',
        teamDetails: {
          billingPolicy: 'public_beta_free',
          deploymentId: input.deploymentId,
          siteId: input.siteId,
          siteUrl: input.siteUrl,
        },
        unit: 'deployment',
        visibility: assignment.visibility,
      }),
    )

    await db
      .prepare(
        `INSERT INTO adjutant_assignment_events
           (id,
            assignment_id,
            software_order_id,
            site_id,
            goal_id,
            run_id,
            event_type,
            visibility,
            summary,
            actor_user_id,
            payload_json,
            email_message_id,
            created_at)
         VALUES (?, ?, ?, ?, ?, ?, 'adjutant.notification.deployed', ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        compactRandomId('adjutant_assignment_event'),
        assignment.id,
        assignment.software_order_id,
        input.siteId,
        assignment.goal_id,
        assignment.current_run_id,
        assignment.visibility,
        summary,
        input.actorUserId,
        payload,
        notification.emailMessageId,
        now,
      )
      .run()
  }

  await db
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
          email_message_id,
          created_at)
       VALUES (?, ?, NULL, ?, 'adjutant.notification.deployed', ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      compactRandomId('site_event'),
      input.siteId,
      input.deploymentId,
      summary,
      input.actorUserId,
      assignment?.current_run_id ?? null,
      payload,
      notification.emailMessageId,
      now,
    )
    .run()

  return notification
}

const readPendingReviewReadyNotifications = async (
  db: D1Database,
): Promise<ReadonlyArray<ReviewReadyNotificationRow>> =>
  db
    .prepare(
      `SELECT site_projects.id AS site_id,
              site_projects.title AS site_title,
              site_projects.active_version_id AS version_id,
              site_projects.active_deployment_id AS deployment_id,
              site_deployments.url AS site_url,
              software_orders.id AS order_id,
              users.id AS target_user_id,
              users.display_name,
              users.primary_email,
              adjutant_assignments.id AS assignment_id,
              adjutant_assignments.goal_id AS assignment_goal_id,
              adjutant_assignments.current_run_id AS assignment_current_run_id,
              adjutant_assignments.visibility AS assignment_visibility
         FROM site_projects
         JOIN site_versions
           ON site_versions.id = site_projects.active_version_id
          AND site_versions.site_id = site_projects.id
         LEFT JOIN site_deployments
           ON site_deployments.id = site_projects.active_deployment_id
          AND site_deployments.site_id = site_projects.id
         LEFT JOIN software_orders
           ON software_orders.id = site_projects.software_order_id
          AND software_orders.archived_at IS NULL
         LEFT JOIN users
           ON users.id = software_orders.user_id
          AND users.deleted_at IS NULL
         LEFT JOIN adjutant_assignments
           ON adjutant_assignments.id = (
                SELECT id
                  FROM adjutant_assignments AS assignment
                 WHERE assignment.site_id = site_projects.id
                   AND assignment.archived_at IS NULL
                 ORDER BY assignment.updated_at DESC
                 LIMIT 1
              )
        WHERE site_projects.archived_at IS NULL
          AND site_projects.active_version_id IS NOT NULL
          AND json_extract(site_versions.metadata_json, '$.customerReviewState') = 'customer_review_ready'
          AND NOT EXISTS (
                SELECT 1
                  FROM site_events
                  JOIN email_messages
                    ON email_messages.id = site_events.email_message_id
                 WHERE site_events.site_id = site_projects.id
                   AND site_events.version_id = site_projects.active_version_id
                   AND site_events.type = 'adjutant.notification.review_ready'
                   AND email_messages.status = 'accepted'
              )
        ORDER BY site_versions.created_at ASC
        LIMIT 10`,
    )
    .all<ReviewReadyNotificationRow>()
    .then(result => result.results)

const sendReviewReadySiteNotification = async (
  env: Parameters<typeof getResendEmailConfig>[0],
  row: ReviewReadyNotificationRow,
): Promise<SiteCustomerNotificationOutcome> => {
  const db = openAgentsDatabase(env)
  const email = row.primary_email?.trim()
  const resend = getResendEmailConfig(env)
  const notification =
    await (async (): Promise<SiteCustomerNotificationOutcome> => {
      if (row.order_id === null || row.target_user_id === null) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_order_notification_target',
        }
      }

      if (email === undefined || email === '') {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_customer_email',
        }
      }

      if (resend === undefined) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'email_config_missing',
        }
      }

      const notificationInput = new OrderSitesTransactionalEmailInput({
        appOrigin: getAppOrigin(env),
        ...(row.assignment_id === null
          ? {}
          : { assignmentId: row.assignment_id }),
        artifactLabel: null,
        artifactUrl: null,
        customerSafeStatus: 'Ready for review',
        displayName: row.display_name ?? email,
        eventRef: row.version_id,
        lifecycleKind: 'review_ready',
        nextAction:
          'Open your order status page, review the latest Site revision, and send any follow-up comment.',
        orderId: row.order_id,
        revisionUrl: siteRevisionUrl(row.site_url, row.version_id),
        safeReason: null,
        siteId: row.site_id,
        siteTitle: row.site_title,
        siteUrl: row.site_url,
        sourceAuthorityRefs: [
          'docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md#email-and-drip-campaign-plan',
          'system.review_ready_site_notification_reconciler.v1',
        ],
        targetRefs: [
          row.order_id,
          row.site_id,
          row.version_id,
          ...(row.deployment_id === null ? [] : [row.deployment_id]),
          ...(row.assignment_id === null ? [] : [row.assignment_id]),
        ],
        to: email,
      })

      const result = await observedEffect(
        'Email.sendReviewReadySiteNotification',
        sendOrderSitesTransactionalEmailWithLedger(
          db,
          resend,
          new OrderSitesTransactionalEmailInput({
            ...notificationInput,
            idempotencyKey:
              buildOrderSitesTransactionalEmailIdempotencyKey(
                notificationInput,
              ),
          }),
          {
            actorUserId: 'system:review-ready-reconciler',
            metadata: {
              assignmentId: row.assignment_id,
              deploymentId: row.deployment_id,
              eventSource: 'review_ready_site_notification_reconciler',
              lifecycleKind: 'review_ready',
              siteId: row.site_id,
              softwareOrderId: row.order_id,
              stage: 'review_ready',
              versionId: row.version_id,
            },
            sourceAuthorityRef:
              'system.review_ready_site_notification_reconciler.v1',
            targetUserId: row.target_user_id,
          },
        ),
      )

      return result.ok
        ? {
            emailMessageId: result.emailMessageId,
            emailStatus: 'accepted',
            providerMessageId: result.providerMessageId,
          }
        : {
            emailMessageId: result.emailMessageId,
            emailStatus: 'failed',
            skipReason: result.errorMessage,
          }
    })()
  const now = currentIsoTimestamp()
  const payload = reviewReadyNotificationPayloadJson({
    deploymentId: row.deployment_id,
    emailMessageId: notification.emailMessageId,
    emailStatus: notification.emailStatus,
    providerMessageId: notification.providerMessageId,
    siteId: row.site_id,
    siteUrl: row.site_url,
    skipReason: notification.skipReason,
    softwareOrderId: row.order_id,
    stage: 'review_ready',
    versionId: row.version_id,
  })
  const summary =
    notification.emailStatus === 'accepted'
      ? 'Autopilot customer review-ready email notification was accepted.'
      : notification.emailStatus === 'failed'
        ? 'Autopilot customer review-ready email notification failed.'
        : 'Autopilot customer review-ready email notification is needed.'

  if (row.assignment_id !== null) {
    await db
      .prepare(
        `INSERT INTO adjutant_assignment_events
           (id,
            assignment_id,
            software_order_id,
            site_id,
            goal_id,
            run_id,
            event_type,
            visibility,
            summary,
            actor_user_id,
            payload_json,
            email_message_id,
            created_at)
         VALUES (?, ?, ?, ?, ?, ?, 'adjutant.notification.review_ready', ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        compactRandomId('adjutant_assignment_event'),
        row.assignment_id,
        row.order_id,
        row.site_id,
        row.assignment_goal_id,
        row.assignment_current_run_id,
        row.assignment_visibility ?? 'public',
        summary,
        'system:review-ready-reconciler',
        payload,
        notification.emailMessageId,
        now,
      )
      .run()
  }

  await db
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
          email_message_id,
          created_at)
       VALUES (?, ?, ?, ?, 'adjutant.notification.review_ready', ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      compactRandomId('site_event'),
      row.site_id,
      row.version_id,
      row.deployment_id,
      summary,
      'system:review-ready-reconciler',
      row.assignment_current_run_id,
      payload,
      notification.emailMessageId,
      now,
    )
    .run()

  return notification
}

const sendPendingReviewReadySiteNotifications = async (
  env: Parameters<typeof getResendEmailConfig>[0],
): Promise<void> => {
  if (getResendEmailConfig(env) === undefined) {
    return
  }

  const pending = await readPendingReviewReadyNotifications(
    openAgentsDatabase(env),
  )

  await Promise.all(
    pending.map(row =>
      sendReviewReadySiteNotification(env, row).catch(error => {
        logWorkerRouteWarning('review_ready_site_notification_failed', {
          error: errorMessage(error),
          siteId: row.site_id,
          versionId: row.version_id,
        })
      }),
    ),
  )
}

const readPendingReviewReadyArtifactNotifications = async (
  db: D1Database,
): Promise<ReadonlyArray<ReviewReadyArtifactNotificationRow>> =>
  db
    .prepare(
      `SELECT order_fulfillment_artifacts.id AS artifact_id,
              order_fulfillment_artifacts.title AS artifact_title,
              order_fulfillment_artifacts.url AS artifact_url,
              order_fulfillment_artifacts.kind AS kind,
              order_fulfillment_artifacts.repository_full_name AS repository_full_name,
              software_orders.id AS order_id,
              users.id AS target_user_id,
              users.display_name,
              users.primary_email,
              adjutant_assignments.id AS assignment_id,
              adjutant_assignments.goal_id AS assignment_goal_id,
              adjutant_assignments.current_run_id AS assignment_current_run_id,
              adjutant_assignments.visibility AS assignment_visibility
         FROM order_fulfillment_artifacts
         JOIN software_orders
           ON software_orders.id = order_fulfillment_artifacts.software_order_id
          AND software_orders.archived_at IS NULL
         LEFT JOIN users
           ON users.id = software_orders.user_id
          AND users.deleted_at IS NULL
         LEFT JOIN adjutant_assignments
           ON adjutant_assignments.id = (
                SELECT id
                  FROM adjutant_assignments AS assignment
                 WHERE assignment.software_order_id = software_orders.id
                   AND assignment.archived_at IS NULL
                 ORDER BY assignment.updated_at DESC
                 LIMIT 1
              )
        WHERE order_fulfillment_artifacts.archived_at IS NULL
          AND order_fulfillment_artifacts.visibility = 'public'
          AND order_fulfillment_artifacts.status = 'customer_review_ready'
          AND adjutant_assignments.id IS NOT NULL
          AND NOT EXISTS (
                SELECT 1
                  FROM adjutant_assignment_events
                  JOIN email_messages
                    ON email_messages.id = adjutant_assignment_events.email_message_id
                 WHERE adjutant_assignment_events.software_order_id = software_orders.id
                   AND adjutant_assignment_events.event_type = 'adjutant.notification.review_ready_artifact'
                   AND json_extract(adjutant_assignment_events.payload_json, '$.artifactId') = order_fulfillment_artifacts.id
                   AND email_messages.status = 'accepted'
              )
        ORDER BY order_fulfillment_artifacts.created_at ASC
        LIMIT 10`,
    )
    .all<ReviewReadyArtifactNotificationRow>()
    .then(result => result.results)

const sendReviewReadyArtifactNotification = async (
  env: Parameters<typeof getResendEmailConfig>[0],
  row: ReviewReadyArtifactNotificationRow,
): Promise<SiteCustomerNotificationOutcome> => {
  const db = openAgentsDatabase(env)
  const email = row.primary_email?.trim()
  const resend = getResendEmailConfig(env)
  const notification =
    await (async (): Promise<SiteCustomerNotificationOutcome> => {
      if (row.target_user_id === null) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_order_notification_target',
        }
      }

      if (row.assignment_id === null) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_assignment',
        }
      }

      if (email === undefined || email === '') {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_customer_email',
        }
      }

      if (resend === undefined) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'email_config_missing',
        }
      }

      const artifactLabel =
        row.kind === 'pull_request'
          ? 'Review pull request'
          : row.kind === 'diff'
            ? 'Review diff'
            : 'Review artifact'
      const notificationInput = new OrderSitesTransactionalEmailInput({
        appOrigin: getAppOrigin(env),
        ...(row.assignment_id === null
          ? {}
          : { assignmentId: row.assignment_id }),
        artifactLabel,
        artifactUrl: row.artifact_url,
        customerSafeStatus: 'Ready for review',
        displayName: row.display_name ?? email,
        eventRef: row.artifact_id,
        lifecycleKind: 'review_ready',
        nextAction:
          'Open your order status page, review the latest deliverable, and send any follow-up comment.',
        orderId: row.order_id,
        revisionUrl: null,
        safeReason: null,
        siteTitle: row.artifact_title,
        siteUrl: null,
        sourceAuthorityRefs: [
          'docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md#email-and-drip-campaign-plan',
          'system.review_ready_artifact_notification_reconciler.v1',
        ],
        targetRefs: [
          row.order_id,
          row.artifact_id,
          ...(row.assignment_id === null ? [] : [row.assignment_id]),
        ],
        to: email,
      })

      const result = await observedEffect(
        'Email.sendReviewReadyArtifactNotification',
        sendOrderSitesTransactionalEmailWithLedger(
          db,
          resend,
          new OrderSitesTransactionalEmailInput({
            ...notificationInput,
            idempotencyKey:
              buildOrderSitesTransactionalEmailIdempotencyKey(
                notificationInput,
              ),
          }),
          {
            actorUserId: 'system:review-ready-artifact-reconciler',
            metadata: {
              artifactId: row.artifact_id,
              assignmentId: row.assignment_id,
              eventSource: 'review_ready_artifact_notification_reconciler',
              lifecycleKind: 'review_ready',
              softwareOrderId: row.order_id,
              stage: 'review_ready',
            },
            sourceAuthorityRef:
              'system.review_ready_artifact_notification_reconciler.v1',
            targetUserId: row.target_user_id,
          },
        ),
      )

      return result.ok
        ? {
            emailMessageId: result.emailMessageId,
            emailStatus: 'accepted',
            providerMessageId: result.providerMessageId,
          }
        : {
            emailMessageId: result.emailMessageId,
            emailStatus: 'failed',
            skipReason: result.errorMessage,
          }
    })()
  const now = currentIsoTimestamp()
  const payload = reviewReadyArtifactNotificationPayloadJson({
    artifactId: row.artifact_id,
    artifactUrl: row.artifact_url,
    emailMessageId: notification.emailMessageId,
    emailStatus: notification.emailStatus,
    providerMessageId: notification.providerMessageId,
    skipReason: notification.skipReason,
    softwareOrderId: row.order_id,
    stage: 'review_ready',
  })
  const summary =
    notification.emailStatus === 'accepted'
      ? 'Autopilot customer artifact review-ready email notification was accepted.'
      : notification.emailStatus === 'failed'
        ? 'Autopilot customer artifact review-ready email notification failed.'
        : 'Autopilot customer artifact review-ready email notification is needed.'

  await db
    .prepare(
      `INSERT INTO adjutant_assignment_events
         (id,
          assignment_id,
          software_order_id,
          site_id,
          goal_id,
          run_id,
          event_type,
          visibility,
          summary,
          actor_user_id,
          payload_json,
          email_message_id,
          created_at)
       VALUES (?, ?, ?, NULL, ?, ?, 'adjutant.notification.review_ready_artifact', ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      compactRandomId('adjutant_assignment_event'),
      row.assignment_id,
      row.order_id,
      row.assignment_goal_id,
      row.assignment_current_run_id,
      row.assignment_visibility ?? 'public',
      summary,
      'system:review-ready-artifact-reconciler',
      payload,
      notification.emailMessageId,
      now,
    )
    .run()

  return notification
}

const sendPendingReviewReadyArtifactNotifications = async (
  env: Parameters<typeof getResendEmailConfig>[0],
): Promise<void> => {
  if (getResendEmailConfig(env) === undefined) {
    return
  }

  const pending = await readPendingReviewReadyArtifactNotifications(
    openAgentsDatabase(env),
  )

  await Promise.all(
    pending.map(row =>
      sendReviewReadyArtifactNotification(env, row).catch(error => {
        logWorkerRouteWarning('review_ready_artifact_notification_failed', {
          artifactId: row.artifact_id,
          error: errorMessage(error),
          orderId: row.order_id,
        })
      }),
    ),
  )
}

const enforceOutOfCreditsPolicy = async (
  env: Env,
  ctx: ExecutionContext | undefined,
  userId: string,
): Promise<void> => {
  const billing = await suspendBillingAccountIfOutOfCredits(
    openAgentsDatabase(env),
    userId,
  )

  if (!billing.exhausted) {
    return
  }

  const canceledRuns = await cancelActiveAgentRunsForBillingExhaustion(
    openAgentsDatabase(env),
    userId,
    {
      balanceCents: billing.balanceCents,
      balanceFormatted: billing.balanceFormatted,
    },
  )

  await Promise.all(
    canceledRuns.map(item => notifyAgentRunSyncScopes(env, item.run.id)),
  )

  const cleanup = Promise.all(
    canceledRuns.map(item => cleanupCanceledAgentRunOnShc(env, item.run)),
  ).then(() => undefined)
  const notify = sendOutOfCreditsNotificationOnce(env, {
    balanceCents: billing.balanceCents,
    balanceFormatted: billing.balanceFormatted,
    userId,
  })

  if (ctx === undefined) {
    await Promise.all([cleanup, notify])

    return
  }

  scheduleBackgroundWork(ctx, cleanup)
  scheduleBackgroundWork(ctx, notify)
}

const makeBillingAwareOmniRunStore = (env: Env, ctx?: ExecutionContext) =>
  makeD1OmniRunStore(openAgentsDatabase(env), {
    afterAgentRunMetered: run =>
      enforceOutOfCreditsPolicy(env, ctx, run.userId),
  })

const tokenUsageLeaderboardsLayer = (env: Env) =>
  TokenUsageLeaderboards.effectCfLayer().pipe(
    Layer.provide(OpenAgentsDatabase.layer),
    Layer.provide(Layer.succeed(WorkerEnvironment, env)),
  )

const emptyEmailCampaignDispatcherResult =
  (): EmailCampaignDispatcherResult => ({
    claimed: 0,
    failed: 0,
    retried: 0,
    sent: 0,
    skipped: 0,
    suppressed: 0,
  })

const emailCampaignDispatcherLayer = (env: EmailCampaignDispatcherBindings) =>
  OpenAgentsDatabase.layer.pipe(
    Layer.provide(Layer.succeed(WorkerEnvironment, env)),
  )

const dispatchDueEmailCampaignSendsScheduled = (
  env: EmailCampaignDispatcherBindings,
): Effect.Effect<EmailCampaignDispatcherResult, never> =>
  Effect.gen(function* () {
    const config = getOpenAgentsWorkerConfig(env)
    const db = yield* OpenAgentsDatabase

    return yield* dispatchDueEmailCampaignSends(db, {
      appOrigin: config.app.origin,
      resend: config.email.resend,
    })
  }).pipe(
    Effect.provide(emailCampaignDispatcherLayer(env)),
    Effect.catch(() => Effect.succeed(emptyEmailCampaignDispatcherResult())),
  )

const runArtanisScheduledTickScheduled = (
  db: D1Database,
  scheduledRunnerEnabled: boolean,
  scheduledTime: number,
): Effect.Effect<void, never> =>
  runArtanisScheduledTickForWorker({
    db,
    scheduledRunnerEnabled,
    scheduledTime,
  }).pipe(
    Effect.asVoid,
    Effect.catch(() => Effect.void),
  )

const readTokenUsageLeaderboardsForUser = (
  env: Env,
  userId: string,
): Promise<AutopilotTokenLeaderboards> =>
  Effect.runPromise(
    Effect.gen(function* () {
      const tokenUsageLeaderboards = yield* TokenUsageLeaderboards

      return yield* Effect.promise(() =>
        tokenUsageLeaderboards.readForUser(userId),
      )
    }).pipe(Effect.provide(tokenUsageLeaderboardsLayer(env))),
  )

const readSelectedOperatorTargetUser = (
  db: D1Database,
  selector: Record<string, unknown>,
): Promise<OperatorTargetUser | undefined> =>
  readOperatorTargetUser(db, selector, OPENAGENTS_ADMIN_EMAILS[0])

const sweepActiveAgentRunBilling = async (
  env: Env,
  ctx?: ExecutionContext,
): Promise<void> => {
  const billUntil = workerRuntime.nowIso()
  const activeRuns = await listActiveAgentRunsForBilling(
    openAgentsDatabase(env),
    100,
  )

  for (const run of activeRuns) {
    await recordContainerUsageDebitForRun(openAgentsDatabase(env), run, {
      billUntil,
    })
    await enforceOutOfCreditsPolicy(env, ctx, run.userId)
  }
}

const handleAdminSyncNotifyApi = async (
  request: Request,
  env: Env,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  if (!(await requireAdminApiToken(request, env))) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  const body = await readJsonObject(request).catch(
    (): Record<string, unknown> => ({}),
  )
  const scopes = [
    ...stringArrayFromUnknown(body.scopes),
    ...(optionalString(body.scope) === undefined
      ? []
      : [optionalString(body.scope)]),
  ].filter((scope): scope is string => scope !== undefined)

  if (scopes.length === 0) {
    return noStoreJsonResponse(
      { error: 'bad_request', reason: 'scope or scopes is required' },
      { status: 400 },
    )
  }

  await notifySyncScopes(env, scopes)

  return noStoreJsonResponse({
    ok: true,
    scopes: [...new Set(scopes)],
  })
}

const providerDeviceLoginSecretKey = (attemptId: string): string =>
  `provider-device-login:${attemptId}`

const providerAuthSecretKey = (providerAccountRef: string): string =>
  `provider-auth:${providerAccountRef}`

const providerSecretRef = (providerAccountRef: string): string =>
  `codex-auth://${providerAccountRef}`

const startedDeviceLoginTtlSeconds = (expiresAt: string): number => {
  const milliseconds = Date.parse(expiresAt) - workerRuntime.now().getTime()

  return Math.max(60, Math.ceil(milliseconds / 1000))
}

const storeStartedCodexDeviceLogin =
  (kv: KVNamespace) =>
  async (
    input: Readonly<{
      attemptId: string
      deviceAuthId: string
      userCode: string
      expiresAt: string
    }>,
  ): Promise<void> => {
    await kv.put(
      providerDeviceLoginSecretKey(input.attemptId),
      JSON.stringify({
        deviceAuthId: input.deviceAuthId,
        userCode: input.userCode,
      }),
      { expirationTtl: startedDeviceLoginTtlSeconds(input.expiresAt) },
    )
  }

const readStartedCodexDeviceLogin =
  (kv: KVNamespace) =>
  async (
    attemptId: string,
  ): Promise<
    Readonly<{ deviceAuthId: string; userCode: string }> | undefined
  > => {
    const value = await kv.get(providerDeviceLoginSecretKey(attemptId), 'json')

    if (
      !isRecord(value) ||
      typeof value.deviceAuthId !== 'string' ||
      typeof value.userCode !== 'string'
    ) {
      return undefined
    }

    return {
      deviceAuthId: value.deviceAuthId,
      userCode: value.userCode,
    }
  }

const deleteStartedCodexDeviceLogin =
  (kv: KVNamespace) =>
  async (attemptId: string): Promise<void> => {
    await kv.delete(providerDeviceLoginSecretKey(attemptId))
  }

const storeConnectedCodexAuth =
  (kv: KVNamespace) =>
  async (
    input: Readonly<{
      providerAccountRef: string
      auth: CodexOAuthAuth
    }>,
  ): Promise<string> => {
    const secretRef = providerSecretRef(input.providerAccountRef)

    await kv.put(
      providerAuthSecretKey(input.providerAccountRef),
      JSON.stringify({
        openai: input.auth,
      }),
    )

    return secretRef
  }

const readConnectedCodexAuthMaterial = async (
  kv: KVNamespace,
  providerAccountRef: string,
): Promise<
  | Readonly<{
      authContentEnv: 'OPENCODE_AUTH_CONTENT'
      authContentJson: string
    }>
  | undefined
> => {
  const raw = await kv.get(providerAuthSecretKey(providerAccountRef), 'text')

  if (raw === null) {
    return undefined
  }

  const parsed = safeJsonRecord(raw)

  if (!isRecord(parsed) || !isRecord(parsed.openai)) {
    return undefined
  }

  const openai = parsed.openai

  if (
    optionalString(openai.type) !== 'oauth' ||
    optionalString(openai.access) === undefined ||
    optionalString(openai.refresh) === undefined
  ) {
    return undefined
  }

  return {
    authContentEnv: 'OPENCODE_AUTH_CONTENT',
    authContentJson: JSON.stringify(parsed),
  }
}

export const handleProgrammaticAgentRegistration = async (
  request: Request,
  env: Env,
  agentRegistrationStore?: AgentRegistrationStore,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const body = await request.json().catch(error => ({
    parseError: errorMessage(error),
  }))
  let parsed: typeof ProgrammaticAgentRegistrationRequest.Type

  try {
    parsed = decodeUnknownWithSchema(ProgrammaticAgentRegistrationRequest, body)
  } catch (error) {
    return withAgentRateLimitHeaders(badRequest(errorMessage(error)))
  }

  const store =
    agentRegistrationStore ??
    makeD1AgentRegistrationStore(openAgentsDatabase(env))

  try {
    const registration = await createProgrammaticAgentRegistration(
      store,
      parsed,
    )

    return withAgentRateLimitHeaders(
      jsonResponse(registration, { status: 201 }),
    )
  } catch (error) {
    if (isUniqueConstraintError(error)) {
      return withAgentRateLimitHeaders(
        jsonResponse({ error: 'agent_registration_conflict' }, { status: 409 }),
      )
    }

    return serverError()
  }
}

const handleProgrammaticAgentMe = async (
  request: Request,
  env: Env,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const bearerToken = readBearerToken(request)

  if (bearerToken === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  const session = await authenticateProgrammaticAgent(
    makeD1AgentRegistrationStore(openAgentsDatabase(env)),
    bearerToken,
  )

  if (session === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  return withAgentRateLimitHeaders(
    jsonResponse({ authenticated: true, agent: session }),
  )
}

const routeAuthHostRequest = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  const url = new URL(request.url)

  if (url.pathname === '/' || url.pathname === '/login') {
    return redirectResponse(getAppOrigin(env))
  }

  if (url.pathname === '/github/callback') {
    const state = url.searchParams.get('state')

    if (state !== null) {
      const attempt = await makeD1GitHubWriteRepository(
        openAgentsDatabase(env),
      ).findAttemptByState(state)

      if (attempt !== undefined) {
        return handleGitHubWriteCallback(request, env, attempt)
      }
    }
  }

  return makeAuthIssuer(env).fetch(request, env, ctx)
}

export { findAuthorizedAgentRunBundle } from './thread-access'

const isRouteAccessError = (
  value: AgentRunBundle | RouteAccessError,
): value is RouteAccessError =>
  value instanceof RouteAccessForbidden || value instanceof RouteAccessNotFound

const threadRouteAccessBundle = (
  env: Env,
  userId: string,
  routeId: string,
): Promise<AgentRunBundle | RouteAccessError> =>
  Effect.runPromise(
    Effect.gen(function* () {
      const threadAccess = yield* ThreadAccessService

      return yield* threadAccess.findAuthorizedBundle({ routeId, userId }).pipe(
        Effect.match({
          onFailure: error => error,
          onSuccess: bundle => bundle,
        }),
      )
    }).pipe(Effect.provide(ThreadAccessService.layer(env))),
  )

const threadRouteAccessError = async (
  env: Env,
  userId: string,
  routeId: string,
): Promise<RouteAccessError | undefined> => {
  const accessResult = await threadRouteAccessBundle(env, userId, routeId)

  return isRouteAccessError(accessResult) ? accessResult : undefined
}

const authorizeSyncPath = async (
  env: Env,
  session: VerifiedSession,
  syncPath: ParsedSyncPath,
): Promise<RouteAccessError | undefined> => {
  if (syncPath.kind === 'workspace') {
    return syncPath.id === session.user.userId
      ? undefined
      : new RouteAccessForbidden({ routeId: syncPath.id })
  }

  if (syncPath.kind === 'team') {
    const role =
      (await readActiveTeamMembershipRole(
        openAgentsDatabase(env),
        syncPath.id,
        session.user.userId,
      )) ?? undefined

    return role === undefined
      ? new RouteAccessForbidden({ routeId: syncPath.id })
      : undefined
  }

  if (syncPath.kind === 'thread' || syncPath.kind === 'agent-run') {
    return threadRouteAccessError(env, session.user.userId, syncPath.id)
  }

  return new RouteAccessForbidden({ routeId: syncPath.id })
}

const threadFileRoutes = makeThreadFileRoutes({
  appendRefreshedSessionCookies,
  publishTeamThreadFileSync,
  readActiveTeamMembershipRole,
  requireBrowserSession,
})

const agentSiteRoutes = makeAgentSiteRoutes({
  agentStoreForEnv: env =>
    makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  appendRefreshedSessionCookies,
  artifactsForEnv: env => env.ARTIFACTS,
  dbForEnv: openAgentsDatabase,
  isAdminEmail: isOpenAgentsAdminEmail,
  requireBrowserSession,
})

const hostedMdkClientForEnv = (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) => {
  const checkout = getOpenAgentsWorkerConfig(env).mdk.checkout
  const routeSecret = redactedValue(checkout.routeSecret)

  if (
    !checkout.configured ||
    checkout.routeUrl === undefined ||
    routeSecret === undefined
  ) {
    return makeMissingOpenAgentsHostedMdkClient(checkout.providerRef)
  }

  return makeOpenAgentsHostedMdkRouteClient(
    {
      configRef: checkout.configRef,
      credentialBindingRef: checkout.credentialBindingRef,
      environment: checkout.environment,
      providerRef: checkout.providerRef,
      webhookBindingRef: checkout.webhookBindingRef,
    },
    {
      checkoutPathBase: checkout.checkoutPathBase,
      ...(checkout.routeKind === 'self_hosted_mdkd_sidecar'
        ? {
            fetch: (input, init) => {
              const request =
                input instanceof Request ? input : new Request(input, init)

              return fetchMdkSidecarRequest(request, env)
            },
          }
        : {}),
      routeSecret,
      routeUrl: checkout.routeUrl,
    },
  )
}

const forumL402SigningBoundaryForEnv = async (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) => {
  const checkout = getOpenAgentsWorkerConfig(env).mdk.checkout
  const routeSecret = redactedValue(checkout.routeSecret)

  if (
    !checkout.configured ||
    checkout.credentialBindingRef === null ||
    routeSecret === undefined
  ) {
    return null
  }

  return makeOpenAgentsL402HmacSigningBoundary({
    secretKeyMaterial: routeSecret,
    signerRef: checkout.credentialBindingRef,
  })
}

const hostedMdkWebhookConfigForEnv = (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) => {
  const checkout = getOpenAgentsWorkerConfig(env).mdk.checkout
  const webhookSecret = redactedValue(checkout.webhookSecret)

  if (webhookSecret === undefined || checkout.webhookBindingRef === null) {
    return undefined
  }

  return {
    bindingRef: checkout.webhookBindingRef,
    secret: webhookSecret,
    source: checkout.webhookSource,
  }
}

const siteCommerceRoutesForEnv = (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) =>
  makeSiteCommerceRoutes({
    authorizeCommerceReviewDecision: request =>
      requireAdminApiToken(request, env),
    authorizePaidActionAgent: async request => {
      const bearerToken = readBearerToken(request)

      if (bearerToken === undefined) {
        return false
      }

      const session = await authenticateProgrammaticAgent(
        makeD1AgentRegistrationStore(openAgentsDatabase(env)),
        bearerToken,
      )

      return session !== undefined
    },
    authorizeMdkAccountBinding: request => requireAdminApiToken(request, env),
    authorizePayoutBridge: request => requireAdminApiToken(request, env),
    buyerPaymentLedgerStore: makeD1BuyerPaymentLedgerStore(
      openAgentsDatabase(env),
    ),
    challengeExpiresAt: () => isoTimestampAfter(currentDate(), 10 * 60_000),
    checkoutCatalog: omegaMdkDemoSitePaymentCatalog,
    checkoutIntentStore: makeD1SiteMdkCheckoutIntentStore(
      openAgentsDatabase(env),
    ),
    hostedMdkClient: hostedMdkClientForEnv(env),
    mdkWebhookConfig: hostedMdkWebhookConfigForEnv(env),
    mdkAccountBindingStore: makeD1SiteMdkAccountBindingStore(
      openAgentsDatabase(env),
    ),
    nowEpochMillis: () => currentDate().getTime(),
    nowIso: () => currentDate().toISOString(),
    payoutLedgerStore: makeD1NexusTreasuryPayoutLedgerStore(
      openAgentsDatabase(env),
    ),
    reviewStore: makeD1SiteCommerceReviewStore(openAgentsDatabase(env)),
  })

const siteReferralRoutes = makeSiteReferralRoutes()

const syncDependencyErrorReason = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const syncRoutes = makeSyncRoutes({
  appendRefreshedSessionCookies,
  authorizeSyncPath: (env, session, syncPath) =>
    Effect.tryPromise({
      catch: syncDependencyErrorReason,
      try: () => authorizeSyncPath(env, session, syncPath),
    }),
  requireBrowserSession: (request, env, ctx) =>
    Effect.tryPromise({
      catch: syncDependencyErrorReason,
      try: () => requireBrowserSession(request, env, ctx),
    }),
})

const providerAccountBrowserHandlers = makeProviderAccountBrowserHandlers({
  appendRefreshedSessionCookies,
  deleteStartedCodexDeviceLogin,
  providerAuthSecretKey,
  readStartedCodexDeviceLogin,
  requireBrowserSession,
  storeConnectedCodexAuth,
  storeStartedCodexDeviceLogin,
})

const providerAccountServiceHandlers = makeProviderAccountServiceHandlers({
  readConnectedCodexAuthMaterial,
  requireProviderServiceActor,
})

const operatorProviderAccountRoutes = makeOperatorProviderAccountRoutes({
  deleteStartedCodexDeviceLogin,
  readConnectedCodexAuthMaterial,
  readSelectedOperatorTargetUser,
  readStartedCodexDeviceLogin,
  requireAdminApiToken,
  storeConnectedCodexAuth,
  storeStartedCodexDeviceLogin,
})

const adminOverviewHandlers = makeAdminOverviewHandlers({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireBrowserSession,
})

const tokenUsageLedgerRoutes = makeTokenUsageLedgerRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireAdminApiToken,
  requireBrowserSession,
})

const mulletRoutes = makeMulletRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireBrowserSession,
})

const providerAccountRoutes = makeProviderAccountRoutes({
  handleGitHubWriteDisconnectApi: (request, env, ctx, connectionRef) =>
    routeEffect('handle_github_write_disconnect_api', () =>
      handleGitHubWriteDisconnectApi(request, env, ctx, connectionRef),
    ),
  handleProviderAccountDisconnectApi: (request, env, ctx, providerAccountRef) =>
    routeEffect('handle_provider_account_disconnect_api', () =>
      providerAccountBrowserHandlers.handleProviderAccountDisconnectApi(
        request,
        env,
        ctx,
        providerAccountRef,
      ),
    ),
  handleProviderAccountGrantIssueApi: (request, env, ctx, providerAccountRef) =>
    routeEffect('handle_provider_account_grant_issue_api', () =>
      providerAccountBrowserHandlers.handleProviderAccountGrantIssueApi(
        request,
        env,
        ctx,
        providerAccountRef,
      ),
    ),
  handleProviderAccountGrantResolveApi: (request, env) =>
    routeEffect('handle_provider_account_grant_resolve_api', () =>
      providerAccountServiceHandlers.handleProviderAccountGrantResolveApi(
        request,
        env,
      ),
    ),
  handleGoogleGeminiGrantResolveApi: (request, env) =>
    routeEffect('handle_google_gemini_grant_resolve_api', () =>
      providerAccountServiceHandlers.handleGoogleGeminiGrantResolveApi(
        request,
        env,
      ),
    ),
  handleGoogleGeminiGenerateContentApi: (request, env, ctx, model) =>
    routeEffect('handle_google_gemini_generate_content_api', () =>
      providerAccountServiceHandlers.handleGoogleGeminiGenerateContentApi(
        request,
        env,
        ctx,
        model,
      ),
    ),
  handleProviderAccountHealthApi: (request, env, providerAccountRef) =>
    routeEffect('handle_provider_account_health_api', () =>
      providerAccountServiceHandlers.handleProviderAccountHealthApi(
        request,
        env,
        providerAccountRef,
      ),
    ),
  handleProviderAccountsListApi: (request, env, ctx) =>
    routeEffect('handle_provider_accounts_list_api', () =>
      providerAccountBrowserHandlers.handleProviderAccountsListApi(
        request,
        env,
        ctx,
      ),
    ),
  handleProviderDeviceLoginConnectedApi: (request, env, attemptId) =>
    routeEffect('handle_provider_device_login_connected_api', () =>
      providerAccountServiceHandlers.handleProviderDeviceLoginConnectedApi(
        request,
        env,
        attemptId,
      ),
    ),
  handleProviderDeviceLoginFailedApi: (request, env, attemptId) =>
    routeEffect('handle_provider_device_login_failed_api', () =>
      providerAccountServiceHandlers.handleProviderDeviceLoginFailedApi(
        request,
        env,
        attemptId,
      ),
    ),
  handleProviderDeviceLoginStartApi: (request, env, ctx) =>
    routeEffect('handle_provider_device_login_start_api', () =>
      providerAccountBrowserHandlers.handleProviderDeviceLoginStartApi(
        request,
        env,
        ctx,
      ),
    ),
  handleProviderDeviceLoginStatusApi: (request, env, ctx, attemptId) =>
    routeEffect('handle_provider_device_login_status_api', () =>
      providerAccountBrowserHandlers.handleProviderDeviceLoginStatusApi(
        request,
        env,
        ctx,
        attemptId,
      ),
    ),
})

const billingApiHandlers = makeBillingApiHandlers({
  appendRefreshedSessionCookies,
  requireBrowserSession,
})

const onboardingRoutes = makeOnboardingRoutes({
  appendRefreshedSessionCookies,
  requireBrowserSession,
  siteReferralOnboarding: ({ env, orderState, referralResult, session }) =>
    sendSiteReferralOnboardingForConsumption(openAgentsDatabase(env), {
      appOrigin: getAppOrigin(env),
      displayName: session.user.name,
      email: session.user.email,
      orderState,
      referralResult,
      resend: getResendEmailConfig(env),
      userId: session.user.userId,
    }),
})

const siteReferralInspectionRoutes = makeSiteReferralInspectionRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireBrowserSession,
})

const agentGoalRoutes = makeAgentGoalRoutes({
  appendRefreshedSessionCookies,
  authenticateRequestActor,
  readActiveTeamMembershipRole,
  readActiveTeamProject,
  requireAdminApiToken,
  requireBrowserSession,
})

const agentOwnerClaimRoutes = makeAgentOwnerClaimRoutes({
  appOrigin: getAppOrigin,
  appendRefreshedSessionCookies,
  makeStore: env => makeD1AgentOwnerClaimStore(openAgentsDatabase(env)),
  requireBrowserSession,
})

const agentProposalRoutes = makeAgentProposalRoutes({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  appOrigin: getAppOrigin,
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  makeStore: env => makeD1AgentProposalStore(openAgentsDatabase(env)),
  recoveryStore: env =>
    makeD1AgentRateLimitRecoveryStore(openAgentsDatabase(env)),
  requireAdminApiToken,
  requireBrowserSession,
})

const agentSearchRoutes = makeAgentSearchRoutes({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
})

const agentScopedGrantRoutes = makeAgentScopedGrantRoutes({
  appOrigin: getAppOrigin,
  appendRefreshedSessionCookies,
  makeStore: env => makeD1AgentScopedGrantStore(openAgentsDatabase(env)),
  requireBrowserSession,
})

const shareRoutes = makeShareRoutes({
  appendRefreshedSessionCookies,
  appOrigin: getAppOrigin,
  authenticateRequestActor,
  isAdminEmail: isOpenAgentsAdminEmail,
  readSelectedOperatorTargetUser,
  requireAdminApiToken,
  requireBrowserSession,
})

const operatorBillingHandlers = makeOperatorBillingHandlers({
  readSelectedOperatorTargetUser,
  requireAdminApiToken,
})

const blueprintRoutes = makeBlueprintRoutes<Env>({
  listActionSubmissions: env =>
    listBlueprintActionSubmissions(openAgentsDatabase(env)),
  listProgramRuns: env => listBlueprintProgramRuns(openAgentsDatabase(env)),
  recordActionSubmissionProposal: (env, input) =>
    recordBlueprintActionSubmissionProposal(openAgentsDatabase(env), input),
  recordProgramRun: (env, input) =>
    recordBlueprintProgramRun(openAgentsDatabase(env), input),
  requireAdminApiToken,
  requireActionSubmissionIntake: async (request, env) =>
    (await requireRunnerCallbackAuth(request, env)) ||
    (await requireAdminApiToken(request, env)),
  requireProgramRunEvidenceIntake: async (request, env) =>
    (await requireRunnerCallbackAuth(request, env)) ||
    (await requireAdminApiToken(request, env)),
})

const blueprintProbeContributionRoutes =
  makeBlueprintProbeContributionRoutes<Env>({
    listContributions: env =>
      listBlueprintProbeContributions(openAgentsDatabase(env)),
    recordContribution: (env, input) =>
      recordBlueprintProbeContribution(openAgentsDatabase(env), input),
    requireAdminApiToken,
    requireContributionIntake: async (request, env) =>
      (await requireRunnerCallbackAuth(request, env)) ||
      (await requireAdminApiToken(request, env)),
  })

const operatorSitesRoutes = makeOperatorSitesRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  notifyCustomerSiteDeployed,
  requireBrowserSession,
})

const operatorOrderTriageRoutes = makeOperatorOrderTriageRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireAdminApiToken,
  requireBrowserSession,
})

const operatorEmailInspectionRoutes = makeOperatorEmailInspectionRoutes({
  appendRefreshedSessionCookies,
  getAppOrigin,
  getResendEmailConfig,
  isOpenAgentsAdminEmail,
  requireAdminApiToken,
  requireBrowserSession,
})

const githubRawContentUrl = ({
  commitSha,
  path,
  repositoryName,
  repositoryOwner,
}: AdjutantTaskPacketRefValidationInput): string => {
  const encodedPath = path.split('/').map(encodeURIComponent).join('/')

  return `https://raw.githubusercontent.com/${encodeURIComponent(repositoryOwner)}/${encodeURIComponent(repositoryName)}/${encodeURIComponent(commitSha)}/${encodedPath}`
}

const githubApiContentUrl = ({
  commitSha,
  path,
  repositoryName,
  repositoryOwner,
}: AdjutantTaskPacketRefValidationInput): string => {
  const encodedPath = path.split('/').map(encodeURIComponent).join('/')
  const url = new URL(
    `https://api.github.com/repos/${encodeURIComponent(repositoryOwner)}/${encodeURIComponent(repositoryName)}/contents/${encodedPath}`,
  )
  url.searchParams.set('ref', commitSha)

  return url.toString()
}

const validateAdjutantTaskPacketRef = async (
  input: AdjutantTaskPacketRefValidationInput,
): Promise<boolean> => {
  const githubAccessToken = input.githubAccessToken?.trim()

  if (githubAccessToken !== undefined && githubAccessToken !== '') {
    const apiResponse = await fetch(githubApiContentUrl(input), {
      headers: {
        accept: 'application/vnd.github+json',
        authorization: `Bearer ${githubAccessToken}`,
        'cache-control': 'no-store',
        'user-agent': 'OpenAgents',
        'x-github-api-version': '2022-11-28',
      },
      method: 'GET',
    })

    if (apiResponse.ok) {
      return true
    }
  }

  const url = githubRawContentUrl(input)
  const response = await fetch(url, {
    headers: {
      'cache-control': 'no-store',
      'user-agent': 'OpenAgents',
    },
    method: 'HEAD',
  })

  if (response.status !== 405) {
    return response.ok
  }

  const fallback = await fetch(url, {
    headers: {
      'cache-control': 'no-store',
      range: 'bytes=0-0',
      'user-agent': 'OpenAgents',
    },
    method: 'GET',
  })

  return fallback.ok
}

const omniHandlers = makeOmniHandlers({
  actorJson,
  appendRefreshedSessionCookies,
  appendTeamAutopilotAnswerBack,
  authenticateRequestActor,
  getAppOrigin,
  getResendEmailConfig,
  getRunnerBackendConfig,
  isOpenAgentsAdminEmail,
  isRouteAccessError,
  makeBillingAwareOmniRunStore,
  postTeamChatMessageForUser,
  readSelectedOperatorTargetUser,
  readTokenUsageLeaderboardsForUser,
  requireAdminApiToken,
  requireBrowserSession,
  requireRunnerCallbackAuth,
  shcDispatchConfig,
  threadRouteAccessBundle,
})

const launchUserAutopilotMission = omniHandlers.launchUserAutopilotMission

const operatorAdjutantRoutes = makeOperatorAdjutantRoutes({
  appendRefreshedSessionCookies,
  buildOperatorAutopilotPreflightPayload:
    omniHandlers.buildOperatorAutopilotPreflightPayload,
  continueUserAutopilotRun: omniHandlers.continueUserAutopilotRun,
  isOpenAgentsAdminEmail,
  launchUserAutopilotMission,
  requireAdminApiToken,
  requireBrowserSession,
  validateAdjutantTaskPacketRef,
})

const operatorArtanisConsoleRoutes = makeOperatorArtanisConsoleRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireAdminApiToken,
  requireBrowserSession,
})

const operatorPylonMarketplaceRoutes = makeOperatorPylonMarketplaceRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  makeStore: env => makeD1PylonMarketplaceJobStore(openAgentsDatabase(env)),
  requireAdminApiToken,
  requireBrowserSession,
})

const nexusPylonVisibilityRoutes = makeNexusPylonVisibilityRoutes({
  appendRefreshedSessionCookies,
  currentIsoTimestamp,
  isOpenAgentsAdminEmail,
  makeLedgerStore: env =>
    makeD1NexusTreasuryPayoutLedgerStore(openAgentsDatabase(env)),
  makePaymentAuthority: (env, context) => {
    const config = getOpenAgentsWorkerConfig(env)
    const ledgerStore = makeD1NexusTreasuryPayoutLedgerStore(
      openAgentsDatabase(env),
    )

    return makeTreasuryPaymentAuthority({
      adapters:
        context.adapterKind === 'hosted_mdk'
          ? [
              makeHostedMdkPayoutAdapter({
                accessToken: redactedValue(config.mdk.accessToken),
                providerRef: context.providerRef,
                resolveDestination: () =>
                  Effect.succeed(context.privatePayoutDestination ?? ''),
              }),
            ]
          : [],
      ledgerStore,
    })
  },
  makePylonApiStore: env => makeD1PylonApiStore(openAgentsDatabase(env)),
  requireAdminApiToken,
  requireBrowserSession,
})

const pylonApiRoutes = makePylonApiRoutes<WorkerBindings>({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  makeStore: env => makeD1PylonApiStore(openAgentsDatabase(env)),
  requireAdminApiToken,
})

const omniRoutes = makeOmniRoutes({
  handleAutopilotFleetApi: (request, env, ctx) =>
    routeEffect('handle_autopilot_fleet_api', () =>
      omniHandlers.handleAutopilotFleetApi(request, env, ctx),
    ),
  handleAutopilotTokenLeaderboardsApi: (request, env, ctx) =>
    routeEffect('handle_autopilot_token_leaderboards_api', () =>
      omniHandlers.handleAutopilotTokenLeaderboardsApi(request, env, ctx),
    ),
  handleBillingCheckoutApi: (request, env, ctx) =>
    routeEffect('handle_billing_checkout_api', () =>
      billingApiHandlers.handleBillingCheckoutApi(request, env, ctx),
    ),
  handleBillingCouponRedeemApi: (request, env, ctx) =>
    routeEffect('handle_billing_coupon_redeem_api', () =>
      billingApiHandlers.handleBillingCouponRedeemApi(request, env, ctx),
    ),
  handleBillingSummaryApi: (request, env, ctx) =>
    routeEffect('handle_billing_summary_api', () =>
      billingApiHandlers.handleBillingSummaryApi(request, env, ctx),
    ),
  handleBillingStripeCheckoutReturnApi: (request, environment) =>
    routeEffect('handle_billing_stripe_checkout_return_api', () =>
      billingApiHandlers.handleBillingStripeCheckoutReturnApi(
        request,
        environment,
      ),
    ),
  handleBillingStripeWebhookApi: (request, environment) =>
    routeEffect('handle_billing_stripe_webhook_api', () =>
      billingApiHandlers.handleBillingStripeWebhookApi(request, environment),
    ),
  handleEmailResendWebhookApi: (request, environment) =>
    routeEffect('handle_email_resend_webhook_api', () =>
      handleEmailResendWebhookApi(request, environment),
    ),
  handleOmniAgentRunDetailApi: (request, env, ctx, runId) =>
    routeEffect('handle_omni_agent_run_detail_api', () =>
      omniHandlers.handleOmniAgentRunDetailApi(request, env, ctx, runId),
    ),
  handleOmniAgentRunEventsApi: (request, env, ctx, runId) =>
    routeEffect('handle_omni_agent_run_events_api', () =>
      omniHandlers.handleOmniAgentRunEventsApi(request, env, ctx, runId),
    ),
  handleOmniAgentRunsApi: (request, env, ctx) =>
    routeEffect('handle_omni_agent_runs_api', () =>
      omniHandlers.handleOmniAgentRunsApi(request, env, ctx),
    ),
  handleOmniDeploymentDetailApi: (request, env, ctx, deployId) =>
    routeEffect('handle_omni_deployment_detail_api', () =>
      omniHandlers.handleOmniDeploymentDetailApi(request, env, ctx, deployId),
    ),
  handleOmniDeploymentEventsApi: (request, env, ctx, deployId) =>
    routeEffect('handle_omni_deployment_events_api', () =>
      omniHandlers.handleOmniDeploymentEventsApi(request, env, ctx, deployId),
    ),
  handleOmniDeploymentsApi: (request, env, ctx) =>
    routeEffect('handle_omni_deployments_api', () =>
      omniHandlers.handleOmniDeploymentsApi(request, env, ctx),
    ),
  handleOmniOperatorAgentRunDetailApi: (request, env, runId) =>
    routeEffect('handle_omni_operator_agent_run_detail_api', () =>
      omniHandlers.handleOmniOperatorAgentRunDetailApi(request, env, runId),
    ),
  handleOmniOperatorAgentRunsApi: (request, env) =>
    routeEffect('handle_omni_operator_agent_runs_api', () =>
      omniHandlers.handleOmniOperatorAgentRunsApi(request, env),
    ),
  handleOmniOperatorBillingCreditsApi: (request, env) =>
    routeEffect('handle_omni_operator_billing_credits_api', () =>
      operatorBillingHandlers.handleOmniOperatorBillingCreditsApi(request, env),
    ),
  handleOmniOperatorDeploymentsApi: (request, env) =>
    routeEffect('handle_omni_operator_deployments_api', () =>
      omniHandlers.handleOmniOperatorDeploymentsApi(request, env),
    ),
  handleOmniOperatorFleetApi: (request, env) =>
    routeEffect('handle_omni_operator_fleet_api', () =>
      omniHandlers.handleOmniOperatorFleetApi(request, env),
    ),
  handleOmniOperatorTeamChatMessagesApi: (request, env, ctx) =>
    routeEffect('handle_omni_operator_team_chat_messages_api', () =>
      omniHandlers.handleOmniOperatorTeamChatMessagesApi(request, env, ctx),
    ),
})

const teamChatRoutes = makeTeamChatRoutes({
  handleTeamChatMessagesApi: (request, env, ctx, teamId, projectId) =>
    routeEffect('handle_team_chat_messages_api', () =>
      handleTeamChatMessagesApi(request, env, ctx, teamId, projectId),
    ),
})

const forumRoutes = makeForumRoutes()

const imageGenerationRoutes = makeImageGenerationRoutes({
  appUrl: env => getOpenAgentsWorkerConfig(env).app.url,
  appendRefreshedSessionCookies,
  requireOperatorAccess: async (env, session) =>
    (await readActiveTeamMembershipRole(
      openAgentsDatabase(env),
      OPENAGENTS_CORE_TEAM_ID,
      session.user.userId,
    )) !== undefined,
  requireBrowserSession,
})

const siteRuntimeRoutes = makeSiteRuntimeRoutes({
  sitesHost: 'sites.openagents.com',
})

const recordPublicAgentFunnelRead = (
  request: Request,
  db: D1Database,
  ctx: ExecutionContext,
  eventKind: ViralAgentFunnelEventKind,
  route: string,
): void => {
  scheduleBackgroundWork(
    ctx,
    recordViralAgentFunnelEvent(db, request, {
      eventKind,
      proofRef: route === '/api/public/proof/otec' ? 'proof:otec' : null,
      route,
      siteSlug: route === '/api/public/proof/otec' ? 'otec' : null,
    }),
  )
}

const exactRoutes: ReadonlyArray<ExactRoute<Env>> = [
  {
    path: '/',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleHomePage(request, env, ctx)),
  },
  {
    path: '/api/public/home',
    handler: request => handlePublicHomeApi(request),
  },
  {
    path: '/api/public/product-promises',
    handler: request => handlePublicProductPromisesApi(request),
  },
  {
    path: '/chat',
    handler: () => Effect.succeed(notFound()),
  },
  {
    path: '/login',
    handler: () => Effect.succeed(redirectResponse('/')),
  },
  {
    path: '/login/github',
    handler: (request, env) =>
      Effect.promise(() => handleGitHubStart(request, env)),
  },
  {
    path: '/auth/github/write/start',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleGitHubWriteStart(request, env, ctx)),
  },
  {
    path: '/auth/callback',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleAuthCallback(request, env, ctx)),
  },
  {
    path: '/auth/logout',
    handler: request => Effect.succeed(handleLogout(request)),
  },
  {
    path: '/logout',
    handler: request => Effect.succeed(handleLogout(request)),
  },
  {
    path: '/api/auth/session',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleSessionApi(request, env, ctx)),
  },
  {
    path: '/api/auth/totals',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleAuthTotalsApi(request, env, ctx)),
  },
  {
    path: '/api/mdk',
    handler: (request, env) => routeMdkSidecarRequest(request, env),
  },
  {
    path: '/api/admin/overview',
    handler: (request, env, ctx) =>
      adminOverviewHandlers.handleAdminOverviewApi(request, env, ctx),
  },
  {
    path: '/api/stats/token-usage/events',
    handler: (request, env) =>
      tokenUsageLedgerRoutes.handleTokenUsageEventsApi(request, env),
  },
  {
    path: '/api/stats/token-usage/aggregate',
    handler: (request, env, ctx) =>
      tokenUsageLedgerRoutes.handleTokenUsageAggregateApi(request, env, ctx),
  },
  {
    path: '/api/stats/token-usage/leaderboards',
    handler: (request, env, ctx) =>
      tokenUsageLedgerRoutes.handleTokenUsageLeaderboardsApi(request, env, ctx),
  },
  {
    path: '/api/stats/token-usage/leaderboard-preference',
    handler: (request, env, ctx) =>
      tokenUsageLedgerRoutes.handleTokenUsageLeaderboardPreferenceApi(
        request,
        env,
        ctx,
      ),
  },
  {
    path: '/api/auth/teams',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleAuthTeamsApi(request, env, ctx)),
  },
  {
    path: '/api/public/pylon-stats',
    handler: (request, env) => handlePublicPylonStatsApi(request, env),
  },
  {
    path: '/api/public/launch-dashboard',
    handler: (request, env) => handlePublicLaunchDashboardApi(request, env),
  },
  {
    path: '/api/public/artanis/report',
    handler: (request, env) => handlePublicArtanisReportApi(request, env),
  },
  {
    path: '/api/blueprint/program-registry',
    handler: (request, env) =>
      blueprintRoutes.handleBlueprintProgramRegistryApi(request, env),
  },
  {
    path: '/api/blueprint/program-runs',
    handler: (request, env) =>
      blueprintRoutes.handleBlueprintProgramRunEvidenceApi(request, env),
  },
  {
    path: '/api/blueprint/action-submissions',
    handler: (request, env) =>
      blueprintRoutes.handleBlueprintActionSubmissionsApi(request, env),
  },
  {
    path: '/api/blueprint/contributions',
    handler: (request, env) =>
      blueprintProbeContributionRoutes.handleBlueprintProbeContributionsApi(
        request,
        env,
      ),
  },
  {
    path: '/api/blueprint/contracts',
    handler: (request, env) =>
      blueprintRoutes.handleBlueprintContractExportApi(request, env),
  },
  {
    path: '/.well-known/openagents.json',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'capability_manifest_read',
        '/.well-known/openagents.json',
      )

      return handleOpenAgentsCapabilityManifestApi(request)
    },
  },
  {
    path: '/AGENTS.md',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'agent_doc_read',
        '/AGENTS.md',
      )

      return handleOpenAgentsAgentOnboarding(request, env.ASSETS)
    },
  },
  {
    path: '/HEARTBEAT.md',
    handler: (request, env) =>
      handleOpenAgentsCompanionFile(request, env.ASSETS, '/HEARTBEAT.md'),
  },
  {
    path: '/RULES.md',
    handler: (request, env) =>
      handleOpenAgentsCompanionFile(request, env.ASSETS, '/RULES.md'),
  },
  {
    path: '/skill.json',
    handler: (request, env) =>
      handleOpenAgentsCompanionFile(request, env.ASSETS, '/skill.json'),
  },
  {
    path: '/api/openapi.json',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'openapi_read',
        '/api/openapi.json',
      )

      return handleOpenAgentsOpenApi(request)
    },
  },
  {
    path: '/api/omni/sdk-seed',
    handler: request => handleOmniApiSdkSeedApi(request),
  },
  {
    path: '/api/developer/signature-packages/validate',
    handler: request => handleSignaturePackageValidationApi(request),
  },
  {
    path: '/api/public/adjutant/activity',
    handler: (request, env) => handlePublicAdjutantActivityApi(request, env),
  },
  {
    path: '/api/public/proof/otec',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'public_proof_read',
        '/api/public/proof/otec',
      )

      return handlePublicOtecProofApi(request, env)
    },
  },
  {
    path: '/api/github-write/connections',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleGitHubWriteConnectionsApi(request, env, ctx)),
  },
  {
    path: '/api/github-write/grants/resolve',
    handler: (request, env) =>
      Effect.promise(() => handleGitHubWriteGrantResolveApi(request, env)),
  },
  {
    path: '/api/admin/sync/notify',
    handler: (request, env) =>
      Effect.promise(() => handleAdminSyncNotifyApi(request, env)),
  },
  {
    path: '/api/agents/register',
    handler: (request, env) =>
      Effect.promise(() => handleProgrammaticAgentRegistration(request, env)),
  },
  {
    path: '/api/agents/me',
    handler: (request, env) =>
      Effect.promise(() => handleProgrammaticAgentMe(request, env)),
  },
  {
    path: '/api/agents/home',
    handler: (request, env) =>
      Effect.promise(() =>
        handleProgrammaticAgentHome(request, openAgentsDatabase(env)),
      ),
  },
]

const routeRequest = makeWorkerRouteRequest({
  cleanProductRouteRedirectLocation,
  exactRoutes,
  handleAppShellPage: (request, env, ctx) =>
    routeEffect('handle_app_shell_page', () =>
      handleAppShellPage(request, env, ctx),
    ),
  handleAssetRequest: (request, env) =>
    routeEffect('handle_asset_request', () => env.ASSETS.fetch(request)),
  handleThreadPage: (request, env, ctx, threadId) =>
    routeEffect('handle_thread_page', () =>
      handleThreadPage(request, env, ctx, threadId),
    ),
  optionalUuid,
  routeAgentGoalRequest: agentGoalRoutes.routeAgentGoalRequest,
  routeAgentOwnerClaimRequest:
    agentOwnerClaimRoutes.routeAgentOwnerClaimRequest,
  routeAgentProposalRequest: agentProposalRoutes.routeAgentProposalRequest,
  routeAgentSearchRequest: agentSearchRoutes.routeAgentSearchRequest,
  routeAgentScopedGrantRequest:
    agentScopedGrantRoutes.routeAgentScopedGrantRequest,
  routeAgentSiteRequest: agentSiteRoutes.routeAgentSiteRequest,
  routeForumRequest: (request, env, ctx) =>
    forumRoutes.routeForumRequest(request, openAgentsDatabase(env), {
      agentStore: makeD1AgentRegistrationStore(openAgentsDatabase(env)),
      hostedMdkClient: hostedMdkClientForEnv(env),
      l402SigningBoundary: () => forumL402SigningBoundaryForEnv(env),
      resolveModeratorActor: async request => {
        const session = await requireBrowserSession(request, env, ctx)

        if (session === undefined) {
          return undefined
        }

        if (!isOpenAgentsAdminEmail(session.user.email)) {
          return {
            _tag: 'Forbidden' as const,
            reason: 'Forum moderation requires an OpenAgents admin session.',
          }
        }

        return {
          _tag: 'Moderator' as const,
          actor: {
            displayName: session.user.name,
            operatorId: session.user.userId,
            slug: session.user.login,
          },
        }
      },
    }),
  routeImageGenerationRequest:
    imageGenerationRoutes.routeImageGenerationRequest,
  routeMulletRequest: mulletRoutes.routeMulletRequest,
  routeOmniRequest: omniRoutes.routeOmniRequest,
  routeOnboardingRequest: onboardingRoutes.routeOnboardingRequest,
  routeNexusPylonVisibilityRequest:
    nexusPylonVisibilityRoutes.routeNexusPylonVisibilityRequest,
  routePylonApiRequest: pylonApiRoutes.routePylonApiRequest,
  routeSiteCommerceRequest: (request, _env, _ctx) =>
    siteCommerceRoutesForEnv(_env).routeSiteCommerceRequest(request),
  routeSiteReferralInspectionRequest:
    siteReferralInspectionRoutes.routeSiteReferralInspectionRequest,
  routeSiteReferralRequest: siteReferralRoutes.routeSiteReferralRequest,
  routeOperatorAdjutantRequest:
    operatorAdjutantRoutes.routeOperatorAdjutantRequest,
  routeOperatorArtanisConsoleRequest:
    operatorArtanisConsoleRoutes.routeOperatorArtanisConsoleRequest,
  routeOperatorEmailInspectionRequest:
    operatorEmailInspectionRoutes.routeOperatorEmailInspectionRequest,
  routeOperatorOrderTriageRequest:
    operatorOrderTriageRoutes.routeOperatorOrderTriageRequest,
  routeOperatorPylonMarketplaceRequest:
    operatorPylonMarketplaceRoutes.routeOperatorPylonMarketplaceRequest,
  routeOperatorProviderAccountRequest: (request, env) => {
    const response =
      operatorProviderAccountRoutes.routeOperatorProviderAccountRequest(
        request,
        env,
      )

    return response === undefined
      ? undefined
      : routeEffectOrResponse(
          routeEffect(
            'route_operator_provider_account_request',
            () => response,
          ),
        )
  },
  routeOperatorSitesRequest: operatorSitesRoutes.routeOperatorSitesRequest,
  routeProviderAccountRequest:
    providerAccountRoutes.routeProviderAccountRequest,
  routeShareRequest: shareRoutes.routeShareRequest,
  routeSyncRequest: syncRoutes.routeSyncRequest,
  routeTeamChatRequest: teamChatRoutes.routeTeamChatRequest,
  routeThreadFileRequest: threadFileRoutes.routeThreadFileRequest,
})

type SyncSocketAttachment = Readonly<{
  cursor: number
  scope: string
}>

const syncSocketClose = (
  socket: WebSocket,
  code: number,
  reason: string,
): void => {
  try {
    socket.close(code, reason)
  } catch {
    // The browser may already have closed the WebSocket while replay is pending.
  }
}

const syncSocketSendJson = (socket: WebSocket, value: unknown): boolean => {
  try {
    socket.send(JSON.stringify(value))
    return true
  } catch {
    syncSocketClose(socket, 1011, 'sync replay failed')
    return false
  }
}

const syncSocketAttachment = (
  value: unknown,
): SyncSocketAttachment | undefined => {
  if (!isRecord(value)) {
    return undefined
  }

  return typeof value.scope === 'string' && typeof value.cursor === 'number'
    ? { cursor: value.cursor, scope: value.scope }
    : undefined
}

const syncCursorFromUrl = (url: URL): number => {
  const cursor = Number(url.searchParams.get('cursor') ?? '0')

  return Number.isInteger(cursor) && cursor >= 0 ? cursor : 0
}

const syncScopeFromRequest = (request: Request, url: URL): string | undefined =>
  request.headers.get('x-openagents-sync-scope') ??
  url.searchParams.get('scope') ??
  undefined

export class SyncRoomDurableObject {
  constructor(
    private readonly state: DurableObjectState,
    private readonly env: Env,
  ) {}

  private async replaySocket(
    socket: WebSocket,
    scope: string,
    cursor: number,
  ): Promise<void> {
    try {
      const repository = makeD1SyncOutboxRepository(
        openAgentsDatabase(this.env),
      )
      const snapshot = await repository.readSnapshot(scope)

      if (cursor > snapshot.cursor) {
        if (
          syncSocketSendJson(socket, cursorGap(scope, snapshot.cursor, cursor))
        ) {
          socket.serializeAttachment({ cursor: snapshot.cursor, scope })
        }
        return
      }

      const patches = await repository.readChangesAfter(scope, cursor)
      let nextCursor = cursor

      for (const patch of patches) {
        if (!syncSocketSendJson(socket, patch)) {
          return
        }

        nextCursor = patch.seq
      }

      socket.serializeAttachment({ cursor: nextCursor, scope })
    } catch (error) {
      logWorkerRouteError('sync_replay_failed', error, {
        errorName: errorName(error),
        scope,
      })
      syncSocketClose(socket, 1011, 'sync replay failed')
    }
  }

  private async broadcastScope(scope: string): Promise<void> {
    await Promise.all(
      this.state.getWebSockets(scope).map(async socket => {
        const attachment = syncSocketAttachment(socket.deserializeAttachment())

        await this.replaySocket(socket, scope, attachment?.cursor ?? 0)
      }),
    )
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)
    const scope = syncScopeFromRequest(request, url)

    if (url.pathname === '/__sync/notify' && request.method === 'POST') {
      if (scope === undefined) {
        return badRequest('scope is required')
      }

      this.state.waitUntil(this.broadcastScope(scope))

      return jsonResponse({ ok: true, scope })
    }

    if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') {
      return badRequest('websocket upgrade is required')
    }

    if (scope === undefined) {
      return badRequest('scope is required')
    }

    const pair = new WebSocketPair()
    const client = pair[0]
    const server = pair[1]
    const cursor = syncCursorFromUrl(url)

    server.serializeAttachment({ cursor, scope })
    this.state.acceptWebSocket(server, [scope])
    this.state.waitUntil(this.replaySocket(server, scope, cursor))

    return new Response(null, {
      status: 101,
      webSocket: client,
    })
  }

  async webSocketMessage(socket: WebSocket, message: string): Promise<void> {
    if (message !== 'replay') {
      return
    }

    const attachment = syncSocketAttachment(socket.deserializeAttachment())

    if (attachment === undefined) {
      syncSocketClose(socket, 1008, 'missing sync attachment')
      return
    }

    await this.replaySocket(socket, attachment.scope, attachment.cursor)
  }

  webSocketClose(
    socket: WebSocket,
    _code: number,
    _reason: string,
    _wasClean: boolean,
  ): void {
    socket.serializeAttachment(null)
  }

  webSocketError(socket: WebSocket): void {
    syncSocketClose(socket, 1011, 'sync socket error')
  }
}

const workerFetchProgram = Effect.gen(function* () {
  const { ctx, env, request, url } = yield* OpenAgentsWorkerRequest

  return yield* Effect.gen(function* () {
    const siteRuntimeResponse = siteRuntimeRoutes.routeSiteRuntimeRequest(
      request,
      env,
    )

    if (siteRuntimeResponse !== undefined) {
      return yield* siteRuntimeResponse
    }

    if (url.hostname === 'auth.openagents.com') {
      return yield* Effect.promise(() =>
        routeAuthHostRequest(request, env, ctx),
      )
    }

    return yield* routeRequest()
  }).pipe(
    Effect.catchCause(cause =>
      Effect.sync(() => {
        logWorkerRouteError('worker_unhandled_exception', Cause.pretty(cause))

        if (url.hostname === 'auth.openagents.com') {
          return redirectResponse(getAppOrigin(env))
        }

        return serverError()
      }),
    ),
  )
})

const runWorkerFetch = (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> =>
  Effect.runPromise(
    workerFetchProgram.pipe(
      Effect.provide(WorkerRequestLayer({ ctx, env, request })),
    ),
  )

export default {
  fetch: runWorkerFetch,
  queue: async (batch, env, ctx): Promise<void> => {
    for (const message of batch.messages) {
      const decoded = S.decodeUnknownSync(AdjutantEnrichmentQueueMessage)(
        message.body,
      )

      const exit = await Effect.runPromiseExit(
        executeQueuedAdjutantEnrichmentJob(env, decoded),
      )

      if (Exit.isFailure(exit)) {
        throw exit.cause
      }

      message.ack()
    }
    void ctx
  },
  scheduled: async (event, env, ctx): Promise<void> => {
    const config = getOpenAgentsWorkerConfig(env)

    await Promise.all([
      sweepActiveAgentRunBilling(env, ctx),
      sendPendingReviewReadyArtifactNotifications(env),
      sendPendingReviewReadySiteNotifications(env),
      observedEffect(
        'ArtanisScheduledRunner.runTick',
        runArtanisScheduledTickScheduled(
          openAgentsDatabase(env),
          config.artanis.scheduledRunnerEnabled,
          event.scheduledTime,
        ),
      ),
      observedEffect(
        'EmailCampaignDispatcher.dispatchDue',
        dispatchDueEmailCampaignSendsScheduled(env),
      ),
    ])
  },
} satisfies ExportedHandler<Env>

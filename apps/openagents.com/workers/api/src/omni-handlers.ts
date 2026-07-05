import { Effect, Layer } from 'effect'

import { applyAdjutantRunLifecycleEvents } from './adjutant-run-lifecycle'
import {
  AgentGoalAccountingService,
  AgentGoalAccountingServiceLive,
  AgentGoalCapacityPolicyServiceLive,
  AgentGoalContinuationQueue,
  AgentGoalContinuationService,
  AgentGoalContinuationServiceLive,
  AgentGoalRuntimeEvent,
} from './agent-goal-runtime'
import {
  type AgentGoalRecord,
  AgentGoalRepository,
  AgentGoalStorageError,
  type AgentGoalVisibility,
} from './agent-goals'
import {
  makeAgentGoalEventRepositoryLayerForEnv,
  makeAgentGoalRepositoryLayerForEnv,
  makeOmniRunStoreForEnv,
} from './agent-runtime-store'
import { appendSessionCookies } from './auth-cookies'
import { requireMinimumRunCredits } from './billing'
import type {
  OpenAgentsWorkerConfigEnv,
  ResendEmailConfig,
  RunnerBackendConfig,
} from './config'
import {
  hasRequiredGitHubWriteScopes,
  issueGitHubWriteGrant,
  makeD1GitHubWriteRepository,
} from './github-write-connections'
import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
} from './http/responses'
import { routeAccessResponse } from './http/route-access-response'
import type { Env } from './index'
import {
  isRecord,
  nestedUnknown,
  optionalBoolean,
  optionalInteger,
  optionalString,
  readJsonObject,
  readRequestSelector,
  safeJsonRecord,
} from './json-boundary'
import { projectAgentRun } from './khala-sync-agent-run-projection'
import {
  logWorkerRouteWarning,
  observedEffect,
  observedPromise,
} from './observability'
import {
  goalRuntimeEventFromRunEvent,
  goalRuntimeEventFromRunStatus,
} from './omni-goal-event-mapping'
import {
  type AgentRunBundle,
  type AgentRunRecord,
  type DeploymentRecord,
  type OmniEventRecord,
  type OmniRunStore,
  agentRunSyncProjectionRaw,
  checkShcControlHealth,
  continueAgentRunOnShc,
  createAgentRunId,
  createGitHubWorkOrder,
  createQueuedAgentRun,
  createQueuedDeployment,
  deploymentStatusFromText,
  dispatchEventForAgentRun,
  dispatchEventForDeployment,
  fetchAgentRunEventsFromShc,
  firstText,
  legacyAgentRunIdFromUuid,
  makeD1OmniRunStore,
  numberOrUndefined,
  parseGithubRepository,
  publicAgentRunBundle,
  publicDeploymentBundle,
  readAgentRunById,
  runStatusFromText,
} from './omni-runs'
import { makeOmniDeploymentRepository } from './omni/deployment-repository'
import { makeOmniDispatchService } from './omni/dispatch-service'
import {
  type OmniError,
  isOmniError,
  omniErrorFromUnknown,
} from './omni/errors'
import { makeOmniRunRepository } from './omni/run-repository'
import { makeOmniRunnerEventService } from './omni/runner-events'
import type { OperatorTargetUser } from './operator-targets'
import {
  issueProviderAccountGrant,
  listProviderAccountsForUser,
  makeD1ProviderAccountRepository,
  recordProviderAccountHealth,
} from './provider-accounts'
import {
  connectedProviderAccountRef,
  providerAccountLaunchBlockMessage,
} from './provider-launch'
import {
  runnerBackendReadinessCheck,
  runnerWorkloadTrustFromSelector,
} from './runner-backend-readiness'
import { openAgentsDatabase, scheduleBackgroundWork } from './runtime'
import { businessDomainDatabaseForEnv } from './business-domain-store'
import { sitesContentDatabaseForEnv } from './sites-content-store'
import { makeSupervisionLongtailMirrorForEnv } from './supervision-longtail-domain-store'
import {
  notifyAgentRunSyncScopes,
  publishAgentGoalEventSync,
  publishAgentGoalSync,
} from './sync-notifier'
import { type TeamChatRunSummary, listTeamChatMessages } from './team-chat'
import {
  type UserTeamProject,
  readActiveTeamMembershipRole,
  readActiveTeamProject,
} from './team-repository'
import { type RouteAccessError } from './thread-access'
import { type AutopilotTokenLeaderboards } from './token-usage'

type BrowserSession = Readonly<{
  user: Readonly<{
    avatarUrl: string
    email: string
    githubId?: string
    login?: string
    name: string
    provider: 'github' | 'email'
    userId: string
  }>
  tokens?: Parameters<typeof appendSessionCookies>[1]
}>

type AuthenticatedActor =
  | Readonly<{
      kind: 'human'
      tokens?: Parameters<typeof appendSessionCookies>[1]
      user: Readonly<{
        avatarUrl: string
        email: string
        githubId?: string
        login?: string
        name: string
        provider: 'github' | 'email'
        userId: string
      }>
    }>
  | Readonly<{
      agent: Readonly<{
        credential: Readonly<{
          id: string
          lastUsedAt: string
          profileMetadataJson: string
          tokenPrefix: string
        }>
        user: Readonly<{
          avatarUrl: string | null
          createdAt: string
          displayName: string
          id: string
          kind: 'agent'
          primaryEmail: string | null
          status: 'active'
          updatedAt: string
        }>
      }>
      kind: 'agent'
    }>

type BillingAwareOmniRunStore = ReturnType<typeof makeD1OmniRunStore>

type ShcDispatchConfig = Readonly<{
  controlApiBearerToken?: string | undefined
  controlApiUrl?: string | undefined
  dispatchMode?: string | undefined
}>

type OmniEffectResult<A> =
  | Readonly<{ _tag: 'Failed'; error: OmniError }>
  | Readonly<{ _tag: 'Succeeded'; value: A }>

const failedOmniEffectResult = (error: OmniError): OmniEffectResult<never> => ({
  _tag: 'Failed',
  error,
})

const succeededOmniEffectResult = <A>(value: A): OmniEffectResult<A> => ({
  _tag: 'Succeeded',
  value,
})

type OmniHandlerDependencies = Readonly<{
  appendRefreshedSessionCookies: (
    response: Response,
    session: BrowserSession,
  ) => Response
  appendTeamAutopilotAnswerBack: (
    env: Env,
    ctx: ExecutionContext,
    runId: string,
  ) => Promise<void>
  authenticateRequestActor: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => Promise<AuthenticatedActor | undefined>
  actorJson: (actor: AuthenticatedActor) => unknown
  getAppOrigin: (env: Env) => string
  getResendEmailConfig: (env: Env) => ResendEmailConfig | undefined
  getRunnerBackendConfig: (env: OpenAgentsWorkerConfigEnv) => RunnerBackendConfig
  isOpenAgentsAdminEmail: (email: string) => boolean
  isRouteAccessError: (
    value: AgentRunBundle | RouteAccessError,
  ) => value is RouteAccessError
  makeBillingAwareOmniRunStore: (
    env: Env,
    ctx?: ExecutionContext,
  ) => BillingAwareOmniRunStore
  postTeamChatMessageForUser: (
    env: Env,
    ctx: ExecutionContext,
    input: {
      body: Record<string, unknown>
      project?: UserTeamProject
      roomThreadId: string
      teamId: string
      userId: string
    },
  ) => Promise<{ payload: Record<string, unknown>; status: number }>
  readSelectedOperatorTargetUser: (
    db: D1Database,
    selector: Record<string, unknown>,
  ) => Promise<OperatorTargetUser | undefined>
  readTokenUsageLeaderboardsForUser: (
    env: Env,
    userId: string,
  ) => Promise<AutopilotTokenLeaderboards>
  requireAdminApiToken: (request: Request, env: Env) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => Promise<BrowserSession | undefined>
  requireRunnerCallbackAuth: (request: Request, env: Env) => Promise<boolean>
  threadRouteAccessBundle: (
    env: Env,
    userId: string,
    runIdOrLegacyThreadId: string,
  ) => Promise<AgentRunBundle | RouteAccessError>
  shcDispatchConfig: (env: Env) => ShcDispatchConfig
}>

type OmniHandlerEnv = Parameters<
  OmniHandlerDependencies['shcDispatchConfig']
>[0]

const optionalUuid = (value: unknown): string | undefined =>
  typeof value === 'string' &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(
    value,
  )
    ? value
    : undefined

const teamChatThreadId = (teamId: string): string => `team:${teamId}:chat`

const teamProjectChatThreadId = (teamId: string, projectId: string): string =>
  `team:${teamId}:project:${projectId}:chat`

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const publicErrorName = (error: unknown): string =>
  isOmniError(error)
    ? error._tag
    : error instanceof Error && error.name.trim() !== ''
      ? error.name
      : 'unknown_error'

const callbackEventDescriptor = (
  event: Record<string, unknown>,
  fallbackSequence: number,
) => ({
  sequence:
    typeof event.sequence === 'number' && Number.isFinite(event.sequence)
      ? event.sequence
      : fallbackSequence,
  type: optionalString(event.type) ?? 'unknown',
})

const runnerCallbackIngestFailureResponse = (
  error: unknown,
  events: ReadonlyArray<Record<string, unknown>>,
  fallbackStart: number,
): Response =>
  noStoreJsonResponse(
    {
      error: isOmniError(error) ? error._tag : 'runner_callback_ingest_failed',
      message: errorMessage(error),
      rejectedEvents: events.map((event, index) => ({
        ...callbackEventDescriptor(event, fallbackStart + index),
        reason: isOmniError(error) ? error._tag : 'runner_callback_ingest_failed',
      })),
    },
    {
      status:
        isOmniError(error) && error._tag === 'OmniRunnerCallbackDecodeError'
          ? 400
          : 500,
    },
  )

const runStatusFromIngestedEvents = (
  explicitStatus: AgentRunRecord['status'] | undefined,
  records: ReadonlyArray<OmniEventRecord>,
): AgentRunRecord['status'] | undefined => {
  if (explicitStatus !== undefined) {
    return explicitStatus
  }

  for (const event of [...records].reverse()) {
    const eventStatus = runStatusFromText(event.status ?? undefined)

    if (eventStatus !== undefined) {
      return eventStatus
    }

    if (
      event.type === 'cloud.run.completed' ||
      event.type === 'runner.completed'
    ) {
      return 'completed'
    }

    if (event.type === 'runner.failed') {
      return 'failed'
    }

    if (event.type === 'runner.canceled') {
      return 'canceled'
    }
  }

  return undefined
}

const notifyAgentRunSyncScopesEffect = (
  env: OmniHandlerEnv,
  runId: string,
): Effect.Effect<void, ReturnType<typeof omniErrorFromUnknown>> =>
  Effect.tryPromise({
    try: () => notifyAgentRunSyncScopes(env, runId),
    catch: error => omniErrorFromUnknown('notify_agent_run_sync_scopes', error),
  }).pipe(Effect.withSpan('OmniSync.notifyAgentRunSyncScopes'))

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

const optionalUserWritableTeamChatKind = (
  value: unknown,
): 'message' | 'autopilot_intent' | 'adjutant_intent' | undefined =>
  value === 'message' ||
  value === 'autopilot_intent' ||
  value === 'adjutant_intent'
    ? value
    : undefined

const optionalRunnerBackend = (
  value: unknown,
): 'shc_vm' | 'gcloud_vm' | undefined =>
  value === 'shc_vm' || value === 'gcloud_vm' ? value : undefined

const DEFAULT_AGENT_GOAL_AGENT_ID = 'openagents-autopilot'

const optionalGoalVisibility = (
  value: unknown,
): AgentGoalVisibility | undefined =>
  value === 'private' || value === 'team' || value === 'public'
    ? value
    : undefined

const positiveTokenBudget = (value: unknown): number | null | undefined => {
  if (value === null) {
    return null
  }

  const parsed = optionalInteger(value)

  return parsed === undefined || parsed <= 0 ? undefined : parsed
}

const projectAgentId = (project: UserTeamProject | undefined): string =>
  project?.agent?.id ?? DEFAULT_AGENT_GOAL_AGENT_ID

const agentGoalRuntimeLayer = (workerEnv: OmniHandlerEnv) => {
  const repositoryLayer = makeAgentGoalRepositoryLayerForEnv(workerEnv)
  const eventRepositoryLayer =
    makeAgentGoalEventRepositoryLayerForEnv(workerEnv)
  const runtimeDependencies = Layer.merge(repositoryLayer, eventRepositoryLayer)

  return Layer.mergeAll(
    runtimeDependencies,
    AgentGoalAccountingServiceLive().pipe(Layer.provide(runtimeDependencies)),
  )
}

const immediateSyncContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: promise => {
    void promise.catch(() => undefined)
  },
})

const resolveAgentRunGoal = (
  workerEnv: OmniHandlerEnv,
  input: Readonly<{
    objective: string
    project?: UserTeamProject | undefined
    projectId?: string | undefined
    selector: Record<string, unknown>
    teamId?: string | undefined
    userId: string
  }>,
): Promise<AgentGoalRecord> =>
  observedEffect(
    'OmniGoal.resolveAgentRunGoal',
    Effect.gen(function* () {
      const repository = yield* AgentGoalRepository
      const scope = {
        agentId:
          optionalString(input.selector.agentId) ??
          projectAgentId(input.project),
        projectId: input.projectId ?? null,
        teamId: input.teamId ?? null,
        userId: input.userId,
      }
      const requestedGoalId = optionalString(input.selector.goalId)

      if (requestedGoalId !== undefined) {
        return yield* repository.getById(requestedGoalId)
      }

      const current = yield* repository.getCurrent(scope)

      if (current !== undefined && current.status === 'active') {
        return current
      }

      return yield* repository.setGoal({
        ...scope,
        objective: input.objective,
        tokenBudget: positiveTokenBudget(input.selector.tokenBudget),
        visibility:
          optionalGoalVisibility(input.selector.goalVisibility) ?? 'private',
      })
    }).pipe(
      Effect.provide(
        makeAgentGoalRepositoryLayerForEnv(workerEnv),
      ),
    ),
  )

const applyGoalRuntimeEventBatch = (
  workerEnv: OmniHandlerEnv,
  events: ReadonlyArray<AgentGoalRuntimeEvent>,
  ctx: ExecutionContext = immediateSyncContext(),
): Promise<void> =>
  observedEffect(
    'OmniGoal.applyRuntimeEvents',
    Effect.gen(function* () {
      const accounting = yield* AgentGoalAccountingService
      const effects = events.map(event =>
        accounting.applyRuntimeEvent(event).pipe(
          Effect.tap(result =>
            Effect.tryPromise({
              try: async () => {
                await publishAgentGoalSync(
                  workerEnv,
                  ctx,
                  result.goal,
                  'runtime',
                )
                if (result.event !== null) {
                  await publishAgentGoalEventSync(
                    workerEnv,
                    ctx,
                    result.goal,
                    result.event,
                    'runtime',
                  )
                }
              },
              catch: error =>
                omniErrorFromUnknown('goal_sync_publication', error),
            }).pipe(Effect.catch(() => Effect.void)),
          ),
          Effect.catchTags({
            AgentGoalNotFound: () => Effect.void,
            AgentGoalStaleUpdate: () => Effect.void,
          }),
        ),
      )

      yield* effects.reduce(
        (previous, next) => previous.pipe(Effect.flatMap(() => next)),
        Effect.void,
      )
    }).pipe(Effect.provide(agentGoalRuntimeLayer(workerEnv))),
  )

const applyGoalRuntimeEvents = (
  workerEnv: OmniHandlerEnv,
  goalId: string | null,
  events: ReadonlyArray<OmniEventRecord>,
  ctx?: ExecutionContext,
): Promise<void> =>
  applyGoalRuntimeEventBatch(
    workerEnv,
    events
      .map(event => goalRuntimeEventFromRunEvent(goalId, event))
      .filter((event): event is AgentGoalRuntimeEvent => event !== undefined),
    ctx,
  )

/**
 * Project a queued/relaunched agent run into `scope.agent_run.<runId>`
 * (KS-6.6, #8416). Never throws — a projection failure only logs a
 * public-safe diagnostic.
 *
 * This is now the ONLY producer at these three creation-time call sites.
 * The legacy `notifySyncScopes(env, syncScopeForAgentRun(run))` poke was
 * deleted (2026-07-05, #8416 final pass): the web client's active-run
 * WebSocket was repointed to the khala-sync `/api/sync/connect` surface
 * (`apps/web/src/subscriptions.ts`, commit `6ff849527f`), proven correct via
 * extensive contract/adapter tests plus a real production deploy, so the
 * legacy dual-write was redundant. See docs/khala-sync/RUNBOOK.md's
 * "2026-07-05 legacy poke deleted" subsection for the full disposition.
 *
 * KS-6.6 event-feed follow-up (#8416): `agent-runtime-store.ts`'s
 * `makeOmniRunStoreForEnv` ALSO fires this SAME run/goal projection (plus a
 * `agent_run_event` companion projection) unconditionally from
 * `store.saveAgentRun`/`store.appendAgentRunEvents` whenever a
 * `KHALA_SYNC_DB` binding exists — including on every ONGOING event append,
 * not just at creation. The three explicit calls below are therefore a
 * harmless duplicate of that universal wiring at creation time (both upsert
 * the same post-image); left in place to avoid touching these
 * already-tested call sites. See docs/khala-sync/RUNBOOK.md's
 * "2026-07-05 producer-completeness follow-up".
 */
const projectAgentRunSyncScope = (
  workerEnv: OmniHandlerEnv,
  run: AgentRunRecord,
): Promise<void> =>
  projectAgentRun(
    {
      binding: workerEnv.KHALA_SYNC_DB,
      log: (event, fields) => logWorkerRouteWarning(event, fields),
    },
    run.id,
    agentRunSyncProjectionRaw(run),
  ).then(() => undefined)

const applyGoalRuntimeStatus = (
  workerEnv: OmniHandlerEnv,
  event: AgentGoalRuntimeEvent | undefined,
  ctx?: ExecutionContext,
): Promise<void> => {
  if (event === undefined) {
    return Promise.resolve()
  }

  return applyGoalRuntimeEventBatch(workerEnv, [event], ctx)
}

export const makeOmniHandlers = (dependencies: OmniHandlerDependencies) => {
  const defaultAgentGoal =
    'Run a bounded repo cleanup mission. Preserve diffs, tests, logs, receipts, blockers, and next recommended order.'

  const githubWorkOrderFromSelector = (
    selector: Record<string, unknown>,
    repository: ReturnType<typeof parseGithubRepository>,
    runId: string,
  ) =>
    createGitHubWorkOrder({
      baseRef: firstText(selector.baseRef, selector.repositoryRef),
      branchName: firstText(selector.branchName, selector.branch),
      commitMessage: firstText(selector.commitMessage),
      issueComment: firstText(selector.issueComment),
      issueNumber: optionalInteger(selector.issueNumber ?? selector.issue),
      issueUrl: firstText(selector.issueUrl),
      openPullRequest: optionalBoolean(selector.openPullRequest),
      pullRequestBody: firstText(selector.pullRequestBody),
      pullRequestTitle: firstText(selector.pullRequestTitle),
      repository,
      runId,
    })

  const publicOmniOverview = (
    agentRuns: Awaited<
      ReturnType<ReturnType<typeof makeD1OmniRunStore>['listAgentRunsForUser']>
    >,
    deployments: Awaited<
      ReturnType<
        ReturnType<typeof makeD1OmniRunStore>['listDeploymentsForUser']
      >
    >,
  ) => ({
    agentRuns: agentRuns.map(publicAgentRunBundle),
    deployments: deployments.map(publicDeploymentBundle),
  })

  type AutopilotOperatorCheckStatus = 'ok' | 'warning' | 'blocked' | 'unknown'

  type AutopilotOperatorCheck = Readonly<{
    details?: Record<string, unknown> | undefined
    message: string
    name: string
    status: AutopilotOperatorCheckStatus
  }>

  const operatorCheck = (
    name: string,
    status: AutopilotOperatorCheckStatus,
    message: string,
    details?: Record<string, unknown>,
  ): AutopilotOperatorCheck => ({
    ...(details === undefined ? {} : { details }),
    message,
    name,
    status,
  })

  const operatorCheckRollup = (
    checks: ReadonlyArray<AutopilotOperatorCheck>,
  ): AutopilotOperatorCheckStatus =>
    checks.some(check => check.status === 'blocked')
      ? 'blocked'
      : checks.some(check => check.status === 'warning')
        ? 'warning'
        : checks.some(check => check.status === 'unknown')
          ? 'unknown'
          : 'ok'

  const readD1TablePresence = async (
    db: D1Database,
    tableName: string,
  ): Promise<boolean> => {
    const row = await db
      .prepare(
        `SELECT name
         FROM sqlite_master
         WHERE type = 'table'
           AND name = ?
         LIMIT 1`,
      )
      .bind(tableName)
      .first<Readonly<{ name: string }>>()

    return row !== null && row !== undefined
  }

  const migrationPreflightCheck = async (
    db: D1Database,
  ): Promise<AutopilotOperatorCheck> => {
    const requiredTables = [
      'agent_runs',
      'agent_run_events',
      'agent_goals',
      'agent_goal_events',
      'provider_accounts',
      'github_write_connections',
      'team_projects',
    ] as const
    const missingTables = (
      await Promise.all(
        requiredTables.map(async table => ({
          exists: await readD1TablePresence(db, table),
          table,
        })),
      )
    )
      .filter(result => !result.exists)
      .map(result => result.table)

    return missingTables.length === 0
      ? operatorCheck(
          'database_migrations',
          'ok',
          'Required Autopilot tables are present.',
          { requiredTables: [...requiredTables] },
        )
      : operatorCheck(
          'database_migrations',
          'blocked',
          'Production D1 is missing required Autopilot tables.',
          { missingTables },
        )
  }

  const projectPreflightCheck = async (
    db: D1Database,
    selector: Record<string, unknown>,
    targetUser: OperatorTargetUser,
  ): Promise<AutopilotOperatorCheck> => {
    const teamId = optionalString(selector.teamId)
    const projectId = optionalString(selector.projectId)

    if (teamId === undefined && projectId === undefined) {
      return operatorCheck(
        'team_project_agent',
        'warning',
        'No team/project scope was selected; run will launch in the target user workspace.',
      )
    }

    if (teamId === undefined || projectId === undefined) {
      return operatorCheck(
        'team_project_agent',
        'blocked',
        'teamId and projectId must be provided together.',
      )
    }

    const role = await readActiveTeamMembershipRole(
      db,
      teamId,
      targetUser.userId,
    )
    const project = await readActiveTeamProject(db, teamId, projectId)

    if (role === undefined) {
      return operatorCheck(
        'team_project_agent',
        'blocked',
        'Target user is not an active member of the selected team.',
        { projectId, teamId },
      )
    }

    if (project === undefined) {
      return operatorCheck(
        'team_project_agent',
        'blocked',
        'Selected team project was not found or is archived.',
        { projectId, teamId },
      )
    }

    return project.agent === undefined
      ? operatorCheck(
          'team_project_agent',
          'warning',
          'Selected project is active but has no agent metadata.',
          { projectId, role, teamId },
        )
      : operatorCheck(
          'team_project_agent',
          'ok',
          'Selected team project and agent metadata are ready.',
          {
            agentId: project.agent.id,
            agentName: project.agent.name,
            projectId,
            role,
            teamId,
          },
        )
  }

  const providerPreflightCheck = async (
    db: D1Database,
    selector: Record<string, unknown>,
    targetUser: OperatorTargetUser,
  ): Promise<AutopilotOperatorCheck> => {
    const requested =
      optionalString(selector.providerAccountRef) ??
      optionalString(selector.providerAccountId)
    const providerBundle = await listProviderAccountsForUser(
      makeD1ProviderAccountRepository(db),
      targetUser.userId,
    )
    const providerAccountRef = connectedProviderAccountRef(
      providerBundle,
      requested,
    )

    if (providerAccountRef !== undefined) {
      return operatorCheck(
        'provider_account',
        'ok',
        'A connected healthy ChatGPT/Codex account is available.',
        {
          accounts: providerBundle.accounts.length,
          providerAccountRef,
        },
      )
    }

    return operatorCheck(
      'provider_account',
      'blocked',
      await providerAccountLaunchBlockMessage(
        db,
        targetUser.userId,
        providerBundle,
        requested,
      ),
      {
        accounts: providerBundle.accounts.length,
        error: 'provider_reconnect_required',
        requested: requested ?? null,
        requiresReconnect: true,
      },
    )
  }

  const githubWritePreflightCheck = async (
    db: D1Database,
    targetUser: OperatorTargetUser,
  ): Promise<AutopilotOperatorCheck> => {
    const githubWriteConnection = await makeD1GitHubWriteRepository(
      db,
    ).findUsableConnectionForUser(targetUser.userId)

    if (
      githubWriteConnection !== undefined &&
      hasRequiredGitHubWriteScopes(githubWriteConnection.scopes)
    ) {
      return operatorCheck(
        'github_write',
        'ok',
        'GitHub writeback is connected with repo/workflow scopes.',
        {
          connectionRef: githubWriteConnection.connectionRef,
          githubLogin: githubWriteConnection.githubLogin,
          scopes: githubWriteConnection.scopes,
        },
      )
    }

    return operatorCheck(
      'github_write',
      'blocked',
      'Connect repo push access before launching Autopilot on the computer.',
      {
        error: 'github_write_connection_required',
        requiredScopes: ['repo', 'workflow'],
      },
    )
  }

  const shcPreflightCheck = async (
    env: OmniHandlerEnv,
  ): Promise<AutopilotOperatorCheck> => {
    const dispatchConfig = dependencies.shcDispatchConfig(env)

    try {
      const health = await checkShcControlHealth(dispatchConfig)

      return health.ok
        ? operatorCheck('shc_control', 'ok', 'SHC control API is reachable.', {
            status: health.status,
          })
        : operatorCheck(
            'shc_control',
            health.status === 'not_configured' ? 'blocked' : 'warning',
            health.error,
            {
              status: health.status,
              targetPath: health.targetPath ?? null,
            },
          )
    } catch (error) {
      return operatorCheck('shc_control', 'blocked', errorMessage(error))
    }
  }

  const callbackPreflightCheck = (
    env: OmniHandlerEnv,
  ): AutopilotOperatorCheck => {
    const dispatchConfig = dependencies.shcDispatchConfig(env)
    const appOrigin = dependencies.getAppOrigin(env).replace(/\/+$/, '')

    return dispatchConfig.dispatchMode === 'live' &&
      dispatchConfig.controlApiUrl !== undefined &&
      dispatchConfig.controlApiBearerToken !== undefined
      ? operatorCheck(
          'runner_callback',
          'ok',
          'Runner callback URL and token reference are available for assignments.',
          {
            ingestUrl: `${appOrigin}/api/omni/agent-runs/:runId/events/ingest`,
            tokenRef: 'runner_callback_token',
          },
        )
      : operatorCheck(
          'runner_callback',
          'blocked',
          'Live SHC dispatch and callback auth must be configured before launching Autopilot.',
          {
            dispatchMode: dispatchConfig.dispatchMode ?? null,
          },
        )
  }

  const runnerBackendPreflightCheck = (
    env: OmniHandlerEnv,
    selector: Record<string, unknown>,
    shcControl: AutopilotOperatorCheck,
    runnerCallback: AutopilotOperatorCheck,
  ): AutopilotOperatorCheck => {
    const check = runnerBackendReadinessCheck({
      callbackStatus: runnerCallback.status,
      config: dependencies.getRunnerBackendConfig(env),
      shcControlStatus: shcControl.status,
      workloadTrust: runnerWorkloadTrustFromSelector(selector),
    })

    return operatorCheck(check.name, check.status, check.message, check.details)
  }

  const latestRunForTargetUser = async (
    store: OmniRunStore,
    targetUser: OperatorTargetUser,
  ): Promise<AgentRunBundle | undefined> => {
    const runs = await store.listAgentRunsForUser(targetUser.userId, 1)

    return runs[0]
  }

  const callbackLagCheck = async (
    env: OmniHandlerEnv,
    run: AgentRunRecord | undefined,
  ): Promise<AutopilotOperatorCheck> => {
    if (run === undefined) {
      return operatorCheck(
        'callback_lag',
        'unknown',
        'No current run was selected for callback lag inspection.',
      )
    }

    try {
      const fetched = await fetchAgentRunEventsFromShc(run, {
        ...dependencies.shcDispatchConfig(env),
        cursor: run.eventCursor,
      })

      return fetched.ok
        ? operatorCheck(
            'callback_lag',
            fetched.events.length === 0 ? 'ok' : 'warning',
            fetched.events.length === 0
              ? 'Cloudflare has ingested all currently visible SHC events.'
              : 'SHC has events that Cloudflare has not ingested yet.',
            {
              pendingEvents: fetched.events.length,
              runEventCursor: run.eventCursor,
              shcNextCursor: fetched.nextCursor,
            },
          )
        : operatorCheck('callback_lag', 'warning', fetched.error, {
            status: fetched.status,
            targetPath: fetched.targetPath ?? null,
          })
    } catch (error) {
      return operatorCheck('callback_lag', 'warning', errorMessage(error))
    }
  }

  const operatorNextSafeAction = (
    checks: ReadonlyArray<AutopilotOperatorCheck>,
    run: AgentRunRecord | undefined,
  ): string => {
    const blocked = checks.find(check => check.status === 'blocked')

    if (blocked !== undefined) {
      return `Resolve ${blocked.name}: ${blocked.message}`
    }

    const callbackLag = checks.find(check => check.name === 'callback_lag')
    const pendingEvents = callbackLag?.details?.pendingEvents

    if (typeof pendingEvents === 'number' && pendingEvents > 0) {
      return 'Retry callbacks for the current run before launching or continuing work.'
    }

    if (
      run !== undefined &&
      (run.status === 'running' || run.status === 'waiting_for_input')
    ) {
      return 'Queue a continuation turn against the current run if it needs new instructions.'
    }

    if (run !== undefined && run.goalId !== null) {
      return 'Continue the durable goal or launch the next run attached to that goal.'
    }

    return 'Create or launch the next Autopilot run.'
  }

  const findOperatorRunBundle = async (
    store: OmniRunStore,
    targetUser: OperatorTargetUser,
    runId: string,
  ): Promise<AgentRunBundle | undefined> =>
    (await store.findAgentRunForUser(targetUser.userId, runId)) ??
    (await (async () => {
      const legacyRunId = legacyAgentRunIdFromUuid(runId)

      return legacyRunId === undefined
        ? undefined
        : store.findAgentRunForUser(targetUser.userId, legacyRunId)
    })())

  const shcActionEventPayloads = (
    payload: unknown,
  ): ReadonlyArray<Record<string, unknown>> => {
    const record = isRecord(payload) ? payload : undefined
    const event = record?.event

    return isRecord(event) ? [event] : []
  }

  const ingestOperatorFetchedRunEvents = async (
    env: OmniHandlerEnv,
    run: AgentRunRecord,
    payloads: ReadonlyArray<Record<string, unknown>>,
    statusText: string | undefined,
  ) => {
    if (payloads.length === 0) {
      return {
        accepted: 0,
        status: run.status,
      }
    }

    const store = dependencies.makeBillingAwareOmniRunStore(env)
    const runnerEvents = makeOmniRunnerEventService()
    const repository = makeOmniRunRepository(store)
    const fallbackStart =
      (await readAgentRunEventCursor(openAgentsDatabase(env), run.id)) + 1
    const nextStatus = runStatusFromText(statusText)
    const records = await observedEffect(
      'OmniOperator.ingestFetchedRunEvents',
      Effect.gen(function* () {
        const decodedRecords = yield* runnerEvents.eventsFromCallbackPayloads(
          run.id,
          fallbackStart,
          payloads,
        )

        yield* repository.appendAgentRunEvents(
          run.id,
          decodedRecords,
          nextStatus,
        )
        yield* Effect.tryPromise({
          try: () => applyGoalRuntimeEvents(env, run.goalId, decodedRecords),
          catch: error =>
            omniErrorFromUnknown(
              'goal_operator_callback_retry_accounting',
              error,
            ),
        }).pipe(Effect.catch(() => Effect.void))
        yield* notifyAgentRunSyncScopesEffect(env, run.id)

        return decodedRecords
      }).pipe(Effect.withSpan('OmniOperator.ingestFetchedRunEvents')),
    )

    return {
      accepted: records.length,
      status: nextStatus ?? run.status,
    }
  }

  const autopilotPreflightPayload = async (
    request: Request,
    env: OmniHandlerEnv,
    selector: Record<string, unknown>,
    targetUser: OperatorTargetUser,
  ) => {
    const db = openAgentsDatabase(env)
    const store = makeOmniRunStoreForEnv(env)
    const selectedRunId =
      optionalString(selector.runId) ?? optionalString(selector.currentRunId)
    const selectedRun =
      selectedRunId === undefined
        ? await latestRunForTargetUser(store, targetUser)
        : await findOperatorRunBundle(store, targetUser, selectedRunId)
    const selectedRunRecord = selectedRun?.run
    const shcControl = await shcPreflightCheck(env)
    const runnerCallback = callbackPreflightCheck(env)
    const checks = [
      await migrationPreflightCheck(db),
      await projectPreflightCheck(db, selector, targetUser),
      await providerPreflightCheck(db, selector, targetUser),
      await githubWritePreflightCheck(db, targetUser),
      shcControl,
      runnerCallback,
      runnerBackendPreflightCheck(env, selector, shcControl, runnerCallback),
    ]
    const includeCallbackLag =
      new URL(request.url).pathname.includes('/checklist') ||
      optionalBoolean(selector.includeCallbackLag) === true
    const checklistChecks = includeCallbackLag
      ? [...checks, await callbackLagCheck(env, selectedRunRecord)]
      : checks
    const status = operatorCheckRollup(checklistChecks)

    return {
      checks: checklistChecks,
      nextSafeAction: operatorNextSafeAction(
        checklistChecks,
        selectedRunRecord,
      ),
      run:
        selectedRunRecord === undefined
          ? null
          : {
              eventCursor: selectedRunRecord.eventCursor,
              goalId: selectedRunRecord.goalId,
              id: selectedRunRecord.id,
              status: selectedRunRecord.status,
              updatedAt: selectedRunRecord.updatedAt,
            },
      status,
      targetUser,
    }
  }

  const handleAutopilotFleetApi = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const providerBundle = await listProviderAccountsForUser(
      makeD1ProviderAccountRepository(openAgentsDatabase(env)),
      session.user.userId,
    )
    const store = dependencies.makeBillingAwareOmniRunStore(env)
    const [agentRuns, deployments] = await Promise.all([
      store.listAgentRunsForUser(session.user.userId, 20),
      store.listDeploymentsForUser(session.user.userId, 10),
    ])
    const accounts = providerBundle.accounts.map(account => ({
      accountLabel: account.accountLabel ?? account.providerAccountRef,
      availableForMission:
        account.publicStatus === 'connected' &&
        account.status === 'connected' &&
        account.health === 'healthy' &&
        account.hasSecretRef,
      connectedAt: account.connectedAt ?? null,
      hasSecretRef: account.hasSecretRef,
      health: account.health,
      id: account.id,
      lastStatusAt: account.lastStatusAt,
      planType: account.planType ?? null,
      provider: account.provider,
      providerAccountRef: account.providerAccountRef,
      publicStatus: account.publicStatus,
    }))

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        accounts,
        fleet: {
          activeSessions: agentRuns.filter(bundle =>
            ['queued', 'running', 'waiting_for_input'].includes(
              bundle.run.status,
            ),
          ).length,
          availableCapacity: accounts.filter(
            account => account.availableForMission,
          ).length,
          connectedAccounts: accounts.filter(
            account => account.publicStatus === 'connected',
          ).length,
          dispatchMode: dependencies.shcDispatchConfig(env).dispatchMode,
          fleetId: 'openagents-autopilot:shc-autopilot',
          primaryRunnerId: 'oa-shc-katy-01',
          routingPolicy: 'shc_primary_gcloud_fallback',
          totalAccounts: accounts.length,
        },
        ...publicOmniOverview(agentRuns, deployments),
      }),
      session,
    )
  }

  const handleAutopilotTokenLeaderboardsApi = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const actor = await observedPromise('Auth.authenticateRequestActor', () =>
      dependencies.authenticateRequestActor(request, env, ctx),
    )

    if (actor === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const userId =
      actor.kind === 'human' ? actor.user.userId : actor.agent.user.id
    const response = noStoreJsonResponse({
      authenticated: true,
      actor: dependencies.actorJson(actor),
      leaderboards: await observedPromise(
        'TokenUsage.readLeaderboardsForUser',
        () => dependencies.readTokenUsageLeaderboardsForUser(env, userId),
      ),
    })

    if (actor.kind === 'human' && actor.tokens !== undefined) {
      appendSessionCookies(response.headers, actor.tokens)
    }

    return response
  }

  const handleOmniOperatorFleetApi = async (
    request: Request,
    env: Env,
  ): Promise<Response> => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    if (!(await dependencies.requireAdminApiToken(request, env))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const selector = await readRequestSelector(request)
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      openAgentsDatabase(env),
      selector,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const providerBundle = await listProviderAccountsForUser(
      makeD1ProviderAccountRepository(openAgentsDatabase(env)),
      targetUser.userId,
    )
    const store = dependencies.makeBillingAwareOmniRunStore(env)
    const [agentRuns, deployments] = await Promise.all([
      store.listAgentRunsForUser(targetUser.userId, 20),
      store.listDeploymentsForUser(targetUser.userId, 10),
    ])

    return noStoreJsonResponse({
      targetUser,
      accounts: providerBundle.accounts.map(account => ({
        accountLabel: account.accountLabel ?? account.providerAccountRef,
        availableForMission:
          account.publicStatus === 'connected' &&
          account.status === 'connected' &&
          account.health === 'healthy' &&
          account.hasSecretRef,
        connectedAt: account.connectedAt ?? null,
        hasSecretRef: account.hasSecretRef,
        health: account.health,
        id: account.id,
        lastStatusAt: account.lastStatusAt,
        planType: account.planType ?? null,
        provider: account.provider,
        providerAccountRef: account.providerAccountRef,
        publicStatus: account.publicStatus,
      })),
      fleet: {
        activeSessions: agentRuns.filter(bundle =>
          ['queued', 'running', 'waiting_for_input'].includes(
            bundle.run.status,
          ),
        ).length,
        dispatchMode: dependencies.shcDispatchConfig(env).dispatchMode,
        fleetId: 'openagents-autopilot:shc-autopilot',
        primaryRunnerId: 'oa-shc-katy-01',
        routingPolicy: 'shc_primary_gcloud_fallback',
      },
      ...publicOmniOverview(agentRuns, deployments),
    })
  }

  const handleOmniOperatorTeamChatMessagesApi = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> => {
    if (request.method !== 'GET' && request.method !== 'POST') {
      return methodNotAllowed(['GET', 'POST'])
    }

    if (!(await dependencies.requireAdminApiToken(request, env))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const selector = await readRequestSelector(request)
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      openAgentsDatabase(env),
      selector,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const teamId = optionalString(selector.teamId)

    if (teamId === undefined) {
      return noStoreJsonResponse(
        {
          error: 'bad_request',
          reason: 'teamId is required',
          targetUser,
        },
        { status: 400 },
      )
    }

    const role = await readActiveTeamMembershipRole(
      openAgentsDatabase(env),
      teamId,
      targetUser.userId,
    )

    if (role === undefined) {
      return noStoreJsonResponse(
        {
          error: 'forbidden',
          reason: 'target user is not an active member of this team',
          targetUser,
        },
        { status: 403 },
      )
    }

    const projectId = optionalString(selector.projectId)
    const project =
      projectId === undefined
        ? undefined
        : await readActiveTeamProject(
            openAgentsDatabase(env),
            teamId,
            projectId,
          )

    if (projectId !== undefined && project === undefined) {
      return noStoreJsonResponse(
        {
          error: 'not_found',
          reason: 'project not found',
          targetUser,
        },
        { status: 404 },
      )
    }

    const roomThreadId =
      project === undefined
        ? teamChatThreadId(teamId)
        : teamProjectChatThreadId(teamId, project.id)

    if (request.method === 'GET') {
      const limit = Math.min(
        Math.max(optionalInteger(selector.limit) ?? 50, 1),
        100,
      )
      const kind = optionalUserWritableTeamChatKind(selector.kind)
      const autopilotThreadId = optionalUuid(selector.threadId)

      return noStoreJsonResponse({
        messages: await listTeamChatMessages(
          openAgentsDatabase(env),
          teamId,
          limit,
          kind,
          autopilotThreadId,
          project?.id ?? null,
        ),
        projectId: project?.id ?? null,
        targetUser,
        teamId,
      })
    }

    const posted = await dependencies.postTeamChatMessageForUser(env, ctx, {
      body: selector,
      ...(project === undefined ? {} : { project }),
      roomThreadId,
      teamId,
      userId: targetUser.userId,
    })

    return noStoreJsonResponse(
      {
        ...posted.payload,
        targetUser,
      },
      { status: posted.status },
    )
  }

  const handleOmniOperatorAgentRunsApi = async (
    request: Request,
    env: Env,
  ): Promise<Response> => {
    if (!(await dependencies.requireAdminApiToken(request, env))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const selector = await readRequestSelector(request)
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      openAgentsDatabase(env),
      selector,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const operatorPath = new URL(request.url).pathname

    if (
      operatorPath === '/api/operator/autopilot/preflight' ||
      operatorPath === '/api/omni/operator/autopilot/preflight' ||
      operatorPath === '/api/omni/operator/autopilot/checklist'
    ) {
      if (request.method !== 'GET' && request.method !== 'POST') {
        return methodNotAllowed(['GET', 'POST'])
      }

      return noStoreJsonResponse(
        await autopilotPreflightPayload(request, env, selector, targetUser),
      )
    }

    const store = makeOmniRunStoreForEnv(env)

    if (request.method === 'GET') {
      const runs = await store.listAgentRunsForUser(targetUser.userId, 30)

      return noStoreJsonResponse({
        agentRuns: runs.map(publicAgentRunBundle),
        targetUser,
      })
    }

    if (request.method !== 'POST') {
      return methodNotAllowed(['GET', 'POST'])
    }

    const billingGate = await requireMinimumRunCredits(
      openAgentsDatabase(env),
      targetUser.userId,
    )

    if (!billingGate.ok) {
      return noStoreJsonResponse(
        {
          billing: billingGate.billing,
          error: 'insufficient_credits',
          message: billingGate.message,
          targetUser,
        },
        { status: 402 },
      )
    }

    const repositoryText =
      firstText(selector.repository, selector.repo) ??
      'OpenAgentsInc/autopilot-omega'
    const goal = firstText(selector.goal, selector.prompt) ?? defaultAgentGoal
    const explicitDispatchGoal = firstText(
      selector.dispatchGoal,
      selector.executionGoal,
    )
    const parsedRepository = parseGithubRepository(repositoryText)
    const repository = {
      ...parsedRepository,
      ref:
        firstText(selector.repositoryRef, selector.baseRef) ??
        parsedRepository.ref,
    }
    const backend = optionalRunnerBackend(selector.runnerBackend) ?? 'shc_vm'
    const projectId = optionalString(selector.projectId)
    const teamId = optionalString(selector.teamId)
    const project =
      projectId === undefined || teamId === undefined
        ? undefined
        : await readActiveTeamProject(
            openAgentsDatabase(env),
            teamId,
            projectId,
          )

    if (
      projectId !== undefined &&
      teamId !== undefined &&
      project === undefined
    ) {
      return noStoreJsonResponse(
        {
          error: 'not_found',
          reason: 'project not found',
          targetUser,
        },
        { status: 404 },
      )
    }

    const timeoutMs = numberOrUndefined(selector.timeoutMs)
    const runId = createAgentRunId()
    const githubWorkOrder = githubWorkOrderFromSelector(
      selector,
      repository,
      runId,
    )
    const providerBundle = await listProviderAccountsForUser(
      makeD1ProviderAccountRepository(openAgentsDatabase(env)),
      targetUser.userId,
    )
    const providerAccountRef = connectedProviderAccountRef(
      providerBundle,
      optionalString(selector.providerAccountRef) ??
        optionalString(selector.providerAccountId),
    )

    if (providerAccountRef === undefined) {
      const message = await providerAccountLaunchBlockMessage(
        openAgentsDatabase(env),
        targetUser.userId,
        providerBundle,
        optionalString(selector.providerAccountRef) ??
          optionalString(selector.providerAccountId),
      )

      return noStoreJsonResponse(
        {
          error: 'provider_reconnect_required',
          legacyError: 'provider_account_required',
          message,
          requiresReconnect: true,
          targetUser,
        },
        { status: 409 },
      )
    }

    const githubWriteRepository = makeD1GitHubWriteRepository(
      openAgentsDatabase(env),
    )
    const githubWriteConnection =
      await githubWriteRepository.findUsableConnectionForUser(targetUser.userId)

    if (
      githubWriteConnection === undefined ||
      !hasRequiredGitHubWriteScopes(githubWriteConnection.scopes)
    ) {
      return noStoreJsonResponse(
        {
          error: 'github_write_connection_required',
          message:
            'Target user must connect repo push access before launching Autopilot on the computer.',
          targetUser,
        },
        { status: 409 },
      )
    }

    const grant = await issueProviderAccountGrant(
      makeD1ProviderAccountRepository(openAgentsDatabase(env)),
      {
        providerAccountRef,
        requestedAction: 'operator_autopilot_mission',
        runnerSessionId: runId,
        userId: targetUser.userId,
      },
    )

    if (grant === undefined) {
      return noStoreJsonResponse(
        { error: 'provider_account_not_found', targetUser },
        { status: 404 },
      )
    }

    const githubWriteGrant = await issueGitHubWriteGrant(
      githubWriteRepository,
      {
        requestedAction: 'operator_autopilot_mission',
        runnerSessionId: runId,
        userId: targetUser.userId,
      },
    )

    if (githubWriteGrant === undefined) {
      return noStoreJsonResponse(
        {
          error: 'github_write_connection_not_found',
          message:
            'Target user must connect repo push access before launching Autopilot on the computer.',
          targetUser,
        },
        { status: 409 },
      )
    }

    const launchGoal = await resolveAgentRunGoal(env, {
      objective: goal,
      project,
      projectId,
      selector,
      teamId,
      userId: targetUser.userId,
    })

    const queued = createQueuedAgentRun({
      appOrigin: dependencies.getAppOrigin(env),
      authGrantRef: grant.grantRef,
      backend,
      githubWriteConnectionRef: githubWriteGrant.connectionRef,
      githubWriteGrantRef: githubWriteGrant.grantRef,
      githubWorkOrder,
      dispatchGoal: explicitDispatchGoal ?? launchGoal.objective,
      goal: launchGoal.objective,
      goalId: launchGoal.id,
      goalStatus: launchGoal.status,
      goalVisibility: launchGoal.visibility,
      providerAccountRef: grant.providerAccountRef,
      ...(projectId === undefined ? {} : { projectId }),
      repository,
      runId,
      timeUsedSeconds: launchGoal.timeUsedSeconds,
      ...(teamId === undefined ? {} : { teamId }),
      timeoutMs,
      tokenBudget: launchGoal.tokenBudget,
      tokensUsed: launchGoal.tokensUsed,
      userId: targetUser.userId,
    })
    await store.saveAgentRun(queued.run, queued.events)
    await applyGoalRuntimeEvents(env, launchGoal.id, queued.events)
    await projectAgentRunSyncScope(env, queued.run)
    await dispatchQueuedAgentRun(env, queued.run)

    const bundle = await store.findAgentRunForUser(
      targetUser.userId,
      queued.run.id,
    )
    const publicBundle =
      bundle === undefined ? undefined : publicAgentRunBundle(bundle)
    const status = publicBundle?.run.status ?? 'queued'

    return noStoreJsonResponse(
      {
        mission: {
          codexRunId: queued.run.id,
          githubWriteConnectionRef: githubWriteGrant.connectionRef,
          githubWorkOrder: {
            branchName: githubWorkOrder.branchName,
            issueNumber: githubWorkOrder.issueNumber ?? null,
            issueUrl: githubWorkOrder.issueUrl ?? null,
            pullRequestTitle: githubWorkOrder.pullRequestTitle,
          },
          providerAccountRef: grant.providerAccountRef,
          repository: `${repository.owner}/${repository.repo}`,
          runnerBackend: backend,
          status,
        },
        ...(publicBundle === undefined ? {} : publicBundle),
        browserStatusUrl: `/api/omni/agent-runs/${queued.run.id}`,
        statusUrl: `/api/omni/operator/agent-runs/${queued.run.id}`,
        streamUrl: `/api/omni/agent-runs/${queued.run.id}/events`,
        targetUser,
      },
      { status: 202 },
    )
  }

  const handleOmniOperatorAgentRunDetailApi = async (
    request: Request,
    env: OmniHandlerEnv,
    runId: string,
  ): Promise<Response> => {
    const operatorPath = new URL(request.url).pathname
    const action = operatorPath.endsWith('/callbacks/retry')
      ? 'callbacks_retry'
      : operatorPath.endsWith('/continue')
        ? 'continue'
        : operatorPath.endsWith('/checklist')
          ? 'checklist'
          : 'detail'

    if (action === 'detail' && request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    if (action === 'callbacks_retry' && request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    if (action === 'continue' && request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    if (
      action === 'checklist' &&
      request.method !== 'GET' &&
      request.method !== 'POST'
    ) {
      return methodNotAllowed(['GET', 'POST'])
    }

    if (!(await dependencies.requireAdminApiToken(request, env))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const selector = await readRequestSelector(request)
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      openAgentsDatabase(env),
      selector,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const store = makeOmniRunStoreForEnv(env)
    const bundle = await findOperatorRunBundle(store, targetUser, runId)

    if (bundle === undefined) {
      return noStoreJsonResponse(
        { error: 'not_found', targetUser },
        { status: 404 },
      )
    }

    if (action === 'checklist') {
      return noStoreJsonResponse(
        await autopilotPreflightPayload(
          request,
          env,
          {
            ...selector,
            includeCallbackLag: true,
            runId: bundle.run.id,
          },
          targetUser,
        ),
      )
    }

    if (action === 'callbacks_retry') {
      const fetched = await fetchAgentRunEventsFromShc(bundle.run, {
        ...dependencies.shcDispatchConfig(env),
        cursor: bundle.run.eventCursor,
      })

      if (!fetched.ok) {
        return noStoreJsonResponse(
          {
            error: 'shc_callback_retry_failed',
            message: fetched.error,
            runId: bundle.run.id,
            status: fetched.status,
            targetPath: fetched.targetPath ?? null,
            targetUser,
          },
          { status: fetched.status === 'not_configured' ? 409 : 502 },
        )
      }

      const ingested = await ingestOperatorFetchedRunEvents(
        env,
        bundle.run,
        fetched.events,
        fetched.runStatus,
      )

      return noStoreJsonResponse({
        accepted: ingested.accepted,
        nextCursor: fetched.nextCursor,
        pendingEvents: fetched.events.length,
        runId: bundle.run.id,
        status: ingested.status,
        targetUser,
      })
    }

    if (action === 'continue') {
      const prompt =
        firstText(selector.prompt, selector.instruction, selector.reason) ??
        'Continue the active OpenAgents goal from the latest durable run state.'

      if (
        bundle.run.status === 'running' ||
        bundle.run.status === 'waiting_for_input' ||
        bundle.run.status === 'queued'
      ) {
        const result = await continueAgentRunOnShc(bundle.run, {
          ...dependencies.shcDispatchConfig(env),
          authGrantRef: bundle.run.authGrantRef ?? undefined,
          prompt,
          turnId:
            optionalString(selector.turnId) ??
            `operator_turn_${createAgentRunId()}`,
        })

        if (!result.ok) {
          return noStoreJsonResponse(
            {
              error: 'shc_continue_failed',
              message: result.error,
              runId: bundle.run.id,
              status: result.status,
              targetPath: result.targetPath ?? null,
              targetUser,
            },
            { status: result.status === 'not_configured' ? 409 : 502 },
          )
        }

        const ingested = await ingestOperatorFetchedRunEvents(
          env,
          bundle.run,
          shcActionEventPayloads(result.payload),
          optionalString(
            nestedUnknown(
              isRecord(result.payload) ? result.payload : undefined,
              ['status'],
            ),
          ),
        )

        return noStoreJsonResponse(
          {
            accepted: true,
            ingestedEvents: ingested.accepted,
            mode: 'follow_up_turn',
            runId: bundle.run.id,
            status: ingested.status,
            targetUser,
          },
          { status: 202 },
        )
      }

      if (bundle.run.goalId === null) {
        return noStoreJsonResponse(
          {
            error: 'run_goal_required',
            message:
              'A stopped run must be attached to a durable goal before it can be continued.',
            runId: bundle.run.id,
            targetUser,
          },
          { status: 409 },
        )
      }

      await requestGoalContinuationAfterCompletedRun(
        env,
        undefined,
        bundle.run.id,
      )

      return noStoreJsonResponse(
        {
          accepted: true,
          goalId: bundle.run.goalId,
          mode: 'goal_continuation',
          runId: bundle.run.id,
          targetUser,
        },
        { status: 202 },
      )
    }

    return noStoreJsonResponse({
      ...publicAgentRunBundle(bundle),
      browserStatusUrl: `/api/omni/agent-runs/${bundle.run.id}`,
      statusUrl: `/api/omni/operator/agent-runs/${bundle.run.id}`,
      streamUrl: `/api/omni/agent-runs/${bundle.run.id}/events`,
      targetUser,
    })
  }

  type UserAutopilotMissionLaunch = Readonly<{
    payload: Record<string, unknown>
    runId: string
  }>

  type UserAutopilotMissionLaunchResult =
    | Readonly<{ launch: UserAutopilotMissionLaunch; ok: true }>
    | Readonly<{ ok: false; response: Response }>

  type UserAutopilotRunContinuation = Readonly<{
    goalId: string | null
    mode: 'follow_up_turn'
    payload: Record<string, unknown>
    runId: string
  }>

  type UserAutopilotRunContinuationResult =
    | Readonly<{ continuation: UserAutopilotRunContinuation; ok: true }>
    | Readonly<{ ok: false; response: Response }>

  const dispatchQueuedAgentRun = async (
    env: Env,
    run: AgentRunRecord,
  ): Promise<void> => {
    const store = dependencies.makeBillingAwareOmniRunStore(env)
    const dispatchConfig = dependencies.shcDispatchConfig(env)
    const dispatchService = makeOmniDispatchService()
    const repository = makeOmniRunRepository(store)
    const runnerEvents = makeOmniRunnerEventService()
    const dispatchResult = await Effect.runPromise(
      Effect.gen(function* () {
        const dispatch = yield* dispatchService.dispatchAgentRun(
          run.assignment,
          dispatchConfig,
        )
        const status = runStatusFromText(dispatch.status) ?? 'queued'
        const event = dispatchEventForAgentRun(run.id, 2, dispatch)

        yield* repository.appendAgentRunEvents(
          run.id,
          [event],
          status,
          dispatch.externalId,
        )
        yield* Effect.tryPromise({
          try: () => applyGoalRuntimeEvents(env, run.goalId, [event]),
          catch: error =>
            omniErrorFromUnknown('goal_dispatch_event_accounting', error),
        }).pipe(Effect.catch(() => Effect.void))
        yield* notifyAgentRunSyncScopesEffect(env, run.id)

        return undefined
      }).pipe(
        Effect.withSpan('OmniLaunch.dispatchQueuedAgentRun'),
        Effect.match({
          onFailure: failedOmniEffectResult,
          onSuccess: succeededOmniEffectResult,
        }),
      ),
    )

    if (dispatchResult._tag === 'Failed') {
      await Effect.runPromise(
        Effect.gen(function* () {
          const event = yield* runnerEvents.eventFromCallbackPayload(
            run.id,
            2,
            {
              source: 'shc',
              status: 'failed',
              summary: errorMessage(dispatchResult.error),
              type: 'runner.dispatch_failed',
            },
          )

          yield* repository.appendAgentRunEvents(run.id, [event], 'failed')
          yield* Effect.tryPromise({
            try: () => applyGoalRuntimeEvents(env, run.goalId, [event]),
            catch: error =>
              omniErrorFromUnknown('goal_dispatch_failure_accounting', error),
          }).pipe(Effect.catch(() => Effect.void))
          yield* notifyAgentRunSyncScopesEffect(env, run.id)
        }).pipe(Effect.withSpan('OmniLaunch.recordAgentRunDispatchFailure')),
      )
    }
  }

  const dispatchQueuedDeployment = async (
    env: OmniHandlerEnv,
    store: OmniRunStore,
    deployment: DeploymentRecord,
  ): Promise<void> => {
    const dispatchService = makeOmniDispatchService()
    const repository = makeOmniDeploymentRepository(store)
    const runnerEvents = makeOmniRunnerEventService()
    const dispatchResult = await Effect.runPromise(
      Effect.gen(function* () {
        const dispatch = yield* dispatchService.dispatchDeployment(
          deployment.assignment,
          dependencies.shcDispatchConfig(env),
        )

        yield* repository.appendDeploymentEvents(
          deployment.id,
          [dispatchEventForDeployment(deployment.id, 2, dispatch)],
          deploymentStatusFromText(dispatch.status) ?? 'queued',
          dispatch.externalId,
        )

        return undefined
      }).pipe(
        Effect.withSpan('OmniLaunch.dispatchQueuedDeployment'),
        Effect.match({
          onFailure: failedOmniEffectResult,
          onSuccess: succeededOmniEffectResult,
        }),
      ),
    )

    if (dispatchResult._tag === 'Failed') {
      await Effect.runPromise(
        Effect.gen(function* () {
          const event = yield* runnerEvents.eventFromCallbackPayload(
            deployment.id,
            2,
            {
              source: 'shc',
              status: 'failed',
              summary: errorMessage(dispatchResult.error),
              type: 'deploy.dispatch_failed',
            },
          )

          yield* repository.appendDeploymentEvents(
            deployment.id,
            [event],
            'failed',
          )
        }).pipe(Effect.withSpan('OmniLaunch.recordDeploymentDispatchFailure')),
      )
    }
  }

  const requestGoalContinuationAfterCompletedRun = async (
    workerEnv: OmniHandlerEnv,
    ctx: ExecutionContext | undefined,
    completedRunId: string,
  ): Promise<void> => {
    const completedRun = await readAgentRunById(
      openAgentsDatabase(workerEnv),
      completedRunId,
    )

    if (completedRun === undefined || completedRun.goalId === null) {
      return
    }

    const completedGoalId = completedRun.goalId
    const completedProviderAccountRef = completedRun.providerAccountRef
    const repositoryLayer = makeAgentGoalRepositoryLayerForEnv(workerEnv)
    const eventRepositoryLayer =
      makeAgentGoalEventRepositoryLayerForEnv(workerEnv)
    const queueLayer = Layer.succeed(AgentGoalContinuationQueue, {
      enqueue: Effect.fn('AgentGoalContinuationQueue.enqueue')(
        function* (input) {
          if (completedProviderAccountRef === null) {
            return yield* new AgentGoalStorageError({
              operation: 'agentGoalContinuation.enqueue',
              error: 'completed run has no provider account ref',
            })
          }

          const continuationRunId = createAgentRunId()
          const providerGrant = yield* Effect.tryPromise({
            try: () =>
              issueProviderAccountGrant(
                makeD1ProviderAccountRepository(openAgentsDatabase(workerEnv)),
                {
                  providerAccountRef: completedProviderAccountRef,
                  requestedAction: 'autopilot_goal_continuation',
                  runnerSessionId: continuationRunId,
                  userId: completedRun.userId,
                },
              ),
            catch: error =>
              new AgentGoalStorageError({
                operation: 'agentGoalContinuation.issueProviderGrant',
                error,
              }),
          })

          if (providerGrant === undefined) {
            return yield* new AgentGoalStorageError({
              operation: 'agentGoalContinuation.issueProviderGrant',
              error: 'provider account not found',
            })
          }

          const githubWorkOrder = completedRun.assignment.githubWorkOrder
          const githubWriteRepository = makeD1GitHubWriteRepository(
            openAgentsDatabase(workerEnv),
          )
          const githubWriteGrant =
            githubWorkOrder === undefined
              ? undefined
              : yield* Effect.tryPromise({
                  try: () =>
                    issueGitHubWriteGrant(githubWriteRepository, {
                      requestedAction: 'autopilot_goal_continuation',
                      runnerSessionId: continuationRunId,
                      userId: completedRun.userId,
                    }),
                  catch: error =>
                    new AgentGoalStorageError({
                      operation: 'agentGoalContinuation.issueGitHubWriteGrant',
                      error,
                    }),
                })

          if (githubWorkOrder !== undefined && githubWriteGrant === undefined) {
            return yield* new AgentGoalStorageError({
              operation: 'agentGoalContinuation.issueGitHubWriteGrant',
              error: 'github write connection not found',
            })
          }

          const queued = createQueuedAgentRun({
            appOrigin: dependencies.getAppOrigin(workerEnv),
            authGrantRef: providerGrant.grantRef,
            backend: completedRun.backend,
            dispatchGoal: input.goal.objective,
            goal: input.goal.objective,
            goalId: input.goal.id,
            goalStatus: input.goal.status,
            goalVisibility: input.goal.visibility,
            ...(githubWriteGrant === undefined
              ? {}
              : {
                  githubWriteConnectionRef: githubWriteGrant.connectionRef,
                  githubWriteGrantRef: githubWriteGrant.grantRef,
                }),
            ...(githubWorkOrder === undefined ? {} : { githubWorkOrder }),
            providerAccountRef: providerGrant.providerAccountRef,
            ...(completedRun.projectId === null
              ? {}
              : { projectId: completedRun.projectId }),
            repository: completedRun.repository,
            runId: continuationRunId,
            runtime: completedRun.runtime,
            ...(completedRun.teamId === null
              ? {}
              : { teamId: completedRun.teamId }),
            timeUsedSeconds: input.goal.timeUsedSeconds,
            timeoutMs: completedRun.assignment.sandbox.timeoutMs,
            tokenBudget: input.goal.tokenBudget,
            tokensUsed: input.goal.tokensUsed,
            userId: completedRun.userId,
          })
          const store = dependencies.makeBillingAwareOmniRunStore(
            workerEnv,
            ctx,
          )

          yield* Effect.tryPromise({
            try: async () => {
              await store.saveAgentRun(queued.run, queued.events)
              await applyGoalRuntimeEvents(
                workerEnv,
                input.goal.id,
                queued.events,
              )
              await projectAgentRunSyncScope(workerEnv, queued.run)
              await dispatchQueuedAgentRun(workerEnv, queued.run)
            },
            catch: error =>
              new AgentGoalStorageError({
                operation: 'agentGoalContinuation.persistAndDispatch',
                error,
              }),
          })

          return { runId: queued.run.id }
        },
      ),
    })
    const runtimeDependencies = Layer.merge(
      repositoryLayer,
      eventRepositoryLayer,
    )
    const continuationDependencies = Layer.mergeAll(
      runtimeDependencies,
      AgentGoalCapacityPolicyServiceLive(),
      queueLayer,
    )

    await observedEffect(
      'OmniGoal.requestContinuationAfterCompletedRun',
      Effect.gen(function* () {
        const continuation = yield* AgentGoalContinuationService

        yield* continuation.requestContinuation({
          accountCapacityAvailable: true,
          durableSnapshotWritten: true,
          expectedGoalId: completedGoalId,
          goalId: completedGoalId,
          providerHealthy: true,
        })
      }).pipe(
        Effect.provide(
          Layer.mergeAll(
            continuationDependencies,
            AgentGoalContinuationServiceLive.pipe(
              Layer.provide(continuationDependencies),
            ),
          ),
        ),
        Effect.catchTags({
          AgentGoalNotFound: () => Effect.void,
          AgentGoalStaleUpdate: () => Effect.void,
          AgentGoalValidationError: () => Effect.void,
        }),
      ),
    )
  }

  const continueUserAutopilotRun = async (
    env: Env,
    ctx: ExecutionContext,
    input: Readonly<{
      prompt: string
      runId: string
      userId: string
    }>,
  ): Promise<UserAutopilotRunContinuationResult> => {
    const store = dependencies.makeBillingAwareOmniRunStore(env, ctx)
    const bundle = await store.findAgentRunForUser(input.userId, input.runId)

    if (bundle === undefined) {
      return {
        ok: false,
        response: noStoreJsonResponse(
          { error: 'not_found', runId: input.runId },
          { status: 404 },
        ),
      }
    }

    if (
      bundle.run.status !== 'running' &&
      bundle.run.status !== 'waiting_for_input' &&
      bundle.run.status !== 'queued'
    ) {
      return {
        ok: false,
        response: noStoreJsonResponse(
          {
            error: 'run_not_continuable',
            runId: bundle.run.id,
            status: bundle.run.status,
          },
          { status: 409 },
        ),
      }
    }

    const result = await continueAgentRunOnShc(bundle.run, {
      ...dependencies.shcDispatchConfig(env),
      authGrantRef: bundle.run.authGrantRef ?? undefined,
      prompt: input.prompt,
      turnId: `adjutant_turn_${createAgentRunId()}`,
    })

    if (!result.ok) {
      return {
        ok: false,
        response: noStoreJsonResponse(
          {
            error: 'shc_continue_failed',
            message: result.error,
            runId: bundle.run.id,
            status: result.status,
            targetPath: result.targetPath ?? null,
          },
          { status: result.status === 'not_configured' ? 409 : 502 },
        ),
      }
    }

    const ingested = await ingestOperatorFetchedRunEvents(
      env,
      bundle.run,
      shcActionEventPayloads(result.payload),
      optionalString(
        nestedUnknown(isRecord(result.payload) ? result.payload : undefined, [
          'status',
        ]),
      ),
    )

    return {
      continuation: {
        goalId: bundle.run.goalId,
        mode: 'follow_up_turn',
        payload: {
          accepted: true,
          ingestedEvents: ingested.accepted,
          status: ingested.status,
        },
        runId: bundle.run.id,
      },
      ok: true,
    }
  }

  const launchUserAutopilotMission = async (
    env: Env,
    ctx: ExecutionContext,
    input: Readonly<{
      selector: Record<string, unknown>
      userId: string
    }>,
  ): Promise<UserAutopilotMissionLaunchResult> => {
    const store = dependencies.makeBillingAwareOmniRunStore(env, ctx)
    const repositoryText =
      firstText(input.selector.repository, input.selector.repo) ??
      'OpenAgentsInc/autopilot-omega'
    const goal =
      firstText(input.selector.goal, input.selector.prompt) ?? defaultAgentGoal
    const explicitDispatchGoal = firstText(
      input.selector.dispatchGoal,
      input.selector.executionGoal,
    )
    const teamId = optionalString(input.selector.teamId)
    const projectId = optionalString(input.selector.projectId)
    const project =
      projectId === undefined || teamId === undefined
        ? undefined
        : await readActiveTeamProject(
            openAgentsDatabase(env),
            teamId,
            projectId,
          )

    if (
      projectId !== undefined &&
      teamId !== undefined &&
      project === undefined
    ) {
      return {
        ok: false,
        response: noStoreJsonResponse(
          {
            error: 'not_found',
            reason: 'project not found',
          },
          { status: 404 },
        ),
      }
    }

    const repository = parseGithubRepository(repositoryText)
    const backend =
      optionalRunnerBackend(input.selector.runnerBackend) ?? 'shc_vm'
    const timeoutMs = numberOrUndefined(input.selector.timeoutMs)
    const runId = createAgentRunId()
    const githubWorkOrder = githubWorkOrderFromSelector(
      input.selector,
      repository,
      runId,
    )
    const billingGate = await requireMinimumRunCredits(
      openAgentsDatabase(env),
      input.userId,
    )

    if (!billingGate.ok) {
      return {
        ok: false,
        response: noStoreJsonResponse(
          {
            billing: billingGate.billing,
            error: 'insufficient_credits',
            message: billingGate.message,
          },
          { status: 402 },
        ),
      }
    }

    const providerBundle = await listProviderAccountsForUser(
      makeD1ProviderAccountRepository(openAgentsDatabase(env)),
      input.userId,
    )
    const providerAccountRef = connectedProviderAccountRef(
      providerBundle,
      optionalString(input.selector.providerAccountRef) ??
        optionalString(input.selector.providerAccountId),
    )

    if (providerAccountRef === undefined) {
      const message = await providerAccountLaunchBlockMessage(
        openAgentsDatabase(env),
        input.userId,
        providerBundle,
        optionalString(input.selector.providerAccountRef) ??
          optionalString(input.selector.providerAccountId),
      )

      return {
        ok: false,
        response: noStoreJsonResponse(
          {
            error: 'provider_reconnect_required',
            legacyError: 'provider_account_required',
            message,
            requiresReconnect: true,
          },
          { status: 409 },
        ),
      }
    }

    const githubWriteRepository = makeD1GitHubWriteRepository(
      openAgentsDatabase(env),
    )
    const githubWriteConnection =
      await githubWriteRepository.findUsableConnectionForUser(input.userId)

    if (
      githubWriteConnection === undefined ||
      !hasRequiredGitHubWriteScopes(githubWriteConnection.scopes)
    ) {
      return {
        ok: false,
        response: noStoreJsonResponse(
          {
            error: 'github_write_connection_required',
            message:
              'Connect repo push access before launching Autopilot on the computer.',
          },
          { status: 409 },
        ),
      }
    }

    const grant = await issueProviderAccountGrant(
      makeD1ProviderAccountRepository(openAgentsDatabase(env)),
      {
        providerAccountRef,
        requestedAction: 'autopilot_mission',
        runnerSessionId: runId,
        userId: input.userId,
      },
    )

    if (grant === undefined) {
      return {
        ok: false,
        response: noStoreJsonResponse(
          { error: 'provider_account_not_found' },
          { status: 404 },
        ),
      }
    }

    const githubWriteGrant = await issueGitHubWriteGrant(
      githubWriteRepository,
      {
        requestedAction: 'autopilot_mission',
        runnerSessionId: runId,
        userId: input.userId,
      },
    )

    if (githubWriteGrant === undefined) {
      return {
        ok: false,
        response: noStoreJsonResponse(
          {
            error: 'github_write_connection_not_found',
            message:
              'Connect repo push access before launching Autopilot on the computer.',
          },
          { status: 409 },
        ),
      }
    }

    const launchGoal = await resolveAgentRunGoal(env, {
      objective: goal,
      project,
      projectId,
      selector: input.selector,
      teamId,
      userId: input.userId,
    })

    const queued = createQueuedAgentRun({
      appOrigin: dependencies.getAppOrigin(env),
      authGrantRef: grant.grantRef,
      backend,
      githubWriteConnectionRef: githubWriteGrant.connectionRef,
      githubWriteGrantRef: githubWriteGrant.grantRef,
      githubWorkOrder,
      dispatchGoal: explicitDispatchGoal ?? launchGoal.objective,
      goal: launchGoal.objective,
      goalId: launchGoal.id,
      goalStatus: launchGoal.status,
      goalVisibility: launchGoal.visibility,
      providerAccountRef: grant.providerAccountRef,
      ...(projectId === undefined ? {} : { projectId }),
      repository,
      runId,
      timeUsedSeconds: launchGoal.timeUsedSeconds,
      ...(teamId === undefined ? {} : { teamId }),
      timeoutMs,
      tokenBudget: launchGoal.tokenBudget,
      tokensUsed: launchGoal.tokensUsed,
      userId: input.userId,
    })
    await store.saveAgentRun(queued.run, queued.events)
    await applyGoalRuntimeEvents(env, launchGoal.id, queued.events)
    scheduleBackgroundWork(ctx, projectAgentRunSyncScope(env, queued.run))
    await dispatchQueuedAgentRun(env, queued.run)

    const bundle = (await store.findAgentRunForUser(
      input.userId,
      queued.run.id,
    )) ?? {
      events: queued.events,
      run: queued.run,
    }
    const publicBundle = publicAgentRunBundle(bundle)
    const missionStatus = publicBundle.run.status
    const payload = {
      mission: {
        codexRunId: queued.run.id,
        githubWriteConnectionRef: githubWriteGrant.connectionRef,
        githubWorkOrder: {
          branchName: githubWorkOrder.branchName,
          issueNumber: githubWorkOrder.issueNumber ?? null,
          issueUrl: githubWorkOrder.issueUrl ?? null,
          pullRequestTitle: githubWorkOrder.pullRequestTitle,
        },
        providerAccountRef: grant.providerAccountRef,
        repository: `${repository.owner}/${repository.repo}`,
        runnerBackend: backend,
        status: missionStatus,
      },
      ...publicBundle,
      runSummary: teamChatRunSummaryFromBundle(bundle),
      statusUrl: `/api/omni/agent-runs/${queued.run.id}`,
      streamUrl: `/api/omni/agent-runs/${queued.run.id}/events`,
    }

    return { launch: { payload, runId: queued.run.id }, ok: true }
  }

  const handleOmniAgentRunsApi = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> => {
    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const store = dependencies.makeBillingAwareOmniRunStore(env, ctx)

    if (request.method === 'GET') {
      const runs = await store.listAgentRunsForUser(session.user.userId, 30)

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({ agentRuns: runs.map(publicAgentRunBundle) }),
        session,
      )
    }

    if (request.method !== 'POST') {
      return methodNotAllowed(['GET', 'POST'])
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const launch = await launchUserAutopilotMission(env, ctx, {
      selector: body,
      userId: session.user.userId,
    })

    if (!launch.ok) {
      return dependencies.appendRefreshedSessionCookies(
        launch.response,
        session,
      )
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(launch.launch.payload, { status: 202 }),
      session,
    )
  }

  const handleOmniAgentRunDetailApi = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
    runId: string,
  ): Promise<Response> => {
    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const accessResult = await dependencies.threadRouteAccessBundle(
      env,
      session.user.userId,
      runId,
    )

    if (dependencies.isRouteAccessError(accessResult)) {
      return routeAccessResponse(accessResult, { surface: 'api' })
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(publicAgentRunBundle(accessResult)),
      session,
    )
  }

  type OmniCursorRow = Readonly<{ event_cursor: number | null }>
  type AgentRunProviderAccountRow = Readonly<{
    provider_account_ref: string | null
    user_id: string
  }>

  const markRunProviderAccountRequiresReauth = async (
    env: Env,
    runId: string,
    reason: string,
  ): Promise<void> => {
    const row = await openAgentsDatabase(env)
      .prepare(
        `SELECT user_id, provider_account_ref
       FROM agent_runs
       WHERE id = ?
       LIMIT 1`,
      )
      .bind(runId)
      .first<AgentRunProviderAccountRow>()
    const providerAccountRef = row?.provider_account_ref ?? undefined

    if (row === null || row === undefined || providerAccountRef === undefined) {
      return
    }

    await recordProviderAccountHealth(
      makeD1ProviderAccountRepository(openAgentsDatabase(env)),
      {
        actorId: 'openagents:runner-ingest',
        health: 'requires_reauth',
        providerAccountRef,
        reason,
      },
    )
  }

  const readAgentRunEventCursor = async (
    db: D1Database,
    runId: string,
  ): Promise<number> => {
    const row = await db
      .prepare(`SELECT event_cursor FROM agent_runs WHERE id = ?`)
      .bind(runId)
      .first<OmniCursorRow>()

    return row?.event_cursor ?? 0
  }

  const readDeploymentEventCursor = async (
    db: D1Database,
    deployId: string,
  ): Promise<number> => {
    const row = await db
      .prepare(`SELECT event_cursor FROM deployments WHERE id = ?`)
      .bind(deployId)
      .first<OmniCursorRow>()

    return row?.event_cursor ?? 0
  }

  const handleOmniAgentRunEventsApi = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
    runId: string,
  ): Promise<Response> => {
    if (request.method === 'POST') {
      if (!(await dependencies.requireRunnerCallbackAuth(request, env))) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      const body = await readJsonObject(request).catch(
        (): Record<string, unknown> => ({}),
      )
      const events = Array.isArray(body.events)
        ? body.events.filter(isRecord)
        : [body].filter(isRecord)
      const store = dependencies.makeBillingAwareOmniRunStore(env, ctx)
      const fallbackStart =
        (await readAgentRunEventCursor(openAgentsDatabase(env), runId)) + 1
      const callbackRun = await readAgentRunById(openAgentsDatabase(env), runId)
      const runnerEvents = makeOmniRunnerEventService()
      const repository = makeOmniRunRepository(store)
      const explicitStatus = runStatusFromText(optionalString(body.status))
      const ingestResult = await Effect.runPromise(
        Effect.gen(function* () {
          const records = yield* runnerEvents.eventsFromCallbackPayloads(
            runId,
            fallbackStart,
            events,
          )
          const status = runStatusFromIngestedEvents(explicitStatus, records)

          yield* repository.appendAgentRunEvents(
            runId,
            records,
            status,
            optionalString(body.externalRunId),
          )
          const maybeProviderReauthReason = (yield* Effect.all(
            records.map(event => runnerEvents.providerReauthReason(event)),
          )).find((reason): reason is string => reason !== undefined)

          return { maybeProviderReauthReason, records, status }
        }).pipe(
          Effect.withSpan('OmniRunnerCallback.persistAgentRunEvents'),
          Effect.match({
            onFailure: failedOmniEffectResult,
            onSuccess: succeededOmniEffectResult,
          }),
        ),
      )

      if (ingestResult._tag === 'Failed') {
        return runnerCallbackIngestFailureResponse(
          ingestResult.error,
          events,
          fallbackStart,
        )
      }

      const postIngestFailures: Array<
        Readonly<{ error: string; operation: string }>
      > = []
      try {
        await applyGoalRuntimeEvents(
          env,
          callbackRun?.goalId ?? null,
          ingestResult.value.records,
        )
      } catch (error) {
        postIngestFailures.push({
          error: publicErrorName(error),
          operation: 'goal_callback_event_accounting',
        })
      }

      try {
        const updatedRun = await readAgentRunById(openAgentsDatabase(env), runId)

        await applyGoalRuntimeStatus(
          env,
          goalRuntimeEventFromRunStatus(
            callbackRun?.goalId ?? null,
            runId,
            ingestResult.value.status,
            ingestResult.value.records,
            updatedRun === undefined
              ? undefined
              : (durationSecondsForRun(updatedRun) ?? undefined),
          ),
        )
      } catch (error) {
        postIngestFailures.push({
          error: publicErrorName(error),
          operation: 'goal_callback_status_accounting',
        })
      }

      try {
        await observedEffect(
          'OmniRunnerCallback.applyAdjutantRunLifecycleEvents',
          // KS-8.12 (#8323): lifecycle events write site_deployments /
          // site_events — ride the sites dual-write mirror seam.
          // KS-8.14 (#8359): the same events also flip software_orders
          // status; compose the business funnel mirror OVER the sites
          // proxy so order writes mirror to the business Postgres twin.
          // KS-8.17 (#8361): the adjutant_assignments / assignment_events /
          // adjustment_requests writes in this same call chain are a THIRD,
          // separate supervision long-tail Postgres twin/flag lane.
          applyAdjutantRunLifecycleEvents(
            businessDomainDatabaseForEnv(env, {
              d1: sitesContentDatabaseForEnv(env),
            }),
            {
              actorUserId: 'openagents:runner-ingest',
              appOrigin: dependencies.getAppOrigin(env),
              artifacts: env.ARTIFACTS,
              emailConfig: dependencies.getResendEmailConfig(env),
              events: ingestResult.value.records,
              runId,
              status: ingestResult.value.status,
            },
            makeSupervisionLongtailMirrorForEnv(env),
          ),
        )
      } catch (error) {
        postIngestFailures.push({
          error: publicErrorName(error),
          operation: 'adjutant_callback_lifecycle',
        })
      }

      if (ingestResult.value.maybeProviderReauthReason !== undefined) {
        scheduleBackgroundWork(
          ctx,
          markRunProviderAccountRequiresReauth(
            env,
            runId,
            ingestResult.value.maybeProviderReauthReason,
          ),
        )
      }
      scheduleBackgroundWork(
        ctx,
        Effect.runPromise(notifyAgentRunSyncScopesEffect(env, runId)),
      )
      if (ingestResult.value.status === 'completed') {
        scheduleBackgroundWork(
          ctx,
          dependencies.appendTeamAutopilotAnswerBack(env, ctx, runId),
        )
        scheduleBackgroundWork(
          ctx,
          requestGoalContinuationAfterCompletedRun(env, ctx, runId).catch(
            () => undefined,
          ),
        )
      }

      return noStoreJsonResponse({
        accepted: ingestResult.value.records.length,
        callbackAccounting:
          postIngestFailures.length === 0
            ? { status: 'ok' }
            : { failures: postIngestFailures, status: 'degraded' },
        runnerStatus: ingestResult.value.status ?? null,
        status: 'ok',
      })
    }

    if (request.method !== 'GET') {
      return methodNotAllowed(['GET', 'POST'])
    }

    const detail = await handleOmniAgentRunDetailApi(request, env, ctx, runId)

    return detail
  }

  const handleOmniDeploymentsApi = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> => {
    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const store = makeOmniRunStoreForEnv(env)

    if (request.method === 'GET') {
      const deployments = await store.listDeploymentsForUser(
        session.user.userId,
        20,
      )

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          deployments: deployments.map(publicDeploymentBundle),
        }),
        session,
      )
    }

    if (request.method !== 'POST') {
      return methodNotAllowed(['GET', 'POST'])
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return forbidden()
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const repository = parseGithubRepository(
      firstText(body.repository, body.repo) ?? 'OpenAgentsInc/autopilot-omega',
    )
    const queued = createQueuedDeployment({
      appOrigin: dependencies.getAppOrigin(env),
      repository,
      userId: session.user.userId,
    })
    await store.saveDeployment(queued.deployment, queued.events)
    await dispatchQueuedDeployment(env, store, queued.deployment)

    const bundle = await store.findDeploymentForUser(
      session.user.userId,
      queued.deployment.id,
    )

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        {
          ...(bundle === undefined ? {} : publicDeploymentBundle(bundle)),
          statusUrl: `/api/omni/deployments/${queued.deployment.id}`,
        },
        { status: 202 },
      ),
      session,
    )
  }

  const handleOmniOperatorDeploymentsApi = async (
    request: Request,
    env: Env,
  ): Promise<Response> => {
    if (!(await dependencies.requireAdminApiToken(request, env))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const selector = await readRequestSelector(request)
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      openAgentsDatabase(env),
      selector,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const store = makeOmniRunStoreForEnv(env)

    if (request.method === 'GET') {
      const deployments = await store.listDeploymentsForUser(
        targetUser.userId,
        20,
      )

      return noStoreJsonResponse({
        deployments: deployments.map(publicDeploymentBundle),
        targetUser,
      })
    }

    if (request.method !== 'POST') {
      return methodNotAllowed(['GET', 'POST'])
    }

    const repository = parseGithubRepository(
      firstText(selector.repository, selector.repo) ??
        'OpenAgentsInc/autopilot-omega',
    )
    const queued = createQueuedDeployment({
      appOrigin: dependencies.getAppOrigin(env),
      repository,
      userId: targetUser.userId,
    })
    await store.saveDeployment(queued.deployment, queued.events)
    await dispatchQueuedDeployment(env, store, queued.deployment)

    const bundle = await store.findDeploymentForUser(
      targetUser.userId,
      queued.deployment.id,
    )

    return noStoreJsonResponse(
      {
        ...(bundle === undefined ? {} : publicDeploymentBundle(bundle)),
        statusUrl: `/api/omni/deployments/${queued.deployment.id}`,
        targetUser,
      },
      { status: 202 },
    )
  }

  const handleOmniDeploymentDetailApi = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
    deployId: string,
  ): Promise<Response> => {
    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const store = makeOmniRunStoreForEnv(env)
    const bundle = await store.findDeploymentForUser(
      session.user.userId,
      deployId,
    )

    if (bundle === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(publicDeploymentBundle(bundle)),
      session,
    )
  }

  const handleOmniDeploymentEventsApi = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
    deployId: string,
  ): Promise<Response> => {
    if (request.method === 'POST') {
      if (!(await dependencies.requireRunnerCallbackAuth(request, env))) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      const body = await readJsonObject(request).catch(
        (): Record<string, unknown> => ({}),
      )
      const events = Array.isArray(body.events)
        ? body.events.filter(isRecord)
        : [body].filter(isRecord)
      const store = makeOmniRunStoreForEnv(env)
      const fallbackStart =
        (await readDeploymentEventCursor(openAgentsDatabase(env), deployId)) + 1
      const runnerEvents = makeOmniRunnerEventService()
      const repository = makeOmniDeploymentRepository(store)
      const ingestResult = await Effect.runPromise(
        Effect.gen(function* () {
          const records = yield* runnerEvents.eventsFromCallbackPayloads(
            deployId,
            fallbackStart,
            events,
          )

          yield* repository.appendDeploymentEvents(
            deployId,
            records,
            deploymentStatusFromText(optionalString(body.status)),
            optionalString(body.externalDeployId),
          )

          return { records }
        }).pipe(
          Effect.withSpan('OmniRunnerCallback.ingestDeploymentEvents'),
          Effect.match({
            onFailure: failedOmniEffectResult,
            onSuccess: succeededOmniEffectResult,
          }),
        ),
      )

      if (ingestResult._tag === 'Failed') {
        return runnerCallbackIngestFailureResponse(
          ingestResult.error,
          events,
          fallbackStart,
        )
      }

      return noStoreJsonResponse({
        accepted: ingestResult.value.records.length,
        status: 'ok',
      })
    }

    if (request.method !== 'GET') {
      return methodNotAllowed(['GET', 'POST'])
    }

    return handleOmniDeploymentDetailApi(request, env, ctx, deployId)
  }

  return {
    buildOperatorAutopilotPreflightPayload: autopilotPreflightPayload,
    continueUserAutopilotRun,
    handleAutopilotFleetApi,
    handleAutopilotTokenLeaderboardsApi,
    handleOmniAgentRunDetailApi,
    handleOmniAgentRunEventsApi,
    handleOmniAgentRunsApi,
    handleOmniDeploymentDetailApi,
    handleOmniDeploymentEventsApi,
    handleOmniDeploymentsApi,
    handleOmniOperatorAgentRunDetailApi,
    handleOmniOperatorAgentRunsApi,
    handleOmniOperatorDeploymentsApi,
    handleOmniOperatorFleetApi,
    handleOmniOperatorTeamChatMessagesApi,
    launchUserAutopilotMission,
    requestGoalContinuationAfterCompletedRun,
  }
}

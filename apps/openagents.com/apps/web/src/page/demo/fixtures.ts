import { SyncPatch, SyncSnapshot } from '@openagentsinc/sync-schema'
import { Context, Effect, Layer, Schema as S } from 'effect'

import {
  AuthBootstrap,
  type OnboardingGitHubRepository,
  type OnboardingStatus,
  completedOnboardingStatus,
  emptyBillingSummary,
  emptyProviderAccountBundle,
  incompleteOnboardingStatus,
} from '../../domain/session'
import {
  AgentGoalResponse,
  type AgentRunApiEvent,
  AgentRunDetailResponse,
  AgentRunLaunchResponse,
  type CustomerOrder,
  type CustomerOrderAdjutantProgress,
  type CustomerOrderStatus,
  type TeamChatMessageRecord,
  TeamChatMessagesResponse,
  TeamChatPostResponse,
  type ThreadFileApiRecord,
  ThreadFileDetailResponse,
  ThreadFilesResponse,
  syncAgentRunScope,
  syncTeamScope,
} from '../loggedIn/model'

export const DEMO_TEAM_ID = 'team_openagents_core'
export const DEMO_TEAM_REF = 'openagents-core-team'
export const DEMO_PROJECT_ID = 'project_artanis'
export const DEMO_PROJECT_REF = 'artanis'
export const DEMO_RUN_ID = 'pylon-release-demo'
export const DEMO_FILE_PLAN_ID = 'file_pylon_release_plan'
export const DEMO_FILE_RESULT_ID = 'file_pylon_release_result'
export const DEMO_PROMPT =
  '@autopilot prepare the Pylon release briefing from the attached plan'
export const DEMO_ORDER_ID = 'software_order_beta_shopify_checkout'
export const DEMO_ORDER_GOAL =
  'Add Stripe credits checkout, show the order status page, and keep the first beta slice public.'

const DEMO_NOW = '2026-06-04T15:00:00.000Z'
const DEMO_USER_ID = 'user_demo_operator'
const DEMO_CUSTOMER_USER_ID = 'user_demo_customer'
const DEMO_AGENT_ID = 'agent_artanis'
const DEMO_GOAL_ID = 'agent_goal_pylon_release_demo'

class DemoFixtureDecodeError extends S.TaggedErrorClass<DemoFixtureDecodeError>()(
  'DemoFixtureDecodeError',
  { fixture: S.String, cause: S.Defect },
) {}

const decodeFixture = <A>(
  name: string,
  value: unknown,
  decode: (value: unknown) => A,
): Effect.Effect<A, DemoFixtureDecodeError> =>
  Effect.try({
    try: () => decode(value),
    catch: error => new DemoFixtureDecodeError({ fixture: name, cause: error }),
  })

const demoAuthor = {
  userId: DEMO_USER_ID,
  name: 'Chris Testerman',
  avatarUrl: null,
  githubUsername: 'chris-testerman',
}

const autopilotAuthor = {
  userId: DEMO_AGENT_ID,
  name: 'Artanis',
  avatarUrl: null,
  githubUsername: null,
}

export const demoAuthBootstrap = AuthBootstrap.make({
  session: {
    userId: DEMO_USER_ID,
    email: 'chris.testerman@openagents.demo',
    name: 'Chris Testerman',
    login: 'chris-testerman',
  },
  teams: [
    {
      id: DEMO_TEAM_ID,
      name: 'OpenAgents Core Team',
      slug: DEMO_TEAM_REF,
      role: 'owner',
      members: [
        {
          userId: DEMO_USER_ID,
          name: 'Chris Testerman',
          email: 'chris.testerman@openagents.demo',
          avatarUrl: null,
          githubUsername: 'chris-testerman',
          githubId: null,
          role: 'owner',
          status: 'active',
          joinedAt: DEMO_NOW,
        },
      ],
      projects: [
        {
          id: DEMO_PROJECT_ID,
          teamId: DEMO_TEAM_ID,
          name: 'Artanis',
          slug: DEMO_PROJECT_REF,
          description: 'Pylon release workroom',
          status: 'active',
          agent: {
            id: DEMO_AGENT_ID,
            name: 'Artanis',
            status: 'active',
            scope: 'project',
            runtime: 'opencode_codex',
            backend: 'shc_vm',
            repository: 'OpenAgentsInc/pylon-demo',
            focus: 'Release briefing',
          },
        },
      ],
    },
  ],
  billing: emptyBillingSummary(),
  onboarding: completedOnboardingStatus(),
  providerAccounts: emptyProviderAccountBundle(),
  isAdmin: false,
})

export const demoOrderRepository: OnboardingGitHubRepository = {
  id: 'github_repo_openagents_beta_shop',
  provider: 'github',
  owner: 'AcmeFoundry',
  name: 'beta-shop',
  fullName: 'AcmeFoundry/beta-shop',
  private: false,
  defaultBranch: 'main',
  htmlUrl: 'https://github.com/AcmeFoundry/beta-shop',
  description: 'Customer storefront for beta checkout and credits.',
}

const demoOrderNow = '2026-06-04T17:20:00.000Z'

export const demoCustomerAuthBootstrap = AuthBootstrap.make({
  session: {
    userId: DEMO_CUSTOMER_USER_ID,
    email: 'alex.customer@openagents.demo',
    name: 'Alex Customer',
    login: 'alex-customer',
  },
  teams: [],
  billing: emptyBillingSummary(),
  onboarding: incompleteOnboardingStatus(),
  providerAccounts: emptyProviderAccountBundle(),
  isAdmin: false,
})

export const demoOrderRepositoriesResponse = {
  repositories: [
    demoOrderRepository,
    {
      id: 'github_repo_acme_ops',
      provider: 'github' as const,
      owner: 'AcmeFoundry',
      name: 'ops-portal',
      fullName: 'AcmeFoundry/ops-portal',
      private: false,
      defaultBranch: 'main',
      htmlUrl: 'https://github.com/AcmeFoundry/ops-portal',
      description: 'Internal admin workflows and reporting.',
    },
    {
      id: 'github_repo_acme_docs',
      provider: 'github' as const,
      owner: 'AcmeFoundry',
      name: 'docs',
      fullName: 'AcmeFoundry/docs',
      private: false,
      defaultBranch: 'main',
      htmlUrl: 'https://github.com/AcmeFoundry/docs',
      description: 'Public documentation and customer setup notes.',
    },
  ],
  tokenStatus: 'available' as const,
}

const orderStatus = (
  step: OnboardingStatus['step'],
  input: Partial<OnboardingStatus> = {},
): OnboardingStatus => ({
  billing: { _tag: 'BillingPending' },
  completedAt: null,
  goal: null,
  repository: { _tag: 'RepositoryUnselected' },
  step,
  updatedAt: demoOrderNow,
  ...input,
})

export const demoOrderRepositorySelectedStatus = orderStatus('goal', {
  repository: {
    _tag: 'RepositorySelected',
    repository: demoOrderRepository,
    selectedAt: demoOrderNow,
  },
})

export const demoOrderGoalSubmittedStatus = orderStatus('billing', {
  goal: DEMO_ORDER_GOAL,
  repository: demoOrderRepositorySelectedStatus.repository,
})

export const demoOrderCompletedStatus = orderStatus('complete', {
  billing: { _tag: 'BillingSkipped', skippedAt: demoOrderNow },
  completedAt: demoOrderNow,
  goal: DEMO_ORDER_GOAL,
  repository: demoOrderRepositorySelectedStatus.repository,
})

const demoCustomerOrderAdjutant = (
  status: CustomerOrderStatus,
): CustomerOrderAdjutantProgress =>
  status === 'needs_customer_input'
    ? {
        activeUrl: null,
        adjustmentStatus: null,
        inputNeeded: true,
        nextAction: 'Reply with the details OpenAgents requested.',
        orderStatus: status,
        reviewNeeded: false,
        siteStatus: null,
        stage: 'waiting_for_input',
      }
    : status === 'agent_running'
      ? {
          activeUrl: null,
          adjustmentStatus: null,
          inputNeeded: false,
          nextAction: 'Autopilot is building the Site version.',
          orderStatus: status,
          reviewNeeded: false,
          siteStatus: null,
          stage: 'running',
        }
      : status === 'agent_queued' || status === 'submitted'
        ? {
            activeUrl: null,
            adjustmentStatus: null,
            inputNeeded: false,
            nextAction: 'Autopilot is queued for this order.',
            orderStatus: status,
            reviewNeeded: false,
            siteStatus: null,
            stage: 'queued',
          }
        : status === 'declined' || status === 'unavailable'
          ? {
              activeUrl: null,
              adjustmentStatus: null,
              inputNeeded: false,
              nextAction: 'OpenAgents cannot continue this order right now.',
              orderStatus: status,
              reviewNeeded: false,
              siteStatus: null,
              stage: 'unavailable',
            }
          : {
              activeUrl: null,
              adjustmentStatus: null,
              inputNeeded: false,
              nextAction:
                'OpenAgents is reviewing the generated Site before release.',
              orderStatus: status,
              reviewNeeded: true,
              siteStatus: null,
              stage: 'reviewing',
            }

export const demoCustomerOrder = (
  status: CustomerOrderStatus,
): CustomerOrder => ({
  id: DEMO_ORDER_ID,
  status,
  visibility: 'public',
  request: DEMO_ORDER_GOAL,
  repository: {
    provider: 'github',
    owner: demoOrderRepository.owner,
    name: demoOrderRepository.name,
    fullName: demoOrderRepository.fullName,
    private: demoOrderRepository.private,
    defaultBranch: demoOrderRepository.defaultBranch,
    htmlUrl: demoOrderRepository.htmlUrl,
  },
  site: null,
  triage: null,
  adjutant: demoCustomerOrderAdjutant(status),
  usageReceipts: [],
  usageSummary: {
    billingMode: 'public_beta_free',
    categories: [],
    totalCreditsChargedCents: 0,
    totalCreditsChargedFormatted: '$0.00',
  },
  publicWorkAcknowledgedAt: demoOrderNow,
  dataUseAcknowledgedAt: demoOrderNow,
  computePaymentAcknowledgedAt: demoOrderNow,
  providerAccountRequired: false,
  freeSliceCents: 5000,
  quoteCents: status === 'quote_ready' ? 30000 : null,
  createdAt: demoOrderNow,
  updatedAt:
    status === 'submitted'
      ? demoOrderNow
      : status === 'scoping'
        ? '2026-06-04T17:20:11.000Z'
        : status === 'agent_queued'
          ? '2026-06-04T17:20:12.500Z'
          : '2026-06-04T17:20:14.000Z',
})

const demoRun = {
  id: DEMO_RUN_ID,
  runtime: 'opencode_codex',
  backend: 'shc_vm',
  runnerId: 'oa-shc-katy-01',
  userId: DEMO_USER_ID,
  teamId: DEMO_TEAM_ID,
  projectId: DEMO_PROJECT_ID,
  repository: {
    provider: 'github',
    owner: 'OpenAgentsInc',
    repo: 'pylon-demo',
    ref: 'main',
  },
  goal: 'prepare the Pylon release briefing from the attached plan',
  externalRunId: 'demo-run-001',
  status: 'running',
  eventCursor: 4,
  createdAt: DEMO_NOW,
  updatedAt: '2026-06-04T15:00:05.000Z',
}

const event = (
  sequence: number,
  summary: string,
  type = 'runner.event',
  artifactRefs: ReadonlyArray<string> = [],
): AgentRunApiEvent & { readonly runId: string } => ({
  id: `demo_event_${sequence}`,
  parentId: DEMO_RUN_ID,
  runId: DEMO_RUN_ID,
  sequence,
  type,
  summary,
  status: sequence >= 7 ? 'complete' : 'running',
  source: 'autopilot-demo',
  payloadJson: JSON.stringify({
    tokenUsage: {
      provider: 'openai',
      model: 'codex-demo',
      input_tokens: 420 + sequence * 10,
      output_tokens: 100 + sequence * 8,
      reasoning_tokens: 40 + sequence * 2,
      total_tokens: 560 + sequence * 20,
    },
  }),
  artifactRefs: [...artifactRefs],
  externalEventId: `demo-external-event-${sequence}`,
  createdAt: `2026-06-04T15:00:${String(sequence).padStart(2, '0')}.000Z`,
})

const firstEvents = [
  event(1, 'Accepted project invocation'),
  event(2, 'Dispatched to oa-shc-katy-01'),
  event(3, 'Checked out OpenAgentsInc/pylon-demo'),
  event(4, 'Loaded pylon-release-plan.md'),
]

const completedEvents = [
  ...firstEvents,
  event(5, 'Completed release briefing shell command', 'tool.shell'),
  event(6, 'Wrote result.md', 'artifact.write', ['result.md']),
  event(7, 'Prepared concise release answer', 'assistant.result', [
    'result.md',
  ]),
]

const teamMessage: TeamChatMessageRecord = {
  id: 'team_message_pylon_invocation',
  teamId: DEMO_TEAM_ID,
  projectId: DEMO_PROJECT_ID,
  kind: 'autopilot_intent',
  body: DEMO_PROMPT,
  autopilotThreadId: DEMO_RUN_ID,
  agentRunId: DEMO_RUN_ID,
  createdAt: DEMO_NOW,
  author: demoAuthor,
}

const answerMessage: TeamChatMessageRecord = {
  id: 'team_message_pylon_answer',
  teamId: DEMO_TEAM_ID,
  projectId: DEMO_PROJECT_ID,
  kind: 'message',
  body: 'Release briefing ready: ship Pylon with the typed rollout checklist, publish result.md, and keep the team room as the source of record.',
  autopilotThreadId: DEMO_RUN_ID,
  agentRunId: DEMO_RUN_ID,
  createdAt: '2026-06-04T15:00:10.500Z',
  author: autopilotAuthor,
}

const runSummary = (status: 'running' | 'completed', eventCount: number) => ({
  runId: DEMO_RUN_ID,
  status,
  runtime: 'opencode_codex',
  backend: 'shc_vm',
  repository: 'OpenAgentsInc/pylon-demo',
  eventCount,
  toolCallCount: status === 'completed' ? 2 : 1,
  tokenTotal: status === 'completed' ? 4860 : 2140,
  durationSeconds: status === 'completed' ? 15 : null,
  updatedAt: status === 'completed' ? '2026-06-04T15:00:10.500Z' : DEMO_NOW,
})

const runWith = (status: 'running' | 'completed', eventCursor: number) => ({
  ...demoRun,
  status,
  eventCursor,
  updatedAt:
    status === 'completed' ? '2026-06-04T15:00:10.500Z' : demoRun.updatedAt,
})

const filePlan: ThreadFileApiRecord = {
  id: DEMO_FILE_PLAN_ID,
  scope: 'team',
  threadId: DEMO_RUN_ID,
  teamId: DEMO_TEAM_ID,
  ownerUserId: DEMO_USER_ID,
  filename: 'pylon-release-plan.md',
  contentType: 'text/markdown',
  sizeBytes: 2840,
  downloadUrl: '/demo2/files/pylon-release-plan.md',
  detailUrl: `/demo2/teams/${DEMO_TEAM_REF}/files/${DEMO_FILE_PLAN_ID}`,
  downloadEnabled: false,
  createdAt: DEMO_NOW,
}

const fileResult: ThreadFileApiRecord = {
  id: DEMO_FILE_RESULT_ID,
  scope: 'team',
  threadId: DEMO_RUN_ID,
  teamId: DEMO_TEAM_ID,
  ownerUserId: DEMO_USER_ID,
  filename: 'result.md',
  contentType: 'text/markdown',
  sizeBytes: 1210,
  downloadUrl: '/demo2/files/result.md',
  detailUrl: `/demo2/teams/${DEMO_TEAM_REF}/files/${DEMO_FILE_RESULT_ID}`,
  downloadEnabled: false,
  createdAt: '2026-06-04T15:00:08.500Z',
}

export const demoThreadFilesResponse = ThreadFilesResponse.make({
  files: [filePlan, fileResult],
})

export const demoThreadFileDetailResponse = ThreadFileDetailResponse.make({
  detail: {
    canManage: false,
    file: filePlan,
    references: [
      {
        id: 'file_ref_invocation',
        fileId: DEMO_FILE_PLAN_ID,
        teamId: DEMO_TEAM_ID,
        threadId: DEMO_RUN_ID,
        messageId: teamMessage.id,
        messageKind: 'team_chat',
        referenceKind: 'input',
        body: teamMessage.body,
        excerpt: 'prepare the Pylon release briefing from the attached plan',
        href: `/demo2/t/${DEMO_RUN_ID}`,
        createdAt: teamMessage.createdAt,
        author: demoAuthor,
      },
      {
        id: 'file_ref_answer',
        fileId: DEMO_FILE_PLAN_ID,
        teamId: DEMO_TEAM_ID,
        threadId: DEMO_RUN_ID,
        messageId: answerMessage.id,
        messageKind: 'team_chat',
        referenceKind: 'answer',
        body: answerMessage.body,
        excerpt: 'Release briefing ready',
        href: `/demo2/t/${DEMO_RUN_ID}`,
        createdAt: answerMessage.createdAt,
        author: autopilotAuthor,
      },
    ],
  },
})

export const demoEmptyTeamMessagesResponse = TeamChatMessagesResponse.make({
  teamId: DEMO_TEAM_ID,
  projectId: DEMO_PROJECT_ID,
  messages: [],
})

export const demoPostResponse = TeamChatPostResponse.make({
  teamId: DEMO_TEAM_ID,
  projectId: DEMO_PROJECT_ID,
  message: {
    ...teamMessage,
    runSummary: runSummary('running', 1),
  },
  run: runWith('running', 1),
  events: firstEvents.slice(0, 1),
  statusUrl: `/demo2/t/${DEMO_RUN_ID}`,
  streamUrl: `/demo2/t/${DEMO_RUN_ID}`,
  threadId: DEMO_RUN_ID,
  threadUrl: `/demo2/t/${DEMO_RUN_ID}`,
})

export const demoLaunchResponse = AgentRunLaunchResponse.make({
  run: runWith('running', 1),
  events: firstEvents.slice(0, 1),
  statusUrl: `/demo2/t/${DEMO_RUN_ID}`,
  streamUrl: `/demo2/t/${DEMO_RUN_ID}`,
})

export const demoActiveRunResponse = AgentRunDetailResponse.make({
  run: runWith('running', 4),
  events: firstEvents,
})

export const demoCompletedRunResponse = AgentRunDetailResponse.make({
  run: runWith('completed', 7),
  events: completedEvents,
})

export const demoGoalResponse = AgentGoalResponse.make({
  goal: {
    id: DEMO_GOAL_ID,
    agentId: DEMO_AGENT_ID,
    userId: null,
    teamId: DEMO_TEAM_ID,
    projectId: DEMO_PROJECT_ID,
    objective: 'Prepare the Pylon release briefing from the attached plan.',
    status: 'active',
    visibility: 'team',
    currentRunId: DEMO_RUN_ID,
    tokenBudget: 25000,
    tokensUsed: 4860,
    timeUsedSeconds: 15,
    remainingTokens: 20140,
    createdAt: DEMO_NOW,
    updatedAt: '2026-06-04T15:00:10.500Z',
    completedAt: null,
    pausedAt: null,
    blockedAt: null,
    canEdit: false,
    canPause: false,
    canResume: false,
    canMakePublic: false,
    publicUrl: null,
  },
})

export const demoRunSnapshot = (response = demoActiveRunResponse) =>
  S.decodeUnknownSync(SyncSnapshot)({
    scope: syncAgentRunScope(DEMO_RUN_ID),
    cursor: response.run.eventCursor,
    collections: {
      agent_runs: { [DEMO_RUN_ID]: response.run },
      agent_run_events: Object.fromEntries(
        response.events.map(event => [event.id, event]),
      ),
      agent_goals: { [DEMO_GOAL_ID]: demoGoalResponse.goal },
    },
  })

export const demoTeamSnapshot = (completed: boolean) =>
  S.decodeUnknownSync(SyncSnapshot)({
    scope: syncTeamScope(DEMO_TEAM_ID),
    cursor: completed ? 3 : 1,
    collections: {
      team_chat_messages: {
        [teamMessage.id]: {
          ...teamMessage,
          runSummary: runSummary(
            completed ? 'completed' : 'running',
            completed ? 7 : 4,
          ),
        },
        ...(completed
          ? {
              [answerMessage.id]: answerMessage,
            }
          : {}),
      },
      missions: {
        [DEMO_RUN_ID]: {
          title: 'Pylon release briefing',
          detail: 'OpenAgentsInc/pylon-demo - release briefing',
          href: `/demo2/t/${DEMO_RUN_ID}`,
          projectId: DEMO_PROJECT_ID,
          status: completed ? 'complete' : 'active',
        },
      },
    },
  })

export const demoCompletedRunPatch = S.decodeUnknownSync(SyncPatch)({
  scope: syncAgentRunScope(DEMO_RUN_ID),
  seq: 7,
  collection: 'agent_runs',
  op: 'put',
  id: DEMO_RUN_ID,
  value: demoCompletedRunResponse.run,
  serverTime: '2026-06-04T15:00:08.500Z',
})

export class DemoScenarioService extends Context.Service<
  DemoScenarioService,
  {
    readonly auth: () => Effect.Effect<AuthBootstrap, DemoFixtureDecodeError>
    readonly emptyMessages: () => Effect.Effect<
      TeamChatMessagesResponse,
      DemoFixtureDecodeError
    >
    readonly postResponse: () => Effect.Effect<
      TeamChatPostResponse,
      DemoFixtureDecodeError
    >
    readonly activeRun: () => Effect.Effect<
      AgentRunDetailResponse,
      DemoFixtureDecodeError
    >
    readonly completedRun: () => Effect.Effect<
      AgentRunDetailResponse,
      DemoFixtureDecodeError
    >
    readonly runSnapshot: () => Effect.Effect<
      SyncSnapshot,
      DemoFixtureDecodeError
    >
    readonly teamSnapshot: (
      completed: boolean,
    ) => Effect.Effect<SyncSnapshot, DemoFixtureDecodeError>
    readonly threadFiles: () => Effect.Effect<
      ThreadFilesResponse,
      DemoFixtureDecodeError
    >
    readonly threadFileDetail: () => Effect.Effect<
      ThreadFileDetailResponse,
      DemoFixtureDecodeError
    >
  }
>()('DemoScenarioService') {}

export const DemoScenarioLive = Layer.succeed(DemoScenarioService, {
  auth: Effect.fn('DemoScenarioService.auth')(() =>
    decodeFixture(
      'auth',
      demoAuthBootstrap,
      S.decodeUnknownSync(AuthBootstrap),
    ),
  ),
  emptyMessages: Effect.fn('DemoScenarioService.emptyMessages')(() =>
    decodeFixture(
      'emptyMessages',
      demoEmptyTeamMessagesResponse,
      S.decodeUnknownSync(TeamChatMessagesResponse),
    ),
  ),
  postResponse: Effect.fn('DemoScenarioService.postResponse')(() =>
    decodeFixture(
      'postResponse',
      demoPostResponse,
      S.decodeUnknownSync(TeamChatPostResponse),
    ),
  ),
  activeRun: Effect.fn('DemoScenarioService.activeRun')(() =>
    decodeFixture(
      'activeRun',
      demoActiveRunResponse,
      S.decodeUnknownSync(AgentRunDetailResponse),
    ),
  ),
  completedRun: Effect.fn('DemoScenarioService.completedRun')(() =>
    decodeFixture(
      'completedRun',
      demoCompletedRunResponse,
      S.decodeUnknownSync(AgentRunDetailResponse),
    ),
  ),
  runSnapshot: Effect.fn('DemoScenarioService.runSnapshot')(() =>
    decodeFixture(
      'runSnapshot',
      demoRunSnapshot(),
      S.decodeUnknownSync(SyncSnapshot),
    ),
  ),
  teamSnapshot: Effect.fn('DemoScenarioService.teamSnapshot')(
    (completed: boolean) =>
      decodeFixture(
        'teamSnapshot',
        demoTeamSnapshot(completed),
        S.decodeUnknownSync(SyncSnapshot),
      ),
  ),
  threadFiles: Effect.fn('DemoScenarioService.threadFiles')(() =>
    decodeFixture(
      'threadFiles',
      demoThreadFilesResponse,
      S.decodeUnknownSync(ThreadFilesResponse),
    ),
  ),
  threadFileDetail: Effect.fn('DemoScenarioService.threadFileDetail')(() =>
    decodeFixture(
      'threadFileDetail',
      demoThreadFileDetailResponse,
      S.decodeUnknownSync(ThreadFileDetailResponse),
    ),
  ),
})

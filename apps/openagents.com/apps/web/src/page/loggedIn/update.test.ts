import {
  ProviderAccountId,
  ProviderAccountRef,
  ProviderConnectionAttemptId,
  IsoTimestamp as ProviderIsoTimestamp,
} from '@openagentsinc/provider-account-schema'
import {
  CollectionName,
  CursorGap,
  EntityId,
  IsoTimestamp,
  SyncPatch,
  SyncScope,
  SyncSequence,
  SyncSnapshot,
} from '@openagentsinc/sync-schema'
import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OnboardingGitHubRepository,
  type OnboardingStatus,
  type ProviderDeviceLoginStartResponse,
  authBootstrapFromSession,
  incompleteOnboardingStatus,
} from '../../domain/session'
import {
  ChatRoute,
  DocsPageRoute,
  ForgeRoute,
  OnboardingRoute,
  SettingsRoute,
  SettingsSectionRoute,
  StatsRoute,
  TeamChatRoute,
  TeamFileRoute,
  TeamProjectChatRoute,
  ThreadRoute,
  WorkspaceRoute,
} from '../../route'
import {
  ClickedAgentGoalAction,
  ClickedEditAgentGoal,
  ClickedNewChat,
  ClickedOnboardingStep,
  ClickedPreviousOnboardingStep,
  ClickedSkipOnboardingBilling,
  ClickedStartProviderDeviceLogin,
  ClickedThreadFileDownload,
  ClickedThreadFileDownloadToggle,
  EnteredAutopilotRunRoute,
  FailedDownloadThreadFile,
  FailedLaunchAutopilotRun,
  ReceivedSyncCursorGap,
  ReceivedSyncPatch,
  RequestedLoadPrefilledWorkspace,
  RequestedLoadTokenUsageStats,
  RequestedPollAutopilotRun,
  SelectedForgeAutomationTemplate,
  SelectedOnboardingRepository,
  SubmittedAgentGoal,
  SubmittedChatComposer,
  SubmittedForgeAutomationRun,
  SubmittedOnboardingGoal,
  SubmittedOnboardingRepository,
  SubmittedThreadFileUpload,
  SucceededDownloadThreadFile,
  SucceededLaunchAutopilotRun,
  SucceededLoadAgentGoal,
  SucceededLoadOnboardingRepositories,
  SucceededLoadPrefilledWorkspace,
  SucceededLoadSyncSnapshot,
  SucceededLoadTeamChatMessages,
  SucceededLoadThreadFileDetail,
  SucceededLoadTokenUsageStats,
  SucceededPostTeamChatMessage,
  SucceededSelectOnboardingRepository,
  SucceededSkipOnboardingBilling,
  SucceededStartProviderDeviceLogin,
  SucceededSubmitOnboardingGoal,
  SucceededUpdateThreadFileDownload,
  UpdatedAgentGoalBudgetDraft,
  UpdatedAgentGoalObjectiveDraft,
  UpdatedOnboardingGoal,
  UpdatedOnboardingManualRepositoryName,
  UpdatedOnboardingManualRepositoryOwner,
  UpdatedTokenUsageStatsFilter,
} from './message'
import {
  ActiveChatRun,
  type AgentRunDetailResponse,
  type AgentRunLaunchResponse,
  type Model,
  type TeamChatMessagesResponse,
  type TeamChatPostResponse,
  type ThreadFileApiRecord,
  type ThreadFileDetailResponse,
  agentRunExternalRefFromNullable,
  init,
  threadFileDetailFromDto,
  threadFileRecordFromDto,
} from './model'
import { initialCommands, update } from './update'

const auth = authBootstrapFromSession({
  email: 'chris@openagents.com',
  name: 'Christopher David',
  userId: 'github:14167547',
})

const team = {
  id: 'team_openagents_core',
  name: 'OpenAgents Core Team',
  slug: 'openagents-core-team',
  role: 'owner',
  members: [
    {
      userId: 'github:14167547',
      name: 'Christopher David',
      email: 'chris@openagents.com',
      avatarUrl: null,
      githubUsername: 'AtlantisPleb',
      githubId: '14167547',
      role: 'owner',
      status: 'active',
      joinedAt: '2026-06-03T00:00:00.000Z',
    },
  ],
}

const authWithTeam = {
  ...auth,
  teams: [team],
}

const authWithIncompleteOnboarding = {
  ...authWithTeam,
  onboarding: incompleteOnboardingStatus(),
}

const onboardingRepository = {
  defaultBranch: 'main',
  description: 'Foldkit and Effect application',
  fullName: 'OpenAgentsInc/autopilot-omega',
  htmlUrl: 'https://github.com/OpenAgentsInc/autopilot-omega',
  id: 'repo_omega',
  name: 'autopilot-omega',
  owner: 'OpenAgentsInc',
  private: true,
  provider: 'github',
} satisfies OnboardingGitHubRepository

const onboardingRepositorySelected = {
  billing: { _tag: 'BillingPending' },
  completedAt: null,
  goal: null,
  repository: {
    _tag: 'RepositorySelected',
    repository: onboardingRepository,
    selectedAt: '2026-06-04T00:00:00.000Z',
  },
  step: 'goal',
  updatedAt: '2026-06-04T00:00:00.000Z',
} satisfies OnboardingStatus

const onboardingGoalSubmitted = {
  ...onboardingRepositorySelected,
  goal: 'Review the first production task',
  step: 'billing',
  updatedAt: '2026-06-04T00:00:01.000Z',
} satisfies OnboardingStatus

const onboardingComplete = {
  ...onboardingGoalSubmitted,
  billing: {
    _tag: 'BillingSkipped',
    skippedAt: '2026-06-04T00:00:02.000Z',
  },
  completedAt: '2026-06-04T00:00:02.000Z',
  step: 'complete',
  updatedAt: '2026-06-04T00:00:02.000Z',
} satisfies OnboardingStatus

const artanisProject = {
  agent: {
    backend: 'SHC',
    focus: 'Pylon',
    id: 'agent_artanis',
    name: 'Artanis',
    repository: 'autopilot-omega',
    runtime: 'Autopilot',
    scope: 'project',
    status: 'active',
  },
  id: 'project_artanis',
  teamId: 'team_openagents_core',
  name: 'Artanis',
  slug: 'artanis',
  description: '',
  status: 'active' as const,
}

const authWithProject = {
  ...auth,
  teams: [{ ...team, projects: [artanisProject] }],
}

const adjutantProject = {
  agent: {
    backend: 'SHC',
    focus: 'Sites',
    id: 'agent_adjutant',
    name: 'Autopilot',
    repository: 'autopilot-omega',
    runtime: 'Autopilot',
    scope: 'project',
    status: 'active',
  },
  id: 'project_adjutant',
  teamId: 'team_openagents_core',
  name: 'Autopilot',
  slug: 'adjutant',
  description: '',
  status: 'active' as const,
}

const authWithAdjutantProject = {
  ...auth,
  teams: [{ ...team, projects: [adjutantProject] }],
}

const activeGoal = {
  id: 'goal_1',
  agentId: 'autopilot',
  userId: 'github:14167547',
  teamId: null,
  projectId: null,
  objective: 'Keep the workroom moving',
  status: 'active' as const,
  visibility: 'private' as const,
  currentRunId: null,
  tokenBudget: 1000,
  tokensUsed: 250,
  timeUsedSeconds: 12,
  remainingTokens: 750,
  createdAt: '2026-06-04T00:00:00.000Z',
  updatedAt: '2026-06-04T00:00:01.000Z',
  completedAt: null,
  pausedAt: null,
  blockedAt: null,
  canEdit: true,
  canPause: true,
  canResume: false,
  canMakePublic: true,
  publicUrl: null,
}

const providerDeviceLoginStartResponse = {
  account: {
    accountLabel: 'chris@openagents.com',
    authMode: 'chatgpt_device_code',
    createdAt: ProviderIsoTimestamp.make('2026-06-03T00:00:00.000Z'),
    hasSecretRef: true,
    health: 'requires_reauth',
    id: ProviderAccountId.make('provider_account_1'),
    lastStatusAt: ProviderIsoTimestamp.make('2026-06-03T00:00:03.000Z'),
    provider: 'chatgpt_codex',
    providerAccountRef: ProviderAccountRef.make('provider-account_1'),
    publicStatus: 'unhealthy',
    status: 'unhealthy',
    updatedAt: ProviderIsoTimestamp.make('2026-06-03T00:00:03.000Z'),
  },
  attempt: {
    createdAt: ProviderIsoTimestamp.make('2026-06-03T00:00:03.000Z'),
    expiresAt: ProviderIsoTimestamp.make('2026-06-03T00:10:03.000Z'),
    id: ProviderConnectionAttemptId.make('provider_attempt_1'),
    method: 'chatgpt_device_code',
    provider: 'chatgpt_codex',
    providerAccountId: ProviderAccountId.make('provider_account_1'),
    providerAccountRef: ProviderAccountRef.make('provider-account_1'),
    source: 'worker_device_code',
    status: 'pending',
    updatedAt: ProviderIsoTimestamp.make('2026-06-03T00:00:03.000Z'),
    userCode: 'ABCD-EFGH',
    verificationUrl: 'https://chatgpt.com/activate',
  },
  expiresAt: '2026-06-03T00:10:03.000Z',
  intervalSeconds: 5,
  providerAccountRef: 'provider-account_1',
  userCode: 'ABCD-EFGH',
  verificationUrl: 'https://chatgpt.com/activate',
} satisfies ProviderDeviceLoginStartResponse

const launchResponse = {
  run: {
    id: 'agent_run_1',
    runtime: 'opencode_codex',
    backend: 'shc_vm',
    runnerId: 'oa-shc-katy-01',
    repository: {
      provider: 'github',
      owner: 'OpenAgentsInc',
      repo: 'autopilot-omega',
      ref: 'main',
    },
    goal: 'Run the smoke test',
    externalRunId: 'shc:oa-shc-katy-01:agent_run_1',
    status: 'running',
    eventCursor: 2,
    createdAt: '2026-06-03T00:00:00.000Z',
    updatedAt: '2026-06-03T00:00:01.000Z',
  },
  events: [
    {
      id: 'event_1',
      parentId: 'agent_run_1',
      sequence: 1,
      type: 'agent_run.accepted',
      summary:
        'OpenAgents accepted the Autopilot assignment for computer dispatch.',
      status: 'queued',
      source: 'openagents',
      payloadJson: null,
      artifactRefs: [],
      externalEventId: null,
      createdAt: '2026-06-03T00:00:00.000Z',
    },
    {
      id: 'event_2',
      parentId: 'agent_run_1',
      sequence: 2,
      type: 'runner.dispatched',
      summary: 'OpenAgents dispatched the assignment to the computer.',
      status: 'running',
      source: 'shc',
      payloadJson:
        '{"usage":{"provider":"openai","model":"gpt-5","totalTokens":42}}',
      artifactRefs: ['result.md'],
      externalEventId: 'shc-event-2',
      createdAt: '2026-06-03T00:00:01.000Z',
    },
  ],
  statusUrl: '/api/omni/agent-runs/agent_run_1',
  streamUrl: '/api/omni/agent-runs/agent_run_1/events',
} satisfies AgentRunLaunchResponse

const queuedLaunchResponse = {
  ...launchResponse,
  events: launchResponse.events.slice(0, 1),
  run: {
    ...launchResponse.run,
    eventCursor: 1,
    externalRunId: null,
    status: 'queued',
    updatedAt: '2026-06-03T00:00:00.000Z',
  },
} satisfies AgentRunLaunchResponse

const teamChatMessage = {
  id: 'team_chat_1',
  teamId: 'team_openagents_core',
  kind: 'message',
  body: 'hello team',
  autopilotThreadId: null,
  agentRunId: null,
  createdAt: '2026-06-03T00:00:00.000Z',
  author: {
    userId: 'github:14167547',
    name: 'Christopher David',
    avatarUrl: null,
    githubUsername: 'AtlantisPleb',
  },
} satisfies TeamChatMessagesResponse['messages'][number]

const teamChatMessagesResponse = {
  teamId: 'team_openagents_core',
  messages: [teamChatMessage],
} satisfies TeamChatMessagesResponse

const teamChatPostResponse = {
  teamId: 'team_openagents_core',
  message: {
    ...teamChatMessage,
    id: 'team_chat_2',
    body: 'new team message',
    createdAt: '2026-06-03T00:00:01.000Z',
  },
} satisfies TeamChatPostResponse

const teamAutopilotPostResponse = {
  teamId: 'team_openagents_core',
  message: {
    ...teamChatMessage,
    id: 'team_chat_3',
    kind: 'autopilot_intent',
    body: `@autopilot ${launchResponse.run.goal}`,
    autopilotThreadId: '8a8f5061-9648-45c8-a216-86a42f85cf12',
    agentRunId: launchResponse.run.id,
    createdAt: '2026-06-03T00:00:02.000Z',
  },
  run: launchResponse.run,
  events: launchResponse.events,
  statusUrl: launchResponse.statusUrl,
  streamUrl: launchResponse.streamUrl,
  threadId: '8a8f5061-9648-45c8-a216-86a42f85cf12',
  threadUrl: '/t/agent_run_1',
} satisfies TeamChatPostResponse

const blockedProviderAccountMessage =
  'chris@openagents.com cannot launch Autopilot. OpenAI invalidated the saved ChatGPT login. Reconnect ChatGPT in Settings -> Connections.'

const teamAutopilotLaunchBlockedPostResponse = {
  teamId: 'team_openagents_core',
  launchError: blockedProviderAccountMessage,
  message: {
    ...teamChatMessage,
    id: 'team_chat_4',
    kind: 'autopilot_intent',
    body: '@autopilot & identify yourself',
    autopilotThreadId: '8a8f5061-9648-45c8-a216-86a42f85cf12',
    agentRunId: null,
    createdAt: '2026-06-03T00:00:03.000Z',
    launchError: blockedProviderAccountMessage,
  },
} satisfies TeamChatPostResponse

const teamThreadFileDto = {
  id: 'file_1',
  scope: 'team',
  threadId: 'team:team_openagents_core:chat',
  teamId: 'team_openagents_core',
  ownerUserId: 'github:14167547',
  filename: 'notes.txt',
  contentType: 'text/plain',
  sizeBytes: 12,
  downloadUrl: '/api/thread-files/file_1/download',
  createdAt: '2026-06-03T00:00:03.000Z',
} satisfies ThreadFileApiRecord

const teamThreadFile = threadFileRecordFromDto(teamThreadFileDto)

const teamThreadFileDetailResponse = {
  detail: {
    canManage: true,
    file: {
      ...teamThreadFileDto,
      detailUrl: '/teams/openagents-core-team/files/file_1',
      downloadEnabled: true,
    },
    references: [
      {
        author: {
          avatarUrl: null,
          githubUsername: 'AtlantisPleb',
          name: 'Christopher David',
          userId: 'github:14167547',
        },
        body: 'Please inspect notes.txt',
        createdAt: '2026-06-03T00:00:04.000Z',
        excerpt: 'Please inspect notes.txt',
        fileId: 'file_1',
        href: '/teams/openagents-core-team/chat#message-team_chat_1',
        id: 'thread_file_message_ref_1',
        messageId: 'team_chat_1',
        messageKind: 'message',
        referenceKind: 'message_attachment',
        teamId: 'team_openagents_core',
        threadId: 'team:team_openagents_core:chat',
      },
    ],
  },
} satisfies ThreadFileDetailResponse

const activeRun = ActiveChatRun({
  events: [],
  metadata: {
    backend: 'shc_vm',
    createdAt: '2026-06-03T00:00:00.000Z',
    eventCursor: 2,
    externalRunRef: agentRunExternalRefFromNullable(
      'shc:oa-shc-katy-01:agent_run_1',
    ),
    goal: 'Run tests',
    repository: 'OpenAgentsInc/autopilot-omega@main',
    runId: 'agent_run_1',
    displayRunId: 'agent_run_1',
    runnerId: 'oa-shc-katy-01',
    runtime: 'opencode_codex',
    status: 'running',
    statusUrl: '/api/omni/agent-runs/agent_run_1',
    streamUrl: '/api/omni/agent-runs/agent_run_1/events',
    tokenTotal: 0,
    tokenUsageEvents: 0,
    updatedAt: '2026-06-03T00:00:01.000Z',
  },
})

const legacyRouteRunId = 'f1f1bd76-fdb6-42c6-b0b6-d82d92f84212'
const legacyStoredRunId = 'agent_run_f1f1bd76fdb642c6b0b6d82d92f84212'
const workspaceScope = 'workspace:github:14167547'
const legacyThreadScope = `thread:${legacyRouteRunId}`
const activeAgentRunScope = 'agent-run:agent_run_1'

const legacyDetailResponse = {
  run: {
    id: legacyStoredRunId,
    runtime: 'opencode_codex',
    backend: 'shc_vm',
    runnerId: 'oa-shc-katy-01',
    repository: {
      provider: 'github',
      owner: 'OpenAgentsInc',
      repo: 'autopilot-omega',
      ref: 'main',
    },
    goal: 'Investigate sidebar mission history',
    externalRunId: `shc:oa-shc-katy-01:${legacyStoredRunId}`,
    status: 'completed',
    eventCursor: 2,
    createdAt: '2026-06-03T00:00:00.000Z',
    updatedAt: '2026-06-03T00:00:02.000Z',
  },
  events: [
    {
      id: 'event_legacy_1',
      parentId: legacyStoredRunId,
      sequence: 1,
      type: 'agent_run.accepted',
      summary:
        'OpenAgents accepted the Autopilot assignment for computer dispatch.',
      status: 'queued',
      source: 'openagents',
      payloadJson: null,
      artifactRefs: [],
      externalEventId: null,
      createdAt: '2026-06-03T00:00:00.000Z',
    },
    {
      id: 'event_legacy_2',
      parentId: legacyStoredRunId,
      sequence: 2,
      type: 'runner.completed',
      summary: 'Computer completed the saved mission.',
      status: 'completed',
      source: 'shc',
      payloadJson:
        '{"usage":{"provider":"openai","model":"gpt-5","totalTokens":84}}',
      artifactRefs: ['result.md'],
      externalEventId: 'shc-event-legacy-2',
      createdAt: '2026-06-03T00:00:02.000Z',
    },
  ],
} satisfies AgentRunDetailResponse

const missionRow = (overrides: Record<string, unknown> = {}) => ({
  active: false,
  attention: false,
  detail: 'autopilot-omega - queued',
  href: '/t/mission-1',
  owner: 'personal',
  status: 'queued',
  title: 'Sync-backed mission',
  ...overrides,
})

const sidebarSection = (model: Model, title: string) =>
  model.sidebar.sessionSections.find(section => section.title === title)

const syncSnapshot = (
  collections: Record<string, Record<string, unknown>>,
  cursor = 1,
  scope = workspaceScope,
) =>
  new SyncSnapshot({
    scope: SyncScope.make(scope),
    cursor: SyncSequence.make(cursor),
    collections,
  })

const syncPatch = (input: {
  collection: string
  id: string
  op: SyncPatch['op']
  patch?: unknown
  scope?: string
  value?: unknown
}) =>
  new SyncPatch({
    scope: SyncScope.make(input.scope ?? workspaceScope),
    seq: SyncSequence.make(2),
    collection: CollectionName.make(input.collection),
    op: input.op,
    id: EntityId.make(input.id),
    ...(input.value === undefined ? {} : { value: input.value }),
    ...(input.patch === undefined ? {} : { patch: input.patch }),
    serverTime: IsoTimestamp.make('2026-06-03T00:00:02.000Z'),
  })

const syncRunProjection = (
  response: AgentRunDetailResponse | AgentRunLaunchResponse,
) => ({
  backend: response.run.backend,
  completedAt: null,
  createdAt: response.run.createdAt,
  eventCursor: response.run.eventCursor,
  externalRunId: response.run.externalRunId,
  failedAt: null,
  goal: response.run.goal,
  id: response.run.id,
  repository: response.run.repository,
  routeId:
    response.run.id === legacyStoredRunId ? legacyRouteRunId : response.run.id,
  runnerId: response.run.runnerId,
  runtime: response.run.runtime,
  startedAt: null,
  status: response.run.status,
  teamId: null,
  updatedAt: response.run.updatedAt,
  userId: auth.session.userId,
})

const syncEventProjection = (
  event: AgentRunDetailResponse['events'][number],
) => ({
  artifactRefs: event.artifactRefs,
  createdAt: event.createdAt,
  externalEventId: event.externalEventId,
  id: event.id,
  payloadJson: event.payloadJson,
  runId: event.parentId,
  sequence: event.sequence,
  source: event.source,
  status: event.status,
  summary: event.summary,
  type: event.type,
})

describe('logged-in Autopilot chat runs', () => {
  test('loads repositories when the authenticated onboarding flow opens', () => {
    const model = init(OnboardingRoute(), authWithIncompleteOnboarding)

    expect(initialCommands(model).map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadOnboardingRepositories',
    ])
  })

  test('loads Forge automation templates into the tuned work-order draft', () => {
    const model = init(ForgeRoute(), authWithTeam)
    const [loadedModel, commands] = update(
      model,
      SelectedForgeAutomationTemplate({
        automationId: 'forge.automation.triage_scope',
      }),
    )

    expect(commands).toEqual([])
    expect(loadedModel.autopilotWorkComposer._tag).toBe(
      'AutopilotWorkComposerIdle',
    )
    expect(loadedModel.autopilotWorkComposerDraft).toMatchObject({
      branch: 'main',
      maxSpendCents: '0',
      repositoryFullName: 'OpenAgentsInc/openagents',
      verificationCommand: 'bun run check:deploy',
    })
    expect(loadedModel.autopilotWorkComposerDraft.objective).toContain(
      'forge.automation.triage_scope',
    )
  })

  test('submits Forge automation runs through the Autopilot work-order command', () => {
    const model = init(ForgeRoute(), authWithTeam)
    const [submittingModel, commands] = update(
      model,
      SubmittedForgeAutomationRun({
        automationId: 'forge.automation.validate_gate',
      }),
    )

    expect(submittingModel.autopilotWorkComposer._tag).toBe(
      'AutopilotWorkComposerSubmitting',
    )
    expect(submittingModel.autopilotWorkComposerDraft.objective).toContain(
      'forge.automation.validate_gate',
    )
    expect(commands.map(command => command.name)).toEqual([
      'SubmitAutopilotWorkComposer',
    ])
    expect(commands[0]?.args).toMatchObject({
      draft: {
        branch: 'main',
        maxSpendCents: '0',
        repositoryFullName: 'OpenAgentsInc/openagents',
        verificationCommand: 'bun run check:deploy',
      },
    })
  })

  test('walks repository selection, goal submission, and billing skip', () => {
    const model = init(OnboardingRoute(), authWithIncompleteOnboarding)
    const [loadedModel] = update(
      model,
      SucceededLoadOnboardingRepositories({
        response: {
          repositories: [onboardingRepository],
          tokenStatus: 'available',
        },
      }),
    )
    const [selectedModel] = update(
      loadedModel,
      SelectedOnboardingRepository({ repositoryId: onboardingRepository.id }),
    )
    const [submittingRepositoryModel, repositoryCommands] = update(
      selectedModel,
      SubmittedOnboardingRepository(),
    )
    const [goalModel] = update(
      submittingRepositoryModel,
      SucceededSelectOnboardingRepository({
        response: { onboarding: onboardingRepositorySelected },
      }),
    )
    const [goalValueModel] = update(
      goalModel,
      UpdatedOnboardingGoal({ value: 'Review the first production task' }),
    )
    const [submittingGoalModel, goalCommands, submitOutMessage] = update(
      goalValueModel,
      SubmittedOnboardingGoal(),
    )
    const [billingModel] = update(
      submittingGoalModel,
      SucceededSubmitOnboardingGoal({
        response: { onboarding: onboardingGoalSubmitted },
      }),
    )
    const [submittingBillingModel, billingCommands] = update(
      billingModel,
      ClickedSkipOnboardingBilling(),
    )
    const [completeModel, completeCommands, completeOutMessage] = update(
      submittingBillingModel,
      SucceededSkipOnboardingBilling({
        response: { onboarding: onboardingComplete },
      }),
    )

    expect(loadedModel.onboarding.selectedRepositoryId).toBe(
      onboardingRepository.id,
    )
    expect(repositoryCommands.map(command => command.name)).toEqual([
      'SelectOnboardingRepository',
    ])
    expect(repositoryCommands[0]?.args).toEqual({
      selection: { repositoryId: onboardingRepository.id },
    })
    expect(goalModel.auth.onboarding.step).toBe('goal')
    expect(goalModel.onboarding.manualRepositoryName).toBe('')
    expect(goalModel.onboarding.manualRepositoryOwner).toBe('')
    expect(goalModel.onboarding.selectedRepositoryId).toBe(
      onboardingRepository.id,
    )
    expect(goalCommands.map(command => command.name)).toEqual([
      'SubmitOnboardingGoal',
    ])
    expect(goalCommands[0]?.args).toEqual({
      goal: 'Review the first production task',
    })
    expect(submitOutMessage).toEqual(Option.none())
    expect(billingModel.auth.onboarding.step).toBe('billing')
    expect(billingCommands.map(command => command.name)).toEqual([
      'SkipOnboardingBilling',
    ])
    expect(completeCommands).toEqual([])
    expect(completeOutMessage).toMatchObject({
      _tag: 'Some',
      value: { _tag: 'CompletedOnboarding' },
    })
    expect(completeModel.auth.onboarding.step).toBe('complete')
  })

  test('submits manual owner and repository during onboarding', () => {
    const model = init(OnboardingRoute(), authWithIncompleteOnboarding)
    const [ownerModel] = update(
      model,
      UpdatedOnboardingManualRepositoryOwner({ value: 'OpenAgentsInc' }),
    )
    const [nameModel] = update(
      ownerModel,
      UpdatedOnboardingManualRepositoryName({ value: 'autopilot-omega' }),
    )
    const [submittingModel, commands] = update(
      nameModel,
      SubmittedOnboardingRepository(),
    )

    expect(submittingModel.onboarding.action).toMatchObject({
      _tag: 'OnboardingActionSubmitting',
      label: 'Saving repository',
    })
    expect(commands.map(command => command.name)).toEqual([
      'SelectOnboardingRepository',
    ])
    expect(commands[0]?.args).toEqual({
      selection: {
        name: 'autopilot-omega',
        owner: 'OpenAgentsInc',
      },
    })
  })

  test('settings repository update clears stale draft after success', () => {
    const nextRepository = {
      ...onboardingRepository,
      fullName: 'OpenAgentsInc/control',
      id: 'repo_control',
      name: 'control',
    } satisfies OnboardingGitHubRepository
    const nextOnboarding = {
      ...onboardingRepositorySelected,
      repository: {
        _tag: 'RepositorySelected',
        repository: nextRepository,
        selectedAt: '2026-06-04T00:00:03.000Z',
      },
      updatedAt: '2026-06-04T00:00:03.000Z',
    } satisfies OnboardingStatus
    const model = init(SettingsSectionRoute({ section: 'connections' }), {
      ...auth,
      onboarding: onboardingRepositorySelected,
    })
    const [ownerModel] = update(
      model,
      UpdatedOnboardingManualRepositoryOwner({ value: 'OpenAgentsInc' }),
    )
    const [nameModel] = update(
      ownerModel,
      UpdatedOnboardingManualRepositoryName({ value: 'control' }),
    )
    const [submittingModel, commands] = update(
      nameModel,
      SubmittedOnboardingRepository(),
    )
    const [savedModel] = update(
      submittingModel,
      SucceededSelectOnboardingRepository({
        response: { onboarding: nextOnboarding },
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'UpdateOnboardingRepository',
    ])
    expect(savedModel.auth.onboarding.repository).toMatchObject({
      _tag: 'RepositorySelected',
      repository: nextRepository,
    })
    expect(savedModel.onboarding.manualRepositoryName).toBe('')
    expect(savedModel.onboarding.manualRepositoryOwner).toBe('')
    expect(savedModel.onboarding.selectedRepositoryId).toBe(nextRepository.id)
  })

  test('moves onboarding backward one step', () => {
    const goalModel = init(OnboardingRoute(), {
      ...authWithIncompleteOnboarding,
      onboarding: onboardingRepositorySelected,
    })
    const [repositoryModel, repositoryCommands] = update(
      goalModel,
      ClickedPreviousOnboardingStep(),
    )
    const billingModel = init(OnboardingRoute(), {
      ...authWithIncompleteOnboarding,
      onboarding: onboardingGoalSubmitted,
    })
    const [goalModelAgain, goalCommands] = update(
      billingModel,
      ClickedPreviousOnboardingStep(),
    )

    expect(repositoryModel.auth.onboarding.step).toBe('repository')
    expect(repositoryModel.onboarding.action).toEqual({
      _tag: 'OnboardingActionIdle',
    })
    expect(repositoryCommands.map(command => command.name)).toEqual([
      'LoadOnboardingRepositories',
    ])
    expect(goalModelAgain.auth.onboarding.step).toBe('goal')
    expect(goalCommands).toEqual([])
  })

  test('jumps onboarding to a clicked setup step', () => {
    const billingModel = init(OnboardingRoute(), {
      ...authWithIncompleteOnboarding,
      onboarding: onboardingGoalSubmitted,
    })
    const [repositoryModel, repositoryCommands] = update(
      billingModel,
      ClickedOnboardingStep({ step: 'repository' }),
    )
    const [goalModel, goalCommands] = update(
      repositoryModel,
      ClickedOnboardingStep({ step: 'goal' }),
    )

    expect(repositoryModel.auth.onboarding.step).toBe('repository')
    expect(repositoryModel.onboarding.action).toEqual({
      _tag: 'OnboardingActionIdle',
    })
    expect(repositoryCommands.map(command => command.name)).toEqual([
      'LoadOnboardingRepositories',
    ])
    expect(goalModel.auth.onboarding.step).toBe('goal')
    expect(goalCommands).toEqual([])
  })

  test('does not launch chat while onboarding is incomplete', () => {
    const [nextModel, commands, outMessage] = update(
      {
        ...init(ChatRoute(), authWithIncompleteOnboarding),
        chatComposerValue: 'Run the smoke test',
      },
      SubmittedChatComposer(),
    )

    expect(nextModel.chatComposerValue).toBe('Run the smoke test')
    expect(commands).toEqual([])
    expect(outMessage).toEqual(Option.none())
  })

  test('clicking new chat clears the current transcript and focuses the composer', () => {
    const model = init(
      DocsPageRoute({ slug: 'get-paid-to-code' }),
      authWithTeam,
    )
    const [nextModel, commands, outMessage] = update(
      {
        ...model,
        chatComposerValue: 'draft',
        chatMessages: [
          {
            author: 'user',
            body: 'hello',
            id: 'user-turn-1',
            label: 'Christopher David',
            status: 'complete',
          },
        ],
        chatRun: activeRun,
      },
      ClickedNewChat(),
    )

    expect(commands.map(command => command.name)).toEqual(['FocusChatComposer'])
    expect(outMessage).toEqual(Option.none())
    expect(nextModel.chatComposerValue).toBe('')
    expect(nextModel.chatMessages).toEqual([])
    expect(nextModel.chatRun).toEqual({ _tag: 'Idle' })
    expect(nextModel.route).toEqual(ChatRoute())
  })

  test('submitting a composer value launches a real SHC Autopilot run', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [submittedModel, commands, outMessage] = update(
      { ...model, chatComposerValue: 'Run the smoke test' },
      SubmittedChatComposer(),
    )

    expect(commands.map(command => command.name)).toEqual([
      'LaunchAutopilotRun',
      'ScrollChatTimelineToEnd',
      'FocusChatComposer',
    ])
    expect(outMessage).toEqual(Option.none())
    expect(submittedModel.chatComposerValue).toBe('')
    expect(submittedModel.chatMessages).toHaveLength(1)
    expect(submittedModel.chatMessages[0]).toMatchObject({
      author: 'user',
      body: 'Run the smoke test',
      status: 'complete',
    })
    expect(submittedModel.chatRun).toMatchObject({
      _tag: 'Launching',
      prompt: 'Run the smoke test',
      requestId: 'chat-request-1',
    })
  })

  test('loads the current personal Autopilot goal for chat workrooms', () => {
    const model = init(ChatRoute(), authWithTeam)
    const commands = initialCommands(model)

    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'LoadAgentGoal',
      'LoadThreadFiles',
      'FocusChatComposer',
      'RequestNotificationPermission',
    ])
    expect(commands[2]?.args).toEqual({
      agentId: 'autopilot',
      href: '/api/autopilot/goals/current?agentId=autopilot',
      scopeKey: 'autopilot:personal:room',
    })
  })

  test('sets, edits, and pauses a workroom goal through the goal reducer', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [loadedModel] = update(
      model,
      SucceededLoadAgentGoal({
        response: { goal: activeGoal },
        scopeKey: 'autopilot:personal:room',
      }),
    )
    const [editingModel] = update(loadedModel, ClickedEditAgentGoal())
    const [objectiveModel] = update(
      editingModel,
      UpdatedAgentGoalObjectiveDraft({ value: 'Ship the goal UI' }),
    )
    const [budgetModel] = update(
      objectiveModel,
      UpdatedAgentGoalBudgetDraft({ value: '2000' }),
    )
    const [submittedModel, commands] = update(budgetModel, SubmittedAgentGoal())
    const [pausedModel, pauseCommands] = update(
      loadedModel,
      ClickedAgentGoalAction({ action: 'pause' }),
    )

    expect(Option.getOrThrow(loadedModel.agentGoalPanel.goal)).toEqual(
      activeGoal,
    )
    expect(editingModel.agentGoalPanel.isEditing).toBe(true)
    expect(commands.map(command => command.name)).toEqual(['SaveAgentGoal'])
    expect(commands[0]?.args).toMatchObject({
      agentId: 'autopilot',
      goalId: 'goal_1',
      objective: 'Ship the goal UI',
      scopeKey: 'autopilot:personal:room',
      tokenBudget: 2000,
    })
    expect(submittedModel.agentGoalPanel.pendingAction).toEqual(
      Option.some('Saving'),
    )
    expect(pauseCommands.map(command => command.name)).toEqual([
      'UpdateAgentGoalAction',
    ])
    expect(pauseCommands[0]?.args).toEqual({
      action: 'pause',
      goalId: 'goal_1',
      scopeKey: 'autopilot:personal:room',
    })
    expect(pausedModel.agentGoalPanel.pendingAction).toEqual(
      Option.some('pause'),
    )
  })

  test('applies live sync goal patches to the current goal panel', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [loadedModel] = update(
      model,
      SucceededLoadAgentGoal({
        response: { goal: activeGoal },
        scopeKey: 'autopilot:personal:room',
      }),
    )
    const [syncedModel] = update(
      loadedModel,
      ReceivedSyncPatch({
        patch: new SyncPatch({
          scope: SyncScope.make('workspace:github:14167547'),
          seq: SyncSequence.make(1),
          collection: CollectionName.make('agent_goals'),
          op: 'put',
          id: EntityId.make('goal_1'),
          value: {
            ...activeGoal,
            status: 'budget_limited',
            tokensUsed: 1000,
            remainingTokens: 0,
            canPause: false,
            canResume: true,
          },
          serverTime: IsoTimestamp.make('2026-06-04T00:00:02.000Z'),
        }),
      }),
    )

    expect(Option.getOrThrow(syncedModel.agentGoalPanel.goal)).toMatchObject({
      id: 'goal_1',
      status: 'budget_limited',
      tokensUsed: 1000,
      remainingTokens: 0,
      canResume: true,
    })
  })

  test('starting ChatGPT device login records the pending provider account and polls', () => {
    const model = init(SettingsRoute(), auth)
    const [startingModel, startingCommands, startingOutMessage] = update(
      model,
      ClickedStartProviderDeviceLogin({
        providerAccountRef: 'provider-account_1',
      }),
    )
    const [pendingModel, pendingCommands, pendingOutMessage] = update(
      startingModel,
      SucceededStartProviderDeviceLogin({
        response: providerDeviceLoginStartResponse,
      }),
    )

    expect(startingModel.providerConnectionAction).toEqual({
      _tag: 'ProviderConnectionStarting',
    })
    expect(startingCommands.map(command => command.name)).toEqual([
      'StartProviderDeviceLogin',
    ])
    expect(startingCommands[0]?.args).toEqual({
      providerAccountRef: 'provider-account_1',
    })
    expect(startingOutMessage).toEqual(Option.none())
    expect(pendingModel.auth.providerAccounts?.accounts).toEqual([
      providerDeviceLoginStartResponse.account,
    ])
    expect(pendingModel.auth.providerAccounts?.attempts).toEqual([
      providerDeviceLoginStartResponse.attempt,
    ])
    expect(pendingModel.providerConnectionAction).toEqual({
      _tag: 'ProviderConnectionPolling',
      attemptId: 'provider_attempt_1',
    })
    expect(pendingCommands.map(command => command.name)).toEqual([
      'PollProviderDeviceLogin',
    ])
    expect(pendingCommands[0]?.args).toEqual({
      attemptId: 'provider_attempt_1',
      delayMillis: 5000,
    })
    expect(pendingOutMessage).toEqual(Option.none())
  })

  test('adding a ChatGPT account requests a new provider account slot', () => {
    const model = init(SettingsRoute(), auth)
    const [, commands, outMessage] = update(
      model,
      ClickedStartProviderDeviceLogin({ createNew: true }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'StartProviderDeviceLogin',
    ])
    expect(commands[0]?.args).toEqual({ createNew: true })
    expect(outMessage).toEqual(Option.none())
  })

  test('opening Stats loads canonical token usage and applies filters', () => {
    const response = {
      schemaVersion: 'openagents.token_usage_aggregate.v1' as const,
      byActor: [],
      byProviderModel: [],
      bySourceRoute: [],
      bySourceRef: [],
      byUsageTruth: [],
      filters: { provider: 'google_gemini' },
      generatedAt: '2026-06-08T12:00:00.000Z',
      recentEvents: [],
      totals: {
        cacheReadTokens: 5,
        cacheWrite1hTokens: 0,
        cacheWrite5mTokens: 0,
        inputTokens: 50,
        outputTokens: 25,
        reasoningTokens: 10,
        totalTokens: 90,
      },
      usageEvents: 1,
    }
    const leaderboards = {
      schemaVersion: 'openagents.token_usage_leaderboards.v1' as const,
      anonymousTotals: response.totals,
      filters: { window: '7d' as const },
      generatedAt: '2026-06-08T12:00:00.000Z',
      globalTotals: response.totals,
      topProviderModels: [],
      topProjects: [],
      topRuns: [],
      topTeams: [],
      topUsers: [],
    }
    const preference = {
      schemaVersion:
        'openagents.token_usage_leaderboard_preference.v1' as const,
      preference: {
        leaderboardParticipation: 'eligible' as const,
        leaderboardVisibility: 'internal' as const,
        subjectKind: 'user' as const,
        subjectRef: 'github:14167547',
        updatedAt: '2026-06-08T12:00:00.000Z',
        updatedByUserId: 'github:14167547',
      },
    }
    const analytics = {
      schemaVersion: 'openagents.inference_analytics.v1' as const,
      window: '7d' as const,
      generatedAt: '2026-06-08T12:00:00.000Z',
      byProvider: [],
      bySupplyLane: [],
      byAdapter: [],
      byModel: [],
      byRoute: [],
      byGlmReplica: [],
      byRequestClass: [],
      byDemandKind: [],
      byDemandSource: [],
      byDemandClient: [
        {
          key: 'external:sdk',
          label: 'external / sdk',
          inputTokens: 50,
          outputTokens: 25,
          totalTokens: 75,
          usageEvents: 1,
          costUsd: 0.01,
          costCoverage: 1,
        },
      ],
      byDay: [
        {
          day: '2026-06-08',
          inputTokens: 50,
          outputTokens: 25,
          totalTokens: 75,
          usageEvents: 1,
          costUsd: 0.01,
        },
      ],
      byDemandClientDay: [
        {
          day: '2026-06-08',
          key: 'external:sdk',
          label: 'external / sdk',
          inputTokens: 50,
          outputTokens: 25,
          totalTokens: 75,
          usageEvents: 1,
          costUsd: 0.01,
        },
      ],
      operational: {
        batchWaitMs: {
          averageMs: 'not_measured' as const,
          p50Ms: 'not_measured' as const,
          p90Ms: 'not_measured' as const,
          p99Ms: 'not_measured' as const,
          sampleCount: 0,
        },
        busyEvents: 0,
        fallbackEvents: 0,
        fallbackRate: 0,
        perceivedTokensPerSecond: {
          averageTokensPerSecond: 'not_measured' as const,
          p50TokensPerSecond: 'not_measured' as const,
          p90TokensPerSecond: 'not_measured' as const,
          p99TokensPerSecond: 'not_measured' as const,
          sampleCount: 0,
        },
        queueWaitMs: {
          averageMs: 'not_measured' as const,
          p50Ms: 'not_measured' as const,
          p90Ms: 'not_measured' as const,
          p99Ms: 'not_measured' as const,
          sampleCount: 0,
        },
        saturationEvents: 0,
        totalWallClockMs: {
          averageMs: 'not_measured' as const,
          p50Ms: 'not_measured' as const,
          p90Ms: 'not_measured' as const,
          p99Ms: 'not_measured' as const,
          sampleCount: 0,
        },
        ttftMs: {
          averageMs: 'not_measured' as const,
          p50Ms: 'not_measured' as const,
          p90Ms: 'not_measured' as const,
          p99Ms: 'not_measured' as const,
          sampleCount: 0,
        },
      },
      glmReplicas: [],
      ownedHourly: {
        blockerRefs: [
          'blocker.inference_analytics.accepted_outcomes_not_measured',
          'blocker.inference_analytics.glm_benchmark_reserved_burn_not_measured',
          'blocker.inference_analytics.glm_keepwarm_burn_not_measured',
          'blocker.inference_analytics.glm_storage_overhead_not_measured',
          'blocker.inference_analytics.owned_hourly_host_lifecycle_derived_window_assumption',
        ],
        acceptedOutcomes: 'not_measured' as const,
        activeDemandBurnUsd: 0,
        activeServingHours: 0,
        benchmarkReservedBurnUsd: 'not_measured' as const,
        costCoverage: 'partial' as const,
        costPerAcceptedOutcomeUsd: 'not_measured' as const,
        demand: [],
        effectiveCostPerServedTokenUsd: 'not_measured' as const,
        externalDemandBurnUsd: 0,
        hourlyBurnUsd: 3.693151,
        idleBurnUsd: 620.449368,
        idleHours: 168,
        internalDemandBurnUsd: 0,
        keepWarmBurnUsd: 'not_measured' as const,
        monthlyBurnUsd: 2696,
        profiles: [
          {
            evidenceRefs: [
              'evidence.gcp.g4_standard_192.spot_usd_2696_month.2026_06_25',
              'evidence.gcp.g4_standard_192.ondemand_usd_13140_month.2026_06_25',
              'evidence.gcp.g4_standard_192.dws_flex_usd_6570_month.2026_06_25',
              'evidence.gcp.g4_standard_384.spot_usd_5392_month.2026_06_25',
              'evidence.gcp.g4_standard_384.ondemand_usd_26280_month.2026_06_25',
              'evidence.gcp.g4_standard_384.dws_flex_usd_13140_month.2026_06_25',
            ],
            gpuCount: 4,
            hourlyComputeUsd: 3.693151,
            hourlyStorageOverheadUsd: 'not_measured' as const,
            machineShape: 'g4-standard-192',
            modelRef: 'openagents/glm-5.2-reap-504b',
            monthlyComputeUsd: 2696,
            monthlyStorageOverheadUsd: 'not_measured' as const,
            profileRef:
              'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.2026_06_25',
            provisioningModel: 'spot' as const,
            sourceRef: 'evidence.gcp.g4_gpu_costs.2026_06_25.owner_estimate',
            supplyLane: 'hydralisk',
          },
        ],
        scenarios: [],
        storageOverheadUsd: 'not_measured' as const,
        unlabeledDemandBurnUsd: 0,
        uptimeHours: 168,
        windowBurnUsd: 620.449368,
      },
      totals: {
        inputTokens: 50,
        outputTokens: 25,
        totalTokens: 75,
        usageEvents: 1,
        costUsd: 0.01,
        costCoverage: 1,
      },
    }
    const model = init(StatsRoute(), { ...authWithTeam, isAdmin: true })
    const commands = initialCommands(model)
    const [filteredModel] = update(
      model,
      UpdatedTokenUsageStatsFilter({
        field: 'provider',
        value: 'google_gemini',
      }),
    )
    const [loadingModel, loadCommands] = update(
      filteredModel,
      RequestedLoadTokenUsageStats(),
    )
    const [loadedModel] = update(
      loadingModel,
      SucceededLoadTokenUsageStats({
        analytics,
        filters: loadingModel.tokenUsageStats.filters,
        leaderboards,
        preference,
        response,
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadTokenUsageStats',
    ])
    expect(filteredModel.tokenUsageStats.filters.provider).toBe('google_gemini')
    expect(loadCommands.map(command => command.name)).toEqual([
      'LoadTokenUsageStats',
    ])
    expect(loadCommands[0]?.args).toMatchObject({
      filters: { provider: 'google_gemini' },
    })
    expect(loadedModel.tokenUsageStats).toMatchObject({
      _tag: 'TokenUsageStatsLoaded',
      analytics: { byDemandClient: [{ key: 'external:sdk' }] },
      response: { usageEvents: 1 },
    })
  })

  test('loads prefilled workspace invites for signed-in holders', () => {
    const model = init(
      WorkspaceRoute({ workspaceId: 'workspace_seed' }),
      authWithTeam,
    )
    const commands = initialCommands(model)
    const [loadingModel, loadCommands] = update(
      model,
      RequestedLoadPrefilledWorkspace({ workspaceId: 'workspace_seed' }),
    )
    const [loadedModel] = update(
      loadingModel,
      SucceededLoadPrefilledWorkspace({
        response: {
          generatedAt: '2026-06-16T12:00:00.000Z',
          viewer: 'holder',
          workspace: {
            id: 'workspace_seed',
            projectName: 'Seeded Storefront Sprint',
            status: 'invited',
            seededMemory: [
              {
                label: 'Website',
                value: 'Public catalog is live.',
                publicSourceRef: 'https://example.com',
              },
            ],
            starterWorkflows: [
              {
                title: 'Draft product page update',
                description: 'Create the first accepted-outcome draft.',
                outcomeKind: 'draft',
                status: 'ready',
              },
            ],
            introReceipt: {
              summary: 'Workspace prepared from public sources.',
              publicSourceRefs: ['https://example.com'],
            },
          },
        },
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadPrefilledWorkspace',
    ])
    expect(commands[1]?.args).toEqual({ workspaceId: 'workspace_seed' })
    expect(loadCommands.map(command => command.name)).toEqual([
      'LoadPrefilledWorkspace',
    ])
    expect(loadedModel.prefilledWorkspace).toMatchObject({
      _tag: 'PrefilledWorkspaceLoaded',
      workspace: {
        id: 'workspace_seed',
        projectName: 'Seeded Storefront Sprint',
      },
    })
  })

  test('opening a team room loads durable team chat history', () => {
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const commands = initialCommands(model)
    const [nextModel] = update(
      model,
      SucceededLoadTeamChatMessages({
        response: teamChatMessagesResponse,
        roomKey: 'team_openagents_core',
        teamId: 'team_openagents_core',
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'LoadAgentGoal',
      'LoadTeamChatMessages',
      'LoadThreadFiles',
      'FocusChatComposer',
      'RequestNotificationPermission',
    ])
    expect(commands[3]?.args).toEqual({
      href: '/api/teams/team_openagents_core/chat/messages',
      roomKey: 'team_openagents_core',
      teamId: 'team_openagents_core',
    })
    expect(nextModel.teamChatMessagesByTeam.team_openagents_core).toEqual([
      teamChatMessage,
    ])
  })

  test('opening a disabled project room does not load project APIs', () => {
    const model = init(
      TeamProjectChatRoute({
        projectRef: 'artanis',
        teamRef: 'openagents-core-team',
      }),
      authWithProject,
    )
    const commands = initialCommands(model)

    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'FocusChatComposer',
      'RequestNotificationPermission',
    ])
  })

  test('plain team messages post durable chat and do not launch Autopilot directly', () => {
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const [submittedModel, commands, outMessage] = update(
      { ...model, chatComposerValue: 'hello team' },
      SubmittedChatComposer(),
    )

    expect(commands.map(command => command.name)).toEqual([
      'PostTeamChatMessage',
      'ScrollChatTimelineToEnd',
      'FocusChatComposer',
    ])
    expect(commands[0]?.args).toEqual({
      body: 'hello team',
      kind: 'message',
      requestId: 'team-chat-request-1',
      roomKey: 'team_openagents_core',
      teamId: 'team_openagents_core',
    })
    expect(commands.map(command => command.name)).not.toContain(
      'LaunchAutopilotRun',
    )
    expect(outMessage).toEqual(Option.none())
    expect(submittedModel.chatComposerValue).toBe('')
    expect(submittedModel.chatMessages).toEqual([])
    expect(submittedModel.chatRun).toEqual({ _tag: 'Idle' })
  })

  test('project messages fail locally while project workrooms are disabled', () => {
    const model = init(
      TeamProjectChatRoute({
        projectRef: 'artanis',
        teamRef: 'openagents-core-team',
      }),
      authWithProject,
    )
    const [, commands] = update(
      { ...model, chatComposerValue: '@autopilot Introduce Artanis' },
      SubmittedChatComposer(),
    )

    expect(commands.map(command => command.name)).toEqual([
      'ScrollChatTimelineToEnd',
      'FocusChatComposer',
    ])
  })

  test('Autopilot project messages post typed supervisor intent without launching a generic run', () => {
    const model = init(
      TeamProjectChatRoute({
        projectRef: 'adjutant',
        teamRef: 'openagents-core-team',
      }),
      authWithAdjutantProject,
    )
    const body =
      '@autopilot softwareOrderId: software_order_otec Build the OTEC Site'
    const [submittedModel, commands] = update(
      { ...model, chatComposerValue: body },
      SubmittedChatComposer(),
    )

    expect(commands.map(command => command.name)).toEqual([
      'PostTeamChatMessage',
      'ScrollChatTimelineToEnd',
      'FocusChatComposer',
    ])
    expect(commands[0]?.args).toEqual({
      body,
      kind: 'adjutant_intent',
      prompt: 'softwareOrderId: software_order_otec Build the OTEC Site',
      projectId: 'project_adjutant',
      requestId: 'team-chat-request-1',
      roomKey: 'team_openagents_core:project:project_adjutant',
      teamId: 'team_openagents_core',
    })
    expect(commands.map(command => command.name)).not.toContain(
      'LaunchAutopilotRun',
    )
    expect(submittedModel.chatRun).toEqual({ _tag: 'Idle' })
  })

  test('plain team messages preserve a leading ampersand', () => {
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const [, commands] = update(
      { ...model, chatComposerValue: '& do you see this?' },
      SubmittedChatComposer(),
    )

    expect(commands[0]?.name).toBe('PostTeamChatMessage')
    expect(commands[0]?.args).toMatchObject({
      body: '& do you see this?',
      kind: 'message',
      roomKey: 'team_openagents_core',
      teamId: 'team_openagents_core',
    })
  })

  test('posted team messages append to durable team state', () => {
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const [nextModel, commands] = update(
      {
        ...model,
        teamChatMessagesByTeam: {
          team_openagents_core: [teamChatMessage],
        },
      },
      SucceededPostTeamChatMessage({
        requestId: 'team-chat-request-2',
        response: teamChatPostResponse,
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'ScrollChatTimelineToEnd',
    ])
    expect(
      nextModel.teamChatMessagesByTeam.team_openagents_core?.map(
        message => message.body,
      ),
    ).toEqual(['hello team', 'new team message'])
  })

  test('exact @autopilot team messages launch through the team chat API', () => {
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const [submittedModel, commands] = update(
      { ...model, chatComposerValue: '@autopilot Run the smoke test' },
      SubmittedChatComposer(),
    )
    const [launchedModel, launchCommands] = update(
      submittedModel,
      SucceededPostTeamChatMessage({
        requestId: 'team-chat-request-1',
        response: teamAutopilotPostResponse,
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'PostTeamChatMessage',
      'ScrollChatTimelineToEnd',
      'FocusChatComposer',
    ])
    expect(commands[0]?.args).toEqual({
      body: '@autopilot Run the smoke test',
      kind: 'autopilot_intent',
      prompt: 'Run the smoke test',
      requestId: 'team-chat-request-1',
      roomKey: 'team_openagents_core',
      teamId: 'team_openagents_core',
    })
    expect(commands.map(command => command.name)).not.toContain(
      'LaunchAutopilotRun',
    )
    expect(submittedModel.chatRun).toMatchObject({
      _tag: 'Launching',
      prompt: 'Run the smoke test',
      requestId: 'team-chat-request-1',
    })
    expect(launchCommands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'ScrollChatTimelineToEnd',
    ])
    expect(launchedModel.chatRun).toMatchObject({
      _tag: 'Active',
      metadata: {
        goal: 'Run the smoke test',
        runId: 'agent_run_1',
      },
    })
    expect(
      launchedModel.teamChatMessagesByTeam.team_openagents_core?.[0],
    ).toMatchObject({
      agentRunId: 'agent_run_1',
      body: '@autopilot Run the smoke test',
      kind: 'autopilot_intent',
    })
  })

  test('trailing @autopilot team messages launch with the visible body preserved', () => {
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const body = 'Introduce yourself. What are your capabilities? @autopilot'
    const [submittedModel, commands] = update(
      { ...model, chatComposerValue: body },
      SubmittedChatComposer(),
    )

    expect(commands.map(command => command.name)).toEqual([
      'PostTeamChatMessage',
      'ScrollChatTimelineToEnd',
      'FocusChatComposer',
    ])
    expect(commands[0]?.args).toEqual({
      body,
      kind: 'autopilot_intent',
      prompt: 'Introduce yourself. What are your capabilities?',
      requestId: 'team-chat-request-1',
      roomKey: 'team_openagents_core',
      teamId: 'team_openagents_core',
    })
    expect(submittedModel.chatRun).toMatchObject({
      _tag: 'Launching',
      prompt: 'Introduce yourself. What are your capabilities?',
      requestId: 'team-chat-request-1',
    })
  })

  test('standalone @autopilot line launches with the surrounding message as prompt', () => {
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const body = 'Introduce yourself. What are your capabilities?\n@autopilot'
    const [, commands] = update(
      { ...model, chatComposerValue: body },
      SubmittedChatComposer(),
    )

    expect(commands[0]?.args).toEqual({
      body,
      kind: 'autopilot_intent',
      prompt: 'Introduce yourself. What are your capabilities?',
      requestId: 'team-chat-request-1',
      roomKey: 'team_openagents_core',
      teamId: 'team_openagents_core',
    })
  })

  test('blocked team Autopilot launches keep the submitted intent visible', () => {
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const [submittedModel, commands] = update(
      { ...model, chatComposerValue: '@autopilot & identify yourself' },
      SubmittedChatComposer(),
    )
    const [blockedModel, blockedCommands] = update(
      submittedModel,
      SucceededPostTeamChatMessage({
        requestId: 'team-chat-request-1',
        response: teamAutopilotLaunchBlockedPostResponse,
      }),
    )

    expect(commands[0]?.args).toMatchObject({
      body: '@autopilot & identify yourself',
      kind: 'autopilot_intent',
      prompt: '& identify yourself',
      roomKey: 'team_openagents_core',
      teamId: 'team_openagents_core',
    })
    expect(
      blockedModel.teamChatMessagesByTeam.team_openagents_core?.[0],
    ).toMatchObject({
      agentRunId: null,
      body: '@autopilot & identify yourself',
      kind: 'autopilot_intent',
      launchError: blockedProviderAccountMessage,
    })
    expect(blockedModel.chatRun).toEqual({
      _tag: 'Failed',
      error: blockedProviderAccountMessage,
    })
    expect(blockedCommands.map(command => command.name)).toEqual([
      'ScrollChatTimelineToEnd',
      'FocusChatComposer',
    ])
  })

  test('successful launch records source and token metadata from runner events', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [submittedModel] = update(
      { ...model, chatComposerValue: 'Run the smoke test' },
      SubmittedChatComposer(),
    )
    const [nextModel, commands] = update(
      submittedModel,
      SucceededLaunchAutopilotRun({
        requestId: 'chat-request-1',
        response: launchResponse,
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'SetAutopilotThreadUrl',
      'LoadSyncSnapshot',
      'ScrollChatTimelineToEnd',
    ])
    expect(commands[0]?.args).toEqual({
      threadId: 'agent_run_1',
    })
    expect(commands[1]?.args).toEqual({
      href: '/api/sync/agent-run/agent_run_1/snapshot',
      scope: activeAgentRunScope,
    })
    expect(nextModel.chatRun).toMatchObject({
      _tag: 'Active',
      metadata: {
        backend: 'shc_vm',
        externalRunRef: {
          _tag: 'AgentRunExternalRefPresent',
          value: 'shc:oa-shc-katy-01:agent_run_1',
        },
        repository: 'OpenAgentsInc/autopilot-omega@main',
        runId: 'agent_run_1',
        displayRunId: 'agent_run_1',
        runtime: 'opencode_codex',
        status: 'running',
        tokenTotal: 42,
        tokenUsageEvents: 1,
      },
    })
  })

  test('successful launch immediately adds the mission to the sidebar', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [submittedModel] = update(
      { ...model, chatComposerValue: 'Run the smoke test' },
      SubmittedChatComposer(),
    )
    const [nextModel] = update(
      submittedModel,
      SucceededLaunchAutopilotRun({
        requestId: 'chat-request-1',
        response: launchResponse,
      }),
    )

    const missions = sidebarSection(nextModel, 'My threads')

    expect(missions?.items[0]).toMatchObject({
      attention: true,
      detail: 'autopilot-omega - running',
      href: '/t/agent_run_1',
      owner: 'personal',
      status: 'active',
      title: 'Run the smoke test',
    })
  })

  test('sync snapshot projects mission rows into the sidebar and cursor store', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [nextModel, commands] = update(
      model,
      SucceededLoadSyncSnapshot({
        scope: workspaceScope,
        snapshot: syncSnapshot({
          missions: {
            'mission-1': missionRow(),
          },
        }),
      }),
    )

    const missions = sidebarSection(nextModel, 'My threads')

    expect(commands).toEqual([])
    expect(nextModel.sync.cursors[workspaceScope]).toBe(1)
    expect(missions?.items).toEqual([
      {
        active: false,
        attention: false,
        detail: 'autopilot-omega - queued',
        href: '/t/mission-1',
        owner: 'personal',
        status: 'queued',
        title: 'Sync-backed mission',
      },
    ])
  })

  test('team sync snapshot keeps team-owned missions in Team threads', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [nextModel] = update(
      model,
      SucceededLoadSyncSnapshot({
        scope: 'team:team_openagents_core',
        snapshot: syncSnapshot(
          {
            missions: {
              'team-mission-1': missionRow({
                href: '/t/team-mission-1',
                owner: 'team',
                teamId: 'team_openagents_core',
                title: 'Gemini ImageGen support',
              }),
            },
          },
          4,
          'team:team_openagents_core',
        ),
      }),
    )

    const missions = sidebarSection(nextModel, 'Team threads')

    expect(missions?.items).toEqual([
      {
        active: false,
        attention: false,
        detail: 'autopilot-omega - queued',
        href: '/t/team-mission-1',
        owner: 'team',
        status: 'queued',
        teamId: 'team_openagents_core',
        title: 'Gemini ImageGen support',
      },
    ])
    expect(sidebarSection(nextModel, 'My threads')).toBeUndefined()
  })

  test('workspace and team mission snapshots preserve each other by ownership', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [withTeamThreads] = update(
      model,
      SucceededLoadSyncSnapshot({
        scope: 'team:team_openagents_core',
        snapshot: syncSnapshot(
          {
            missions: {
              'team-mission-1': missionRow({
                href: '/t/team-mission-1',
                owner: 'team',
                teamId: 'team_openagents_core',
                title: 'Team owned mission',
              }),
            },
          },
          4,
          'team:team_openagents_core',
        ),
      }),
    )
    const [withBothSections] = update(
      withTeamThreads,
      SucceededLoadSyncSnapshot({
        scope: workspaceScope,
        snapshot: syncSnapshot({
          missions: {
            'mission-1': missionRow({
              href: '/t/mission-1',
              title: 'Personal mission',
            }),
          },
        }),
      }),
    )

    expect(
      sidebarSection(withBothSections, 'My threads')?.items.map(
        item => item.href,
      ),
    ).toEqual(['/t/mission-1'])
    expect(
      sidebarSection(withBothSections, 'Team threads')?.items.map(
        item => item.href,
      ),
    ).toEqual(['/t/team-mission-1'])
  })

  test('sync snapshot hides project mission rows while project workrooms are disabled', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [nextModel] = update(
      model,
      SucceededLoadSyncSnapshot({
        scope: workspaceScope,
        snapshot: syncSnapshot({
          missions: {
            'mission-1': missionRow(),
            'mission-2': missionRow({
              href: '/t/project-mission-2',
              owner: 'project',
              projectId: 'project_artanis',
              teamId: 'team_openagents_core',
              title: 'Artanis project smoke: verify answer-back',
            }),
          },
        }),
      }),
    )

    const missions = sidebarSection(nextModel, 'My threads')

    expect(missions?.items).toEqual([
      {
        active: false,
        attention: false,
        detail: 'autopilot-omega - queued',
        href: '/t/mission-1',
        owner: 'personal',
        status: 'queued',
        title: 'Sync-backed mission',
      },
    ])
    expect(sidebarSection(nextModel, 'Team threads')).toBeUndefined()
  })

  test('sync snapshot keeps the current project mission route visible', () => {
    const model = init(
      ThreadRoute({ threadId: 'project-mission-2' }),
      authWithTeam,
    )
    const [nextModel] = update(
      model,
      SucceededLoadSyncSnapshot({
        scope: workspaceScope,
        snapshot: syncSnapshot({
          missions: {
            'mission-1': missionRow(),
            'mission-2': missionRow({
              href: '/t/project-mission-2',
              owner: 'project',
              projectId: 'project_imagegen_support',
              teamId: 'team_openagents_core',
              title: 'Gemini ImageGen support',
            }),
          },
        }),
      }),
    )

    const myThreads = sidebarSection(nextModel, 'My threads')
    const teamThreads = sidebarSection(nextModel, 'Team threads')

    expect(myThreads?.items.map(item => item.href)).toEqual(['/t/mission-1'])
    expect(teamThreads?.items.map(item => item.href)).toEqual([
      '/t/project-mission-2',
    ])
  })

  test('sync patches upsert sidebar missions and update cursors', () => {
    const model = init(ChatRoute(), auth)
    const [nextModel] = update(
      model,
      ReceivedSyncPatch({
        patch: syncPatch({
          collection: 'missions',
          id: 'mission-2',
          op: 'put',
          value: missionRow({
            detail: 'autopilot-omega - running',
            href: '/t/mission-2',
            status: 'active',
            title: 'Live sync mission',
          }),
        }),
      }),
    )

    const missions = sidebarSection(nextModel, 'My threads')

    expect(nextModel.sync.cursors[workspaceScope]).toBe(2)
    expect(
      nextModel.sync.collectionByScope[workspaceScope]?.missions?.['mission-2'],
    ).toMatchObject({
      title: 'Live sync mission',
    })
    expect(missions?.items[0]).toMatchObject({
      attention: true,
      href: '/t/mission-2',
      owner: 'personal',
      status: 'active',
      title: 'Live sync mission',
    })
  })

  test('sync mission upserts sort sidebar missions by latest activity', () => {
    const model = init(ChatRoute(), auth)
    const [snapshotModel] = update(
      model,
      SucceededLoadSyncSnapshot({
        scope: workspaceScope,
        snapshot: syncSnapshot({
          missions: {
            'mission-1': missionRow({
              href: '/t/mission-1',
              title: 'First mission',
              updatedAt: '2026-06-03T00:00:01.000Z',
            }),
            'mission-2': missionRow({
              href: '/t/mission-2',
              title: 'Second mission',
              updatedAt: '2026-06-03T00:00:02.000Z',
            }),
          },
        }),
      }),
    )
    const [patchedModel] = update(
      snapshotModel,
      ReceivedSyncPatch({
        patch: syncPatch({
          collection: 'missions',
          id: 'mission-2',
          op: 'put',
          value: missionRow({
            href: '/t/mission-2',
            status: 'active',
            title: 'Second mission updated',
            updatedAt: '2026-06-03T00:00:03.000Z',
          }),
        }),
      }),
    )

    const missions = sidebarSection(patchedModel, 'My threads')

    expect(missions?.items.map(item => item.href)).toEqual([
      '/t/mission-2',
      '/t/mission-1',
    ])
    expect(missions?.items[0]?.title).toBe('Second mission updated')
  })

  test('team sync patches project team chat messages and files', () => {
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const [withMessage] = update(
      model,
      ReceivedSyncPatch({
        patch: syncPatch({
          collection: 'team_chat_messages',
          id: teamChatMessage.id,
          op: 'put',
          scope: 'team:team_openagents_core',
          value: teamChatMessage,
        }),
      }),
    )
    const [withFile] = update(
      withMessage,
      ReceivedSyncPatch({
        patch: syncPatch({
          collection: 'thread_files',
          id: teamThreadFile.id,
          op: 'put',
          scope: 'team:team_openagents_core',
          value: teamThreadFileDto,
        }),
      }),
    )

    expect(withMessage.teamChatMessagesByTeam.team_openagents_core).toEqual([
      teamChatMessage,
    ])
    expect(withFile.threadFilesByScope).toMatchObject({
      'team-files:team_openagents_core': [teamThreadFile],
      'thread:team:team_openagents_core:chat': [teamThreadFile],
    })
    expect(withFile.sync.cursors['team:team_openagents_core']).toBe(2)
  })

  test('file upload commands carry the selected file from the event boundary', () => {
    const file = new File(['hello'], 'hello.txt', { type: 'text/plain' })
    const model = init(
      TeamChatRoute({ teamRef: 'openagents-core-team' }),
      authWithTeam,
    )
    const [nextModel, commands] = update(
      model,
      SubmittedThreadFileUpload({
        file,
        inputId: 'thread-file-upload-team',
        scopeKey: 'thread:team:team_openagents_core:chat',
        teamId: 'team_openagents_core',
        threadId: 'team:team_openagents_core:chat',
      }),
    )

    expect(nextModel.threadFileUpload).toMatchObject({
      _tag: 'ThreadFileUploading',
      scopeKey: 'thread:team:team_openagents_core:chat',
    })
    expect(commands.map(command => command.name)).toEqual(['UploadThreadFile'])
    expect(commands[0]?.args).toMatchObject({
      file,
      inputId: 'thread-file-upload-team',
      scopeKey: 'thread:team:team_openagents_core:chat',
      teamId: 'team_openagents_core',
      threadId: 'team:team_openagents_core:chat',
    })
  })

  test('team file detail routes load the file detail API', () => {
    const model = init(
      TeamFileRoute({
        fileId: 'file_1',
        teamRef: 'openagents-core-team',
      }),
      authWithTeam,
    )
    const commands = initialCommands(model)

    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'LoadThreadFileDetail',
      'RequestNotificationPermission',
    ])
    expect(commands[2]?.args).toEqual({
      fileId: 'file_1',
      href: '/api/thread-files/file_1?teamId=team_openagents_core',
    })
  })

  test('loaded file details are cached and reflected in file lists', () => {
    const model = {
      ...init(
        TeamFileRoute({
          fileId: 'file_1',
          teamRef: 'openagents-core-team',
        }),
        authWithTeam,
      ),
      threadFileDetailErrorsById: {
        file_1: 'stale failure',
      },
      threadFilesByScope: {
        'team-files:team_openagents_core': [teamThreadFile],
        'thread:team:team_openagents_core:chat': [teamThreadFile],
      },
    }
    const [nextModel, commands] = update(
      model,
      SucceededLoadThreadFileDetail({
        fileId: 'file_1',
        response: teamThreadFileDetailResponse,
      }),
    )

    expect(commands).toEqual([])
    expect(nextModel.threadFileDetailErrorsById.file_1).toBeUndefined()
    expect(nextModel.threadFileDetailsById.file_1).toEqual(
      threadFileDetailFromDto(teamThreadFileDetailResponse.detail),
    )
    expect(
      nextModel.threadFilesByScope['team-files:team_openagents_core']?.[0],
    ).toMatchObject({
      detailUrl: '/teams/openagents-core-team/files/file_1',
      downloadEnabled: true,
    })
    expect(
      nextModel.threadFilesByScope[
        'thread:team:team_openagents_core:chat'
      ]?.[0],
    ).toMatchObject({
      detailUrl: '/teams/openagents-core-team/files/file_1',
      downloadEnabled: true,
    })
  })

  test('download toggle requests the file detail patch API', () => {
    const model = init(
      TeamFileRoute({
        fileId: 'file_1',
        teamRef: 'openagents-core-team',
      }),
      authWithTeam,
    )
    const [_nextModel, commands] = update(
      model,
      ClickedThreadFileDownloadToggle({
        downloadEnabled: false,
        fileId: 'file_1',
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'UpdateThreadFileDownload',
    ])
    expect(commands[0]?.args).toEqual({
      downloadEnabled: false,
      fileId: 'file_1',
    })
  })

  test('file download requests the protected API through an app command', () => {
    const model = {
      ...init(
        TeamFileRoute({
          fileId: 'file_1',
          teamRef: 'openagents-core-team',
        }),
        authWithTeam,
      ),
      threadFileDownloadErrorsById: {
        file_1: 'Previous error',
      },
    }
    const [nextModel, commands] = update(
      model,
      ClickedThreadFileDownload({
        downloadUrl: '/api/thread-files/file_1/download',
        fileId: 'file_1',
        filename: 'notes.txt',
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'DownloadThreadFile',
    ])
    expect(commands[0]?.args).toEqual({
      downloadUrl: '/api/thread-files/file_1/download',
      fileId: 'file_1',
      filename: 'notes.txt',
    })
    expect(nextModel.threadFileDownloadErrorsById.file_1).toBeUndefined()
  })

  test('file download completion updates the page error state', () => {
    const model = {
      ...init(
        TeamFileRoute({
          fileId: 'file_1',
          teamRef: 'openagents-core-team',
        }),
        authWithTeam,
      ),
      threadFileDownloadErrorsById: {},
    }
    const [failedModel] = update(
      model,
      FailedDownloadThreadFile({
        error: 'File object not found.',
        fileId: 'file_1',
      }),
    )
    const [succeededModel] = update(
      failedModel,
      SucceededDownloadThreadFile({ fileId: 'file_1' }),
    )

    expect(failedModel.threadFileDownloadErrorsById.file_1).toBe(
      'File object not found.',
    )
    expect(succeededModel.threadFileDownloadErrorsById.file_1).toBeUndefined()
  })

  test('download toggle results update the detail cache and file lists', () => {
    const disabledDetailResponse = {
      detail: {
        ...teamThreadFileDetailResponse.detail,
        file: {
          ...teamThreadFileDetailResponse.detail.file,
          downloadEnabled: false,
        },
      },
    } satisfies ThreadFileDetailResponse
    const model = {
      ...init(
        TeamFileRoute({
          fileId: 'file_1',
          teamRef: 'openagents-core-team',
        }),
        authWithTeam,
      ),
      threadFileDetailsById: {
        file_1: threadFileDetailFromDto(teamThreadFileDetailResponse.detail),
      },
      threadFilesByScope: {
        'team-files:team_openagents_core': [
          threadFileRecordFromDto(teamThreadFileDetailResponse.detail.file),
        ],
      },
    }
    const [nextModel, commands] = update(
      model,
      SucceededUpdateThreadFileDownload({
        fileId: 'file_1',
        response: disabledDetailResponse,
      }),
    )

    expect(commands).toEqual([])
    expect(nextModel.threadFileDetailsById.file_1?.file.downloadEnabled).toBe(
      false,
    )
    expect(
      nextModel.threadFilesByScope['team-files:team_openagents_core']?.[0]
        ?.downloadEnabled,
    ).toBe(false)
  })

  test('cursor gaps mark the stream failed and reload the scope snapshot', () => {
    const model = init(ChatRoute(), auth)
    const [nextModel, commands] = update(
      model,
      ReceivedSyncCursorGap({
        gap: new CursorGap({
          scope: SyncScope.make(workspaceScope),
          expectedSeq: SyncSequence.make(2),
          receivedSeq: SyncSequence.make(4),
        }),
      }),
    )

    expect(nextModel.sync.connectionByScope[workspaceScope]).toEqual({
      error: 'cursor gap',
      status: 'failed',
    })
    expect(commands.map(command => command.name)).toEqual(['LoadSyncSnapshot'])
    expect(commands[0]?.args).toEqual({
      href: '/api/sync/workspace/github%3A14167547/snapshot',
      scope: workspaceScope,
    })
  })

  test('queued launch response exits the local launching state for polling', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [submittedModel] = update(
      { ...model, chatComposerValue: 'Identify yourself' },
      SubmittedChatComposer(),
    )
    const [nextModel, commands] = update(
      submittedModel,
      SucceededLaunchAutopilotRun({
        requestId: 'chat-request-1',
        response: queuedLaunchResponse,
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'SetAutopilotThreadUrl',
      'LoadSyncSnapshot',
      'ScrollChatTimelineToEnd',
    ])
    expect(nextModel.chatRun).toMatchObject({
      _tag: 'Active',
      metadata: {
        externalRunRef: {
          _tag: 'AgentRunExternalRefMissing',
        },
        status: 'queued',
      },
    })
  })

  test('active runs poll the real run detail API', () => {
    const model = {
      ...init(ChatRoute(), auth),
      chatRun: activeRun,
    }
    const [_nextModel, commands] = update(
      model,
      RequestedPollAutopilotRun({ runId: 'agent_run_1' }),
    )

    expect(commands.map(command => command.name)).toEqual(['FetchAutopilotRun'])
  })

  test('opening a mission route loads and renders the saved run transcript from sync', () => {
    const model = init(ChatRoute(), auth)
    const [loadingModel, commands, outMessage] = update(
      {
        ...model,
        chatMessages: [
          {
            author: 'user',
            body: 'stale visible transcript',
            id: 'stale-user-turn',
            label: 'Christopher David',
            status: 'complete',
          },
        ],
      },
      EnteredAutopilotRunRoute({ runId: legacyRouteRunId }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'ScrollChatTimelineToEnd',
    ])
    expect(commands[0]?.args).toEqual({
      href: `/api/sync/thread/${legacyRouteRunId}/snapshot`,
      scope: legacyThreadScope,
    })
    expect(outMessage).toEqual(Option.none())
    expect(loadingModel.chatMessages).toEqual([])
    expect(loadingModel.chatRun).toEqual({
      _tag: 'Loading',
      runId: legacyRouteRunId,
    })

    const [loadedModel] = update(
      loadingModel,
      SucceededLoadSyncSnapshot({
        scope: legacyThreadScope,
        snapshot: syncSnapshot(
          {
            agent_runs: {
              [legacyStoredRunId]: syncRunProjection(legacyDetailResponse),
            },
            agent_run_events: Object.fromEntries(
              legacyDetailResponse.events.map(event => [
                event.id,
                syncEventProjection(event),
              ]),
            ),
          },
          3,
          legacyThreadScope,
        ),
      }),
    )

    expect(loadedModel.chatMessages).toMatchObject([
      {
        author: 'user',
        body: 'Investigate sidebar mission history',
        label: 'Christopher David',
      },
    ])
    expect(loadedModel.chatRun).toMatchObject({
      _tag: 'Active',
      metadata: {
        displayRunId: legacyRouteRunId,
        runId: legacyStoredRunId,
        status: 'completed',
        tokenTotal: 84,
      },
      events: [
        {
          summary:
            'OpenAgents accepted the Autopilot assignment for computer dispatch.',
        },
        {
          summary: 'Computer completed the saved mission.',
          tokenTotal: 84,
        },
      ],
    })
  })

  test('empty thread sync snapshot falls back to the compatibility detail API', () => {
    const model = init(ChatRoute(), auth)
    const [loadingModel] = update(
      model,
      EnteredAutopilotRunRoute({ runId: legacyRouteRunId }),
    )
    const [_nextModel, commands] = update(
      loadingModel,
      SucceededLoadSyncSnapshot({
        scope: legacyThreadScope,
        snapshot: syncSnapshot({}, 0, legacyThreadScope),
      }),
    )

    expect(commands.map(command => command.name)).toEqual(['FetchAutopilotRun'])
    expect(commands[0]?.args).toEqual({ runId: legacyRouteRunId })
  })

  test('agent-run sync event patches update the active transcript', () => {
    const model = {
      ...init(ChatRoute(), auth),
      chatMessages: [
        {
          author: 'user' as const,
          body: launchResponse.run.goal,
          id: 'user-turn-1',
          label: auth.session.name,
          status: 'complete' as const,
        },
      ],
      chatRun: activeRun,
      sync: {
        ...init(ChatRoute(), auth).sync,
        collectionByScope: {
          [activeAgentRunScope]: {
            agent_runs: {
              agent_run_1: syncRunProjection(launchResponse),
            },
            agent_run_events: {},
          },
        },
      },
    }
    const [nextModel, commands] = update(
      model,
      ReceivedSyncPatch({
        patch: syncPatch({
          collection: 'agent_run_events',
          id: 'event_3',
          op: 'put',
          scope: activeAgentRunScope,
          value: syncEventProjection({
            id: 'event_3',
            parentId: 'agent_run_1',
            sequence: 3,
            type: 'runner.completed',
            summary: 'OpenCode completed the synced run.',
            status: 'completed',
            source: 'runner',
            payloadJson:
              '{"usage":{"provider":"openai","model":"gpt-5","totalTokens":9}}',
            artifactRefs: ['result.md'],
            externalEventId: 'shc-event-3',
            createdAt: '2026-06-03T00:00:03.000Z',
          }),
        }),
      }),
    )

    expect(commands.map(command => command.name)).toEqual([])
    expect(nextModel.chatRun).toMatchObject({
      _tag: 'Active',
      metadata: {
        eventCursor: 3,
        tokenTotal: 9,
        tokenUsageEvents: 1,
      },
      events: [
        {
          summary: 'OpenCode completed the synced run.',
          tokenTotal: 9,
        },
      ],
    })
  })

  test('agent-run sync run patches update active metadata without rebuilding events', () => {
    const model = {
      ...init(ChatRoute(), auth),
      chatMessages: [
        {
          author: 'user' as const,
          body: launchResponse.run.goal,
          id: 'user-turn-1',
          label: auth.session.name,
          status: 'complete' as const,
        },
      ],
      chatRun: ActiveChatRun({
        events: [
          {
            artifactRefs: [],
            createdAt: '2026-06-03T00:00:03.000Z',
            externalEventId: Option.none(),
            id: 'event_3',
            payloadJson: Option.none(),
            sequence: 3,
            source: 'runner',
            status: Option.some('running'),
            summary: 'Still running',
            tokenModel: Option.none(),
            tokenProvider: Option.none(),
            tokenTotal: 0,
            type: 'runner.progress',
          },
        ],
        metadata: activeRun.metadata,
      }),
      sync: {
        ...init(ChatRoute(), auth).sync,
        collectionByScope: {
          [activeAgentRunScope]: {
            agent_runs: {
              agent_run_1: syncRunProjection(launchResponse),
            },
            agent_run_events: {},
          },
        },
      },
    }
    const [nextModel, commands] = update(
      model,
      ReceivedSyncPatch({
        patch: syncPatch({
          collection: 'agent_runs',
          id: 'agent_run_1',
          op: 'put',
          scope: activeAgentRunScope,
          value: {
            ...syncRunProjection({
              ...launchResponse,
              run: {
                ...launchResponse.run,
                eventCursor: 3,
                status: 'completed',
                updatedAt: '2026-06-03T00:00:04.000Z',
              },
            }),
          },
        }),
      }),
    )

    expect(commands.map(command => command.name)).toEqual([])
    expect(nextModel.chatRun).toMatchObject({
      _tag: 'Active',
      metadata: {
        eventCursor: 3,
        status: 'completed',
        updatedAt: '2026-06-03T00:00:04.000Z',
      },
      events: [
        {
          id: 'event_3',
          summary: 'Still running',
        },
      ],
    })
  })

  test('launch failure is shown as real Worker or SHC failure state', () => {
    const model = init(ChatRoute(), authWithTeam)
    const [submittedModel] = update(
      { ...model, chatComposerValue: 'Run the smoke test' },
      SubmittedChatComposer(),
    )
    const [nextModel] = update(
      submittedModel,
      FailedLaunchAutopilotRun({
        error:
          'Connect repo push access before launching Autopilot on the computer.',
        requestId: 'chat-request-1',
      }),
    )

    expect(nextModel.chatRun).toEqual({
      _tag: 'Failed',
      error:
        'Connect repo push access before launching Autopilot on the computer.',
    })
  })
})

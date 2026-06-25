import {
  ProviderAccountId,
  ProviderAccountRef,
  ProviderConnectionAttemptId,
  IsoTimestamp as ProviderIsoTimestamp,
} from '@openagentsinc/provider-account-schema'
import {
  SyncScope,
  SyncSequence,
  SyncSnapshot,
  TokenUsageAggregateResponse,
} from '@openagentsinc/sync-schema'
import { Schema as S } from 'effect'
import { Scene } from 'foldkit'
import { describe, test } from 'vitest'

import {
  type AuthBootstrap,
  type OnboardingGitHubRepository,
  completedOnboardingStatus,
  emptyBillingSummary,
  incompleteOnboardingStatus,
} from '../../domain/session'
import { GotLoggedInMessage } from '../../message'
import { LoggedIn } from '../../model'
import {
  AdminRoute,
  ChatRoute,
  ForgeRoute,
  GymOssRoute,
  ImagesRoute,
  type LoggedInRoute,
  MulletRoute,
  OnboardingRoute,
  OrderDetailRoute,
  OrderRoute,
  ProRoute,
  SettingsRoute,
  SettingsSectionRoute,
  StatsRoute,
  TeamChatRoute,
  TeamFileRoute,
  TeamFilesRoute,
  TeamProjectChatRoute,
  ThreadRoute,
  WorkspaceRoute,
  teamChatRouter,
  threadRouter,
} from '../../route'
import { update } from '../../update'
import { view } from '../../view'
import { SubmitCustomerOrder } from './customer-order/transitions'
import {
  CompletedFocusChatComposer,
  CompletedScrollChatTimelineToEnd,
  CompletedSetAutopilotThreadUrl,
  SelectedCustomerSiteElementContext,
  SucceededLaunchAutopilotRun,
  SucceededLoadAdminAdjutantAssignments,
  SucceededLoadAdminOverview,
  SucceededLoadAgentGoal,
  SucceededLoadArtanisOperatorConsole,
  SucceededLoadArtanisOperatorGoal,
  SucceededLoadMulletBootstrap,
  SucceededLoadOnboardingRepositories,
  SucceededLoadPrefilledWorkspace,
  SucceededLoadSyncSnapshot,
  SucceededLoadTokenUsageStats,
  SucceededSubmitCustomerOrder,
  SucceededUploadThreadFile,
} from './message'
import {
  ActiveChatRun,
  AdminAdjutantAssignmentsLoaded,
  AdminAdjutantReviewLoaded,
  type AgentRunLaunchResponse,
  type ArtanisOperatorConsoleResponse,
  AutopilotWorkListLoaded,
  type AutopilotWorkSummary,
  CustomerOneCohortLoaded,
  type CustomerOneCohortProjection,
  type CustomerOneCohortProjectionRow,
  type CustomerOrder,
  CustomerOrderLoaded,
  CustomerOrdersLoaded,
  CustomerSiteBuilderEventsFailed,
  CustomerSiteBuilderEventsLoaded,
  CustomerSiteBuilderEventsLoading,
  CustomerSiteBuilderFileReadFailed,
  CustomerSiteBuilderFileReadLoaded,
  CustomerSiteBuilderFileReadLoading,
  CustomerSiteBuilderFilesFailed,
  CustomerSiteBuilderFilesLoaded,
  CustomerSiteBuilderFilesLoading,
  CustomerSiteBuilderSessionFailed,
  CustomerSiteBuilderSessionLoaded,
  CustomerSiteBuilderSessionLoading,
  CustomerSiteFeedbackLoaded,
  CustomerSiteRevisionsLoaded,
  PollingProviderDeviceLogin,
  ProviderAccountPoolLoaded,
  type ThreadFileApiRecord,
  type ThreadFileDetailApiRecord,
  agentRunExternalRefFromNullable,
  optionFromNullableString,
  threadFileDetailFromDto,
  threadFileRecordFromDto,
} from './model'
import { InstallSitePreviewElementTargetBridge } from './site-preview-bridge'
import {
  FocusChatComposer,
  LaunchAutopilotRun,
  LoadSyncSnapshot,
  ScrollChatTimelineToEnd,
  SetAutopilotThreadUrl,
  UploadThreadFile,
} from './update'

const auth: AuthBootstrap = {
  session: {
    email: 'chris@openagents.com',
    name: 'Christopher David',
    userId: 'github:14167547',
    login: 'chris',
    avatarUrl: 'https://avatars.githubusercontent.com/u/14167547?v=4',
  },
  teams: [
    {
      id: 'team_openagents_core',
      name: 'OpenAgents Core Team',
      slug: 'openagents-core-team',
      role: 'owner',
      members: [
        {
          avatarUrl: null,
          email: 'chris@openagents.com',
          githubId: '14167547',
          githubUsername: 'chris',
          joinedAt: '2026-06-02T20:00:00.000Z',
          name: 'Christopher David',
          role: 'owner',
          status: 'active',
          userId: 'github:14167547',
        },
      ],
    },
  ],
  billing: emptyBillingSummary(),
  onboarding: completedOnboardingStatus(),
  isAdmin: true,
}

const authWithIncompleteOnboarding: AuthBootstrap = {
  ...auth,
  onboarding: incompleteOnboardingStatus(),
}

const onboardingRepository = (index: number): OnboardingGitHubRepository => {
  const padded = String(index).padStart(2, '0')
  const name = `repo-${padded}`

  return {
    defaultBranch: 'main',
    description: `Repository ${padded}`,
    fullName: `OpenAgentsInc/${name}`,
    htmlUrl: `https://github.com/OpenAgentsInc/${name}`,
    id: `repo_${padded}`,
    name,
    owner: 'OpenAgentsInc',
    private: true,
    provider: 'github',
  }
}

const onboardingRepositories = [
  onboardingRepository(1),
  onboardingRepository(2),
  onboardingRepository(3),
  onboardingRepository(4),
  onboardingRepository(5),
  onboardingRepository(6),
  onboardingRepository(7),
  onboardingRepository(8),
]

const customerOrderFixture = (
  overrides: Partial<CustomerOrder> = {},
): CustomerOrder => ({
  adjutant: {
    activeUrl: null,
    adjustmentStatus: null,
    inputNeeded: false,
    nextAction: 'Autopilot is queued for this order.',
    orderStatus: 'submitted',
    reviewNeeded: false,
    siteStatus: null,
    stage: 'queued',
  },
  computePaymentAcknowledgedAt: '2026-06-04T12:00:00.000Z',
  createdAt: '2026-06-04T12:01:00.000Z',
  dataUseAcknowledgedAt: '2026-06-04T12:00:00.000Z',
  freeSliceCents: 5000,
  id: 'software_order_fixture',
  providerAccountRequired: false,
  publicWorkAcknowledgedAt: '2026-06-04T12:00:00.000Z',
  quoteCents: null,
  repository: null,
  request: 'Build a customer software request.',
  site: null,
  status: 'submitted',
  triage: null,
  updatedAt: '2026-06-04T12:01:00.000Z',
  usageReceipts: [],
  usageSummary: {
    billingMode: 'public_beta_free',
    categories: [],
    totalCreditsChargedCents: 0,
    totalCreditsChargedFormatted: '$0.00',
  },
  visibility: 'public',
  ...overrides,
})

const authWithSavedRepository: AuthBootstrap = {
  ...auth,
  onboarding: {
    ...completedOnboardingStatus(),
    repository: {
      _tag: 'RepositorySelected',
      repository: onboardingRepository(1),
      selectedAt: '2026-06-04T00:00:00.000Z',
    },
    updatedAt: '2026-06-04T00:00:00.000Z',
  },
}

const forgeWorkOrderFixture = (
  overrides: Partial<AutopilotWorkSummary> = {},
): AutopilotWorkSummary => ({
  createdAt: '2026-06-10T00:00:00.000Z',
  promiseRef: {
    blockerRefs: [],
    promiseId: 'forge.metrics.test',
    registryVersion: '2026-06-16.5',
  },
  routing: {
    availabilityState: 'selected',
    buyerDebitRequired: false,
    fallbackLeaseIntentCount: 0,
    fallbackRunnerKind: 'openagents_shc',
    laneRef: 'lane.autopilot_work.requester_pylon_own_job',
    meterKind: 'none',
    pylonAssignmentIntentCount: 0,
    selectedRunnerKind: 'requester_pylon',
    source: 'requester_pylon',
  },
  state: 'scheduled',
  updatedAt: '2026-06-10T01:00:00.000Z',
  workOrderRef: 'wo_forge_metrics_1',
  ...overrides,
})

const providerPoolLoadedFixture = () =>
  ProviderAccountPoolLoaded({
    response: {
      accounts: [],
      activeLeases: [],
      generatedAt: '2026-06-16T12:00:00.000Z',
      nextSelection: {
        accountLabel: null,
        activeLeaseCount: null,
        leaseLimit: null,
        provider: null,
        providerAccountRef: null,
        selectionReason: 'No live selection needed for scene fixture.',
        status: 'none',
      },
      policyVersion: 'provider-pool.policy.test',
      provider: 'google-gemini',
      summary: {
        activeLeaseCount: 0,
        cooldown: 0,
        eligible: 3,
        lowCredit: 0,
        requiresReauth: 0,
        total: 3,
        unhealthy: 0,
      },
    },
  })

const customerOneCohortProjectionFixture = (
  overrides: Partial<CustomerOneCohortProjection> = {},
): CustomerOneCohortProjection => ({
  authority: 'evidence_only',
  blockerRefs: ['reason.customer_one.cohort_completion_bundles_missing'],
  caveatRefs: [],
  cohortProjectionVersion: 'customer-one-cohort-projection:v1',
  counts: {
    blocked: 0,
    candidate: 0,
    deferred: 0,
    delivery_reviewed: 0,
    first_run_started: 0,
    invited: 0,
    loop_completed: 0,
    workspace_seeded: 0,
  },
  gate: {
    reasonRefs: ['reason.customer_one.cohort_completion_bundles_missing'],
    state: 'blocked',
  },
  generatedAt: '2026-06-17T20:00:00.000Z',
  rows: [],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: ['cohort_row_written', 'privacy_review_recorded'],
  },
  target: {
    maximumTargetTeams: 5,
    minimumCompletedTeams: 3,
  },
  ...overrides,
})

const completedCohortProjectionRow = (
  index: number,
): CustomerOneCohortProjectionRow => ({
  artifactRef: `artifact.customer-one.team-${index}.delivery.v1`,
  blockerRefs: [],
  caveatRefs: [],
  completionBundleRef: `completion.customer-one.team-${index}.bundle.v1`,
  countsTowardD3Completion: true,
  displayLabel: `Team ${index}`,
  privacyReviewRef: `privacy.customer-one.team-${index}.review.v1`,
  reviewRef: `review.customer-one.team-${index}.human.v1`,
  routingRef: `routing.customer-one.team-${index}.owned-node.v1`,
  runRef: `run.customer-one.team-${index}.primary.v1`,
  state: 'loop_completed',
  teamCohortRef: `cohort.team.private-customer-${index}.v1`,
  templateRef: 'forge.template.ecommerce.inventory_campaign.v1',
  verificationRef: `verification.customer-one.team-${index}.smoke.v1`,
  verticalRef: 'vertical.private-customer-ecommerce.v1',
  workspaceRef: `workspace.customer-one.private-customer-${index}.v1`,
})

const readyCustomerOneCohortProjectionFixture =
  (): CustomerOneCohortProjection => {
    const rows = [
      completedCohortProjectionRow(1),
      completedCohortProjectionRow(2),
      completedCohortProjectionRow(3),
    ]

    return customerOneCohortProjectionFixture({
      blockerRefs: [],
      counts: {
        blocked: 0,
        candidate: 0,
        deferred: 0,
        delivery_reviewed: 0,
        first_run_started: 0,
        invited: 0,
        loop_completed: 3,
        workspace_seeded: 0,
      },
      gate: {
        reasonRefs: [],
        state: 'ready',
      },
      rows,
    })
  }

const workspaceScope = 'workspace:github:14167547'
const syncedMissionModel = (route: LoggedInRoute = ChatRoute()) => {
  const [model] = LoggedIn.update(
    LoggedIn.init(route, auth),
    SucceededLoadSyncSnapshot({
      scope: workspaceScope,
      snapshot: new SyncSnapshot({
        scope: SyncScope.make(workspaceScope),
        cursor: SyncSequence.make(1),
        collections: {
          missions: {
            '11111111-1111-4111-8111-111111111111': {
              id: '11111111-1111-4111-8111-111111111111',
              title: 'Run the smoke test',
              detail: 'autopilot-omega - completed',
              href: '/t/11111111-1111-4111-8111-111111111111',
              owner: 'personal',
              status: 'complete',
              updatedAt: '2026-06-03T00:00:01.000Z',
            },
            '22222222-2222-4222-8222-222222222222': {
              id: '22222222-2222-4222-8222-222222222222',
              title: 'Investigate sidebar missions',
              detail: 'autopilot-omega - running',
              href: '/t/22222222-2222-4222-8222-222222222222',
              owner: 'team',
              status: 'active',
              teamId: 'team_openagents_core',
              updatedAt: '2026-06-03T00:01:01.000Z',
            },
          },
        },
      }),
    }),
  )

  return model
}

const authWithArtanisProject: AuthBootstrap = {
  ...auth,
  teams: [
    {
      id: 'team_openagents_core',
      name: 'OpenAgents Core Team',
      slug: 'openagents-core-team',
      role: 'owner',
      members: auth.teams[0]?.members ?? [],
      projects: [
        {
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
          description: 'OpenAgents public-agent and Pylon workstream.',
          id: 'project_artanis',
          name: 'Artanis',
          slug: 'artanis',
          status: 'active',
          teamId: 'team_openagents_core',
        },
      ],
    },
  ],
}

const authWithUnhealthyProviderAccount: AuthBootstrap = {
  ...auth,
  providerAccounts: {
    accounts: [
      {
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
    ],
    attempts: [
      {
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
    ],
  },
}

const authWithHealthyProviderAccount: AuthBootstrap = {
  ...auth,
  providerAccounts: {
    accounts: [
      {
        accountLabel: 'chris@openagents.com',
        authMode: 'chatgpt_device_code',
        connectedAt: ProviderIsoTimestamp.make('2026-06-03T00:00:03.000Z'),
        createdAt: ProviderIsoTimestamp.make('2026-06-03T00:00:00.000Z'),
        hasSecretRef: true,
        health: 'healthy',
        id: ProviderAccountId.make('provider_account_1'),
        lastStatusAt: ProviderIsoTimestamp.make('2026-06-03T00:00:03.000Z'),
        provider: 'chatgpt_codex',
        providerAccountRef: ProviderAccountRef.make('provider-account_1'),
        publicStatus: 'connected',
        status: 'connected',
        updatedAt: ProviderIsoTimestamp.make('2026-06-03T00:00:03.000Z'),
      },
    ],
    attempts: [],
  },
}

const authWithMultipleProviderAccounts: AuthBootstrap = {
  ...auth,
  providerAccounts: {
    accounts: [
      {
        accountLabel: 'primary@openagents.com',
        authMode: 'chatgpt_device_code',
        connectedAt: ProviderIsoTimestamp.make('2026-06-03T00:00:03.000Z'),
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
      {
        accountLabel: 'backup@openagents.com',
        authMode: 'chatgpt_device_code',
        connectedAt: ProviderIsoTimestamp.make('2026-06-03T00:00:04.000Z'),
        createdAt: ProviderIsoTimestamp.make('2026-06-03T00:00:01.000Z'),
        hasSecretRef: true,
        health: 'healthy',
        id: ProviderAccountId.make('provider_account_2'),
        lastStatusAt: ProviderIsoTimestamp.make('2026-06-03T00:00:04.000Z'),
        provider: 'chatgpt_codex',
        providerAccountRef: ProviderAccountRef.make('provider-account_2'),
        publicStatus: 'connected',
        status: 'connected',
        updatedAt: ProviderIsoTimestamp.make('2026-06-03T00:00:04.000Z'),
      },
    ],
    attempts: [],
  },
}

const activeGoal = {
  id: 'goal_scene_1',
  agentId: 'autopilot',
  userId: 'github:14167547',
  teamId: null,
  projectId: null,
  objective: 'Keep the public workroom progressing',
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

const artanisConsoleResponse = {
  agentId: 'agent_artanis',
  consoleRef: 'operator.artanis.console',
  status: {
    blockerRefs: ['blocker.public.artanis.operator_approval_needed'],
    healthState: 'stale',
    lastTickRef: 'tick.public.artanis.scene_1',
    loopState: 'running',
    nextTickDisplay: 'in 1 minute',
    pendingApprovalCount: 1,
    publicationLagLabel: 'Forum publication queued',
    publicationLagState: 'warning',
    runtimeState: 'running',
  },
  latestRows: [
    {
      kind: 'loop_tick',
      recordRef: 'tick.public.artanis.scene_1',
      state: 'completed',
      updatedAtDisplay: 'just now',
    },
  ],
  steering: {
    approvalDecisions: [],
    goalCommands: [],
    privateEvidencePackRefs: ['evidence.private.artanis.scene'],
    rawWorkroomStateRefs: ['workroom.private.artanis.scene'],
    supportedApprovalActions: ['approve_risky_action', 'reject_risky_action'],
    supportedGoalActions: [
      'create_goal',
      'pause_goal',
      'resume_goal',
      'cancel_goal',
      'reprioritize_goal',
    ],
  },
  approvalGates: {
    effectiveGateRefs: ['approval_gate.operator.artanis.scene'],
    gateCount: 1,
    gates: [
      {
        actionRef: 'action.operator.artanis.publish_forum',
        authorityReceiptRefs: ['receipt.authority.artanis.scene'],
        caveatRefs: ['caveat.public.artanis.scene'],
        effective: true,
        expiresAtDisplay: 'in 3 hours',
        gateRef: 'approval_gate.operator.artanis.scene',
        kind: 'forum_publish',
        label: 'Publish Forum update',
        operatorReceiptRefs: ['receipt.operator.artanis.scene'],
        privateEvidenceRefs: ['evidence.private.artanis.scene'],
        publicStatusRefs: ['status.public.artanis.scene'],
        rollbackPosture: 'operator_rollback_available',
        rollbackRefs: ['rollback.operator.artanis.scene'],
        state: 'pending',
        updatedAtDisplay: 'just now',
      },
    ],
  },
  workRouting: {
    proposalCount: 1,
    proposals: [
      {
        approvalRequirementRefs: ['approval.operator.artanis.scene'],
        blockerRefs: [],
        capability: 'pylon_job_intake',
        costCaveatRefs: ['cost.public.artanis.scene'],
        operatorDetailRefs: ['detail.operator.artanis.scene'],
        proposalRef: 'work.operator.artanis.scene',
        resourceMode: 'overnight_full',
        risk: 'operator_approval_required',
        sourceEvidenceRefs: ['evidence.public.artanis.scene'],
        spendLimitRefs: ['spend.operator.artanis.scene'],
        state: 'proposed',
        target: 'pylon_marketplace',
        updatedAtDisplay: 'just now',
        workClass: 'inference',
      },
    ],
    riskyProposalRefs: ['work.operator.artanis.scene'],
  },
  publicationQueue: {
    deliverableIntentRefs: ['forum_intent.operator.artanis.scene'],
    deliveredCount: 0,
    intentCount: 1,
    intents: [
      {
        blockerRefs: [],
        deliveryState: 'queued',
        intentRef: 'forum_intent.operator.artanis.scene',
        postRef: null,
        targetForumRef: 'forum.openagents',
        targetTopicRef: 'topic.artanis.launch',
        targetTopicState: 'open',
        updatedAtDisplay: 'just now',
      },
    ],
  },
} satisfies ArtanisOperatorConsoleResponse

const activeArtanisGoal = {
  ...activeGoal,
  agentId: 'agent_artanis',
  id: 'goal_artanis_scene_1',
  objective: 'Maintain the Pylon v0.2 launch.',
  teamId: 'team_openagents_core',
}

const blockedProviderAccountMessage =
  'chris@openagents.com cannot launch Autopilot. OpenAI invalidated the saved ChatGPT login. Reconnect ChatGPT in Settings -> Connections.'

const launchResponse = {
  run: {
    id: 'agent_run_scene_1',
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
    externalRunId: 'shc:oa-shc-katy-01:agent_run_scene_1',
    status: 'running',
    eventCursor: 1,
    createdAt: '2026-06-03T00:00:00.000Z',
    updatedAt: '2026-06-03T00:00:01.000Z',
  },
  events: [
    {
      id: 'event_scene_1',
      parentId: 'agent_run_scene_1',
      sequence: 1,
      type: 'runner.dispatched',
      summary: 'OpenAgents dispatched the assignment to the computer.',
      status: 'running',
      source: 'shc',
      payloadJson:
        '{"usage":{"provider":"openai","model":"gpt-5","totalTokens":17}}',
      artifactRefs: [],
      externalEventId: 'shc-event-scene-1',
      createdAt: '2026-06-03T00:00:01.000Z',
    },
  ],
  statusUrl: '/api/omni/agent-runs/agent_run_scene_1',
  streamUrl: '/api/omni/agent-runs/agent_run_scene_1/events',
} satisfies AgentRunLaunchResponse

const teamAutopilotThreadId = '8a8f5061-9648-45c8-a216-86a42f85cf12'

const activeTeamRun = ActiveChatRun({
  events: [
    {
      artifactRefs: [],
      createdAt: '2026-06-03T00:00:01.000Z',
      externalEventId: optionFromNullableString('shc-event-team-1'),
      id: 'event_team_1',
      payloadJson: optionFromNullableString(undefined),
      sequence: 1,
      source: 'shc',
      status: optionFromNullableString('running'),
      summary: 'OpenAgents dispatched the assignment to the computer.',
      tokenModel: optionFromNullableString(undefined),
      tokenProvider: optionFromNullableString(undefined),
      tokenTotal: 0,
      type: 'runner.dispatched',
    },
    {
      artifactRefs: [],
      createdAt: '2026-06-03T00:03:20.000Z',
      externalEventId: optionFromNullableString('shc-event-team-2'),
      id: 'event_team_2',
      payloadJson: optionFromNullableString(
        JSON.stringify({
          part: {
            tool: 'bash',
            type: 'tool',
          },
          type: 'tool_use',
        }),
      ),
      sequence: 2,
      source: 'runner',
      status: optionFromNullableString('completed'),
      summary: 'Shell command completed.',
      tokenModel: optionFromNullableString(undefined),
      tokenProvider: optionFromNullableString(undefined),
      tokenTotal: 0,
      type: 'tool_use.completed',
    },
  ],
  metadata: {
    backend: 'shc_vm',
    createdAt: '2026-06-03T00:00:00.000Z',
    displayRunId: teamAutopilotThreadId,
    eventCursor: 2,
    externalRunRef: agentRunExternalRefFromNullable(
      'shc:oa-shc-katy-01:agent_run_scene_1',
    ),
    goal: 'summarize that PDF',
    repository: 'OpenAgentsInc/autopilot-omega@main',
    runId: 'agent_run_scene_1',
    runnerId: 'oa-shc-katy-01',
    runtime: 'opencode_codex',
    status: 'completed',
    statusUrl: '/api/omni/agent-runs/agent_run_scene_1',
    streamUrl: '/api/omni/agent-runs/agent_run_scene_1/events',
    tokenTotal: 0,
    tokenUsageEvents: 0,
    updatedAt: '2026-06-03T00:03:20.000Z',
  },
})

const teamThreadFileDto = {
  contentType: 'text/plain',
  createdAt: '2026-06-03T00:00:03.000Z',
  detailUrl: '/teams/openagents-core-team/files/file_1',
  downloadEnabled: true,
  downloadUrl: '/api/thread-files/file_1/download',
  filename: 'notes.txt',
  id: 'file_1',
  ownerUserId: 'github:14167547',
  scope: 'team',
  sizeBytes: 12,
  teamId: 'team_openagents_core',
  threadId: 'team:team_openagents_core:chat',
} satisfies ThreadFileApiRecord

const teamThreadFile = threadFileRecordFromDto(teamThreadFileDto)

const teamThreadFileDetailDto = {
  canManage: true,
  file: teamThreadFileDto,
  references: [
    {
      author: {
        avatarUrl: null,
        githubUsername: 'chris',
        name: 'Christopher David',
        userId: 'github:14167547',
      },
      body: 'Please inspect notes.txt before the Autopilot run.',
      createdAt: '2026-06-03T00:00:04.000Z',
      excerpt: 'Please inspect notes.txt before the Autopilot run.',
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
} satisfies ThreadFileDetailApiRecord

const teamThreadFileDetail = threadFileDetailFromDto(teamThreadFileDetailDto)

describe('logged-in workroom sidebar', () => {
  test('renders authenticated onboarding outside the workroom sidebar shell', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedIn.init(OnboardingRoute(), authWithIncompleteOnboarding),
      ),
      Scene.expect(
        Scene.selector('[data-component="logged-in-onboarding-shell"]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-component="logged-in-workroom-shell"]'),
      ).not.toExist(),
      Scene.expect(
        Scene.role('navigation', { name: 'OpenAgents navigation' }),
      ).toBeAbsent(),
      Scene.expect(Scene.role('button', { name: 'New thread' })).not.toExist(),
      Scene.expect(Scene.text('OpenAgents Core Team')).not.toExist(),
      Scene.expect(
        Scene.role('heading', { name: 'Choose the repo' }),
      ).toExist(),
    )
  })

  test('searches and paginates authenticated onboarding repositories', () => {
    const [model] = LoggedIn.update(
      LoggedIn.init(OnboardingRoute(), authWithIncompleteOnboarding),
      SucceededLoadOnboardingRepositories({
        response: {
          repositories: onboardingRepositories,
          tokenStatus: 'available',
        },
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.text('OpenAgentsInc/repo-01')).toExist(),
      Scene.expect(Scene.text('OpenAgentsInc/repo-07')).not.toExist(),
      Scene.expect(Scene.text('1-6 of 8')).toExist(),
      Scene.click(Scene.role('button', { name: 'Next' })),
      Scene.expect(Scene.text('OpenAgentsInc/repo-07')).toExist(),
      Scene.expect(Scene.text('OpenAgentsInc/repo-01')).not.toExist(),
      Scene.type(Scene.label('Search repositories'), 'repo-08'),
      Scene.expect(Scene.text('OpenAgentsInc/repo-08')).toExist(),
      Scene.expect(Scene.text('OpenAgentsInc/repo-07')).not.toExist(),
      Scene.expect(Scene.label('Owner')).toExist(),
      Scene.expect(Scene.label('Repository')).toExist(),
    )
  })

  test('goes back from the onboarding goal step to repository selection', () => {
    const [model] = LoggedIn.update(
      LoggedIn.init(OnboardingRoute(), {
        ...authWithIncompleteOnboarding,
        onboarding: {
          billing: { _tag: 'BillingPending' },
          completedAt: null,
          goal: null,
          repository: {
            _tag: 'RepositorySelected',
            repository: onboardingRepository(1),
            selectedAt: '2026-06-04T00:00:00.000Z',
          },
          step: 'goal',
          updatedAt: '2026-06-04T00:00:00.000Z',
        },
      }),
      SucceededLoadOnboardingRepositories({
        response: {
          repositories: onboardingRepositories,
          tokenStatus: 'available',
        },
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(
        Scene.role('heading', { name: 'What should the agent build?' }),
      ).toExist(),
      Scene.click(Scene.role('button', { name: 'Back' })),
      Scene.expect(
        Scene.role('heading', { name: 'Choose the repo' }),
      ).toExist(),
    )
  })

  test('renders Forge triage counts without double-counting scheduled backlog', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(ForgeRoute(), auth),
        autopilotWorkList: AutopilotWorkListLoaded({
          response: {
            generatedAt: '2026-06-16T12:00:00.000Z',
            promiseId: 'forge.metrics.test',
            workOrders: [forgeWorkOrderFixture()],
          },
        }),
        providerAccountPool: providerPoolLoadedFixture(),
        customerOneCohort: CustomerOneCohortLoaded({
          response: customerOneCohortProjectionFixture(),
        }),
      }),
      Scene.expect(
        Scene.selector('[data-component="forge-factory-dashboard"]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-forge-detail-panel-value-key="runs-triaged"]'),
      ).toHaveAttr('data-forge-detail-panel-value-text', '1'),
      Scene.expect(Scene.selector('[data-forge-automation-total]')).toHaveAttr(
        'data-forge-automation-total',
        '8',
      ),
      Scene.expect(
        Scene.selector('[data-forge-stage-key="triage"]'),
      ).toHaveAttr('data-forge-stage-automation-count', '1'),
      Scene.expect(
        Scene.selector('[data-forge-dogfood-panel="true"]'),
      ).toHaveAttr('data-forge-dogfood-status', 'live'),
      Scene.expect(
        Scene.selector('[data-forge-dogfood-metric="open-work"]'),
      ).toHaveAttr('data-forge-dogfood-value', '1'),
      Scene.expect(
        Scene.selector('[data-forge-dogfood-metric="eligible-nodes"]'),
      ).toHaveAttr('data-forge-dogfood-value', '3'),
      Scene.expect(
        Scene.selector('[data-forge-routing-metric="requester-pylon"]'),
      ).toHaveAttr('data-forge-routing-value', '1'),
      Scene.expect(
        Scene.selector('[data-forge-routing-metric="fallback-lanes"]'),
      ).toHaveAttr('data-forge-routing-value', '0'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-readiness="true"]'),
      ).toHaveAttr('data-forge-cohort-gate', 'blocked'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-readiness="true"]'),
      ).toHaveAttr('data-forge-cohort-completed', '0'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-metric="target-teams"]'),
      ).toHaveAttr('data-forge-cohort-value', '3-5'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-metric="completion-bundles"]'),
      ).toHaveAttr('data-forge-cohort-value', '0'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-metric="privacy-reviews"]'),
      ).toHaveAttr('data-forge-cohort-value', '0'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-metric="gate-status"]'),
      ).toHaveAttr('data-forge-cohort-value', 'Blocked'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-empty="true"]'),
      ).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Run Scope triage' }),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-forge-automation-tuning="true"]'),
      ).toExist(),
    )
  })

  test('renders Forge cohort readiness as ready from public-safe rows only', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(ForgeRoute(), auth),
        customerOneCohort: CustomerOneCohortLoaded({
          response: readyCustomerOneCohortProjectionFixture(),
        }),
        providerAccountPool: providerPoolLoadedFixture(),
      }),
      Scene.expect(
        Scene.selector('[data-forge-cohort-readiness="true"]'),
      ).toHaveAttr('data-forge-cohort-gate', 'ready'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-readiness="true"]'),
      ).toHaveAttr('data-forge-cohort-completed', '3'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-metric="completion-bundles"]'),
      ).toHaveAttr('data-forge-cohort-value', '3'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-metric="privacy-reviews"]'),
      ).toHaveAttr('data-forge-cohort-value', '3'),
      Scene.expect(
        Scene.selector('[data-forge-cohort-metric="gate-status"]'),
      ).toHaveAttr('data-forge-cohort-value', 'Ready'),
      Scene.expect(Scene.text('Team 1')).toExist(),
      Scene.expect(Scene.text('Team 2')).toExist(),
      Scene.expect(Scene.text('Team 3')).toExist(),
      Scene.expect(Scene.text('private-customer')).not.toExist(),
      Scene.expect(Scene.text('workspace.customer-one')).not.toExist(),
      Scene.expect(Scene.text('cohort.team')).not.toExist(),
    )
  })

  test('renders Forge stage progress summaries from loaded Runs', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(ForgeRoute(), auth),
        autopilotWorkList: AutopilotWorkListLoaded({
          response: {
            generatedAt: '2026-06-16T12:00:00.000Z',
            promiseId: 'forge.metrics.test',
            workOrders: [
              forgeWorkOrderFixture({
                state: 'scheduled',
                workOrderRef: 'wo_forge_scheduled',
              }),
              forgeWorkOrderFixture({
                state: 'queued_or_running',
                workOrderRef: 'wo_forge_running',
              }),
              forgeWorkOrderFixture({
                state: 'delivered',
                workOrderRef: 'wo_forge_delivered',
              }),
              forgeWorkOrderFixture({
                state: 'accepted',
                workOrderRef: 'wo_forge_accepted',
              }),
              forgeWorkOrderFixture({
                state: 'blocked',
                workOrderRef: 'wo_forge_blocked',
              }),
              forgeWorkOrderFixture({
                state: 'rejected',
                workOrderRef: 'wo_forge_rejected',
              }),
              forgeWorkOrderFixture({
                state: 'invalid',
                workOrderRef: 'wo_forge_invalid',
              }),
              forgeWorkOrderFixture({
                state: 'invalid',
                workOrderRef: '/Users/christopher/private-work',
              }),
            ],
          },
        }),
        providerAccountPool: providerPoolLoadedFixture(),
      }),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="triage"]'),
      ).toHaveAttr('data-forge-stage-progress-pending', '1'),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="triage"]'),
      ).toHaveAttr('data-forge-stage-progress-completed', '0'),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="codegen"]'),
      ).toHaveAttr('data-forge-stage-progress-active', '1'),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="codegen"]'),
      ).toHaveAttr('data-forge-stage-progress-completed', '0'),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="codegen"]'),
      ).toHaveAttr('data-forge-stage-progress-provenance', 'live'),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="validate"]'),
      ).toHaveAttr('data-forge-stage-progress-completed', '1'),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="release"]'),
      ).toHaveAttr('data-forge-stage-progress-completed', '1'),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="monitor"]'),
      ).toHaveAttr('data-forge-stage-progress-blocked', '1'),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="monitor"]'),
      ).toHaveAttr('data-forge-stage-progress-failed', '3'),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress="monitor"]'),
      ).toHaveAttr('data-forge-stage-progress-completed', '0'),
      Scene.expect(Scene.text('1 unsafe Run ref(s) omitted')).toExist(),
      Scene.expect(Scene.text('/Users/christopher/private-work')).not.toExist(),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress-run="wo_forge_running"]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress-run="wo_forge_delivered"]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-forge-stage-progress-run="wo_forge_accepted"]'),
      ).toExist(),
    )
  })

  test('shows the customer email promise on submitted orders', () => {
    const customerAuth: AuthBootstrap = {
      ...auth,
      session: {
        ...auth.session,
        email: 'alex.customer@example.com',
        name: 'Alex Customer',
        userId: 'github:customer',
      },
    }
    const order = {
      adjutant: {
        activeUrl: 'https://sites.openagents.com/typed-repo',
        adjustmentStatus: null,
        inputNeeded: false,
        nextAction: 'Open the live Site and send any adjustment request.',
        orderStatus: 'delivered' as const,
        reviewNeeded: false,
        siteStatus: 'approved',
        stage: 'deployed' as const,
      },
      computePaymentAcknowledgedAt: '2026-06-04T12:00:00.000Z',
      createdAt: '2026-06-04T12:01:00.000Z',
      dataUseAcknowledgedAt: '2026-06-04T12:00:00.000Z',
      freeSliceCents: 5000,
      id: 'software_order_test',
      providerAccountRequired: false,
      publicWorkAcknowledgedAt: '2026-06-04T12:00:00.000Z',
      quoteCents: null,
      repository: {
        defaultBranch: 'main',
        fullName: 'OpenAgentsInc/typed-repo',
        htmlUrl: 'https://github.com/OpenAgentsInc/typed-repo',
        name: 'typed-repo',
        owner: 'OpenAgentsInc',
        private: false,
        provider: 'github' as const,
      },
      request: 'Add Stripe credits checkout.',
      site: {
        activeUrl: 'https://sites.openagents.com/typed-repo',
        activeDeploymentId: 'site_deployment_typed_repo',
        activeVersionId: 'site_version_typed_repo',
        feedbackCount: 1,
        id: 'site_project_typed_repo',
        latestBuildStatus: 'saved',
        latestSavedVersionId: 'site_version_typed_repo',
        openFeedbackCount: 1,
        status: 'approved',
      },
      triage: null,
      status: 'submitted' as const,
      updatedAt: '2026-06-04T12:01:00.000Z',
      usageReceipts: [
        {
          billingMode: 'public_beta_free' as const,
          category: 'generation' as const,
          createdAt: '2026-06-04T12:02:00.000Z',
          creditsChargedCents: 0,
          creditsChargedFormatted: '$0.00',
          details: {
            billingNote: 'Public beta Site generation is free.',
          },
          id: 'adjutant_usage_receipt_generation',
          quantity: 1,
          summary: 'Adjutant Site generation run was queued.',
          unit: 'run',
        },
      ],
      usageSummary: {
        billingMode: 'public_beta_free' as const,
        categories: [
          {
            category: 'generation' as const,
            creditsChargedCents: 0,
            creditsChargedFormatted: '$0.00',
            quantity: 1,
            receiptCount: 1,
            unit: 'run',
          },
        ],
        totalCreditsChargedCents: 0,
        totalCreditsChargedFormatted: '$0.00',
      },
      visibility: 'public' as const,
    }

    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(
          OrderDetailRoute({ orderId: 'software_order_test' }),
          customerAuth,
        ),
        customerOrder: CustomerOrderLoaded({ order }),
        customerSiteFeedback: CustomerSiteFeedbackLoaded({
          feedback: [
            {
              body: 'Please rebuild the hero so it looks credible.',
              createdAt: '2026-06-05T14:30:00.000Z',
              deploymentId: 'site_deployment_typed_repo',
              id: 'site_feedback_typed_repo',
              orderId: 'software_order_test',
              siteId: 'site_project_typed_repo',
              status: 'submitted',
              updatedAt: '2026-06-05T14:30:00.000Z',
              versionId: 'site_version_typed_repo',
            },
          ],
        }),
        customerSiteRevisions: CustomerSiteRevisionsLoaded({
          revisions: [
            {
              active: true,
              activatedAt: '2026-06-05T14:25:00.000Z',
              buildStatus: 'saved',
              createdAt: '2026-06-05T14:20:00.000Z',
              deploymentId: 'site_deployment_typed_repo',
              deploymentStatus: 'active',
              id: 'site_version_typed_repo',
              originCreatedAt: '2026-06-05T14:30:00.000Z',
              originSummary: 'Please rebuild the hero so it looks credible.',
              reviewState: 'customer_review_ready',
              savedAt: '2026-06-05T14:22:00.000Z',
              siteId: 'site_project_typed_repo',
              sourceCommitSha: 'abc123',
              sourceHash: 'sha256:typedrepo',
              url: 'https://sites.openagents.com/typed-repo',
            },
            {
              active: false,
              activatedAt: '2026-06-05T14:05:00.000Z',
              buildStatus: 'saved',
              createdAt: '2026-06-05T14:00:00.000Z',
              deploymentId: 'site_deployment_typed_repo_previous',
              deploymentStatus: null,
              id: 'site_version_typed_repo_previous',
              originCreatedAt: null,
              originSummary: 'Initial typed repo Site request.',
              reviewState: 'internal_draft',
              savedAt: '2026-06-05T14:02:00.000Z',
              siteId: 'site_project_typed_repo',
              sourceCommitSha: 'def456',
              sourceHash: 'sha256:typedrepo-previous',
              url: 'https://sites.openagents.com/typed-repo/versions/site_version_typed_repo_previous',
            },
          ],
        }),
      }),
      Scene.expect(
        Scene.text(
          "We'll email you at alex.customer@example.com within 24 hours with your completed work.",
        ),
      ).toExist(),
      Scene.expect(Scene.text('alex.customer@example.com')).toExist(),
      Scene.expect(Scene.text('Autopilot')).toExist(),
      Scene.expect(Scene.text('Deployed')).toExist(),
      Scene.expect(
        Scene.text('Open the live Site and send any adjustment request.'),
      ).toExist(),
      Scene.expect(Scene.text('Review needed')).not.toExist(),
      Scene.expect(Scene.text('Input needed')).not.toExist(),
      Scene.expect(Scene.text('Live URL')).toExist(),
      Scene.expect(Scene.text('Usage')).toExist(),
      Scene.expect(Scene.text('Public beta free')).toExist(),
      Scene.expect(Scene.text('1 run / $0.00')).toExist(),
      Scene.expect(
        Scene.selector('[data-component="site-editor-shell"]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-component="site-editor-shell"]'),
      ).toHaveClass('grid-cols-1'),
      Scene.expect(
        Scene.selector('[data-component="site-editor-shell"]'),
      ).toHaveClass('lg:grid-cols-[minmax(0,1fr)_21rem]'),
      Scene.expect(
        Scene.selector('[data-component="site-editor-sidebar"]'),
      ).toHaveAttr('data-sidebar-width-px', '336'),
      Scene.expect(Scene.text('Site editor')).toExist(),
      Scene.expect(Scene.text('Panel width')).toExist(),
      Scene.expect(Scene.text('336px')).toExist(),
      Scene.expect(Scene.text('Version history')).toExist(),
      Scene.expect(Scene.text('Inspect mode')).toExist(),
      Scene.expect(Scene.text('No element selected')).toExist(),
      Scene.expect(Scene.text('Code viewer')).toExist(),
      Scene.expect(
        Scene.text('Select an element to view source context'),
      ).toExist(),
      Scene.Mount.resolve(
        InstallSitePreviewElementTargetBridge({
          allowedOrigin: 'https://sites.openagents.com',
        }),
        SelectedCustomerSiteElementContext({
          context: {
            attributes: [
              { name: 'class', value: 'bridge-target' },
              { name: 'href', value: '#bridge' },
            ],
            htmlSnippet:
              '<a class="bridge-target" href="#bridge">Bridge target</a>',
            selector: 'main a[href="#bridge"]',
            tag: 'a',
            text: 'Bridge target',
          },
        }),
        message => GotLoggedInMessage({ message }),
      ),
      Scene.expect(
        Scene.text('<a class="bridge-target" href="#bridge">Bridge target</a>'),
      ).toExist(),
      Scene.expect(Scene.role('textbox', { name: 'Follow-up' })).toHaveValue(
        [
          'Target element: <a class="bridge-target" href="#bridge">Bridge target</a>',
          'Selector: main a[href="#bridge"]',
          'Requested change: ',
        ].join('\n'),
      ),
      Scene.expect(
        Scene.text('Please rebuild the hero so it looks credible.'),
      ).toExist(),
      Scene.expect(Scene.text('Initial typed repo Site request.')).toExist(),
      Scene.expect(
        Scene.selector(
          'a[href="https://sites.openagents.com/typed-repo/versions/site_version_typed_repo_previous"]',
        ),
      ).toHaveAttr(
        'href',
        'https://sites.openagents.com/typed-repo/versions/site_version_typed_repo_previous',
      ),
      Scene.expect(Scene.text('Revisions')).toExist(),
      Scene.expect(Scene.text('Latest revision')).toExist(),
      Scene.expect(
        Scene.text('Latest revision live / customer review ready'),
      ).toExist(),
      Scene.expect(
        Scene.text('Please rebuild the hero so it looks credible.'),
      ).toExist(),
      Scene.expect(Scene.role('textbox', { name: 'Follow-up' })).toExist(),
      Scene.expect(Scene.role('button', { name: 'Send follow-up' })).toExist(),
      Scene.expect(Scene.text('approved')).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'https://sites.openagents.com/typed-repo' }),
      ).toHaveAttr('href', 'https://sites.openagents.com/typed-repo'),
      Scene.click(
        Scene.role('button', {
          name: 'Start follow-up from site_version_typed_repo_previous',
        }),
      ),
      Scene.expect(Scene.role('textbox', { name: 'Follow-up' })).toHaveValue(
        'Follow up on site_version_typed_repo_previous: ',
      ),
      Scene.click(
        Scene.role('button', {
          name: 'Target Investment case',
        }),
      ),
      Scene.expect(Scene.text('Selected element')).toExist(),
      Scene.expect(
        Scene.text('<a class="button" href="#returns">Investment case</a>'),
      ).toExist(),
      Scene.expect(Scene.role('textbox', { name: 'Follow-up' })).toHaveValue(
        [
          'Target element: <a class="button" href="#returns">Investment case</a>',
          'Selector: main a[href="#returns"]',
          'Requested change: ',
        ].join('\n'),
      ),
      Scene.expect(Scene.text('selected-element/a.html')).toExist(),
      Scene.expect(Scene.text('site_version_typed_repo')).toExist(),
      Scene.expect(Scene.text('html')).toExist(),
      Scene.expect(Scene.role('button', { name: 'Copy snippet' })).toHaveAttr(
        'data-copy-text',
        '<a class="button" href="#returns">Investment case</a>',
      ),
    )
  })

  test('renders customer Site builder states', () => {
    const customerAuth: AuthBootstrap = {
      ...auth,
      session: {
        ...auth.session,
        email: 'alex.customer@example.com',
        name: 'Alex Customer',
        userId: 'github:customer',
      },
    }
    const order = customerOrderFixture({
      id: 'software_order_builder',
      request: 'Build an OTEC public Site.',
      site: {
        activeDeploymentId: 'site_deployment_builder',
        activeUrl: null,
        activeVersionId: 'site_version_builder',
        feedbackCount: 1,
        id: 'site_project_builder',
        latestBuildStatus: 'saved',
        latestSavedVersionId: 'site_version_builder',
        openFeedbackCount: 1,
        status: 'approved',
      },
      status: 'agent_running',
    })
    const baseModel = {
      ...LoggedIn.init(
        OrderDetailRoute({ orderId: 'software_order_builder' }),
        customerAuth,
      ),
      customerOrder: CustomerOrderLoaded({ order }),
    }

    Scene.scene(
      { update, view },
      Scene.with(baseModel),
      Scene.expect(Scene.text('Site builder')).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Start or reconnect builder' }),
      ).toExist(),
      Scene.expect(
        Scene.text('No builder session is open in this browser yet.'),
      ).toExist(),
    )

    Scene.scene(
      { update, view },
      Scene.with({
        ...baseModel,
        customerSiteBuilderSession: CustomerSiteBuilderSessionLoading(),
      }),
      Scene.expect(Scene.text('Opening the builder session...')).toExist(),
    )

    Scene.scene(
      { update, view },
      Scene.with({
        ...baseModel,
        customerSiteBuilderSession: CustomerSiteBuilderSessionFailed({
          error: 'Builder unavailable.',
        }),
      }),
      Scene.expect(Scene.text('Builder unavailable.')).toExist(),
    )

    Scene.scene(
      { update, view },
      Scene.with({
        ...baseModel,
        customerSiteBuilderEvents: CustomerSiteBuilderEventsLoaded({
          events: [
            {
              createdAt: '2026-06-05T15:02:00.000Z',
              eventKind: 'preview_created',
              id: 'site_builder_event_preview',
              phaseKind: 'preview',
              sequence: 7,
              status: 'succeeded',
              summary: 'Preview passed the customer visibility filter.',
              title: 'Preview ready',
            },
          ],
        }),
        customerSiteBuilderFileRead: CustomerSiteBuilderFileReadLoaded({
          file: {
            byteSize: 512,
            contentHash: 'sha256:builder-preview',
            createdAt: '2026-06-05T15:00:00.000Z',
            hasPreview: true,
            id: 'site_builder_file_index',
            language: 'html',
            path: 'src/index.html',
            previewText: '<main>OTEC public Site</main>',
            sequence: 3,
            updatedAt: '2026-06-05T15:01:00.000Z',
            visibility: 'customer',
          },
        }),
        customerSiteBuilderFiles: CustomerSiteBuilderFilesLoaded({
          fileTree: [
            {
              byteSize: 512,
              contentHash: 'sha256:builder-preview',
              createdAt: '2026-06-05T15:00:00.000Z',
              hasPreview: true,
              id: 'site_builder_file_index',
              language: 'html',
              path: 'src/index.html',
              segments: ['src', 'index.html'],
              sequence: 3,
              updatedAt: '2026-06-05T15:01:00.000Z',
              visibility: 'customer',
            },
          ],
          files: [
            {
              byteSize: 512,
              contentHash: 'sha256:builder-preview',
              createdAt: '2026-06-05T15:00:00.000Z',
              hasPreview: true,
              id: 'site_builder_file_index',
              language: 'html',
              path: 'src/index.html',
              sequence: 3,
              updatedAt: '2026-06-05T15:01:00.000Z',
              visibility: 'customer',
            },
          ],
        }),
        customerSiteBuilderSelectedFilePath: 'src/index.html',
        customerSiteBuilderSession: CustomerSiteBuilderSessionLoaded({
          session: {
            activePreview: {
              id: 'site_builder_preview_1',
              previewUrl: 'https://sites.openagents.com/previews/otec',
              status: 'ready',
              updatedAt: '2026-06-05T15:03:00.000Z',
            },
            activePreviewId: 'site_builder_preview_1',
            createdAt: '2026-06-05T15:00:00.000Z',
            currentPhase: {
              phaseKind: 'preview',
              sequence: 3,
              status: 'succeeded',
              summary: 'Preview generated and ready.',
              title: 'Preview',
            },
            id: 'site_builder_session_1',
            messages: [
              {
                actorKind: 'agent',
                body: 'I added a credible OTEC structure and generated files.',
                createdAt: '2026-06-05T15:04:00.000Z',
                id: 'site_builder_message_1',
                sequence: 4,
              },
            ],
            orderId: 'software_order_builder',
            phases: [
              {
                phaseKind: 'planning',
                sequence: 1,
                status: 'succeeded',
                summary: 'The Site brief was parsed.',
                title: 'Planning',
              },
              {
                phaseKind: 'preview',
                sequence: 3,
                status: 'succeeded',
                summary: 'Preview generated and ready.',
                title: 'Preview',
              },
            ],
            promptSummary: 'Build an OTEC public Site.',
            siteId: 'site_project_builder',
            status: 'review_ready',
            updatedAt: '2026-06-05T15:04:00.000Z',
          },
        }),
      }),
      Scene.expect(Scene.text('review ready')).toExist(),
      Scene.expect(Scene.text('A review-ready result is available.')).toExist(),
      Scene.expect(
        Scene.text('https://sites.openagents.com/previews/otec'),
      ).toExist(),
      Scene.expect(Scene.text('Preview ready')).toExist(),
      Scene.expect(Scene.role('button', { name: 'src/index.html' })).toExist(),
      Scene.expect(Scene.text('<main>OTEC public Site</main>')).toExist(),
      Scene.expect(
        Scene.text('I added a credible OTEC structure and generated files.'),
      ).toExist(),
    )

    Scene.scene(
      { update, view },
      Scene.with({
        ...baseModel,
        customerSiteBuilderEvents: CustomerSiteBuilderEventsLoading(),
        customerSiteBuilderFileRead: CustomerSiteBuilderFileReadLoading({
          path: 'src/index.html',
        }),
        customerSiteBuilderFiles: CustomerSiteBuilderFilesLoading(),
        customerSiteBuilderSession: CustomerSiteBuilderSessionLoaded({
          session: {
            activePreview: null,
            activePreviewId: null,
            createdAt: '2026-06-05T15:00:00.000Z',
            currentPhase: null,
            id: 'site_builder_session_loading',
            messages: [],
            orderId: 'software_order_builder',
            phases: [],
            promptSummary: 'Build an OTEC public Site.',
            siteId: 'site_project_builder',
            status: 'building',
            updatedAt: '2026-06-05T15:00:00.000Z',
          },
        }),
      }),
      Scene.expect(Scene.text('Loading generated files...')).toExist(),
      Scene.expect(Scene.text('Loading event stream...')).toExist(),
    )

    Scene.scene(
      { update, view },
      Scene.with({
        ...baseModel,
        customerSiteBuilderEvents: CustomerSiteBuilderEventsFailed({
          error: 'Events unavailable.',
        }),
        customerSiteBuilderFileRead: CustomerSiteBuilderFileReadFailed({
          error: 'File unavailable.',
          path: 'src/index.html',
        }),
        customerSiteBuilderFiles: CustomerSiteBuilderFilesFailed({
          error: 'Files unavailable.',
        }),
        customerSiteBuilderSession: CustomerSiteBuilderSessionLoaded({
          session: {
            activePreview: null,
            activePreviewId: null,
            createdAt: '2026-06-05T15:00:00.000Z',
            currentPhase: null,
            id: 'site_builder_session_failed_children',
            messages: [],
            orderId: 'software_order_builder',
            phases: [],
            promptSummary: 'Build an OTEC public Site.',
            siteId: 'site_project_builder',
            status: 'failed',
            updatedAt: '2026-06-05T15:00:00.000Z',
          },
        }),
      }),
      Scene.expect(Scene.text('Events unavailable.')).toExist(),
      Scene.expect(Scene.text('Files unavailable.')).toExist(),
    )
  })

  test('renders customer software workstreams and new request creation', () => {
    const siteOrder = customerOrderFixture({
      adjutant: {
        activeUrl: 'https://sites.openagents.com/otec',
        adjustmentStatus: null,
        inputNeeded: false,
        nextAction: 'Open the live Site and send any adjustment request.',
        orderStatus: 'delivered',
        reviewNeeded: false,
        siteStatus: 'approved',
        stage: 'deployed',
      },
      createdAt: '2026-06-05T12:01:00.000Z',
      id: 'software_order_site',
      request: 'Build an OTEC public Site.',
      site: {
        activeDeploymentId: 'site_deployment_otec',
        activeUrl: 'https://sites.openagents.com/otec',
        activeVersionId: 'site_version_otec',
        feedbackCount: 1,
        id: 'site_project_otec',
        latestBuildStatus: 'saved',
        latestSavedVersionId: 'site_version_otec',
        openFeedbackCount: 0,
        status: 'approved',
      },
      status: 'delivered',
    })
    const codeOrder = customerOrderFixture({
      createdAt: '2026-06-04T12:01:00.000Z',
      id: 'software_order_pr',
      request: 'Open a pull request for README cleanup.',
      repository: {
        defaultBranch: 'main',
        fullName: 'OpenAgentsInc/example',
        htmlUrl: 'https://github.com/OpenAgentsInc/example',
        name: 'example',
        owner: 'OpenAgentsInc',
        private: false,
        provider: 'github',
      },
    })
    const createdOrder = customerOrderFixture({
      createdAt: '2026-06-05T13:01:00.000Z',
      id: 'software_order_created',
      request: 'Add a billing page.',
    })

    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(OrderRoute(), auth),
        customerOrders: CustomerOrdersLoaded({
          orders: [siteOrder, codeOrder],
        }),
      }),
      Scene.expect(
        Scene.role('heading', { name: 'Software requests' }),
      ).toExist(),
      Scene.expect(Scene.text('Build an OTEC public Site.')).toExist(),
      Scene.expect(
        Scene.text('Open a pull request for README cleanup.'),
      ).toExist(),
      Scene.expect(Scene.text('Site request')).toExist(),
      Scene.expect(Scene.text('Software request')).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'https://sites.openagents.com/otec' }),
      ).toHaveAttr('href', 'https://sites.openagents.com/otec'),
      Scene.expect(
        Scene.selector('a[href="/orders/software_order_site"]'),
      ).toHaveAttr('href', '/orders/software_order_site'),
      Scene.expect(
        Scene.selector('a[href="/orders/software_order_pr"]'),
      ).toHaveAttr('href', '/orders/software_order_pr'),
      Scene.type(
        Scene.role('textbox', { name: 'New software request' }),
        'Add a billing page.',
      ),
      Scene.click(Scene.role('button', { name: 'Submit request' })),
      Scene.Command.resolve(
        SubmitCustomerOrder,
        SucceededSubmitCustomerOrder({
          response: { order: createdOrder },
        }),
        message => GotLoggedInMessage({ message }),
      ),
      Scene.expect(Scene.text('Add a billing page.')).toExist(),
      Scene.expect(
        Scene.role('textbox', { name: 'New software request' }),
      ).toHaveValue(''),
    )
  })

  test('renders the image generation surface', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(ImagesRoute(), auth)),
      Scene.expect(
        Scene.role('heading', { name: 'Generate images' }),
      ).toExist(),
      Scene.expect(Scene.label('Prompt')).toExist(),
      Scene.expect(Scene.text('Provider')).toExist(),
      Scene.expect(Scene.text('Model')).toExist(),
      Scene.expect(Scene.role('button', { name: 'Generate' })).toExist(),
    )
  })

  test('renders the owner-gated GPT-OSS Gym latency playground surface', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(GymOssRoute(), auth)),
      Scene.expect(
        Scene.role('heading', { name: 'GPT-OSS latency playground' }),
      ).toExist(),
      // The interactive surface is mounted via a self-contained custom element.
      // (The neutral lane label + honest-number assertions are covered in
      // gymOss.test.ts; here we only assert the page+element render.)
      Scene.expect(Scene.selector('oa-gym-oss-controller')).toExist(),
    )
  })

  test('renders the /pro operator console shell + teaching Overview for any signed-in user', () => {
    // Explicitly a NON-admin, NON-Core-Team user: /pro is open to any signed-in
    // user, so the console shell + teaching empty state must render for them.
    const plainUser: AuthBootstrap = { ...auth, isAdmin: false, teams: [] }

    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(ProRoute(), plainUser)),
      // The bespoke top-level Pro shell, NOT the workroom sidebar shell.
      Scene.expect(
        Scene.selector('[data-component="logged-in-pro-shell"]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-component="logged-in-workroom-shell"]'),
      ).not.toExist(),
      Scene.expect(Scene.selector('[data-component="pro-console"]')).toExist(),
      Scene.expect(
        Scene.selector('[data-component="pro-top-strip"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-component="pro-register"]')).toExist(),
      // The teaching Overview empty state with its honest forward affordances.
      Scene.expect(
        Scene.selector('[data-component="pro-overview-empty"]'),
      ).toExist(),
      Scene.expect(
        Scene.role('heading', {
          name: 'Pro is a power-user operator console',
        }),
      ).toExist(),
      // #6215: sharing lives at /trace, so the console links out to the public
      // shareable trace surfaces as LIVE affordances.
      Scene.expect(
        Scene.role('link', { name: 'View a shared trace' }),
      ).toExist(),
      Scene.expect(Scene.role('link', { name: 'Compare traces' })).toExist(),
      // The remaining forward affordance stays an honest disabled placeholder.
      Scene.expect(
        Scene.role('button', { name: 'Connect a coding agent' }),
      ).toBeDisabled(),
    )
  })

  test('renders the unified sidebar on the chat route', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(ChatRoute(), auth)),
      Scene.expect(
        Scene.role('navigation', { name: 'OpenAgents navigation' }),
      ).toExist(),
      Scene.expect(Scene.role('link', { name: 'OpenAgents' })).toHaveAttr(
        'href',
        '/',
      ),
      Scene.expect(Scene.text('Workroom')).not.toExist(),
      Scene.expect(Scene.text('Dashboard')).toBeAbsent(),
      Scene.expect(Scene.text('Billing')).toExist(),
      Scene.expect(Scene.text('Settings')).toExist(),
      Scene.expect(Scene.role('link', { name: 'Files' })).toHaveAttr(
        'href',
        '/teams/openagents-core-team/files',
      ),
      Scene.expect(Scene.text('OpenAgents Core Team')).toExist(),
      Scene.expect(Scene.text('Threads')).not.toExist(),
      Scene.expect(Scene.text('My threads')).not.toExist(),
      Scene.expect(Scene.text('Team threads')).not.toExist(),
      Scene.expect(Scene.text('Run the smoke test')).not.toExist(),
      Scene.expect(Scene.text('Investigate sidebar missions')).not.toExist(),
      Scene.expect(Scene.text('Christopher David')).toExist(),
      Scene.expect(
        Scene.selector(
          'img[src="https://avatars.githubusercontent.com/u/14167547?v=4"]',
        ),
      ).toExist(),
      Scene.expect(Scene.role('button', { name: 'New thread' })).toExist(),
    )
  })

  test('renders the private Mullet scenario editor and loaded scenario tables', () => {
    const [model] = LoggedIn.update(
      LoggedIn.init(MulletRoute(), auth),
      SucceededLoadMulletBootstrap({
        response: {
          access: {
            operatorEmail: 'chris@openagents.com',
            visibility: 'private',
          },
          authorityBoundary: {
            canAssignLiveWork: false,
            canMutateProviders: false,
            canPromotePublicClaims: false,
            canSettlePayouts: false,
            canSpendWalletFunds: false,
          },
          routes: ['/api/mullet/bootstrap', '/api/mullet/runs'],
          schemaVersion: '2026-06-08.v1',
        },
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.role('heading', { name: 'Mullet' })).toExist(),
      Scene.expect(Scene.text('Tinybox SHC power')).toExist(),
      Scene.expect(Scene.text('Tinybox residential')).toExist(),
      Scene.expect(Scene.text('100 MW 80/20 facility')).toExist(),
      Scene.expect(Scene.text('Scenario editor')).toExist(),
      Scene.expect(Scene.text('Electricity price')).toExist(),
      Scene.expect(Scene.text('Provider share')).toExist(),
      Scene.expect(Scene.text('Hourly dispatch')).toExist(),
      Scene.expect(Scene.text('Accepted work')).toExist(),
      Scene.expect(Scene.text('Party-specific view')).toExist(),
      Scene.expect(Scene.text('Buyer revenue basis')).toExist(),
      Scene.expect(Scene.text('Accepted outcomes/kWh')).toExist(),
      Scene.expect(Scene.text('Decision flips')).toExist(),
      Scene.expect(Scene.text('Acceptance rate')).toExist(),
      Scene.expect(Scene.text('Raw GPU rate')).toExist(),
      Scene.expect(Scene.text('Modeled')).toExist(),
      Scene.expect(Scene.text('Measured')).toExist(),
      Scene.expect(Scene.text('Accepted')).toExist(),
      Scene.expect(Scene.text('Paid')).toExist(),
      Scene.expect(Scene.text('Settled')).toExist(),
      Scene.expect(Scene.text('Missing measured energy')).toExist(),
      Scene.expect(Scene.text('Missing accepted-work demand')).toExist(),
      Scene.expect(Scene.text('Missing settlement evidence')).toExist(),
      Scene.expect(Scene.text('Missing Margot baseline import')).toExist(),
      Scene.expect(Scene.text('Missing readiness proof')).toExist(),
      Scene.expect(Scene.text('Missing payout proof')).toExist(),
    )
  })

  test('renders Stats token totals, filters, anonymous rows, and safe drilldowns', () => {
    const tokenCounts = {
      cacheReadTokens: 20,
      cacheWrite1hTokens: 3,
      cacheWrite5mTokens: 2,
      inputTokens: 120,
      outputTokens: 60,
      reasoningTokens: 15,
      totalTokens: 220,
    }
    const [model] = LoggedIn.update(
      LoggedIn.init(StatsRoute(), auth),
      SucceededLoadTokenUsageStats({
        filters: {
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
        },
        leaderboards: {
          schemaVersion: 'openagents.token_usage_leaderboards.v1',
          anonymousTotals: {
            ...tokenCounts,
            totalTokens: 15,
          },
          filters: {
            since: '2026-06-01T12:00:00.000Z',
            window: '7d',
          },
          generatedAt: '2026-06-08T12:00:00.000Z',
          globalTotals: {
            ...tokenCounts,
            totalTokens: 235,
          },
          topProviderModels: [
            {
              key: 'google_gemini:gemini-2.5-flash',
              label: 'google_gemini / gemini-2.5-flash',
              tokenCounts,
              usageEvents: 2,
            },
          ],
          topProjects: [
            {
              key: 'repository:OpenAgentsInc/autopilot-omega',
              label: 'repository / OpenAgentsInc/autopilot-omega',
              tokenCounts,
              usageEvents: 1,
            },
          ],
          topRuns: [
            {
              key: 'run:probe-run:scene',
              label: 'run / probe-run:scene',
              tokenCounts,
              usageEvents: 1,
            },
          ],
          topTeams: [
            {
              accountRef: null,
              anonymous: false,
              teamId: 'team_openagents_core',
              tokenCounts,
              usageEvents: 1,
              userId: null,
            },
          ],
          topUsers: [
            {
              accountRef: null,
              anonymous: false,
              teamId: null,
              tokenCounts,
              usageEvents: 1,
              userId: 'github:14167547',
            },
          ],
        },
        preference: {
          schemaVersion: 'openagents.token_usage_leaderboard_preference.v1',
          preference: {
            leaderboardParticipation: 'eligible',
            leaderboardVisibility: 'internal',
            subjectKind: 'user',
            subjectRef: 'github:14167547',
            updatedAt: '2026-06-08T12:00:00.000Z',
            updatedByUserId: 'github:14167547',
          },
        },
        response: S.decodeUnknownSync(TokenUsageAggregateResponse)({
          schemaVersion: 'openagents.token_usage_aggregate.v1',
          byActor: [
            {
              accountRef: null,
              anonymous: false,
              teamId: 'team_openagents_core',
              tokenCounts,
              usageEvents: 1,
              userId: 'github:14167547',
            },
            {
              accountRef: null,
              anonymous: true,
              teamId: null,
              tokenCounts: {
                ...tokenCounts,
                totalTokens: 15,
              },
              usageEvents: 1,
              userId: null,
            },
          ],
          byProviderModel: [
            {
              key: 'google_gemini:gemini-2.5-flash',
              label: 'google_gemini / gemini-2.5-flash',
              tokenCounts,
              usageEvents: 2,
            },
          ],
          bySourceRoute: [
            {
              key: 'omega:omega_provider_broker',
              label: 'omega / omega_provider_broker',
              tokenCounts,
              usageEvents: 1,
            },
            {
              key: 'probe:probe_direct_provider',
              label: 'probe / probe_direct_provider',
              tokenCounts: {
                ...tokenCounts,
                totalTokens: 15,
              },
              usageEvents: 1,
            },
          ],
          bySourceRef: [
            {
              key: 'run:probe-run:scene',
              label: 'run / probe-run:scene',
              tokenCounts,
              usageEvents: 1,
            },
            {
              key: 'repository:OpenAgentsInc/autopilot-omega',
              label: 'repository / OpenAgentsInc/autopilot-omega',
              tokenCounts,
              usageEvents: 1,
            },
          ],
          byUsageTruth: [
            {
              key: 'exact',
              label: 'exact',
              tokenCounts,
              usageEvents: 1,
            },
            {
              key: 'estimated',
              label: 'estimated',
              tokenCounts: {
                ...tokenCounts,
                totalTokens: 15,
              },
              usageEvents: 1,
            },
          ],
          filters: {},
          generatedAt: '2026-06-08T12:00:00.000Z',
          recentEvents: [
            {
              schemaVersion: 'openagents.token_usage_event.record.v1',
              actor: {
                teamId: 'team_openagents_core',
                userId: 'github:14167547',
              },
              backendProfile: null,
              cost: null,
              demand: {
                demandKind: 'external',
                demandSource: 'public-api',
                demandClient: 'sdk',
              },
              eventId: 'token_event_scene_1',
              idempotencyKey: 'scene:1',
              ingestedAt: '2026-06-08T12:00:00.000Z',
              model: 'gemini-2.5-flash',
              observedAt: '2026-06-08T11:59:00.000Z',
              privacy: {
                leaderboardEligible: true,
                privacyOptOut: false,
              },
              producerSystem: 'omega',
              provider: 'google_gemini',
              safeMetadata: {
                providerRequestStatus: 'succeeded',
                rawPrompt: 'should never render',
              },
              sourceRefs: {
                repositoryRef: 'OpenAgentsInc/autopilot-omega',
                runRef: 'probe-run:scene',
              },
              sourceRoute: 'omega_provider_broker',
              tokenCounts,
              usageTruth: 'exact',
            },
            {
              schemaVersion: 'openagents.token_usage_event.record.v1',
              actor: {},
              backendProfile: null,
              cost: null,
              demand: {
                demandKind: 'unlabeled',
              },
              eventId: 'token_event_scene_2',
              idempotencyKey: 'scene:2',
              ingestedAt: '2026-06-08T12:01:00.000Z',
              model: null,
              observedAt: '2026-06-08T12:01:00.000Z',
              privacy: {
                leaderboardEligible: false,
                privacyOptOut: true,
              },
              producerSystem: 'probe',
              provider: null,
              safeMetadata: {
                providerRequestStatus: 'estimated',
                sourcePath: '/Users/chris/private/repo',
              },
              sourceRefs: {
                anonymizedSourceRef: 'probe-session-hash:anonymous',
              },
              sourceRoute: 'probe_direct_provider',
              tokenCounts: {
                ...tokenCounts,
                totalTokens: 15,
              },
              usageTruth: 'estimated',
            },
          ],
          totals: {
            ...tokenCounts,
            totalTokens: 235,
          },
          usageEvents: 2,
        }),
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.role('heading', { name: 'Token ledger' })).toExist(),
      Scene.expect(Scene.text('Total tokens')).toExist(),
      Scene.expect(Scene.text('235')).toExist(),
      Scene.expect(Scene.text('Provider')).toExist(),
      Scene.expect(Scene.text('Source route')).toExist(),
      Scene.expect(Scene.text('eligible / internal')).toExist(),
      Scene.expect(Scene.text('Top users')).toExist(),
      Scene.expect(Scene.text('Top teams')).toExist(),
      Scene.expect(Scene.text('Provider/model leaderboard')).toExist(),
      Scene.expect(Scene.text('google_gemini / gemini-2.5-flash')).toExist(),
      Scene.expect(Scene.text('omega / omega_provider_broker')).toExist(),
      Scene.expect(Scene.text('run / probe-run:scene')).toExist(),
      Scene.expect(
        Scene.text('repository / OpenAgentsInc/autopilot-omega'),
      ).toExist(),
      Scene.expect(Scene.text('Anonymous/anonymized source')).toExist(),
      Scene.expect(Scene.text('Privacy opt-out')).toExist(),
      Scene.expect(
        Scene.text(
          'repo:OpenAgentsInc/autopilot-omega | run:probe-run:scene | providerRequestStatus:succeeded',
        ),
      ).toExist(),
      Scene.expect(Scene.text('should never render')).not.toExist(),
      Scene.expect(Scene.text('/Users/chris/private/repo')).not.toExist(),
      Scene.expect(Scene.text('rawPrompt')).not.toExist(),
    )
  })

  test('renders prefilled workspace memory, starters, and intro receipt', () => {
    const [model] = LoggedIn.update(
      LoggedIn.init(WorkspaceRoute({ workspaceId: 'workspace_seed' }), auth),
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

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(
        Scene.selector('[data-route="prefilled-workspace"]'),
      ).toExist(),
      Scene.expect(
        Scene.role('heading', { name: 'Seeded Storefront Sprint' }),
      ).toExist(),
      Scene.expect(
        Scene.text('Workspace prepared from public sources.'),
      ).toExist(),
      Scene.expect(Scene.text('1. Draft product page update')).toExist(),
      Scene.expect(Scene.text('Public catalog is live.')).toExist(),
      Scene.expect(Scene.role('link', { name: 'Open Work' })).toHaveAttr(
        'href',
        '/autopilot/work',
      ),
    )
  })

  test('renders the admin overview for configured admins', () => {
    const [overviewModel] = LoggedIn.update(
      LoggedIn.init(AdminRoute(), auth),
      SucceededLoadAdminOverview({
        response: {
          users: [
            {
              userId: 'github:14167547',
              kind: 'human',
              displayName: 'Christopher David',
              email: 'chris@openagents.com',
              githubUsername: 'chris',
              status: 'active',
              onboardingStep: 'complete',
              onboardingCompletedAt: '2026-06-04T12:00:00.000Z',
              softwareOrderCount: 1,
              createdAt: '2026-06-04T11:00:00.000Z',
              updatedAt: '2026-06-04T12:00:00.000Z',
            },
          ],
          softwareOrders: [
            {
              id: 'software_order_scene',
              userId: 'github:14167547',
              userDisplayName: 'Christopher David',
              userEmail: 'chris@openagents.com',
              status: 'submitted',
              visibility: 'public',
              request:
                '# Admin panel request\n\nAdd an admin panel with users and software orders.\n\n- Show users\n- Show software orders',
              repositoryFullName: 'bensilone/openagents',
              currentRunId: null,
              siteProjectId: 'site_project_otec',
              siteTitle: 'OTEC Site',
              siteSlug: 'otec',
              siteStatus: 'draft',
              siteAccessMode: 'public',
              siteVisibility: 'public',
              siteActiveVersionId: 'site_version_1',
              siteActiveDeploymentId: 'site_deployment_1',
              siteActiveUrl: 'https://sites.openagents.com/otec',
              siteVersionCount: 2,
              siteLatestVersionId: 'site_version_2',
              siteLatestVersionStatus: 'saved',
              siteLatestVersionSourceKind: 'autopilot_generated',
              siteLatestVersionCreatedAt: '2026-06-04T12:04:00.000Z',
              siteDeploymentCount: 1,
              siteLatestDeploymentId: 'site_deployment_1',
              siteLatestDeploymentStatus: 'active',
              siteLatestDeploymentRuntimeKind: 'omega_static_r2',
              siteLatestDeploymentUpdatedAt: '2026-06-04T12:05:00.000Z',
              siteStorageBindingCount: 2,
              siteStorageBindingSummary: 'd1:SITE_DB, r2:SITE_ASSETS',
              siteEnvironmentValueCount: 1,
              siteEnvironmentKeySummary: 'OPENAI_API_KEY:secret',
              siteAccessGrantCount: 1,
              siteLatestEventType: 'site_version.saved',
              siteLatestEventSummary: 'Saved Site version site_version_2.',
              siteLatestEventCreatedAt: '2026-06-04T12:04:00.000Z',
              siteLatestCompatibilityId: 'site_compatibility_check_1',
              siteLatestCompatibilityStatus: 'ready',
              siteLatestCompatibilityCustomerSafeStatus:
                'The Site is compatible with static hosting.',
              siteLatestCompatibilityCustomerSafeNextAction:
                'Run build validation.',
              siteLatestCompatibilityBlockerCount: 0,
              siteLatestCompatibilityWarningCount: 1,
              siteLatestCompatibilityCreatedAt: '2026-06-04T12:03:00.000Z',
              siteLatestBuildValidationId: 'site_build_validation_1',
              siteLatestBuildValidationStatus: 'passed',
              siteLatestBuildValidationSourceHash: 'sha256:site',
              siteLatestBuildValidationCustomerSafeStatus:
                'The latest build passed.',
              siteLatestBuildValidationCustomerSafeNextAction:
                'Review and deploy the saved version.',
              siteLatestBuildValidationBlockerCount: 0,
              siteLatestBuildValidationWarningCount: 0,
              siteLatestBuildValidationCreatedAt: '2026-06-04T12:04:30.000Z',
              createdAt: '2026-06-04T12:01:00.000Z',
              updatedAt: '2026-06-04T12:01:00.000Z',
              archivedAt: null,
            },
          ],
        },
      }),
    )
    const [model] = LoggedIn.update(
      overviewModel,
      SucceededLoadAdminAdjutantAssignments({
        response: {
          assignments: [
            {
              agentId: 'agent_adjutant',
              archivedAt: null,
              assignedByUserId: 'github:14167547',
              assignmentKind: 'site_generation',
              blockedAt: null,
              commitSha: 'abc1234',
              completedAt: null,
              createdAt: '2026-06-04T12:02:00.000Z',
              currentRunId: 'agent_run_otec',
              goalId: 'agent_goal_otec',
              id: 'adjutant_assignment_otec',
              objective: 'Build OTEC Site.',
              projectId: null,
              siteId: 'site_project_otec',
              softwareOrderId: 'software_order_scene',
              status: 'review_needed',
              taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
              teamId: null,
              updatedAt: '2026-06-04T12:03:00.000Z',
              visibility: 'public',
            },
          ],
        },
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.role('link', { name: 'Admin' })).toHaveAttr(
        'href',
        '/admin',
      ),
      Scene.expect(Scene.role('heading', { name: 'Overview' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Sites' })).toExist(),
      Scene.expect(Scene.text('Christopher David')).toExist(),
      Scene.expect(Scene.text('chris@openagents.com')).toExist(),
      Scene.expect(Scene.text('software_order_scene')).toExist(),
      Scene.expect(Scene.text('otec')).toExist(),
      Scene.expect(Scene.text('Saved Site version site_version_2.')).toExist(),
      Scene.expect(Scene.text('d1:SITE_DB, r2:SITE_ASSETS')).toExist(),
      Scene.expect(Scene.text('OPENAI_API_KEY:secret')).toExist(),
      Scene.expect(Scene.text('1 access grant')).toExist(),
      Scene.expect(
        Scene.text('The Site is compatible with static hosting.'),
      ).toExist(),
      Scene.expect(Scene.text('The latest build passed.')).toExist(),
      Scene.expect(Scene.text('0 blockers / 1 warnings')).toExist(),
      Scene.expect(Scene.text('Public proof')).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'https://sites.openagents.com/otec' }),
      ).toHaveAttr('href', 'https://sites.openagents.com/otec'),
      Scene.expect(Scene.role('button', { name: 'Review' })).toExist(),
      Scene.expect(Scene.role('button', { name: 'Generate' })).toExist(),
      Scene.expect(Scene.role('button', { name: 'Deploy' })).toExist(),
      Scene.expect(Scene.role('button', { name: 'Disable' })).toExist(),
      Scene.expect(Scene.text('Open full request')).toExist(),
      Scene.expect(
        Scene.role('heading', { name: 'Admin panel request' }),
      ).toExist(),
      Scene.expect(Scene.text('Show software orders')).toExist(),
      Scene.expect(Scene.role('button', { name: 'Refresh' })).toExist(),
    )
  })

  test('renders the admin Adjutant review with Site usage receipts', () => {
    const [overviewModel] = LoggedIn.update(
      LoggedIn.init(AdminRoute(), auth),
      SucceededLoadAdminOverview({
        response: {
          users: [],
          softwareOrders: [],
        },
      }),
    )
    const assignment = {
      agentId: 'agent_adjutant',
      archivedAt: null,
      assignedByUserId: 'github:14167547',
      assignmentKind: 'site_generation',
      blockedAt: null,
      commitSha: '489af93c',
      completedAt: null,
      createdAt: '2026-06-05T00:00:00.000Z',
      currentRunId: 'agent_run_adjutant_otec',
      goalId: 'agent_goal_adjutant_otec',
      id: 'adjutant_assignment_otec',
      objective: 'Build Ben OTEC public Site.',
      projectId: 'project_adjutant',
      siteId: 'site_project_otec',
      softwareOrderId: 'software_order_otec',
      status: 'review_needed',
      taskSpecPath: 'docs/autopilot-tasks/2026-06-05-adjutant-ben-otec.md',
      teamId: 'team_openagents_core',
      updatedAt: '2026-06-05T00:10:00.000Z',
      visibility: 'public' as const,
    }
    const review = {
      assignmentEvents: [
        {
          createdAt: '2026-06-05T00:01:00.000Z',
          id: 'adjutant_assignment_event_launch',
          runId: 'agent_run_adjutant_otec',
          summary: 'Adjutant launch queued for Site generation.',
          type: 'adjutant.launch_queued',
        },
      ],
      currentRun: {
        backend: 'shc_vm',
        createdAt: '2026-06-05T00:01:00.000Z',
        eventCursor: 7,
        externalRunId: null,
        id: 'agent_run_adjutant_otec',
        runtime: 'opencode_codex',
        status: 'completed',
        updatedAt: '2026-06-05T00:08:00.000Z',
      },
      deployments: [
        {
          activatedAt: '2026-06-05T00:11:00.000Z',
          disabledAt: null,
          externalDeploymentId: null,
          id: 'site_deployment_otec',
          rolledBackAt: null,
          runtimeKind: 'omega_static_r2',
          status: 'active',
          updatedAt: '2026-06-05T00:11:00.000Z',
          url: 'https://sites.openagents.com/otec',
          versionId: 'site_version_otec',
        },
      ],
      enrichment: {
        exaConfigured: false,
        latestRun: null,
        nextAction: 'Configure Exa before running enrichment.',
        queries: [],
        researchBrief: null,
        sourceCards: [],
        sourceRefs: [],
        status: 'not_configured',
      },
      goal: {
        agentId: 'agent_adjutant',
        currentRunId: 'agent_run_adjutant_otec',
        id: 'agent_goal_adjutant_otec',
        status: 'active',
        timeUsedSeconds: 600,
        tokenBudget: 120000,
        tokensUsed: 42000,
        updatedAt: '2026-06-05T00:10:00.000Z',
        visibility: 'public',
      },
      nextAction:
        'Review saved Site version and deploy only after the Sites checklist passes.',
      order: {
        createdAt: '2026-06-05T00:00:00.000Z',
        currentRunId: 'agent_run_adjutant_otec',
        id: 'software_order_otec',
        repositoryFullName: 'OpenAgentsInc/autopilot-omega',
        request: 'Build the OTEC floating datacenter Site.',
        status: 'delivered',
        updatedAt: '2026-06-05T00:10:00.000Z',
        visibility: 'public',
      },
      researchBrief: null,
      site: {
        accessMode: 'public',
        activeDeploymentId: 'site_deployment_otec',
        activeVersionId: null,
        id: 'site_project_otec',
        slug: 'otec',
        status: 'generated',
        title: 'Ben OTEC Site',
        visibility: 'public',
      },
      siteEvents: [
        {
          createdAt: '2026-06-05T00:08:00.000Z',
          id: 'site_event_saved',
          runId: 'agent_run_adjutant_otec',
          summary: 'Saved Site version site_version_otec.',
          type: 'site_version.saved',
        },
      ],
      usageReceipts: [
        {
          adjustmentId: null,
          assignmentId: 'adjutant_assignment_otec',
          billingLedgerEntryId: null,
          billingMode: 'public_beta_free' as const,
          category: 'generation' as const,
          createdAt: '2026-06-05T00:01:00.000Z',
          creditsChargedCents: 0,
          creditsChargedFormatted: '$0.00',
          currency: 'USD',
          id: 'adjutant_usage_generation',
          publicDetails: {},
          quantity: 1,
          runId: 'agent_run_adjutant_otec',
          siteId: 'site_project_otec',
          softwareOrderId: 'software_order_otec',
          summary: 'Adjutant Site generation run was queued.',
          teamDetails: {},
          unit: 'run',
          visibility: 'public' as const,
        },
        {
          adjustmentId: null,
          assignmentId: 'adjutant_assignment_otec',
          billingLedgerEntryId: null,
          billingMode: 'public_beta_free' as const,
          category: 'hosting' as const,
          createdAt: '2026-06-05T00:11:00.000Z',
          creditsChargedCents: 0,
          creditsChargedFormatted: '$0.00',
          currency: 'USD',
          id: 'adjutant_usage_hosting',
          publicDetails: {},
          quantity: 1,
          runId: 'agent_run_adjutant_otec',
          siteId: 'site_project_otec',
          softwareOrderId: 'software_order_otec',
          summary: 'Adjutant activated public Site hosting.',
          teamDetails: {},
          unit: 'deployment',
          visibility: 'public' as const,
        },
      ],
      usageSummary: {
        billingMode: 'public_beta_free' as const,
        categories: [
          {
            category: 'generation' as const,
            creditsChargedCents: 0,
            creditsChargedFormatted: '$0.00',
            quantity: 1,
            receiptCount: 1,
            unit: 'run',
          },
          {
            category: 'hosting' as const,
            creditsChargedCents: 0,
            creditsChargedFormatted: '$0.00',
            quantity: 1,
            receiptCount: 1,
            unit: 'deployment',
          },
        ],
        totalCreditsChargedCents: 0,
        totalCreditsChargedFormatted: '$0.00',
      },
      versions: [
        {
          buildCommand: 'bun run build',
          buildStatus: 'saved',
          createdAt: '2026-06-05T00:08:00.000Z',
          createdByRunId: 'agent_run_adjutant_otec',
          id: 'site_version_otec',
          rejectedAt: null,
          savedAt: '2026-06-05T00:08:00.000Z',
          sourceCommitSha: '489af93c',
          sourceKind: 'autopilot_generated',
          workerModuleR2Key: null,
        },
      ],
    }

    Scene.scene(
      { update, view },
      Scene.with({
        ...overviewModel,
        adminAdjutantAssignments: AdminAdjutantAssignmentsLoaded({
          assignments: [assignment],
        }),
        adminAdjutantReview: AdminAdjutantReviewLoaded({
          assignment,
          review,
        }),
      }),
      Scene.expect(
        Scene.role('heading', { name: 'Autopilot reviews' }),
      ).toExist(),
      Scene.expect(Scene.text('adjutant_assignment_otec')).toExist(),
      Scene.expect(Scene.text('Site: Ben OTEC Site (otec)')).toExist(),
      Scene.expect(Scene.text('Goal: agent_goal_adjutant_otec')).toExist(),
      Scene.expect(Scene.text('Usage receipts')).toExist(),
      Scene.expect(Scene.text('Billing: Public beta free')).toExist(),
      Scene.expect(Scene.text('Total: $0.00')).toExist(),
      Scene.expect(Scene.text('generation: 1 run / $0.00')).toExist(),
      Scene.expect(Scene.text('hosting: 1 deployment / $0.00')).toExist(),
      Scene.expect(
        Scene.text('Autopilot Site generation run was queued.'),
      ).toExist(),
      Scene.expect(
        Scene.text('Autopilot activated public Site hosting.'),
      ).toExist(),
      Scene.expect(
        Scene.text('Saved Site version site_version_otec.'),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'https://sites.openagents.com/otec' }),
      ).toHaveAttr('href', 'https://sites.openagents.com/otec'),
      Scene.expect(
        Scene.role('button', { name: 'Deploy with checklist' }),
      ).toExist(),
    )
  })

  test('highlights the active mission route in the sidebar', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        syncedMissionModel(
          ThreadRoute({
            threadId: '22222222-2222-4222-8222-222222222222',
          }),
        ),
      ),
      Scene.expect(Scene.text('Threads')).not.toExist(),
      Scene.expect(Scene.text('My threads')).toExist(),
      Scene.expect(Scene.text('Team threads')).toExist(),
      Scene.expect(Scene.text('Run the smoke test')).toExist(),
      Scene.expect(Scene.text('Investigate sidebar missions')).toExist(),
      Scene.expect(
        Scene.selector('a[href="/t/22222222-2222-4222-8222-222222222222"]'),
      ).toHaveClass('border-[#ffb400]/70'),
    )
  })

  test('renders linked child thread routes without the removed chat top bar', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(
          ThreadRoute({ threadId: teamAutopilotThreadId }),
          auth,
        ),
        teamChatMessagesByTeam: {
          team_openagents_core: [
            {
              agentRunId: 'agent_run_scene_1',
              author: {
                avatarUrl: null,
                githubUsername: 'chris',
                name: 'Christopher David',
                userId: 'github:14167547',
              },
              autopilotThreadId: teamAutopilotThreadId,
              body: 'summarize that PDF',
              createdAt: '2026-06-03T00:00:00.000Z',
              id: 'team_chat_parent',
              kind: 'autopilot_intent',
              teamId: 'team_openagents_core',
            },
          ],
        },
      }),
      Scene.expect(Scene.text('Workroom')).not.toExist(),
      Scene.expect(
        Scene.selector('[data-component="chat-workroom"]'),
      ).toHaveClass('h-full'),
      Scene.expect(
        Scene.selector('[data-component="thread-back-to-chat"]'),
      ).not.toExist(),
      Scene.expect(Scene.text('Back to chat')).not.toExist(),
    )
  })

  test('renders run-id child thread routes without the removed chat top bar', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(ThreadRoute({ threadId: 'agent_run_scene_1' }), auth),
        teamChatMessagesByTeam: {
          team_openagents_core: [
            {
              agentRunId: 'agent_run_scene_1',
              author: {
                avatarUrl: null,
                githubUsername: 'chris',
                name: 'Christopher David',
                userId: 'github:14167547',
              },
              autopilotThreadId: teamAutopilotThreadId,
              body: 'summarize that PDF',
              createdAt: '2026-06-03T00:00:00.000Z',
              id: 'team_chat_parent',
              kind: 'autopilot_intent',
              teamId: 'team_openagents_core',
            },
          ],
        },
      }),
      Scene.expect(
        Scene.selector('[data-component="chat-workroom"]'),
      ).toHaveClass('h-full'),
      Scene.expect(
        Scene.selector('[data-component="thread-back-to-chat"]'),
      ).not.toExist(),
    )
  })

  test('links team rooms to team chat routes instead of the generic chat route', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedIn.init(TeamChatRoute({ teamRef: 'openagents-core-team' }), auth),
      ),
      Scene.expect(
        Scene.text('Team room / OpenAgents Core Team'),
      ).not.toExist(),
      Scene.expect(
        Scene.selector('a[href="/teams/openagents-core-team/chat"]'),
      ).toHaveAttr('href', teamChatRouter({ teamRef: 'openagents-core-team' })),
      Scene.expect(
        Scene.selector('[data-component="chat-workroom"]'),
      ).toHaveClass('h-full'),
      Scene.expect(
        Scene.selector('[data-component="chat-workroom"]'),
      ).toHaveClass('flex-1'),
      Scene.expect(
        Scene.selector('[data-component="session-side-panel"]'),
      ).toExist(),
      Scene.expect(Scene.text('Autopilot goal')).toExist(),
      Scene.expect(Scene.label('Upload file')).toHaveAttr('type', 'file'),
      Scene.expect(Scene.text('9 members')).not.toExist(),
      Scene.expect(Scene.text('Repo writeback run')).not.toExist(),
      Scene.expect(Scene.text('Running sync')).not.toExist(),
      Scene.expect(Scene.text('Pull request')).not.toExist(),
      Scene.expect(Scene.text('Search')).not.toExist(),
      Scene.expect(Scene.text('@ file')).not.toExist(),
      Scene.expect(Scene.text('image')).not.toExist(),
      Scene.expect(Scene.text('mode: build')).not.toExist(),
      Scene.expect(Scene.text('Context')).not.toExist(),
      Scene.expect(Scene.text('Files')).toExist(),
      Scene.expect(Scene.text('Review')).not.toExist(),
    )
  })

  test('renders the workroom goal controls in the side panel', () => {
    const [model] = LoggedIn.update(
      LoggedIn.init(ChatRoute(), auth),
      SucceededLoadAgentGoal({
        response: { goal: activeGoal },
        scopeKey: 'autopilot:personal:room',
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.text('Autopilot goal')).toExist(),
      Scene.expect(
        Scene.text('Keep the public workroom progressing'),
      ).toExist(),
      Scene.expect(Scene.role('button', { name: 'Edit goal' })).toExist(),
      Scene.expect(Scene.role('button', { name: 'Pause' })).toExist(),
    )
  })

  test('uploads team chat files from the composer file picker', () => {
    const file = new File(['hello world'], 'notes.txt', { type: 'text/plain' })

    Scene.scene(
      { update, view },
      Scene.with(
        LoggedIn.init(TeamChatRoute({ teamRef: 'openagents-core-team' }), auth),
      ),
      Scene.changeFiles(Scene.label('Upload file'), [file]),
      Scene.expect(Scene.text('Uploading')).toExist(),
      Scene.Command.resolve(
        UploadThreadFile,
        SucceededUploadThreadFile({
          response: { file: teamThreadFileDto },
          scopeKey: 'thread:team:team_openagents_core:chat',
        }),
        message => GotLoggedInMessage({ message }),
      ),
      Scene.expect(Scene.text('notes.txt uploaded.')).toExist(),
    )
  })

  test('uploads team chat files dropped anywhere on the composer dock', () => {
    const file = new File(['dropped'], 'dropped.txt', { type: 'text/plain' })

    Scene.scene(
      { update, view },
      Scene.with(
        LoggedIn.init(TeamChatRoute({ teamRef: 'openagents-core-team' }), auth),
      ),
      Scene.dropFiles(
        Scene.selector('[data-component="session-prompt-dock"]'),
        [file],
      ),
      Scene.expect(Scene.text('Uploading')).toExist(),
      Scene.Command.resolve(
        UploadThreadFile,
        SucceededUploadThreadFile({
          response: {
            file: { ...teamThreadFileDto, filename: 'dropped.txt' },
          },
          scopeKey: 'thread:team:team_openagents_core:chat',
        }),
        message => GotLoggedInMessage({ message }),
      ),
      Scene.expect(Scene.text('dropped.txt uploaded.')).toExist(),
    )
  })

  test('links team file rows to first-party file detail pages', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(
          TeamFilesRoute({ teamRef: 'openagents-core-team' }),
          auth,
        ),
        threadFilesByScope: {
          'team-files:team_openagents_core': [teamThreadFile],
        },
      }),
      Scene.expect(Scene.text('OpenAgents Core Team files')).toExist(),
      Scene.expect(
        Scene.selector('a[href="/teams/openagents-core-team/files/file_1"]'),
      ).toHaveAttr('href', '/teams/openagents-core-team/files/file_1'),
      Scene.expect(
        Scene.selector('a[href="/api/thread-files/file_1/download"]'),
      ).not.toExist(),
    )
  })

  test('renders team file detail pages with download controls and message backlinks', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(
          TeamFileRoute({
            fileId: 'file_1',
            teamRef: 'openagents-core-team',
          }),
          auth,
        ),
        threadFileDetailsById: {
          file_1: teamThreadFileDetail,
        },
      }),
      Scene.expect(Scene.text('Team file')).toExist(),
      Scene.expect(Scene.role('heading', { name: 'notes.txt' })).toExist(),
      Scene.expect(
        Scene.selector('a[href="/api/thread-files/file_1/download"]'),
      ).not.toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Download raw file' }),
      ).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Disable download' }),
      ).toExist(),
      Scene.expect(Scene.text('Message references')).toExist(),
      Scene.expect(
        Scene.selector(
          'a[href="/teams/openagents-core-team/chat#message-team_chat_1"]',
        ),
      ).toHaveAttr(
        'href',
        '/teams/openagents-core-team/chat#message-team_chat_1',
      ),
      Scene.expect(
        Scene.text('Please inspect notes.txt before the Autopilot run.'),
      ).toExist(),
    )
  })

  test('renders file download API errors on the file detail page', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(
          TeamFileRoute({
            fileId: 'file_1',
            teamRef: 'openagents-core-team',
          }),
          auth,
        ),
        threadFileDetailsById: {
          file_1: teamThreadFileDetail,
        },
        threadFileDownloadErrorsById: {
          file_1: 'Unauthorized. Reopen settings and refresh your session.',
        },
      }),
      Scene.expect(
        Scene.text('Unauthorized. Reopen settings and refresh your session.'),
      ).toExist(),
    )
  })

  test('renders team Autopilot intents as compact linked run cards', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(
          TeamChatRoute({ teamRef: 'openagents-core-team' }),
          auth,
        ),
        chatRun: activeTeamRun,
        teamChatMessagesByTeam: {
          team_openagents_core: [
            {
              agentRunId: 'agent_run_scene_1',
              author: {
                avatarUrl: null,
                githubUsername: 'chris',
                name: 'Christopher David',
                userId: 'github:14167547',
              },
              autopilotThreadId: teamAutopilotThreadId,
              body: 'summarize that PDF',
              createdAt: '2026-06-03T00:00:00.000Z',
              id: 'team_chat_parent',
              kind: 'autopilot_intent',
              teamId: 'team_openagents_core',
            },
          ],
        },
      }),
      Scene.expect(Scene.text('@autopilot summarize that PDF')).toExist(),
      Scene.expect(Scene.text('Autopilot run')).toExist(),
      Scene.expect(Scene.text('opencode_codex on shc_vm')).toExist(),
      Scene.expect(Scene.text('Succeeded in 3m 20s')).toExist(),
      Scene.expect(Scene.text('2 events')).toExist(),
      Scene.expect(Scene.text('1 tool call')).toExist(),
      Scene.expect(Scene.text('linked child thread')).not.toExist(),
      Scene.expect(Scene.text('Open full Autopilot thread.')).not.toExist(),
      Scene.expect(Scene.selector('a[href="/t/agent_run_scene_1"]')).toHaveAttr(
        'href',
        threadRouter({ threadId: 'agent_run_scene_1' }),
      ),
      Scene.expect(
        Scene.selector(
          '[data-component="user-message"] a[href="/t/agent_run_scene_1"]',
        ),
      ).not.toExist(),
      Scene.expect(
        Scene.selector(`a[href="/t/${teamAutopilotThreadId}"]`),
      ).not.toExist(),
      Scene.expect(Scene.text('Autopilot running')).not.toExist(),
      Scene.expect(Scene.text('Computer workroom')).not.toExist(),
      Scene.expect(Scene.text('OpenAgents -> computer')).not.toExist(),
    )
  })

  test('renders persisted team Autopilot run summaries after refresh', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(
          TeamChatRoute({ teamRef: 'openagents-core-team' }),
          auth,
        ),
        teamChatMessagesByTeam: {
          team_openagents_core: [
            {
              agentRunId: 'agent_run_scene_1',
              author: {
                avatarUrl: null,
                githubUsername: 'chris',
                name: 'Christopher David',
                userId: 'github:14167547',
              },
              autopilotThreadId: teamAutopilotThreadId,
              body: '@autopilot summarize that PDF',
              createdAt: '2026-06-03T00:00:00.000Z',
              id: 'team_chat_parent',
              kind: 'autopilot_intent',
              runSummary: {
                backend: 'shc_vm',
                durationSeconds: 200,
                eventCount: 42,
                repository: 'OpenAgentsInc/autopilot-omega@main',
                runId: 'agent_run_scene_1',
                runtime: 'opencode_codex',
                status: 'completed',
                tokenTotal: 31556,
                toolCallCount: 7,
                updatedAt: '2026-06-03T00:03:20.000Z',
              },
              teamId: 'team_openagents_core',
            },
          ],
        },
      }),
      Scene.expect(Scene.text('Succeeded in 3m 20s')).toExist(),
      Scene.expect(Scene.text('42 events')).toExist(),
      Scene.expect(Scene.text('7 tool calls')).toExist(),
      Scene.expect(Scene.text('31556 tokens')).toExist(),
      Scene.expect(Scene.text('linked child thread')).not.toExist(),
      Scene.expect(Scene.text('Open full Autopilot thread.')).not.toExist(),
    )
  })

  test('hides Artanis project navigation while project workrooms are disabled', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(ChatRoute(), authWithArtanisProject)),
      Scene.expect(Scene.text('Projects')).not.toExist(),
      Scene.expect(Scene.text('Artanis')).not.toExist(),
    )
  })

  test('renders the Artanis operator console for admins in Autopilot', () => {
    const [loadedConsoleModel] = LoggedIn.update(
      LoggedIn.init(ChatRoute(), auth),
      SucceededLoadArtanisOperatorConsole({
        response: artanisConsoleResponse,
      }),
    )
    const [model] = LoggedIn.update(
      loadedConsoleModel,
      SucceededLoadArtanisOperatorGoal({
        response: { goal: activeArtanisGoal },
        scopeKey: 'agent_artanis:team_openagents_core:operator',
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.text('Artanis operator')).toExist(),
      Scene.expect(Scene.text('Lifecycle')).toExist(),
      Scene.expect(Scene.label('Artanis operator goal')).toHaveValue(
        'Maintain the Pylon v0.2 launch.',
      ),
      Scene.expect(Scene.role('button', { name: 'Reprioritize' })).toExist(),
      Scene.expect(Scene.role('button', { name: 'Pause' })).toExist(),
      Scene.expect(Scene.role('button', { name: 'Cancel' })).toExist(),
      Scene.expect(Scene.text('Private refs')).toExist(),
      Scene.expect(Scene.text('evidence.private.artanis.scene')).toExist(),
      Scene.expect(Scene.text('workroom.private.artanis.scene')).toExist(),
      Scene.expect(Scene.text('Approval gates')).toExist(),
      Scene.expect(Scene.text('forum publish')).toExist(),
      Scene.expect(Scene.text('receipt.operator.artanis.scene')).toExist(),
      Scene.expect(Scene.role('button', { name: 'Approve' })).toExist(),
      Scene.expect(Scene.role('button', { name: 'Reject' })).toExist(),
      Scene.expect(Scene.text('Work routing')).toExist(),
      Scene.expect(
        Scene.text('pylon marketplace / pylon job intake'),
      ).toExist(),
      Scene.expect(Scene.text('Publication queue')).toExist(),
      Scene.expect(Scene.text('1 ready / 1 total')).toExist(),
      Scene.expect(Scene.text('2026-06-07T05:20:00.000Z')).not.toExist(),
    )
  })

  test('hides the Artanis operator console from non-admin sessions', () => {
    const [model] = LoggedIn.update(
      LoggedIn.init(ChatRoute(), { ...auth, isAdmin: false }),
      SucceededLoadArtanisOperatorConsole({
        response: artanisConsoleResponse,
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.text('Artanis operator')).not.toExist(),
      Scene.expect(Scene.text('Private refs')).not.toExist(),
      Scene.expect(Scene.text('evidence.private.artanis.scene')).not.toExist(),
      Scene.expect(Scene.text('receipt.operator.artanis.scene')).not.toExist(),
    )
  })

  test('does not render Artanis project details while project workrooms are disabled', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedIn.init(
          TeamProjectChatRoute({
            projectRef: 'artanis',
            teamRef: 'openagents-core-team',
          }),
          authWithArtanisProject,
        ),
      ),
      Scene.expect(Scene.text('Artanis')).not.toExist(),
      Scene.expect(Scene.text('Agent')).not.toExist(),
      Scene.expect(Scene.text('Autopilot')).not.toExist(),
      Scene.expect(Scene.text('Pylon')).not.toExist(),
      Scene.expect(Scene.role('link', { name: 'Go to Chat' })).toHaveAttr(
        'href',
        '/autopilot',
      ),
    )
  })

  test('renders blocked team Autopilot intents without a dead child link', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(
          TeamChatRoute({ teamRef: 'openagents-core-team' }),
          auth,
        ),
        teamChatMessagesByTeam: {
          team_openagents_core: [
            {
              agentRunId: null,
              author: {
                avatarUrl: null,
                githubUsername: 'chris',
                name: 'Christopher David',
                userId: 'github:14167547',
              },
              autopilotThreadId: teamAutopilotThreadId,
              body: '@autopilot Do you see the document?',
              createdAt: '2026-06-03T00:00:00.000Z',
              id: 'team_chat_blocked',
              kind: 'autopilot_intent',
              launchError: blockedProviderAccountMessage,
              teamId: 'team_openagents_core',
            },
            {
              agentRunId: null,
              author: {
                avatarUrl: null,
                githubUsername: 'chris',
                name: 'Christopher David',
                userId: 'github:14167547',
              },
              autopilotThreadId: teamAutopilotThreadId,
              body: 'Do you see the document? If you can, summarize it. @autopilot',
              createdAt: '2026-06-03T00:00:01.000Z',
              id: 'team_chat_legacy_blocked',
              kind: 'autopilot_intent',
              teamId: 'team_openagents_core',
            },
          ],
        },
      }),
      Scene.expect(Scene.text('@autopilot Do you see the document?')).toExist(),
      Scene.expect(
        Scene.text(
          'Do you see the document? If you can, summarize it. @autopilot',
        ),
      ).toExist(),
      Scene.expect(
        Scene.text(
          '@autopilot Do you see the document? If you can, summarize it. @autopilot',
        ),
      ).not.toExist(),
      Scene.expect(Scene.text('ChatGPT not connected')).toExist(),
      Scene.expect(Scene.text('Reconnect required')).toExist(),
      Scene.expect(
        Scene.text(
          'ChatGPT is not connected. Reconnect ChatGPT in Settings before launching Autopilot.',
        ),
      ).toExist(),
      Scene.expect(Scene.text(blockedProviderAccountMessage)).not.toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Reconnect ChatGPT' }),
      ).toHaveAttr('href', '/settings/connections'),
      Scene.expect(
        Scene.selector(`a[href="/t/${teamAutopilotThreadId}"]`),
      ).not.toExist(),
      Scene.expect(Scene.text('Autopilot launch unavailable')).toExist(),
      Scene.expect(Scene.text('no run created')).toExist(),
    )
  })

  test('does not show stale ChatGPT reconnect copy after reconnection', () => {
    Scene.scene(
      { update, view },
      Scene.with({
        ...LoggedIn.init(
          TeamChatRoute({ teamRef: 'openagents-core-team' }),
          authWithHealthyProviderAccount,
        ),
        teamChatMessagesByTeam: {
          team_openagents_core: [
            {
              agentRunId: null,
              author: {
                avatarUrl: null,
                githubUsername: 'chris',
                name: 'Christopher David',
                userId: 'github:14167547',
              },
              autopilotThreadId: teamAutopilotThreadId,
              body: '@autopilot Do you see the document?',
              createdAt: '2026-06-03T00:00:00.000Z',
              id: 'team_chat_blocked',
              kind: 'autopilot_intent',
              launchError: blockedProviderAccountMessage,
              teamId: 'team_openagents_core',
            },
          ],
        },
      }),
      Scene.expect(Scene.text('ChatGPT connected')).toExist(),
      Scene.expect(Scene.text('ready to retry')).toExist(),
      Scene.expect(Scene.text('ChatGPT not connected')).not.toExist(),
      Scene.expect(Scene.text('Reconnect required')).not.toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Reconnect ChatGPT' }),
      ).not.toExist(),
    )
  })

  test('submits the composer on Enter while leaving Shift+Enter as a draft edit', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(ChatRoute(), auth)),
      Scene.type(Scene.label('Message Autopilot'), 'Run the smoke test'),
      Scene.keydown(Scene.label('Message Autopilot'), 'Enter'),
      Scene.expect(Scene.text('Run the smoke test')).toExist(),
      Scene.expect(Scene.text('Launch Autopilot run')).toExist(),
      Scene.expect(Scene.text('OpenAgents -> computer')).toExist(),
      Scene.expect(
        Scene.text('source: browser submit -> /api/omni/agent-runs'),
      ).not.toExist(),
      Scene.expect(Scene.label('Message Autopilot')).toHaveValue(''),
      Scene.Command.resolve(
        LaunchAutopilotRun,
        SucceededLaunchAutopilotRun({
          requestId: 'chat-request-1',
          response: launchResponse,
        }),
        message => GotLoggedInMessage({ message }),
      ),
      Scene.expect(Scene.text('Autopilot running')).toExist(),
      Scene.expect(Scene.text('OpenCode on computer')).toExist(),
      Scene.expect(Scene.text('tokens: 17')).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Open run metadata' }),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'generated by: SHC/OpenCode runner callbacks ingested by OpenAgents Worker',
        ),
      ).not.toExist(),
      Scene.Command.resolveAll(
        [
          SetAutopilotThreadUrl,
          CompletedSetAutopilotThreadUrl(),
          message => GotLoggedInMessage({ message }),
        ],
        [
          LoadSyncSnapshot,
          SucceededLoadSyncSnapshot({
            scope: 'agent-run:agent_run_scene_1',
            snapshot: new SyncSnapshot({
              scope: SyncScope.make('agent-run:agent_run_scene_1'),
              cursor: SyncSequence.make(0),
              collections: {},
            }),
          }),
          message => GotLoggedInMessage({ message }),
        ],
        [
          ScrollChatTimelineToEnd,
          CompletedScrollChatTimelineToEnd(),
          message => GotLoggedInMessage({ message }),
        ],
        [
          FocusChatComposer,
          CompletedFocusChatComposer(),
          message => GotLoggedInMessage({ message }),
        ],
        [
          ScrollChatTimelineToEnd,
          CompletedScrollChatTimelineToEnd(),
          message => GotLoggedInMessage({ message }),
        ],
      ),
    )

    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(ChatRoute(), auth)),
      Scene.type(Scene.label('Message Autopilot'), 'Line one'),
      Scene.keydown(Scene.label('Message Autopilot'), 'Enter', {
        shiftKey: true,
      }),
      Scene.expect(Scene.text('Launch Autopilot run')).not.toExist(),
      Scene.expect(Scene.label('Message Autopilot')).toHaveValue('Line one'),
    )
  })

  test('switches to the settings sidebar on account routes', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(SettingsRoute(), auth)),
      Scene.expect(
        Scene.role('navigation', { name: 'OpenAgents navigation' }),
      ).toExist(),
      Scene.expect(Scene.role('link', { name: 'Back to App' })).toHaveAttr(
        'href',
        '/autopilot',
      ),
      Scene.expect(Scene.text('General')).toExist(),
      Scene.expect(Scene.text('Connections')).toExist(),
      Scene.expect(Scene.text('Organization')).toExist(),
      Scene.expect(Scene.text('Members')).toExist(),
      Scene.expect(Scene.text('API keys')).not.toExist(),
      Scene.expect(Scene.role('button', { name: 'New thread' })).not.toExist(),
      Scene.expect(Scene.role('heading', { name: 'General' })).toExist(),
      Scene.expect(Scene.text('Your account')).toExist(),
      Scene.expect(Scene.text('OpenAgents Core Team')).toExist(),
      Scene.expect(Scene.role('link', { name: 'Connect ChatGPT' })).toHaveAttr(
        'href',
        '/settings/connections',
      ),
    )
  })

  test('renders connected account details inside the settings sections', () => {
    const model = LoggedIn.init(
      SettingsSectionRoute({ section: 'connections' }),
      authWithUnhealthyProviderAccount,
    )

    Scene.scene(
      { update, view },
      Scene.with({
        ...model,
        providerConnectionAction: PollingProviderDeviceLogin({
          attemptId: 'provider_attempt_1',
        }),
      }),
      Scene.expect(Scene.role('heading', { name: 'Connections' })).toExist(),
      Scene.expect(Scene.text('ChatGPT accounts')).toExist(),
      Scene.expect(Scene.text('requires reconnect')).toExist(),
      Scene.expect(Scene.text('chris@openagents.com')).toExist(),
      Scene.expect(
        Scene.text('Open the OpenAI device page and enter this code'),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'OpenAgents does not ask for your ChatGPT password here. Sign in with OpenAI on the device page, enter the code, then return here.',
        ),
      ).toExist(),
      Scene.expect(Scene.text('ABCD-EFGH')).toExist(),
      Scene.expect(Scene.text('Waiting for confirmation...')).not.toExist(),
      Scene.expect(
        Scene.text(
          'Open the OpenAI device page, enter the code, then return here. OpenAgents is checking for completion.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Open OpenAI device page' }),
      ).toHaveAttr('href', 'https://chatgpt.com/activate'),
      Scene.expect(
        Scene.role('link', { name: 'Open OpenAI device page' }),
      ).toHaveAttr('target', '_blank'),
      Scene.expect(
        Scene.role('link', { name: 'Open OpenAI device page' }),
      ).toHaveAttr('rel', 'noopener noreferrer'),
      Scene.expect(Scene.role('button', { name: 'Reconnect' })).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Waiting for OpenAI sign-in' }),
      ).toExist(),
      Scene.expect(Scene.role('button', { name: 'Check status' })).toExist(),
      Scene.expect(Scene.text('Default repository')).toExist(),
      Scene.expect(Scene.label('Owner')).toHaveValue(''),
      Scene.expect(Scene.label('Repository')).toHaveValue(''),
      Scene.expect(
        Scene.role('button', { name: 'Load repositories' }),
      ).toExist(),
      Scene.expect(Scene.role('button', { name: 'Save repository' })).toExist(),
      Scene.expect(Scene.text('Provider and repository access')).not.toExist(),
      Scene.expect(
        Scene.text('OpenCode with Codex connected-account grant'),
      ).not.toExist(),
    )
  })

  test('disables repository save after the selected default is already saved', () => {
    const [model] = LoggedIn.update(
      LoggedIn.init(
        SettingsSectionRoute({ section: 'connections' }),
        authWithSavedRepository,
      ),
      SucceededLoadOnboardingRepositories({
        response: {
          repositories: onboardingRepositories,
          tokenStatus: 'available',
        },
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.text('Default repository saved.')).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Repository saved' }),
      ).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Save repository' }),
      ).not.toExist(),
      Scene.click(
        Scene.role('button', { name: 'OpenAgentsInc/repo-02private / main' }),
      ),
      Scene.expect(
        Scene.text('Save to update the default repository.'),
      ).toExist(),
      Scene.expect(Scene.role('button', { name: 'Save repository' })).toExist(),
    )
  })

  test('renders multiple ChatGPT accounts in settings connections', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedIn.init(
          SettingsSectionRoute({ section: 'connections' }),
          authWithMultipleProviderAccounts,
        ),
      ),
      Scene.expect(Scene.text('primary@openagents.com')).toExist(),
      Scene.expect(Scene.text('backup@openagents.com')).toExist(),
      Scene.expect(Scene.text('requires reconnect')).toExist(),
      Scene.expect(Scene.text('connected')).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Add ChatGPT account' }),
      ).toExist(),
    )
  })
})

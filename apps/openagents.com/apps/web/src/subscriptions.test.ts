import { describe, expect, test } from 'vitest'

import {
  authBootstrapFromSession,
  incompleteOnboardingStatus,
} from './domain/session'
import { Demo, LoggedIn, LoggedOut } from './model'
import {
  SubmittedAutopilotOnboardingTurn,
  UpdatedAutopilotOnboardingComposer,
} from './page/loggedOut/message'
import { update as loggedOutUpdate } from './page/loggedOut/update'
import { AutopilotRoute, AutopilotVerticalRoute } from './route'
import {
  ActiveChatRun,
  agentRunExternalRefFromNullable,
} from './page/loggedIn/model'
import { authorizedThreadRoute } from './page/loggedIn/thread-route'
import {
  ChatRoute,
  DashboardRoute,
  DemoOrderRoute,
  DemoRoute,
  TeamChatRoute,
  TeamFileRoute,
  TeamFilesRoute,
  ThreadRoute,
} from './route'
import {
  autopilotRunPollDependenciesForModel,
  demoClockDependenciesForModel,
  demoKeyboardDependenciesForModel,
  demoPlaybackDependenciesForModel,
  onboardingStreamDependenciesForModel,
  syncMessageFromPayload,
  syncStreamDependenciesForModel,
  workspaceSyncDependenciesForModel,
} from './subscriptions'

const auth = authBootstrapFromSession({
  email: 'chris@openagents.com',
  name: 'Christopher David',
  userId: 'github:14167547',
})

const authWithoutCoreTeam = auth

const authWithTeam = {
  ...auth,
  teams: [
    {
      id: 'team_openagents_core',
      name: 'OpenAgents Core Team',
      slug: 'openagents-core-team',
      role: 'owner',
      members: [],
    },
  ],
}

const authWithIncompleteOnboarding = {
  ...authWithTeam,
  onboarding: incompleteOnboardingStatus(),
}

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
    updatedAt: '2026-06-03T00:00:00.000Z',
  },
})

const activeRunModel = (
  route: ChatRoute | TeamChatRoute | ThreadRoute | DashboardRoute,
) => ({
  ...LoggedIn.init(route, authWithTeam),
  chatRun: activeRun,
})

describe('Autopilot run polling subscriptions', () => {
  test('polls busy active runs as a sync safety net', () => {
    expect(
      autopilotRunPollDependenciesForModel(activeRunModel(ChatRoute())),
    ).toEqual({
      isActive: true,
      runId: 'agent_run_1',
    })

    expect(
      autopilotRunPollDependenciesForModel(
        activeRunModel(TeamChatRoute({ teamRef: 'openagents-core-team' })),
      ),
    ).toEqual({
      isActive: true,
      runId: 'agent_run_1',
    })

    expect(
      autopilotRunPollDependenciesForModel(
        activeRunModel(ThreadRoute({ threadId: 'thread-1' })),
      ),
    ).toEqual({
      isActive: true,
      runId: 'agent_run_1',
    })
  })

  test('does not poll completed active runs', () => {
    expect(
      autopilotRunPollDependenciesForModel({
        ...activeRunModel(ChatRoute()),
        chatRun: {
          ...activeRun,
          metadata: {
            ...activeRun.metadata,
            status: 'completed',
          },
        },
      }),
    ).toEqual({
      isActive: false,
      runId: '',
    })
  })

  test('does not poll on non-chat routes', () => {
    expect(
      autopilotRunPollDependenciesForModel(activeRunModel(DashboardRoute())),
    ).toEqual({
      isActive: false,
      runId: '',
    })
  })
})

describe('workspace sync subscriptions', () => {
  test('decodes sync patch websocket payloads through the shared server message schema', () => {
    expect(
      syncMessageFromPayload(
        JSON.stringify({
          collection: 'agent_runs',
          id: 'agent_run_1',
          op: 'patch',
          patch: { status: 'running' },
          scope: 'agent-run:agent_run_1',
          seq: 3,
          serverTime: '2026-06-04T00:00:00.000Z',
        }),
      ),
    ).toMatchObject({
      _tag: 'ReceivedSyncPatch',
      patch: {
        collection: 'agent_runs',
        id: 'agent_run_1',
        scope: 'agent-run:agent_run_1',
      },
    })
  })

  test('decodes cursor gap websocket payloads through the shared server message schema', () => {
    expect(
      syncMessageFromPayload(
        JSON.stringify({
          expectedSeq: 4,
          receivedSeq: 6,
          scope: 'workspace:github:14167547',
        }),
      ),
    ).toEqual({
      _tag: 'ReceivedSyncCursorGap',
      gap: {
        expectedSeq: 4,
        receivedSeq: 6,
        scope: 'workspace:github:14167547',
      },
    })
  })

  test('returns a structured sync failure message for invalid websocket payloads', () => {
    expect(syncMessageFromPayload('{')).toEqual({
      _tag: 'FailedSyncStream',
      error: 'Sync stream message could not be decoded.',
      scope: '',
    })
  })

  test('subscribes the logged-in app to the user workspace scope', () => {
    expect(
      workspaceSyncDependenciesForModel(
        LoggedIn.init(ChatRoute(), authWithTeam),
      ),
    ).toEqual({
      cursor: 0,
      isActive: true,
      scope: 'workspace:github:14167547',
      streamHref: '/api/sync/workspace/github%3A14167547/stream?cursor=0',
    })
  })

  test('does not subscribe incomplete onboarding models to product sync streams', () => {
    expect(
      syncStreamDependenciesForModel(
        LoggedIn.init(ChatRoute(), authWithIncompleteOnboarding),
      ),
    ).toEqual({
      isActive: false,
      scopeKey: '',
      targets: [],
    })
  })

  test('does not subscribe authenticated models without Core Team access', () => {
    expect(
      syncStreamDependenciesForModel(
        LoggedIn.init(ChatRoute(), authWithoutCoreTeam),
      ),
    ).toEqual({
      isActive: false,
      scopeKey: '',
      targets: [],
    })
  })

  test('subscribes active chat routes to workspace and agent-run scopes', () => {
    expect(syncStreamDependenciesForModel(activeRunModel(ChatRoute()))).toEqual(
      {
        isActive: true,
        scopeKey: 'workspace:github:14167547|agent-run:agent_run_1',
        targets: [
          {
            cursor: 0,
            scope: 'workspace:github:14167547',
            streamHref: '/api/sync/workspace/github%3A14167547/stream?cursor=0',
          },
          {
            cursor: 0,
            scope: 'agent-run:agent_run_1',
            streamHref: '/api/sync/agent-run/agent_run_1/stream?cursor=0',
          },
        ],
      },
    )
  })

  test('does not subscribe thread routes before thread access is authorized', () => {
    expect(
      syncStreamDependenciesForModel(
        activeRunModel(ThreadRoute({ threadId: 'thread-1' })),
      ),
    ).toEqual({
      isActive: true,
      scopeKey: 'workspace:github:14167547|agent-run:agent_run_1',
      targets: [
        {
          cursor: 0,
          scope: 'workspace:github:14167547',
          streamHref: '/api/sync/workspace/github%3A14167547/stream?cursor=0',
        },
        {
          cursor: 0,
          scope: 'agent-run:agent_run_1',
          streamHref: '/api/sync/agent-run/agent_run_1/stream?cursor=0',
        },
      ],
    })
  })

  test('subscribes authorized thread routes to workspace, thread, and active run scopes', () => {
    expect(
      syncStreamDependenciesForModel({
        ...activeRunModel(ThreadRoute({ threadId: 'thread-1' })),
        threadRoute: authorizedThreadRoute('thread-1', 'agent_run_1'),
      }),
    ).toEqual({
      isActive: true,
      scopeKey:
        'workspace:github:14167547|thread:thread-1|agent-run:agent_run_1',
      targets: [
        {
          cursor: 0,
          scope: 'workspace:github:14167547',
          streamHref: '/api/sync/workspace/github%3A14167547/stream?cursor=0',
        },
        {
          cursor: 0,
          scope: 'thread:thread-1',
          streamHref: '/api/sync/thread/thread-1/stream?cursor=0',
        },
        {
          cursor: 0,
          scope: 'agent-run:agent_run_1',
          streamHref: '/api/sync/agent-run/agent_run_1/stream?cursor=0',
        },
      ],
    })
  })

  test('subscribes team chat routes to workspace and team scopes', () => {
    expect(
      syncStreamDependenciesForModel(
        LoggedIn.init(
          TeamChatRoute({ teamRef: 'openagents-core-team' }),
          authWithTeam,
        ),
      ),
    ).toMatchObject({
      isActive: true,
      scopeKey: 'workspace:github:14167547|team:team_openagents_core',
      targets: [
        {
          cursor: 0,
          scope: 'workspace:github:14167547',
          streamHref: '/api/sync/workspace/github%3A14167547/stream?cursor=0',
        },
        {
          cursor: 0,
          scope: 'team:team_openagents_core',
          streamHref: '/api/sync/team/team_openagents_core/stream?cursor=0',
        },
      ],
    })
  })

  test('subscribes team files routes to workspace and team scopes', () => {
    expect(
      syncStreamDependenciesForModel(
        LoggedIn.init(
          TeamFilesRoute({ teamRef: 'openagents-core-team' }),
          authWithTeam,
        ),
      ),
    ).toMatchObject({
      isActive: true,
      scopeKey: 'workspace:github:14167547|team:team_openagents_core',
      targets: [
        {
          cursor: 0,
          scope: 'workspace:github:14167547',
          streamHref: '/api/sync/workspace/github%3A14167547/stream?cursor=0',
        },
        {
          cursor: 0,
          scope: 'team:team_openagents_core',
          streamHref: '/api/sync/team/team_openagents_core/stream?cursor=0',
        },
      ],
    })
  })

  test('subscribes team file detail routes to workspace and team scopes', () => {
    expect(
      syncStreamDependenciesForModel(
        LoggedIn.init(
          TeamFileRoute({
            fileId: 'file_1',
            teamRef: 'openagents-core-team',
          }),
          authWithTeam,
        ),
      ),
    ).toMatchObject({
      isActive: true,
      scopeKey: 'workspace:github:14167547|team:team_openagents_core',
      targets: [
        {
          cursor: 0,
          scope: 'workspace:github:14167547',
          streamHref: '/api/sync/workspace/github%3A14167547/stream?cursor=0',
        },
        {
          cursor: 0,
          scope: 'team:team_openagents_core',
          streamHref: '/api/sync/team/team_openagents_core/stream?cursor=0',
        },
      ],
    })
  })
})

describe('demo keyboard subscriptions', () => {
  test('does not install playback controls for the fullscreen training demo', () => {
    const demo = Demo.init(DemoRoute())

    expect(demoKeyboardDependenciesForModel(demo)).toEqual({
      isActive: false,
      key: '',
    })
    expect(demoPlaybackDependenciesForModel(demo)).toEqual({
      cursorMs: 0,
      isActive: false,
      key: '',
    })
    expect(demoClockDependenciesForModel(demo)).toEqual({
      isActive: false,
      key: '',
    })
  })

  test('listens for spacebar while demo playback can be toggled', () => {
    const demo = Demo.init(DemoOrderRoute())

    expect(demoKeyboardDependenciesForModel(demo)).toEqual({
      isActive: true,
      key: 'demo:customer-order',
    })
    expect(
      demoKeyboardDependenciesForModel({ ...demo, playback: 'paused' }),
    ).toEqual({
      isActive: true,
      key: 'demo:customer-order',
    })
    expect(
      demoKeyboardDependenciesForModel({ ...demo, playback: 'complete' }),
    ).toEqual({
      isActive: false,
      key: '',
    })
  })
})

describe('demo playback subscriptions', () => {
  test('starts initial playback from the first cue and resumes after current cue index', () => {
    const demo = Demo.init(DemoOrderRoute())

    expect(demoPlaybackDependenciesForModel(demo)).toEqual({
      cursorMs: -1,
      isActive: true,
      key: 'demo:customer-order',
    })
    expect(demoPlaybackDependenciesForModel({ ...demo, cueIndex: 0 })).toEqual({
      cursorMs: 0,
      isActive: true,
      key: 'demo:customer-order',
    })
    expect(
      demoPlaybackDependenciesForModel({ ...demo, cueIndex: 1800 }),
    ).toEqual({
      cursorMs: 1800,
      isActive: true,
      key: 'demo:customer-order',
    })
    expect(
      demoPlaybackDependenciesForModel({
        ...demo,
        cueIndex: 1800,
        playback: 'paused',
      }),
    ).toEqual({
      cursorMs: 0,
      isActive: false,
      key: '',
    })
  })
})

describe('demo clock subscriptions', () => {
  test('ticks the visible timer only while playback is running', () => {
    const demo = Demo.init(DemoOrderRoute())

    expect(demoClockDependenciesForModel(demo)).toEqual({
      isActive: true,
      key: 'demo:customer-order',
    })
    expect(
      demoClockDependenciesForModel({ ...demo, playback: 'paused' }),
    ).toEqual({
      isActive: false,
      key: '',
    })
    expect(
      demoClockDependenciesForModel({ ...demo, playback: 'complete' }),
    ).toEqual({
      isActive: false,
      key: '',
    })
  })
})

describe('autopilot onboarding stream subscription', () => {
  const loggedOutBase = () => LoggedOut.init(AutopilotRoute())

  test('is inactive with no pending turn', () => {
    expect(onboardingStreamDependenciesForModel(loggedOutBase())).toEqual({
      isActive: false,
      turnId: '',
      sessionId: '',
      userText: '',
      vertical: 'general',
    })
  })

  test('activates and carries the pending turn once a turn is submitted', () => {
    const [typed] = loggedOutUpdate(
      loggedOutBase(),
      UpdatedAutopilotOnboardingComposer({ value: 'I run a bakery' }),
    )
    const [submitted] = loggedOutUpdate(
      typed,
      SubmittedAutopilotOnboardingTurn(),
    )

    const deps = onboardingStreamDependenciesForModel(submitted)
    expect(deps.isActive).toBe(true)
    expect(deps.userText).toBe('I run a bakery')
    // A first turn mints a fresh session id at the stream boundary.
    expect(deps.sessionId).toMatch(/^ob_/)
    expect(deps.turnId).not.toBe('')
    expect(deps.vertical).toBe('general')
  })

  test('carries the bounded legal vertical for /autopilot/legal', () => {
    const [typed] = loggedOutUpdate(
      LoggedOut.init(AutopilotVerticalRoute({ vertical: 'legal' })),
      UpdatedAutopilotOnboardingComposer({ value: 'Help with an NDA' }),
    )
    const [submitted] = loggedOutUpdate(
      typed,
      SubmittedAutopilotOnboardingTurn(),
    )

    const deps = onboardingStreamDependenciesForModel(submitted)
    expect(deps.isActive).toBe(true)
    expect(deps.vertical).toBe('legal')
  })
})

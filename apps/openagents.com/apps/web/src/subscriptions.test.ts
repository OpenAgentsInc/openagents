import { describe, expect, test } from 'vitest'
import { evo } from 'foldkit/struct'

import {
  authBootstrapFromSession,
  incompleteOnboardingStatus,
} from './domain/session'
import {
  GYM_RUN_PROGRESS_SCOPE,
  gymRunProgressStreamOpen,
} from './page/loggedOut/gym/runProgressFeed'
import { Demo, LoggedIn, LoggedOut } from './model'
import {
  LoadedStoredAutopilotOnboarding,
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
  GymRoute,
  HomeRoute,
  KhalaRoute,
  StatsRoute,
  TassadarRoute,
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
  gymRunProgressPollDependenciesForModel,
  gymRunProgressStreamDependenciesForModel,
  khalaTokensServedChannelMixPollDependenciesForModel,
  khalaTokensServedModelMixPollDependenciesForModel,
  khalaTokensServedPollDependenciesForModel,
  khalaTokensServedStreamDependenciesForModel,
  onboardingResumeDependenciesForModel,
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

describe('gym run-progress poll subscription (#6261)', () => {
  test('polls while on the /gym route', () => {
    expect(
      gymRunProgressPollDependenciesForModel(LoggedOut.init(GymRoute())),
    ).toEqual({ isActive: true })
  })

  test('does not poll off the /gym route', () => {
    expect(
      gymRunProgressPollDependenciesForModel(LoggedOut.init(HomeRoute())),
    ).toEqual({ isActive: false })
  })

  test('the reconcile poll is the socket-down fallback only (not while open)', () => {
    const open = evo(LoggedOut.init(GymRoute()), {
      gymRunProgressStream: gymRunProgressStreamOpen,
    })
    expect(gymRunProgressPollDependenciesForModel(open)).toEqual({
      isActive: false,
    })
  })
})

describe('gym run-progress realtime stream subscription (#6261)', () => {
  test('opens the WebSocket to the run-progress sync scope on /gym', () => {
    const dependencies = gymRunProgressStreamDependenciesForModel(
      LoggedOut.init(GymRoute()),
    )
    expect(dependencies.isActive).toBe(true)
    expect(dependencies.scope).toBe(GYM_RUN_PROGRESS_SCOPE)
    expect(dependencies.scope).toBe('public-gym-run-progress:network')
    expect(dependencies.streamHref).toBe(
      '/api/sync/public-gym-run-progress/network/stream?cursor=0',
    )
  })

  test('is gated to the /gym route (inactive elsewhere)', () => {
    expect(
      gymRunProgressStreamDependenciesForModel(LoggedOut.init(HomeRoute()))
        .isActive,
    ).toBe(false)
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

describe('autopilot onboarding resume subscription', () => {
  const loggedOutBase = () => LoggedOut.init(AutopilotRoute())

  test('is inactive when no in-flight turn was restored', () => {
    expect(onboardingResumeDependenciesForModel(loggedOutBase())).toEqual({
      isActive: false,
      sessionId: '',
      turnIndex: 0,
      offset: '',
    })
  })

  test('activates for a restored in-flight turn, defaulting the offset to 0', () => {
    const [restored] = loggedOutUpdate(
      loggedOutBase(),
      LoadedStoredAutopilotOnboarding({
        session: {
          sessionId: 'ob_resume_sub',
          vertical: null,
          status: 'interviewing',
          transcript: [{ role: 'user', content: 'hi' }],
          outputSpec: {},
          inFlight: {
            streamId: 'onboarding:ob_resume_sub:0',
            turnIndex: 0,
            replySoFar: '',
            lastOffset: null,
          },
          updatedAt: 1_700_000_000_000,
        },
      }),
    )

    const deps = onboardingResumeDependenciesForModel(restored)
    expect(deps).toEqual({
      isActive: true,
      sessionId: 'ob_resume_sub',
      turnIndex: 0,
      // No tracked offset => resume from 0 (the durable replay re-streams the
      // whole in-flight turn from the start and continues to EOF).
      offset: '0',
    })
  })

  test('carries a tracked offset so a second reload resumes further along', () => {
    const [restored] = loggedOutUpdate(
      loggedOutBase(),
      LoadedStoredAutopilotOnboarding({
        session: {
          sessionId: 'ob_resume_sub',
          vertical: null,
          status: 'interviewing',
          transcript: [{ role: 'user', content: 'hi' }],
          outputSpec: {},
          inFlight: {
            streamId: 'onboarding:ob_resume_sub:0',
            turnIndex: 0,
            replySoFar: 'partial',
            lastOffset: '128',
          },
          updatedAt: 1_700_000_000_000,
        },
      }),
    )

    expect(onboardingResumeDependenciesForModel(restored).offset).toBe('128')
  })
})

describe('Khala tokens-served live surfaces', () => {
  // The homepage hero's top-left pill shows the SAME live total the /khala
  // counter does, so Home must subscribe to the SAME live delta stream +
  // reconcile poll (no parallel data source). /khala stays live; /tassadar
  // (which shows the back button, not the pill) stays inactive.
  test('the homepage subscribes to the live tokens-served stream once seeded (#6324)', () => {
    // Gated on the snapshot settling so the socket opens at the seeded cursor.
    const landingUnseeded = LoggedOut.init(HomeRoute())
    expect(
      khalaTokensServedStreamDependenciesForModel(landingUnseeded).isActive,
    ).toBe(false)

    const landingSeeded = evo(landingUnseeded, {
      khalaTokensServedStream: stream =>
        evo(stream, { snapshotLoaded: () => true }),
    })
    expect(
      khalaTokensServedStreamDependenciesForModel(landingSeeded).isActive,
    ).toBe(true)
    // The reconcile poll stays route-gated (independent of the snapshot gate).
    expect(
      khalaTokensServedPollDependenciesForModel(LoggedOut.init(HomeRoute()))
        .isActive,
    ).toBe(true)
  })

  test('/khala and /home stay live for the tokens-served stream once the snapshot has settled (#6324)', () => {
    // The socket is GATED on the snapshot load settling, so it opens at the
    // SEEDED cursor instead of racing the snapshot and replaying from 0.
    const khalaUnseeded = LoggedOut.init(KhalaRoute())
    expect(
      khalaTokensServedStreamDependenciesForModel(khalaUnseeded).isActive,
    ).toBe(false)

    const khalaSeeded = evo(khalaUnseeded, {
      khalaTokensServedStream: stream =>
        evo(stream, { snapshotLoaded: () => true, cursor: () => 7364 }),
    })
    const khalaDeps = khalaTokensServedStreamDependenciesForModel(khalaSeeded)
    expect(khalaDeps.isActive).toBe(true)
    // Opens at the SEEDED cursor (not 0) — only new deltas, no full replay.
    expect(khalaDeps.cursor).toBe(7364)
    expect(khalaDeps.streamHref).toBe(
      '/api/sync/public-khala-tokens-served/network/stream?cursor=7364',
    )

    const homeSeeded = evo(LoggedOut.init(HomeRoute()), {
      khalaTokensServedStream: stream =>
        evo(stream, { snapshotLoaded: () => true }),
    })
    expect(
      khalaTokensServedStreamDependenciesForModel(homeSeeded).isActive,
    ).toBe(true)
  })

  test('/tassadar (back-button slot, no pill) is NOT live for the tokens-served stream', () => {
    expect(
      khalaTokensServedStreamDependenciesForModel(
        LoggedOut.init(TassadarRoute()),
      ).isActive,
    ).toBe(false)
    expect(
      khalaTokensServedPollDependenciesForModel(
        LoggedOut.init(TassadarRoute()),
      ).isActive,
    ).toBe(false)
  })

  test('the model-family-mix poll is gated to the /stats surface only (#6392)', () => {
    // The model-mix chart only renders on /stats, so its refresh poll activates
    // there and nowhere else — it must NOT fire on /home or /khala where there
    // is no model-mix panel.
    expect(
      khalaTokensServedModelMixPollDependenciesForModel(
        LoggedOut.init(StatsRoute()),
      ).isActive,
    ).toBe(true)
    expect(
      khalaTokensServedModelMixPollDependenciesForModel(
        LoggedOut.init(HomeRoute()),
      ).isActive,
    ).toBe(false)
    expect(
      khalaTokensServedModelMixPollDependenciesForModel(
        LoggedOut.init(KhalaRoute()),
      ).isActive,
    ).toBe(false)
    expect(
      khalaTokensServedChannelMixPollDependenciesForModel(
        LoggedOut.init(StatsRoute()),
      ).isActive,
    ).toBe(true)
    expect(
      khalaTokensServedChannelMixPollDependenciesForModel(
        LoggedOut.init(HomeRoute()),
      ).isActive,
    ).toBe(false)
  })
})

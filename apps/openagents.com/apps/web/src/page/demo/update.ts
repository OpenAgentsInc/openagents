import { Match as M, Option } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { OnboardingRoute, OrderRoute } from '../../route'
import * as LoggedIn from '../loggedIn'
import {
  ClickedSkipOnboardingBilling,
  EnteredAutopilotRunRoute,
  SelectedOnboardingRepository,
  SubmittedChatComposer,
  SubmittedOnboardingGoal,
  SubmittedOnboardingRepository,
  SucceededFetchAutopilotRun,
  SucceededLoadAgentGoal,
  SucceededLoadCustomerOrder,
  SucceededLoadOnboardingRepositories,
  SucceededLoadSyncSnapshot,
  SucceededLoadTeamChatMessages,
  SucceededLoadThreadFileDetail,
  SucceededLoadThreadFiles,
  SucceededPostTeamChatMessage,
  SucceededSelectOnboardingRepository,
  SucceededSkipOnboardingBilling,
  SucceededSubmitOnboardingGoal,
  UpdatedChatComposer,
  UpdatedOnboardingGoal,
} from '../loggedIn/message'
import { syncAgentRunScope, syncTeamScope } from '../loggedIn/model'
import { ClosedRunMetadataDialog, LaunchingChatRun } from '../loggedIn/model'
import { teamChatRoomKey, teamFilesScopeKey } from '../loggedIn/update'
import {
  DEMO_FILE_PLAN_ID,
  DEMO_ORDER_GOAL,
  DEMO_PROJECT_ID,
  DEMO_PROMPT,
  DEMO_RUN_ID,
  DEMO_TEAM_ID,
  demoActiveRunResponse,
  demoAuthBootstrap,
  demoCompletedRunResponse,
  demoCustomerAuthBootstrap,
  demoCustomerOrder,
  demoEmptyTeamMessagesResponse,
  demoGoalResponse,
  demoOrderCompletedStatus,
  demoOrderGoalSubmittedStatus,
  demoOrderRepositoriesResponse,
  demoOrderRepositorySelectedStatus,
  demoPostResponse,
  demoRunSnapshot,
  demoTeamSnapshot,
  demoThreadFileDetailResponse,
  demoThreadFilesResponse,
} from './fixtures'
import { Message } from './message'
import {
  type DemoCue,
  type DemoCueName,
  Model,
  demoFileRoute,
  demoFilesRoute,
  demoProjectRoute,
  demoThreadRoute,
} from './model'
import { cuesForDemoKey, nextDemoCue, previousDemoCue } from './playback'

type UpdateReturn = readonly [Model, ReadonlyArray<Command.Command<Message>>]
const withUpdateReturn = M.withReturnType<UpdateReturn>()

const DEMO_DURATION_MS = 15000
const roomKey = teamChatRoomKey(DEMO_TEAM_ID, DEMO_PROJECT_ID)

const runLoggedIn = (
  model: Model,
  message: LoggedIn.Message,
): LoggedIn.Model => {
  const [loggedIn] = LoggedIn.update(model.loggedIn, message)

  return loggedIn
}

const withLoggedInMessage = (model: Model, message: LoggedIn.Message): Model =>
  evo(model, { loggedIn: () => runLoggedIn(model, message) })

const withLoggedInRoute = (
  model: Model,
  route: LoggedIn.Model['route'],
): Model =>
  evo(model, {
    loggedIn: loggedIn => evo(loggedIn, { route: () => route }),
  })

const applyCue = (model: Model, cueName: DemoCueName): Model => {
  if (cueName === 'LoadedProjectRoom') {
    return withLoggedInMessage(
      withLoggedInMessage(
        withLoggedInRoute(model, demoProjectRoute()),
        SucceededLoadTeamChatMessages({
          response: demoEmptyTeamMessagesResponse,
          roomKey,
          teamId: DEMO_TEAM_ID,
        }),
      ),
      SucceededLoadThreadFiles({
        response: demoThreadFilesResponse,
        scopeKey: teamFilesScopeKey(DEMO_TEAM_ID),
      }),
    )
  }

  if (cueName === 'FilledComposer') {
    return withLoggedInMessage(
      model,
      UpdatedChatComposer({ value: DEMO_PROMPT }),
    )
  }

  if (cueName === 'SubmittedPrompt') {
    const submitted = evo(withLoggedInMessage(model, SubmittedChatComposer()), {
      loggedIn: loggedIn =>
        evo(loggedIn, {
          chatComposerValue: () => '',
          chatRun: () =>
            LaunchingChatRun({
              prompt: DEMO_PROMPT.replace(/^@autopilot\s+/, ''),
              requestId: 'team-chat-request-1',
            }),
          runMetadataDialog: () => ClosedRunMetadataDialog(),
        }),
    })
    return withLoggedInMessage(
      submitted,
      SucceededPostTeamChatMessage({
        requestId: 'team-chat-request-1',
        response: demoPostResponse,
      }),
    )
  }

  if (cueName === 'ReceivedRunEvents') {
    return withLoggedInMessage(
      withLoggedInMessage(
        model,
        SucceededLoadSyncSnapshot({
          scope: syncAgentRunScope(DEMO_RUN_ID),
          snapshot: demoRunSnapshot(demoActiveRunResponse),
        }),
      ),
      SucceededLoadSyncSnapshot({
        scope: syncTeamScope(DEMO_TEAM_ID),
        snapshot: demoTeamSnapshot(false),
      }),
    )
  }

  if (cueName === 'LoadedRunContext') {
    return withLoggedInMessage(
      withLoggedInMessage(
        model,
        SucceededLoadAgentGoal({
          response: demoGoalResponse,
          scopeKey: [DEMO_PROJECT_ID, DEMO_TEAM_ID].join(':'),
        }),
      ),
      SucceededLoadThreadFiles({
        response: demoThreadFilesResponse,
        scopeKey: teamFilesScopeKey(DEMO_TEAM_ID),
      }),
    )
  }

  if (cueName === 'OpenedThread') {
    const routed = withLoggedInRoute(model, demoThreadRoute())
    return withLoggedInMessage(
      routed,
      EnteredAutopilotRunRoute({ runId: DEMO_RUN_ID }),
    )
  }

  if (cueName === 'CompletedRun') {
    return withLoggedInMessage(
      withLoggedInMessage(
        model,
        SucceededFetchAutopilotRun({
          runId: DEMO_RUN_ID,
          response: demoCompletedRunResponse,
        }),
      ),
      SucceededLoadThreadFiles({
        response: demoThreadFilesResponse,
        scopeKey: teamFilesScopeKey(DEMO_TEAM_ID),
      }),
    )
  }

  if (cueName === 'ReturnedToProjectRoom') {
    return withLoggedInMessage(
      withLoggedInRoute(model, demoProjectRoute()),
      SucceededLoadSyncSnapshot({
        scope: syncTeamScope(DEMO_TEAM_ID),
        snapshot: demoTeamSnapshot(true),
      }),
    )
  }

  if (cueName === 'OpenedTeamFiles') {
    return withLoggedInMessage(
      withLoggedInRoute(model, demoFilesRoute()),
      SucceededLoadThreadFiles({
        response: demoThreadFilesResponse,
        scopeKey: teamFilesScopeKey(DEMO_TEAM_ID),
      }),
    )
  }

  if (cueName === 'OpenedFileDetail') {
    return withLoggedInMessage(
      withLoggedInRoute(model, demoFileRoute()),
      SucceededLoadThreadFileDetail({
        fileId: DEMO_FILE_PLAN_ID,
        response: demoThreadFileDetailResponse,
      }),
    )
  }

  if (cueName === 'LoadedOrderRepositories') {
    return withLoggedInMessage(
      withLoggedInRoute(model, OnboardingRoute()),
      SucceededLoadOnboardingRepositories({
        response: demoOrderRepositoriesResponse,
      }),
    )
  }

  if (cueName === 'SelectedOrderRepository') {
    return withLoggedInMessage(
      withLoggedInMessage(
        withLoggedInMessage(
          model,
          SelectedOnboardingRepository({
            repositoryId: 'github_repo_openagents_beta_shop',
          }),
        ),
        SubmittedOnboardingRepository(),
      ),
      SucceededSelectOnboardingRepository({
        response: { onboarding: demoOrderRepositorySelectedStatus },
      }),
    )
  }

  if (cueName === 'FilledOrderGoal') {
    return withLoggedInMessage(
      model,
      UpdatedOnboardingGoal({ value: DEMO_ORDER_GOAL }),
    )
  }

  if (cueName === 'SubmittedOrderGoal') {
    return withLoggedInMessage(
      withLoggedInMessage(model, SubmittedOnboardingGoal()),
      SucceededSubmitOnboardingGoal({
        response: { onboarding: demoOrderGoalSubmittedStatus },
      }),
    )
  }

  if (cueName === 'ConfirmedPublicWork') {
    return withLoggedInMessage(
      withLoggedInRoute(
        withLoggedInMessage(
          withLoggedInMessage(model, ClickedSkipOnboardingBilling()),
          SucceededSkipOnboardingBilling({
            response: { onboarding: demoOrderCompletedStatus },
          }),
        ),
        OrderRoute(),
      ),
      SucceededLoadCustomerOrder({
        response: {
          order: demoCustomerOrder('submitted'),
        },
      }),
    )
  }

  if (cueName === 'LoadedSubmittedOrder') {
    return withLoggedInMessage(
      withLoggedInRoute(model, OrderRoute()),
      SucceededLoadCustomerOrder({
        response: {
          order: demoCustomerOrder('submitted'),
        },
      }),
    )
  }

  if (cueName === 'AdvancedOrderScoping') {
    return withLoggedInMessage(
      model,
      SucceededLoadCustomerOrder({
        response: {
          order: demoCustomerOrder('scoping'),
        },
      }),
    )
  }

  if (cueName === 'AdvancedOrderQueued') {
    return withLoggedInMessage(
      model,
      SucceededLoadCustomerOrder({
        response: {
          order: demoCustomerOrder('agent_queued'),
        },
      }),
    )
  }

  if (cueName === 'AdvancedOrderRunning') {
    return withLoggedInMessage(
      model,
      SucceededLoadCustomerOrder({
        response: {
          order: demoCustomerOrder('agent_running'),
        },
      }),
    )
  }

  return evo(model, { playback: () => 'complete' })
}

const initialDemoModel = (model: Model): Model => {
  if (model.mode === 'training') {
    return Model({
      cueIndex: -1,
      elapsedMs: 0,
      loggedIn: LoggedIn.init(demoProjectRoute(), demoAuthBootstrap),
      maybeSelectedTrainingSceneNodeId: Option.none(),
      mode: 'training',
      playback: 'complete',
      routeKey: model.routeKey,
    })
  }

  if (model.mode === 'order') {
    return Model({
      cueIndex: -1,
      elapsedMs: 0,
      loggedIn: LoggedIn.init(OnboardingRoute(), demoCustomerAuthBootstrap),
      maybeSelectedTrainingSceneNodeId: Option.none(),
      mode: 'order',
      playback: 'paused',
      routeKey: model.routeKey,
    })
  }

  return Model({
    cueIndex: -1,
    elapsedMs: 0,
    loggedIn: LoggedIn.init(demoProjectRoute(), demoAuthBootstrap),
    maybeSelectedTrainingSceneNodeId: Option.none(),
    mode: 'workroom',
    playback: 'paused',
    routeKey: model.routeKey,
  })
}

const replayDemoModel = (model: Model): Model =>
  model.mode === 'training'
    ? initialDemoModel(model)
    : model.mode === 'order'
      ? Model({
          cueIndex: -1,
          elapsedMs: 0,
          loggedIn: LoggedIn.init(OnboardingRoute(), demoCustomerAuthBootstrap),
          maybeSelectedTrainingSceneNodeId: Option.none(),
          mode: 'order',
          playback: 'playing',
          routeKey: model.routeKey,
        })
      : Model({
          cueIndex: -1,
          elapsedMs: 0,
          loggedIn: LoggedIn.init(demoProjectRoute(), demoAuthBootstrap),
          maybeSelectedTrainingSceneNodeId: Option.none(),
          mode: 'workroom',
          playback: 'playing',
          routeKey: model.routeKey,
        })

const applyManualCue = (model: Model, cue: DemoCue): Model =>
  evo(applyCue(model, cue.name), {
    cueIndex: () => cue.atMs,
    elapsedMs: () => cue.atMs,
    playback: () => 'paused',
  })

const jumpToCue = (model: Model, targetCue: DemoCue): Model =>
  cuesForDemoKey(model.routeKey).reduce(
    (next, cue) =>
      cue.atMs <= targetCue.atMs ? applyManualCue(next, cue) : next,
    initialDemoModel(model),
  )

export const update = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tagsExhaustive({
      GotLoggedInDemoMessage: ({ message }) => [
        withLoggedInMessage(model, message),
        [],
      ],
      AdvancedDemoCue: ({ cue }) => {
        if (model.mode === 'training' || model.playback !== 'playing') {
          return [model, []]
        }

        return [
          evo(applyCue(model, cue.name), {
            cueIndex: index => Math.max(index, cue.atMs),
          }),
          [],
        ]
      },
      ClickedReplayDemo: () => [replayDemoModel(model), []],
      ClickedPauseDemo: () => [
        model.mode === 'training'
          ? model
          : evo(model, { playback: () => 'paused' }),
        [],
      ],
      ClickedResumeDemo: () => [
        model.mode === 'training'
          ? model
          : evo(model, { playback: () => 'playing' }),
        [],
      ],
      ClickedPreviousDemoStep: () => [
        model.mode === 'training'
          ? model
          : jumpToCue(model, previousDemoCue(model.routeKey, model.cueIndex)),
        [],
      ],
      ClickedNextDemoStep: () => [
        model.mode === 'training'
          ? model
          : jumpToCue(model, nextDemoCue(model.routeKey, model.cueIndex)),
        [],
      ],
      PressedDemoSpacebar: () => [
        model.mode === 'training'
          ? model
          : evo(model, {
              playback: playback =>
                playback === 'playing' ? 'paused' : 'playing',
            }),
        [],
      ],
      SelectedTrainingSceneNode: ({ nodeId }) => [
        evo(model, {
          maybeSelectedTrainingSceneNodeId: () => Option.some(nodeId),
        }),
        [],
      ],
      TickedDemoPlayback: ({ deltaMs }) => {
        if (model.mode === 'training' || model.playback !== 'playing') {
          return [model, []]
        }

        const elapsedMs = Math.min(DEMO_DURATION_MS, model.elapsedMs + deltaMs)

        return [
          evo(model, {
            elapsedMs: () => elapsedMs,
            playback: () =>
              elapsedMs >= DEMO_DURATION_MS ? 'complete' : model.playback,
          }),
          [],
        ]
      },
    }),
  )

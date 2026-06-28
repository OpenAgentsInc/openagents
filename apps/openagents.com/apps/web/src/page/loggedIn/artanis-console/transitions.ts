import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import {
  errorMessageFromUnknown,
  requestJson,
} from '../commands/api'
import {
  FailedArtanisOperatorApprovalAction,
  FailedLoadArtanisOperatorDashboard,
  FailedArtanisOperatorGoalAction,
  FailedLoadArtanisOperatorConsole,
  FailedLoadArtanisOperatorGoal,
  FailedSaveArtanisOperatorGoal,
  SucceededArtanisOperatorApprovalAction,
  SucceededLoadArtanisOperatorDashboard,
  SucceededArtanisOperatorGoalAction,
  SucceededLoadArtanisOperatorConsole,
  SucceededLoadArtanisOperatorGoal,
  SucceededSaveArtanisOperatorGoal,
  type Message,
} from '../message'
import {
  AgentGoalAction,
  ArtanisOperatorApprovalAction,
  AgentGoalResponse,
  ArtanisOperatorConsoleFailed,
  ArtanisOperatorConsoleLoaded,
  ArtanisOperatorConsoleLoading,
  ArtanisOperatorConsoleResponse,
  ArtanisOperatorDashboardFailed,
  ArtanisOperatorDashboardLoaded,
  ArtanisOperatorDashboardLoading,
  ArtanisOperatorDashboardResponse,
  ArtanisOperatorGoalPanelModel,
  type Model,
} from '../model'
import { noUpdate, type UpdateReturn } from '../transition'

const ARTANIS_AGENT_ID = 'agent_artanis'
const DEFAULT_ARTANIS_OBJECTIVE =
  'Maintain the Pylon v0.2 launch, Forum coordination, and Model Lab improvement loop.'

type ArtanisOperatorGoalScopeRequest = Readonly<{
  href: string
  scopeKey: string
  teamId: string
}>

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const artanisScopeForModel = (
  model: Model,
): ArtanisOperatorGoalScopeRequest | undefined => {
  if (!model.auth.isAdmin || model.route._tag !== 'Chat') {
    return undefined
  }

  const team = model.auth.teams[0]

  if (team === undefined) {
    return undefined
  }

  const params = new URLSearchParams({
    agentId: ARTANIS_AGENT_ID,
    teamId: team.id,
  })

  return {
    href: `/api/autopilot/goals/current?${params.toString()}`,
    scopeKey: `${ARTANIS_AGENT_ID}:${team.id}:operator`,
    teamId: team.id,
  }
}

const modelWithArtanisGoal = (
  model: Model,
  goal: Model['artanisOperatorGoalPanel']['goal'],
  scopeKey: string,
): Model =>
  evo(model, {
    artanisOperatorGoalPanel: panel =>
      ArtanisOperatorGoalPanelModel({
        ...panel,
        error: Option.none(),
        goal,
        objectiveDraft: Option.match(goal, {
          onNone: () =>
            panel.objectiveDraft.trim() === ''
              ? DEFAULT_ARTANIS_OBJECTIVE
              : panel.objectiveDraft,
          onSome: current => current.objective,
        }),
        pendingAction: Option.none(),
        scopeKey,
      }),
  })

export const LoadArtanisOperatorConsole = Command.define(
  'LoadArtanisOperatorConsole',
  {},
  SucceededLoadArtanisOperatorConsole,
  FailedLoadArtanisOperatorConsole,
)(() =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.artanisOperatorConsole.load',
      request: '/api/operator/artanis/console',
      schema: ArtanisOperatorConsoleResponse,
    })

    return SucceededLoadArtanisOperatorConsole({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadArtanisOperatorConsole({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadArtanisOperatorDashboard = Command.define(
  'LoadArtanisOperatorDashboard',
  { callerIdFilter: S.String, threadRef: S.String },
  SucceededLoadArtanisOperatorDashboard,
  FailedLoadArtanisOperatorDashboard,
)(({ callerIdFilter, threadRef }) =>
  Effect.gen(function* () {
    const params = new URLSearchParams()
    const callerId = callerIdFilter.trim()

    if (callerId !== '') {
      params.set('caller_id', callerId)
    }

    if (threadRef.trim() !== '') {
      params.set('thread_ref', threadRef.trim())
    }

    const query = params.toString()
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.artanisOperatorDashboard.load',
      request: `/api/operator/artanis/dashboard${query === '' ? '' : `?${query}`}`,
      schema: ArtanisOperatorDashboardResponse,
    })

    return SucceededLoadArtanisOperatorDashboard({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadArtanisOperatorDashboard({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadArtanisOperatorGoal = Command.define(
  'LoadArtanisOperatorGoal',
  { href: S.String, scopeKey: S.String },
  SucceededLoadArtanisOperatorGoal,
  FailedLoadArtanisOperatorGoal,
)(({ href, scopeKey }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.artanisOperatorGoal.load',
      request: href,
      schema: AgentGoalResponse,
    })

    return SucceededLoadArtanisOperatorGoal({ response, scopeKey })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadArtanisOperatorGoal({
          error: errorMessageFromUnknown(error),
          scopeKey,
        }),
      ),
    ),
  ),
)

export const SaveArtanisOperatorGoal = Command.define(
  'SaveArtanisOperatorGoal',
  {
    goalId: S.optionalKey(S.String),
    objective: S.String,
    scopeKey: S.String,
    teamId: S.String,
  },
  SucceededSaveArtanisOperatorGoal,
  FailedSaveArtanisOperatorGoal,
)((input) =>
  Effect.gen(function* () {
    const maybeGoalId = input.goalId
    const response = yield* requestJson({
      init: {
        body: JSON.stringify(
          maybeGoalId === undefined
            ? {
                agentId: ARTANIS_AGENT_ID,
                objective: input.objective,
                teamId: input.teamId,
                tokenBudget: null,
                visibility: 'team',
              }
            : { objective: input.objective },
        ),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: maybeGoalId === undefined ? 'POST' : 'PATCH',
      },
      name: 'loggedIn.artanisOperatorGoal.save',
      request:
        maybeGoalId === undefined
          ? '/api/autopilot/goals'
          : `/api/autopilot/goals/${encodeURIComponent(maybeGoalId)}`,
      schema: AgentGoalResponse,
    })

    return SucceededSaveArtanisOperatorGoal({
      response,
      scopeKey: input.scopeKey,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedSaveArtanisOperatorGoal({
          error: errorMessageFromUnknown(error),
          scopeKey: input.scopeKey,
        }),
      ),
    ),
  ),
)

const artanisGoalActionPath = (
  goalId: string,
  action: AgentGoalAction,
): string =>
  `/api/autopilot/goals/${encodeURIComponent(goalId)}/${action === 'make_public' ? 'visibility' : action}`

const artanisGoalActionInit = (action: AgentGoalAction): RequestInit => ({
  ...(action === 'make_public'
    ? { body: JSON.stringify({ visibility: 'public' }) }
    : {}),
  cache: 'no-store',
  credentials: 'include',
  headers: {
    accept: 'application/json',
    ...(action === 'make_public' ? { 'content-type': 'application/json' } : {}),
  },
  method: 'POST',
})

export const UpdateArtanisOperatorGoalAction = Command.define(
  'UpdateArtanisOperatorGoalAction',
  { action: AgentGoalAction, goalId: S.String, scopeKey: S.String },
  SucceededArtanisOperatorGoalAction,
  FailedArtanisOperatorGoalAction,
)(({ action, goalId, scopeKey }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: artanisGoalActionInit(action),
      name: 'loggedIn.artanisOperatorGoal.action',
      request: artanisGoalActionPath(goalId, action),
      schema: AgentGoalResponse,
    })

    return SucceededArtanisOperatorGoalAction({ action, response, scopeKey })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedArtanisOperatorGoalAction({
          action,
          error: errorMessageFromUnknown(error),
          scopeKey,
        }),
      ),
    ),
  ),
)

export const UpdateArtanisOperatorApprovalAction = Command.define(
  'UpdateArtanisOperatorApprovalAction',
  { action: ArtanisOperatorApprovalAction, gateRef: S.String },
  SucceededArtanisOperatorApprovalAction,
  FailedArtanisOperatorApprovalAction,
)(({ action, gateRef }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
        method: 'POST',
      },
      name: 'loggedIn.artanisOperatorApproval.action',
      request: `/api/operator/artanis/approval-gates/${encodeURIComponent(gateRef)}/${action}`,
      schema: ArtanisOperatorConsoleResponse,
    })

    return SucceededArtanisOperatorApprovalAction({
      action,
      gateRef,
      response,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedArtanisOperatorApprovalAction({
          action,
          error: errorMessageFromUnknown(error),
          gateRef,
        }),
      ),
    ),
  ),
)

export const updateArtanisConsole = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadArtanisOperatorConsole: () => [
        evo(model, {
          artanisOperatorConsole: () => ArtanisOperatorConsoleLoading(),
        }),
        [LoadArtanisOperatorConsole({})],
        Option.none(),
      ],
      SucceededLoadArtanisOperatorConsole: ({ response }) => [
        evo(model, {
          artanisOperatorConsole: () =>
            ArtanisOperatorConsoleLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadArtanisOperatorConsole: ({ error }) => [
        evo(model, {
          artanisOperatorConsole: () =>
            ArtanisOperatorConsoleFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      RequestedLoadArtanisOperatorDashboard: ({
        callerIdFilter,
        threadRef,
      }) => [
        evo(model, {
          artanisOperatorDashboard: () =>
            ArtanisOperatorDashboardLoading({ callerIdFilter, threadRef }),
        }),
        [LoadArtanisOperatorDashboard({ callerIdFilter, threadRef })],
        Option.none(),
      ],
      SucceededLoadArtanisOperatorDashboard: ({ response }) => [
        evo(model, {
          artanisOperatorDashboard: () =>
            ArtanisOperatorDashboardLoaded({ response }),
          artanisOperatorDashboardCallerIdFilter: () =>
            response.callerIdFilter ?? model.artanisOperatorDashboardCallerIdFilter,
        }),
        [],
        Option.none(),
      ],
      FailedLoadArtanisOperatorDashboard: ({ error }) => [
        evo(model, {
          artanisOperatorDashboard: () =>
            ArtanisOperatorDashboardFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      UpdatedArtanisOperatorDashboardCallerIdFilter: ({ value }) => [
        evo(model, {
          artanisOperatorDashboardCallerIdFilter: () => value,
        }),
        [],
        Option.none(),
      ],
      SubmittedArtanisOperatorDashboardFilter: () => [
        model,
        [
          LoadArtanisOperatorDashboard({
            callerIdFilter: model.artanisOperatorDashboardCallerIdFilter,
            threadRef: '',
          }),
        ],
        Option.none(),
      ],
      SelectedArtanisOperatorDashboardThread: ({ threadRef }) => [
        model,
        [
          LoadArtanisOperatorDashboard({
            callerIdFilter: model.artanisOperatorDashboardCallerIdFilter,
            threadRef,
          }),
        ],
        Option.none(),
      ],
      RequestedLoadArtanisOperatorGoal: ({ scopeKey }) => [
        evo(model, {
          artanisOperatorGoalPanel: panel =>
            ArtanisOperatorGoalPanelModel({
              ...panel,
              error: Option.none(),
              pendingAction: Option.some('Loading'),
              scopeKey,
            }),
        }),
        [],
        Option.none(),
      ],
      SucceededLoadArtanisOperatorGoal: ({ response, scopeKey }) => [
        modelWithArtanisGoal(
          model,
          response.goal === null ? Option.none() : Option.some(response.goal),
          scopeKey,
        ),
        [],
        Option.none(),
      ],
      FailedLoadArtanisOperatorGoal: ({ error, scopeKey }) =>
        scopeKey !== model.artanisOperatorGoalPanel.scopeKey
          ? noUpdate(model)
          : [
              evo(model, {
                artanisOperatorGoalPanel: panel =>
                  ArtanisOperatorGoalPanelModel({
                    ...panel,
                    error: Option.some(error),
                    pendingAction: Option.none(),
                  }),
              }),
              [],
              Option.none(),
            ],
      UpdatedArtanisOperatorGoalObjectiveDraft: ({ value }) => [
        evo(model, {
          artanisOperatorGoalPanel: panel =>
            ArtanisOperatorGoalPanelModel({
              ...panel,
              objectiveDraft: value,
            }),
        }),
        [],
        Option.none(),
      ],
      SubmittedArtanisOperatorGoal: () => {
        const scope = artanisScopeForModel(model)

        if (scope === undefined) {
          return noUpdate(model)
        }

        const objective = model.artanisOperatorGoalPanel.objectiveDraft.trim()

        if (objective === '') {
          return [
            evo(model, {
              artanisOperatorGoalPanel: panel =>
                ArtanisOperatorGoalPanelModel({
                  ...panel,
                  error: Option.some('Goal is required.'),
                }),
            }),
            [],
            Option.none(),
          ]
        }

        return [
          evo(model, {
            artanisOperatorGoalPanel: panel =>
              ArtanisOperatorGoalPanelModel({
                ...panel,
                error: Option.none(),
                pendingAction: Option.some('Saving'),
                scopeKey: scope.scopeKey,
              }),
          }),
          [
            SaveArtanisOperatorGoal({
              ...Option.match(model.artanisOperatorGoalPanel.goal, {
                onNone: () => ({}),
                onSome: goal => ({ goalId: goal.id }),
              }),
              objective,
              scopeKey: scope.scopeKey,
              teamId: scope.teamId,
            }),
          ],
          Option.none(),
        ]
      },
      SucceededSaveArtanisOperatorGoal: ({ response, scopeKey }) => [
        modelWithArtanisGoal(
          model,
          response.goal === null ? Option.none() : Option.some(response.goal),
          scopeKey,
        ),
        [],
        Option.none(),
      ],
      FailedSaveArtanisOperatorGoal: ({ error, scopeKey }) =>
        scopeKey !== model.artanisOperatorGoalPanel.scopeKey
          ? noUpdate(model)
          : [
              evo(model, {
                artanisOperatorGoalPanel: panel =>
                  ArtanisOperatorGoalPanelModel({
                    ...panel,
                    error: Option.some(error),
                    pendingAction: Option.none(),
                  }),
              }),
              [],
              Option.none(),
            ],
      ClickedArtanisOperatorGoalAction: ({ action }) =>
        Option.match(model.artanisOperatorGoalPanel.goal, {
          onNone: () => noUpdate(model),
          onSome: goal => [
            evo(model, {
              artanisOperatorGoalPanel: panel =>
                ArtanisOperatorGoalPanelModel({
                  ...panel,
                  error: Option.none(),
                  pendingAction: Option.some(action),
                }),
            }),
            [
              UpdateArtanisOperatorGoalAction({
                action,
                goalId: goal.id,
                scopeKey: model.artanisOperatorGoalPanel.scopeKey,
              }),
            ],
            Option.none(),
          ],
        }),
      SucceededArtanisOperatorGoalAction: ({ action, response, scopeKey }) => [
        modelWithArtanisGoal(
          model,
          action === 'clear' || response.goal === null
            ? Option.none()
            : Option.some(response.goal),
          scopeKey,
        ),
        [],
        Option.none(),
      ],
      FailedArtanisOperatorGoalAction: ({ error, scopeKey }) =>
        scopeKey !== model.artanisOperatorGoalPanel.scopeKey
          ? noUpdate(model)
          : [
              evo(model, {
                artanisOperatorGoalPanel: panel =>
                  ArtanisOperatorGoalPanelModel({
                    ...panel,
                    error: Option.some(error),
                    pendingAction: Option.none(),
                  }),
              }),
              [],
              Option.none(),
            ],
      ClickedArtanisOperatorApprovalAction: ({ action, gateRef }) => [
        model,
        [UpdateArtanisOperatorApprovalAction({ action, gateRef })],
        Option.none(),
      ],
      SucceededArtanisOperatorApprovalAction: ({ response }) => [
        evo(model, {
          artanisOperatorConsole: () =>
            ArtanisOperatorConsoleLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedArtanisOperatorApprovalAction: ({ error }) => [
        evo(model, {
          artanisOperatorConsole: () =>
            ArtanisOperatorConsoleFailed({ error }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )

export const artanisOperatorInitialCommands = (
  model: Model,
): ReadonlyArray<Command.Command<Message>> => {
  const scope = artanisScopeForModel(model)

  return scope === undefined
    ? model.auth.isAdmin && model.route._tag === 'OperatorDashboard'
      ? [
          LoadArtanisOperatorDashboard({
            callerIdFilter: model.artanisOperatorDashboardCallerIdFilter,
            threadRef: '',
          }),
        ]
      : []
    : [
        LoadArtanisOperatorConsole({}),
        LoadArtanisOperatorGoal({
          href: scope.href,
          scopeKey: scope.scopeKey,
        }),
      ]
}

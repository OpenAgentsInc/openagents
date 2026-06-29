import { Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'

import {
  errorMessageFromUnknown,
  requestJson,
} from '../commands/api'
import {
  FailedAgentGoalAction,
  FailedLoadAgentGoal,
  FailedSaveAgentGoal,
  SucceededAgentGoalAction,
  SucceededLoadAgentGoal,
  SucceededSaveAgentGoal,
} from '../message'
import {
  AgentGoalAction,
  AgentGoalResponse,
} from '../model'

export const LoadAgentGoal = Command.define(
  'LoadAgentGoal',
  { href: S.String, scopeKey: S.String },
  SucceededLoadAgentGoal,
  FailedLoadAgentGoal,
)(({ href, scopeKey }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.agentGoal.load',
      request: href,
      schema: AgentGoalResponse,
    })

    return SucceededLoadAgentGoal({ response, scopeKey })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAgentGoal({
          error: errorMessageFromUnknown(error),
          scopeKey,
        }),
      ),
    ),
  ),
)

export const SaveAgentGoal = Command.define(
  'SaveAgentGoal',
  {
    agentId: S.String,
    goalId: S.optionalKey(S.String),
    objective: S.String,
    scopeKey: S.String,
    tokenBudget: S.NullOr(S.Int),
    teamId: S.optionalKey(S.String),
    projectId: S.optionalKey(S.String),
  },
  SucceededSaveAgentGoal,
  FailedSaveAgentGoal,
)((input) =>
  Effect.gen(function* () {
    const body = JSON.stringify({
      agentId: input.agentId,
      objective: input.objective,
      tokenBudget: input.tokenBudget,
      ...(input.teamId === undefined ? {} : { teamId: input.teamId }),
      ...(input.projectId === undefined ? {} : { projectId: input.projectId }),
    })
    const response = yield* requestJson({
      init: {
        body,
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: input.goalId === undefined ? 'POST' : 'PATCH',
      },
      name: 'loggedIn.agentGoal.save',
      request:
        input.goalId === undefined
          ? '/api/autopilot/goals'
          : `/api/autopilot/goals/${encodeURIComponent(input.goalId)}`,
      schema: AgentGoalResponse,
    })

    return SucceededSaveAgentGoal({
      response,
      scopeKey: input.scopeKey,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedSaveAgentGoal({
          error: errorMessageFromUnknown(error),
          scopeKey: input.scopeKey,
        }),
      ),
    ),
  ),
)

const actionRequest = (
  _goalId: string,
  action: AgentGoalAction,
): RequestInit => ({
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

const actionPath = (goalId: string, action: AgentGoalAction): string =>
  `/api/autopilot/goals/${encodeURIComponent(goalId)}/${action === 'make_public' ? 'visibility' : action}`

export const UpdateAgentGoalAction = Command.define(
  'UpdateAgentGoalAction',
  { action: AgentGoalAction, goalId: S.String, scopeKey: S.String },
  SucceededAgentGoalAction,
  FailedAgentGoalAction,
)(({ action, goalId, scopeKey }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: actionRequest(goalId, action),
      name: 'loggedIn.agentGoal.action',
      request: actionPath(goalId, action),
      schema: AgentGoalResponse,
    })

    return SucceededAgentGoalAction({ action, response, scopeKey })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedAgentGoalAction({
          action,
          error: errorMessageFromUnknown(error),
          scopeKey,
        }),
      ),
    ),
  ),
)

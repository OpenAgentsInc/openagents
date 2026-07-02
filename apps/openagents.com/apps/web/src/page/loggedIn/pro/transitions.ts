import { Effect, Match as M, Option } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedLoadProAgentDashboard,
  Message,
  SucceededLoadProAgentDashboard,
} from '../message'
import {
  Model,
  ProAgentDashboardFailed,
  ProAgentDashboardLoaded,
  ProAgentDashboardLoading,
  ProAgentDashboardResponse,
} from '../model'
import { type UpdateReturn, noUpdate } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

export const LoadProAgentDashboard = Command.define(
  'LoadProAgentDashboard',
  SucceededLoadProAgentDashboard,
  FailedLoadProAgentDashboard,
)(
  requestJson({
    init: {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    },
    name: 'loggedIn.pro.agentDashboard.load',
    request: '/api/operator/pro/status',
    schema: ProAgentDashboardResponse,
  }).pipe(
    Effect.map(response => SucceededLoadProAgentDashboard({ response })),
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadProAgentDashboard({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const updatePro = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadProAgentDashboard: () => [
        evo(model, { proAgentDashboard: () => ProAgentDashboardLoading() }),
        [LoadProAgentDashboard()],
        Option.none(),
      ],
      SucceededLoadProAgentDashboard: ({ response }) => [
        evo(model, {
          proAgentDashboard: () => ProAgentDashboardLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadProAgentDashboard: ({ error }) => [
        evo(model, {
          proAgentDashboard: () => ProAgentDashboardFailed({ error }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )

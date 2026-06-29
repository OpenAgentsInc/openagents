import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import {
  errorMessageFromUnknown,
  requestJson,
} from '../commands/api'
import {
  FailedAutopilotDecisionAction,
  FailedLoadAutopilotDecisions,
  Message,
  SucceededAutopilotDecisionAction,
  SucceededLoadAutopilotDecisions,
} from '../message'
import {
  AutopilotDecisionActFailed,
  AutopilotDecisionActSubmitting,
  AutopilotDecisionActSucceeded,
  AutopilotDecisionActionResponse,
  AutopilotDecisionListResponse,
  AutopilotDecisionsFailed,
  AutopilotDecisionsLoaded,
  AutopilotDecisionsLoading,
  AutopilotWorkReviewAction,
  Model,
} from '../model'
import { type UpdateReturn } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const autopilotDecisionsPath = (): string => '/api/autopilot/decisions'

const autopilotDecisionActionsPath = (decisionRef: string): string =>
  `/api/autopilot/decisions/${encodeURIComponent(decisionRef)}/actions`

export const LoadAutopilotDecisions = Command.define(
  'LoadAutopilotDecisions',
  {},
  SucceededLoadAutopilotDecisions,
  FailedLoadAutopilotDecisions,
)(() =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.autopilotDecisions.list.load',
      request: autopilotDecisionsPath(),
      schema: AutopilotDecisionListResponse,
    })

    return SucceededLoadAutopilotDecisions({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAutopilotDecisions({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const SubmitAutopilotDecisionAction = Command.define(
  'SubmitAutopilotDecisionAction',
  {
    action: AutopilotWorkReviewAction,
    decisionRef: S.String,
  },
  SucceededAutopilotDecisionAction,
  FailedAutopilotDecisionAction,
)(({ action, decisionRef }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ action }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
          'idempotency-key': `browser-decision:${action}:${decisionRef}`,
        },
        method: 'POST',
      },
      name: 'loggedIn.autopilotDecisions.action.submit',
      request: autopilotDecisionActionsPath(decisionRef),
      schema: AutopilotDecisionActionResponse,
    })

    return SucceededAutopilotDecisionAction({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedAutopilotDecisionAction({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const updateAutopilotDecisions = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadAutopilotDecisions: () => [
        evo(model, { autopilotDecisions: () => AutopilotDecisionsLoading() }),
        [LoadAutopilotDecisions({})],
        Option.none(),
      ],
      SucceededLoadAutopilotDecisions: ({ response }) => [
        evo(model, {
          autopilotDecisions: () => AutopilotDecisionsLoaded({ response }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadAutopilotDecisions: ({ error }) => [
        evo(model, {
          autopilotDecisions: () => AutopilotDecisionsFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      SubmittedAutopilotDecisionAction: ({ action, decisionRef }) => [
        evo(model, {
          autopilotDecisionAct: () =>
            AutopilotDecisionActSubmitting({ action, decisionRef }),
        }),
        [SubmitAutopilotDecisionAction({ action, decisionRef })],
        Option.none(),
      ],
      SucceededAutopilotDecisionAction: ({ response }) => [
        evo(model, {
          autopilotDecisionAct: () =>
            AutopilotDecisionActSucceeded({ response }),
          autopilotDecisions: () => AutopilotDecisionsLoading(),
        }),
        [LoadAutopilotDecisions({})],
        Option.none(),
      ],
      FailedAutopilotDecisionAction: ({ error }) => [
        evo(model, {
          autopilotDecisionAct: () => AutopilotDecisionActFailed({ error }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => [model, [], Option.none()]),
  )

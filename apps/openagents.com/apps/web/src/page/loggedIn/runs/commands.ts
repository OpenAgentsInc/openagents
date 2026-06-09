import { Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'

import {
  errorMessageFromUnknown,
  requestJson,
} from '../commands/api'
import {
  FailedFetchAutopilotRun,
  FailedLaunchAutopilotRun,
  SucceededFetchAutopilotRun,
  SucceededLaunchAutopilotRun,
} from '../message'
import {
  AgentRunDetailResponse,
  AgentRunLaunchResponse,
} from '../model'

export const LaunchAutopilotRun = Command.define(
  'LaunchAutopilotRun',
  { prompt: S.String, requestId: S.String },
  SucceededLaunchAutopilotRun,
  FailedLaunchAutopilotRun,
)(({ prompt, requestId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({
          goal: prompt,
          prompt,
          runnerBackend: 'shc_vm',
        }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.autopilot.launch',
      request: '/api/omni/agent-runs',
      schema: AgentRunLaunchResponse,
    })

    return SucceededLaunchAutopilotRun({ requestId, response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLaunchAutopilotRun({
          error: errorMessageFromUnknown(error),
          requestId,
        }),
      ),
    ),
  ),
)

export const FetchAutopilotRun = Command.define(
  'FetchAutopilotRun',
  { runId: S.String },
  SucceededFetchAutopilotRun,
  FailedFetchAutopilotRun,
)(({ runId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.autopilot.fetch',
      request: `/api/omni/agent-runs/${encodeURIComponent(runId)}`,
      schema: AgentRunDetailResponse,
    })

    return SucceededFetchAutopilotRun({ response, runId })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedFetchAutopilotRun({
          error: errorMessageFromUnknown(error),
          runId,
        }),
      ),
    ),
  ),
)

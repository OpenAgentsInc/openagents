import type {
  AgentRunAssignment,
  AppDeployAssignment,
} from '@openagentsinc/sync-schema'
import { Context, Effect, Layer } from 'effect'

import {
  type DispatchResult,
  dispatchAgentRunToShc,
  dispatchDeploymentToShc,
} from '../omni-runs'
import { type OmniError, omniDispatchErrorFromUnknown } from './errors'

export type OmniDispatchConfig = Readonly<{
  controlApiBearerToken?: string | undefined
  controlApiUrl?: string | undefined
  dispatchMode?: string | undefined
  fetcher?: typeof fetch | undefined
}>

export type OmniDispatchServiceDependencies = Readonly<{
  dispatchAgentRunToShc?: typeof dispatchAgentRunToShc | undefined
  dispatchDeploymentToShc?: typeof dispatchDeploymentToShc | undefined
}>

export type OmniDispatchServiceShape = Readonly<{
  dispatchAgentRun: (
    assignment: AgentRunAssignment,
    config: OmniDispatchConfig,
  ) => Effect.Effect<DispatchResult, OmniError>
  dispatchDeployment: (
    assignment: AppDeployAssignment,
    config: OmniDispatchConfig,
  ) => Effect.Effect<DispatchResult, OmniError>
}>

export class OmniDispatchService extends Context.Service<
  OmniDispatchService,
  OmniDispatchServiceShape
>()('openagents/OmniDispatchService') {}

const dispatchEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniError> =>
  Effect.tryPromise({
    try: run,
    catch: error => omniDispatchErrorFromUnknown(operation, error),
  })

export const makeOmniDispatchService = (
  dependencies: OmniDispatchServiceDependencies = {},
): OmniDispatchServiceShape => {
  const dispatchAgentRun =
    dependencies.dispatchAgentRunToShc ?? dispatchAgentRunToShc
  const dispatchDeployment =
    dependencies.dispatchDeploymentToShc ?? dispatchDeploymentToShc

  return {
    dispatchAgentRun: (assignment, config) =>
      dispatchEffect('dispatch_agent_run_to_shc', () =>
        dispatchAgentRun(assignment, config),
      ).pipe(Effect.withSpan('OmniDispatchService.dispatchAgentRun')),
    dispatchDeployment: (assignment, config) =>
      dispatchEffect('dispatch_deployment_to_shc', () =>
        dispatchDeployment(assignment, config),
      ).pipe(Effect.withSpan('OmniDispatchService.dispatchDeployment')),
  }
}

export const OmniDispatchServiceLive = Layer.succeed(
  OmniDispatchService,
  makeOmniDispatchService(),
)

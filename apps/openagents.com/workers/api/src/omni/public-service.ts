import { Context, Effect, Layer } from 'effect'

import {
  type AgentRunBundle,
  type DeploymentBundle,
  publicAgentRunBundle,
  publicDeploymentBundle,
} from '../omni-runs'
import { type OmniError, omniProjectionErrorFromUnknown } from './errors'

export type PublicAgentRunBundle = ReturnType<typeof publicAgentRunBundle>
export type PublicDeploymentBundle = ReturnType<typeof publicDeploymentBundle>

export type OmniPublicProjectionServiceShape = Readonly<{
  agentRunBundle: (
    bundle: AgentRunBundle,
  ) => Effect.Effect<PublicAgentRunBundle, OmniError>
  agentRunBundles: (
    bundles: ReadonlyArray<AgentRunBundle>,
  ) => Effect.Effect<ReadonlyArray<PublicAgentRunBundle>, OmniError>
  deploymentBundle: (
    bundle: DeploymentBundle,
  ) => Effect.Effect<PublicDeploymentBundle, OmniError>
  deploymentBundles: (
    bundles: ReadonlyArray<DeploymentBundle>,
  ) => Effect.Effect<ReadonlyArray<PublicDeploymentBundle>, OmniError>
}>

export class OmniPublicProjectionService extends Context.Service<
  OmniPublicProjectionService,
  OmniPublicProjectionServiceShape
>()('openagents/OmniPublicProjectionService') {}

const projectionEffect = <A>(
  operation: string,
  run: () => A,
): Effect.Effect<A, OmniError> =>
  Effect.try({
    try: run,
    catch: error => omniProjectionErrorFromUnknown(operation, error),
  })

export const makeOmniPublicProjectionService =
  (): OmniPublicProjectionServiceShape => ({
    agentRunBundle: bundle =>
      projectionEffect('public_agent_run_bundle', () =>
        publicAgentRunBundle(bundle),
      ).pipe(Effect.withSpan('OmniPublicProjectionService.agentRunBundle')),
    agentRunBundles: bundles =>
      projectionEffect('public_agent_run_bundles', () =>
        bundles.map(publicAgentRunBundle),
      ).pipe(Effect.withSpan('OmniPublicProjectionService.agentRunBundles')),
    deploymentBundle: bundle =>
      projectionEffect('public_deployment_bundle', () =>
        publicDeploymentBundle(bundle),
      ).pipe(Effect.withSpan('OmniPublicProjectionService.deploymentBundle')),
    deploymentBundles: bundles =>
      projectionEffect('public_deployment_bundles', () =>
        bundles.map(publicDeploymentBundle),
      ).pipe(Effect.withSpan('OmniPublicProjectionService.deploymentBundles')),
  })

export const OmniPublicProjectionServiceLive = Layer.succeed(
  OmniPublicProjectionService,
  makeOmniPublicProjectionService(),
)

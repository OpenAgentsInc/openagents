import { Context, Effect, Layer } from 'effect'

import {
  type DeploymentBundle,
  type DeploymentRecord,
  type OmniEventRecord,
  type OmniRunStore,
} from '../omni-runs'
import { type OmniError, omniErrorFromUnknown } from './errors'

export type OmniDeploymentRepositoryShape = Readonly<{
  appendDeploymentEvents: (
    deployId: string,
    events: ReadonlyArray<OmniEventRecord>,
    status?: DeploymentRecord['status'],
    externalDeployId?: string,
  ) => Effect.Effect<void, OmniError>
  findDeploymentForUser: (
    userId: string,
    deployId: string,
  ) => Effect.Effect<DeploymentBundle | undefined, OmniError>
  listDeploymentsForUser: (
    userId: string,
    limit: number,
  ) => Effect.Effect<ReadonlyArray<DeploymentBundle>, OmniError>
  saveDeployment: (
    deployment: DeploymentRecord,
    events: ReadonlyArray<OmniEventRecord>,
  ) => Effect.Effect<void, OmniError>
}>

export class OmniDeploymentRepository extends Context.Service<
  OmniDeploymentRepository,
  OmniDeploymentRepositoryShape
>()('openagents/OmniDeploymentRepository') {}

const repositoryEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniError> =>
  Effect.tryPromise({
    try: run,
    catch: error => omniErrorFromUnknown(operation, error),
  })

export const makeOmniDeploymentRepository = (
  store: OmniRunStore,
): OmniDeploymentRepositoryShape => ({
  appendDeploymentEvents: (deployId, events, status, externalDeployId) =>
    repositoryEffect('append_deployment_events', () =>
      store.appendDeploymentEvents(deployId, events, status, externalDeployId),
    ).pipe(Effect.withSpan('OmniDeploymentRepository.appendDeploymentEvents')),
  findDeploymentForUser: (userId, deployId) =>
    repositoryEffect('find_deployment_for_user', () =>
      store.findDeploymentForUser(userId, deployId),
    ).pipe(Effect.withSpan('OmniDeploymentRepository.findDeploymentForUser')),
  listDeploymentsForUser: (userId, limit) =>
    repositoryEffect('list_deployments_for_user', () =>
      store.listDeploymentsForUser(userId, limit),
    ).pipe(Effect.withSpan('OmniDeploymentRepository.listDeploymentsForUser')),
  saveDeployment: (deployment, events) =>
    repositoryEffect('save_deployment', () =>
      store.saveDeployment(deployment, events),
    ).pipe(Effect.withSpan('OmniDeploymentRepository.saveDeployment')),
})

export const makeOmniDeploymentRepositoryLayer = (store: OmniRunStore) =>
  Layer.succeed(OmniDeploymentRepository, makeOmniDeploymentRepository(store))

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { buildPublicTassadarRunSummaryEnvelopeForRequest } from './public-tassadar-run-summary-routes'
import {
  TrainingPublicDistributedRunScaleEndpoint,
  projectTrainingPublicDistributedRunScaleFromEnvelope,
} from './training-public-distributed-run-scale'

export { TrainingPublicDistributedRunScaleEndpoint }

type PublicDistributedRunScaleDependencies<Bindings> = Readonly<{
  buildSummaryEnvelope?: (
    request: Request,
    env: Bindings,
  ) => Promise<Record<string, unknown>>
}>

const defaultBuildSummaryEnvelope =
  buildPublicTassadarRunSummaryEnvelopeForRequest as (
    request: Request,
    env: unknown,
  ) => Promise<Record<string, unknown>>

export const handleTrainingPublicDistributedRunScaleApi = <Bindings>(
  request: Request,
  env: Bindings,
  dependencies: PublicDistributedRunScaleDependencies<Bindings> = {},
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.promise(() =>
        (dependencies.buildSummaryEnvelope ?? defaultBuildSummaryEnvelope)(
          request,
          env,
        ),
      ).pipe(
        Effect.map(envelope =>
          noStoreJsonResponse(
            projectTrainingPublicDistributedRunScaleFromEnvelope(envelope),
          ),
        ),
      )

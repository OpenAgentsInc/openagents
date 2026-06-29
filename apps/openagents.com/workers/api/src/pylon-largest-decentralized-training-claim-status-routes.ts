import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { buildPublicTassadarRunSummaryEnvelopeForRequest } from './public-tassadar-run-summary-routes'
import {
  PylonLargestDecentralizedTrainingClaimEndpoint,
  projectPylonLargestDecentralizedTrainingClaimStatusFromEnvelope,
} from './pylon-largest-decentralized-training-claim-status'

export { PylonLargestDecentralizedTrainingClaimEndpoint }

type PylonLargestClaimStatusDependencies<Bindings> = Readonly<{
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

export const handlePylonLargestDecentralizedTrainingClaimStatusApi = <Bindings>(
  request: Request,
  env: Bindings,
  dependencies: PylonLargestClaimStatusDependencies<Bindings> = {},
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
            projectPylonLargestDecentralizedTrainingClaimStatusFromEnvelope(
              envelope,
            ),
          ),
        ),
      )

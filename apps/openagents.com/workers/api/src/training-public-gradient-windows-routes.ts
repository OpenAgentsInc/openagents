import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TrainingPublicGradientWindowsEndpoint,
  projectTrainingPublicGradientWindows,
} from './training-public-gradient-windows'

export { TrainingPublicGradientWindowsEndpoint }

export const handleTrainingPublicGradientWindowsApi = (
  request: Request,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse(projectTrainingPublicGradientWindows()),
      )

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TrainingPostTrainingVibeTestCloseoutEndpoint,
  projectTrainingPostTrainingVibeTestCloseout,
} from './training-post-training-vibe-test-closeout'

export { TrainingPostTrainingVibeTestCloseoutEndpoint }

export const handleTrainingPostTrainingVibeTestCloseoutApi = (
  request: Request,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse(
          projectTrainingPostTrainingVibeTestCloseout(),
        ),
      )

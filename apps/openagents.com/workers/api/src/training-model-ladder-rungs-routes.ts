import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TrainingModelLadderRungsEndpoint,
  projectTrainingModelLadderRungs,
} from './training-model-ladder-rungs'

export { TrainingModelLadderRungsEndpoint }

export const handleTrainingModelLadderRungsApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectTrainingModelLadderRungs()))

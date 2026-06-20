import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TrainingAblationDeriskingLedgerEndpoint,
  projectTrainingAblationDeriskingLedger,
} from './training-ablation-derisking-ledger'

export { TrainingAblationDeriskingLedgerEndpoint }

export const handleTrainingAblationDeriskingLedgerApi = (
  request: Request,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse(projectTrainingAblationDeriskingLedger()),
      )

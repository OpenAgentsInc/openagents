import { Effect } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { glmPoolHeartbeatLatestRecordOracle } from './glm-pool-heartbeat'
import { projectGlmFleetReadinessForEnv } from './glm-fleet-readiness'
import type { SupplyLaneCredentialEnv } from './model-serving-policy'

export type GlmFleetReadinessDeps = Readonly<{
  enabled: boolean
  env: SupplyLaneCredentialEnv
}>

export const handleGlmFleetReadiness = (
  request: Request,
  deps: GlmFleetReadinessDeps,
) =>
  Effect.sync<Response>(() => {
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'GET') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }

    return noStoreJsonResponse(
      projectGlmFleetReadinessForEnv(
        deps.env,
        glmPoolHeartbeatLatestRecordOracle,
      ),
    )
  })

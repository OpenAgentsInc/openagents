import { Effect, Schema as S } from 'effect'

import {
  handleBusinessFunnelDashboardApi,
  type BusinessFunnelRuntime,
  systemBusinessFunnelRuntime,
} from './business-funnel-dashboard'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

class BusinessFunnelDashboardReadFailure extends S.TaggedErrorClass<BusinessFunnelDashboardReadFailure>()(
  'BusinessFunnelDashboardReadFailure',
  {
    cause: S.Unknown,
  },
) {}

// Payload staleness is declared in business-funnel-dashboard.ts via the shared
// public-projection-staleness contract.
export const handlePublicBusinessFunnelDashboardApi = (
  request: Request,
  db: D1Database,
  runtime: BusinessFunnelRuntime = systemBusinessFunnelRuntime,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.tryPromise({
    try: () => handleBusinessFunnelDashboardApi(db, runtime),
    catch: cause => new BusinessFunnelDashboardReadFailure({ cause }),
  }).pipe(
    Effect.map(payload => noStoreJsonResponse(payload)),
    Effect.catch(() =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'business_funnel_dashboard_read_failed' },
          { status: 500 },
        ),
      ),
    ),
  )
}

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  projectLiquidityMarketSkeleton,
  projectRiskMarketSkeleton,
} from './open-markets-skeletons'
import { projectOpenMarketsSurface } from './open-markets-surface'

/**
 * Read-only route handlers for the unified open-markets surface and the inert
 * liquidity & risk market skeletons (EPIC #5510, issue #5514).
 *
 * All handlers are GET-only and emit no-store JSON projections. They are
 * inert: no handler accepts a body, mutates state, quotes a fillable price,
 * binds a policy, or moves money.
 */

export const handleOpenMarketsSurfaceApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectOpenMarketsSurface()))

export const handleLiquidityMarketSkeletonApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectLiquidityMarketSkeleton()))

export const handleRiskMarketSkeletonApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectRiskMarketSkeleton()))

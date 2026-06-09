import { Match as M, Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

export const ThreadRouteIdle = ts('ThreadRouteIdle', {})
export const ThreadRouteResolving = ts('ThreadRouteResolving', {
  routeId: S.String,
  scope: S.String,
})
export const ThreadRouteAuthorized = ts('ThreadRouteAuthorized', {
  routeId: S.String,
  runId: S.String,
  scope: S.String,
})
export const ThreadRouteUnavailable = ts('ThreadRouteUnavailable', {
  reason: S.String,
  routeId: S.String,
})
export const ThreadRouteState = S.Union([
  ThreadRouteIdle,
  ThreadRouteResolving,
  ThreadRouteAuthorized,
  ThreadRouteUnavailable,
])
export type ThreadRouteState = typeof ThreadRouteState.Type

export const threadRouteScope = (routeId: string): string => `thread:${routeId}`

export const resolvingThreadRoute = (
  routeId: string,
): typeof ThreadRouteResolving.Type =>
  ThreadRouteResolving({
    routeId,
    scope: threadRouteScope(routeId),
  })

export const authorizedThreadRoute = (
  routeId: string,
  runId: string,
): typeof ThreadRouteAuthorized.Type =>
  ThreadRouteAuthorized({
    routeId,
    runId,
    scope: threadRouteScope(routeId),
  })

export const unavailableThreadRoute = (
  routeId: string,
  reason: string,
): typeof ThreadRouteUnavailable.Type =>
  ThreadRouteUnavailable({
    reason,
    routeId,
  })

export const authorizedThreadRouteScope = (
  state: ThreadRouteState,
): string | undefined =>
  M.value(state).pipe(
    M.tag('ThreadRouteAuthorized', state => state.scope),
    M.orElse(() => undefined),
  )

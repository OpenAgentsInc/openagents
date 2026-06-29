import { Effect } from 'effect'

type HttpResponse = globalThis.Response

export type RouteHandler<Env> = (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
) => Effect.Effect<HttpResponse>

export type ExactRoute<Env> = Readonly<{
  handler: RouteHandler<Env>
  path: string
}>

const routePathMatches = (routePath: string, pathname: string): boolean => {
  const routeSegments = routePath.split('/')
  const pathSegments = pathname.split('/')

  if (routeSegments.length !== pathSegments.length) {
    return false
  }

  return routeSegments.every((segment, index) => {
    const pathSegment = pathSegments[index]
    if (pathSegment === undefined) {
      return false
    }
    return segment.startsWith(':') || segment === pathSegment
  })
}

export const routeExact = <Env>(
  routes: ReadonlyArray<ExactRoute<Env>>,
  pathname: string,
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> | undefined =>
  routes
    .find(route => routePathMatches(route.path, pathname))
    ?.handler(request, env, ctx)

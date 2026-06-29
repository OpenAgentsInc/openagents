import { Effect } from 'effect'

type HttpResponse = globalThis.Response

export type RouteHandler<Bindings> = (
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) => Effect.Effect<HttpResponse>

export type ExactRoute<Bindings> = Readonly<{
  handler: RouteHandler<Bindings>
  path: string
}>

export const routeExact = <Bindings>(
  routes: ReadonlyArray<ExactRoute<Bindings>>,
  pathname: string,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> | undefined =>
  routes.find(route => route.path === pathname)?.handler(request, env, ctx)

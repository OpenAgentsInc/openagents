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

export const routeExact = <Env>(
  routes: ReadonlyArray<ExactRoute<Env>>,
  pathname: string,
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> | undefined =>
  routes.find(route => route.path === pathname)?.handler(request, env, ctx)

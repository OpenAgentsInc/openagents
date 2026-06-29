import type { ExactRoute } from '../http/router'

export type ExactRouteRegistry<Env> = Readonly<{
  paths: ReadonlyArray<string>
  routes: ReadonlyArray<ExactRoute<Env>>
}>

export class DuplicateExactRoutePathError extends Error {
  constructor(path: string) {
    super(`Duplicate exact route path: ${path}`)
    this.name = 'DuplicateExactRoutePathError'
  }
}

export const exactRoutePaths = <Env>(
  routes: ReadonlyArray<ExactRoute<Env>>,
): ReadonlyArray<string> => routes.map(route => route.path)

export const makeExactRouteRegistry = <Env>(
  routes: ReadonlyArray<ExactRoute<Env>>,
): ExactRouteRegistry<Env> => {
  const seen = new Set<string>()
  for (const route of routes) {
    if (seen.has(route.path)) {
      throw new DuplicateExactRoutePathError(route.path)
    }
    seen.add(route.path)
  }

  return {
    paths: exactRoutePaths(routes),
    routes,
  }
}

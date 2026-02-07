import type { RouteId, RouteMatch } from "../app/route.js"

export type LoaderKey = string

const stableEntries = (input: Readonly<Record<string, string>>): ReadonlyArray<readonly [string, string]> =>
  Object.entries(input).sort(([aKey, aVal], [bKey, bVal]) =>
    aKey === bKey ? aVal.localeCompare(bVal) : aKey.localeCompare(bKey)
  )

const stableSearch = (search: URLSearchParams): string => {
  const entries = Array.from(search.entries()).sort(([aKey, aVal], [bKey, bVal]) =>
    aKey === bKey ? aVal.localeCompare(bVal) : aKey.localeCompare(bKey)
  )
  return new URLSearchParams(entries).toString()
}

export const makeLoaderKey = (input: {
  readonly routeId: RouteId
  readonly match: RouteMatch
  readonly sessionScopeKey: string
}): LoaderKey => {
  const params = stableEntries(input.match.params)
  const search = stableSearch(input.match.search)

  // Keep this stable and unambiguous. We use JSON to avoid delimiter collisions.
  return JSON.stringify([input.routeId, input.match.pathname, params, search, input.sessionScopeKey])
}


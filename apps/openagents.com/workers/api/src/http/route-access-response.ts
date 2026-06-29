import { Match as M } from 'effect'

import type { RouteAccessError } from '../thread-access'
import { noStoreJsonResponse, redirectResponse } from './responses'

type RouteAccessResponseTarget =
  | Readonly<{ surface: 'api' }>
  | Readonly<{ href: string; surface: 'product' }>

export const routeAccessResponse = (
  error: RouteAccessError,
  target: RouteAccessResponseTarget,
): Response => {
  if (target.surface === 'product') {
    return redirectResponse(target.href)
  }

  return M.value(error).pipe(
    M.tagsExhaustive({
      RouteAccessForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      RouteAccessNotFound: () =>
        noStoreJsonResponse({ error: 'not_found' }, { status: 404 }),
    }),
  )
}

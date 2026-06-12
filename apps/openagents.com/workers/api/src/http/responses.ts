import { jsonResponse } from '@openagentsinc/sync-worker'

export const redirectResponse = (
  location: string,
  cookies: ReadonlyArray<string> = [],
): Response => {
  const headers = new Headers({
    'cache-control': 'no-store',
    location,
  })

  for (const cookie of cookies) {
    headers.append('set-cookie', cookie)
  }

  return new Response(null, { status: 302, headers })
}

export const methodNotAllowed = (
  allowedMethods: ReadonlyArray<string>,
): Response => {
  const headers = new Headers({
    allow: allowedMethods.join(', '),
    'cache-control': 'no-store',
  })

  return jsonResponse({ error: 'method_not_allowed' }, { status: 405, headers })
}

export const serverError = (): Response =>
  jsonResponse({ error: 'internal_server_error' }, { status: 500 })

export const unauthorized = (): Response =>
  jsonResponse({ error: 'unauthorized' }, { status: 401 })

export const forbidden = (): Response => {
  const headers = new Headers({ 'cache-control': 'no-store' })

  return jsonResponse({ error: 'forbidden' }, { status: 403, headers })
}

export const noStoreJsonResponse = (
  value: unknown,
  init: ResponseInit = {},
): Response => {
  const headers = new Headers(init.headers)
  headers.set('cache-control', 'no-store')

  return jsonResponse(value, { ...init, headers })
}

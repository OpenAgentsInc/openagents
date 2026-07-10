export type HttpHeaderEntries = ReadonlyArray<readonly [string, string]>

export type JsonHttpResult<Body = unknown> = Readonly<{
  kind: 'json'
  body: Body
  status: number
  statusText?: string | undefined
  headers: HttpHeaderEntries
}>

export type StreamHttpResult = Readonly<{
  kind: 'stream'
  body: ReadableStream<Uint8Array> | null
  status: number
  statusText?: string | undefined
  headers: HttpHeaderEntries
}>

export type HttpResult<Body = unknown> = JsonHttpResult<Body> | StreamHttpResult

export type HttpResultInit = Readonly<{
  status?: number | undefined
  statusText?: string | undefined
  headers?: HeadersInit | undefined
}>

export type HttpHeadersDecorator = (headers: Headers) => void

const headerEntries = (headers: Headers): HttpHeaderEntries => [
  ...[...headers.entries()]
    .filter(([name]) => name.toLowerCase() !== 'set-cookie')
    .map(([name, value]) => [name, value] as const),
  ...headers
    .getSetCookie()
    .map(value => ['set-cookie', value] as const),
]

const headersFromEntries = (entries: HttpHeaderEntries): Headers =>
  entries.reduce((headers, [name, value]) => {
    headers.append(name, value)
    return headers
  }, new Headers())

export const jsonHttpResult = <Body>(
  body: Body,
  init: HttpResultInit = {},
): JsonHttpResult<Body> => {
  const headers = new Headers(init.headers)
  headers.set('content-type', 'application/json')

  return {
    kind: 'json',
    body,
    status: init.status ?? 200,
    ...(init.statusText === undefined ? {} : { statusText: init.statusText }),
    headers: headerEntries(headers),
  }
}

export const noStoreJsonResult = <Body>(
  body: Body,
  init: HttpResultInit = {},
): JsonHttpResult<Body> => {
  const headers = new Headers(init.headers)
  headers.set('cache-control', 'no-store')

  return jsonHttpResult(body, { ...init, headers })
}

export const methodNotAllowedResult = (
  allowedMethods: ReadonlyArray<string>,
): JsonHttpResult<Readonly<{ error: 'method_not_allowed' }>> =>
  noStoreJsonResult(
    { error: 'method_not_allowed' as const },
    {
      status: 405,
      headers: { allow: allowedMethods.join(', ') },
    },
  )

export const streamHttpResult = (
  body: ReadableStream<Uint8Array> | null,
  init: HttpResultInit,
): StreamHttpResult => ({
  kind: 'stream',
  body,
  status: init.status ?? 200,
  ...(init.statusText === undefined ? {} : { statusText: init.statusText }),
  headers: headerEntries(new Headers(init.headers)),
})

export const decorateJsonHttpResultHeaders = <Body>(
  result: JsonHttpResult<Body>,
  decorate: HttpHeadersDecorator,
): JsonHttpResult<Body> => {
  const headers = headersFromEntries(result.headers)
  decorate(headers)

  return { ...result, headers: headerEntries(headers) }
}

export const materializeHttpResult = (result: HttpResult): Response =>
  new Response(
    result.kind === 'json' ? JSON.stringify(result.body) : result.body,
    {
      status: result.status,
      ...(result.statusText === undefined
        ? {}
        : { statusText: result.statusText }),
      headers: headersFromEntries(result.headers),
    },
  )

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

  return materializeHttpResult(streamHttpResult(null, { status: 302, headers }))
}

export const methodNotAllowed = (
  allowedMethods: ReadonlyArray<string>,
): Response => materializeHttpResult(methodNotAllowedResult(allowedMethods))

export const serverError = (): Response =>
  materializeHttpResult(
    jsonHttpResult({ error: 'internal_server_error' }, { status: 500 }),
  )

export const unauthorized = (): Response =>
  materializeHttpResult(
    jsonHttpResult({ error: 'unauthorized' }, { status: 401 }),
  )

export const forbidden = (): Response =>
  materializeHttpResult(
    noStoreJsonResult({ error: 'forbidden' }, { status: 403 }),
  )

export const noStoreJsonResponse = (
  value: unknown,
  init: ResponseInit = {},
): Response => materializeHttpResult(noStoreJsonResult(value, init))

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

/**
 * The outcome of matching a single trailing ref against a path prefix.
 *
 * - `no_match` — the pathname is outside the prefix, or is the bare prefix with
 *   no ref after it. The caller must return `undefined` so the dispatcher falls
 *   through to the next route.
 * - `malformed` — the pathname IS inside the prefix, but its tail is not a
 *   legal percent-encoded sequence (for example a bare `%`). The caller must
 *   CLAIM the path and answer the same way it answers an unknown ref.
 * - `ref` — a decoded ref to look up.
 */
export type PathRefMatch =
  | Readonly<{ _tag: 'malformed' }>
  | Readonly<{ _tag: 'no_match' }>
  | Readonly<{ _tag: 'ref'; ref: string }>

/**
 * Decode one URL path segment without letting a malformed percent-escape become
 * a Worker-level DEFECT.
 *
 * `decodeURIComponent('%')` throws `URIError`. Path matchers run inside the
 * `Effect.gen` body of `makeWorkerRouteRequest`, where a thrown error is not a
 * typed failure but a defect: it escapes to the `Effect.catchCause` in
 * `index.ts`, which answers 500 `internal_server_error` AND files a
 * `severity: 'critical'` `unhandled_exception` backend incident. A caller
 * typing a stray `%` into a public URL is not a critical backend fault, so the
 * decode is guarded at the seam instead.
 */
export const safeDecodeUriComponent = (value: string): string | undefined => {
  try {
    return decodeURIComponent(value)
  } catch {
    return undefined
  }
}

/**
 * Match the whole tail of `pathname` after `prefix` as one opaque ref.
 *
 * Used by the public receipt readers, whose refs are already-issued opaque
 * public identifiers and so are the entire remainder of the path. A malformed
 * escape is reported as `malformed` rather than thrown, so the reader can
 * answer 404 exactly as it does for a well-formed unknown ref — the two are
 * indistinguishable to a caller, and 404 does not leak that the ref was parsed
 * at all.
 */
export const pathRefFromPrefix = (
  pathname: string,
  prefix: string,
): PathRefMatch => {
  if (!pathname.startsWith(prefix) || pathname.length <= prefix.length) {
    return { _tag: 'no_match' }
  }

  const decoded = safeDecodeUriComponent(pathname.slice(prefix.length))

  return decoded === undefined
    ? { _tag: 'malformed' }
    : { _tag: 'ref', ref: decoded }
}

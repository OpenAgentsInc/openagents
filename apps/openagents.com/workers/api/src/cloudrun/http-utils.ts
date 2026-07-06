/**
 * CFG-9 (#8524): pure request helpers for the Cloud Run entry, kept free of
 * the worker import graph so unit tests stay light.
 */

/**
 * Cloud Run terminates TLS at its front proxy; origin-derived values
 * (OpenAuth redirect URIs, cookie security) need the public https origin,
 * reported via `X-Forwarded-Proto`.
 */
export const withForwardedProto = (request: Request): Request => {
  if (request.headers.get('x-forwarded-proto') !== 'https') {
    return request
  }
  const url = new URL(request.url)
  if (url.protocol === 'https:') {
    return request
  }
  url.protocol = 'https:'
  return new Request(url, request)
}

/**
 * Rewrite the request host from `X-Forwarded-Host` when the deployment
 * says its front proxy sets it (`OPENAGENTS_TRUST_FORWARDED_HOST=1`).
 * Needed because the worker routes by hostname (auth.openagents.com vs the
 * app host) while Cloud Run's own URL always carries the run.app host —
 * the CFG-10 load balancer (and smoke tooling) forwards the original host
 * here. No privilege derives from the host alone: it only selects the
 * issuer-vs-app route tree, and redirect targets stay allowlisted.
 */
export const withForwardedHost = (
  request: Request,
  trust: boolean,
): Request => {
  if (!trust) return request
  const forwarded = request.headers
    .get('x-forwarded-host')
    ?.split(',')[0]
    ?.trim()
  if (forwarded === undefined || forwarded === '') return request
  const url = new URL(request.url)
  if (url.host === forwarded) return request
  url.host = forwarded
  return new Request(url, request)
}

/** Constant-shape bearer check for `POST /internal/cron`. */
export const cronAuthorized = (
  request: Request,
  token: string | undefined,
): boolean => {
  if (token === undefined || token.length === 0) return false
  const header = request.headers.get('authorization') ?? ''
  return header === `Bearer ${token}`
}

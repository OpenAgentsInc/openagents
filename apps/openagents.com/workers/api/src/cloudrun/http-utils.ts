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

/** Constant-shape bearer check for `POST /internal/cron`. */
export const cronAuthorized = (
  request: Request,
  token: string | undefined,
): boolean => {
  if (token === undefined || token.length === 0) return false
  const header = request.headers.get('authorization') ?? ''
  return header === `Bearer ${token}`
}

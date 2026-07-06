/**
 * Aiur's own session cookie jar (AIUR-1, #8499). Deliberately independent
 * from `apps/openagents.com/workers/api/src/auth-cookies.ts` — Aiur is a
 * separate Cloudflare Worker on a separate origin (`aiur.openagents.com`),
 * so its session cookies must never collide with (or be confused for) the
 * main site's `oa_access`/`oa_refresh` cookies even though the shape is the
 * same pattern.
 */

export const AIUR_ACCESS_COOKIE = 'aiur_access'
export const AIUR_REFRESH_COOKIE = 'aiur_refresh'
export const AIUR_AUTH_STATE_COOKIE = 'aiur_auth_state'
export const AIUR_SESSION_MAX_AGE_SECONDS = 60 * 60 * 24 * 30
export const AIUR_AUTH_STATE_MAX_AGE_SECONDS = 60 * 10

export const parseCookies = (request: Request): ReadonlyMap<string, string> => {
  const cookies = new Map<string, string>()
  const header = request.headers.get('cookie')

  if (header === null) {
    return cookies
  }

  for (const part of header.split(';')) {
    const [rawName, ...rawValue] = part.trim().split('=')

    if (rawName === undefined || rawName === '') {
      continue
    }

    cookies.set(rawName, decodeURIComponent(rawValue.join('=')))
  }

  return cookies
}

export const serializeCookie = (
  name: string,
  value: string,
  maxAgeSeconds: number,
  path = '/',
): string =>
  [
    `${name}=${encodeURIComponent(value)}`,
    `Max-Age=${Math.max(0, Math.floor(maxAgeSeconds))}`,
    `Path=${path}`,
    'HttpOnly',
    'Secure',
    'SameSite=Lax',
  ].join('; ')

export const expiredCookie = (name: string, path = '/'): string =>
  serializeCookie(name, '', 0, path)

export type AiurTokens = Readonly<{ access: string; refresh: string }>

export const appendSessionCookies = (
  headers: Headers,
  tokens: AiurTokens,
): void => {
  headers.append(
    'set-cookie',
    serializeCookie(
      AIUR_ACCESS_COOKIE,
      tokens.access,
      AIUR_SESSION_MAX_AGE_SECONDS,
    ),
  )
  headers.append(
    'set-cookie',
    serializeCookie(
      AIUR_REFRESH_COOKIE,
      tokens.refresh,
      AIUR_SESSION_MAX_AGE_SECONDS,
    ),
  )
}

export const appendClearSessionCookies = (headers: Headers): void => {
  headers.append('set-cookie', expiredCookie(AIUR_ACCESS_COOKIE))
  headers.append('set-cookie', expiredCookie(AIUR_REFRESH_COOKIE))
}

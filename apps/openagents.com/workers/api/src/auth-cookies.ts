import type { Tokens } from '@openauthjs/openauth/client'

export const ACCESS_COOKIE = 'oa_access'
export const REFRESH_COOKIE = 'oa_refresh'
export const AUTH_STATE_COOKIE = 'oa_auth_state'
export const SESSION_MAX_AGE_SECONDS = 60 * 60 * 24 * 400
export const AUTH_STATE_MAX_AGE_SECONDS = 60 * 10

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
  domain?: string,
): string =>
  [
    `${name}=${encodeURIComponent(value)}`,
    `Max-Age=${Math.max(0, Math.floor(maxAgeSeconds))}`,
    `Path=${path}`,
    ...(domain === undefined ? [] : [`Domain=${domain}`]),
    'HttpOnly',
    'Secure',
    'SameSite=Lax',
  ].join('; ')

export const expiredCookie = (
  name: string,
  path = '/',
  domain?: string,
): string => serializeCookie(name, '', 0, path, domain)

export const appendSessionCookies = (
  headers: Headers,
  tokens: Tokens,
): void => {
  headers.append(
    'set-cookie',
    serializeCookie(ACCESS_COOKIE, tokens.access, SESSION_MAX_AGE_SECONDS),
  )
  headers.append(
    'set-cookie',
    serializeCookie(REFRESH_COOKIE, tokens.refresh, SESSION_MAX_AGE_SECONDS),
  )
}

const sessionCookieClearPaths = ['/', '/auth'] as const

const sessionCookieClearDomains = (
  hostname?: string,
): ReadonlyArray<string> => {
  const normalized = hostname?.trim().toLowerCase()

  if (
    normalized === undefined ||
    normalized === '' ||
    normalized === 'localhost' ||
    normalized.includes(':') ||
    !normalized.includes('.')
  ) {
    return []
  }

  return [normalized, `.${normalized}`]
}

export const appendClearSessionCookies = (
  headers: Headers,
  hostname?: string,
): void => {
  for (const path of sessionCookieClearPaths) {
    headers.append('set-cookie', expiredCookie(ACCESS_COOKIE, path))
    headers.append('set-cookie', expiredCookie(REFRESH_COOKIE, path))

    for (const domain of sessionCookieClearDomains(hostname)) {
      headers.append('set-cookie', expiredCookie(ACCESS_COOKIE, path, domain))
      headers.append('set-cookie', expiredCookie(REFRESH_COOKIE, path, domain))
    }
  }
}

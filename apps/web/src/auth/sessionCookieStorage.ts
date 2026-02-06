import { CookieSessionStorage } from '@workos/authkit-session';

/**
 * Minimal Request/Response storage adapter for @workos/authkit-session.
 *
 * We avoid TanStack Start's internal middleware context here; callers can take the
 * returned Set-Cookie header (from `saveSession/clearSession`) and apply it to
 * their Response explicitly.
 */
export class WebCookieSessionStorage extends CookieSessionStorage<Request, Response> {
  getSession(request: Request): Promise<string | null> {
    const cookieHeader = request.headers.get('cookie');
    if (!cookieHeader) return Promise.resolve(null);

    const cookies = parseCookieHeader(cookieHeader);
    const value = cookies[this.cookieName];
    return Promise.resolve(value ? decodeURIComponent(value) : null);
  }
}

function parseCookieHeader(cookieHeader: string): Record<string, string> {
  return Object.fromEntries(
    cookieHeader.split(';').map((cookie) => {
      const [key, ...valueParts] = cookie.trim().split('=');
      return [key, valueParts.join('=')];
    }),
  );
}

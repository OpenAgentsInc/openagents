import type { MiddlewareHandler } from 'hono';
import type { OpenClawEnv } from '../types';
import { err } from '../response';

export function extractServiceToken(headers: Headers): string | null {
  const auth = headers.get('authorization');
  if (auth && auth.toLowerCase().startsWith('bearer ')) {
    const token = auth.slice(7).trim();
    return token.length > 0 ? token : null;
  }

  const headerToken = headers.get('x-openagents-service-token');
  if (headerToken) {
    const token = headerToken.trim();
    return token.length > 0 ? token : null;
  }

  return null;
}

export function isServiceTokenValid(headers: Headers, expected: string | undefined): boolean {
  if (!expected) return false;
  const token = extractServiceToken(headers);
  return !!token && token === expected;
}

export const requireServiceToken: MiddlewareHandler<{ Bindings: OpenClawEnv }> = async (c, next) => {
  const expected = c.env.OPENAGENTS_SERVICE_TOKEN;
  if (!isServiceTokenValid(c.req.raw.headers, expected)) {
    return c.json(err('unauthorized', 'unauthorized'), 401);
  }
  return next();
};

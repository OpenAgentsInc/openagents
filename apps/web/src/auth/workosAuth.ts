import { AuthKitCore, getConfigurationProvider, getWorkOS, sessionEncryption } from '@workos/authkit-session';
import { Effect } from 'effect';
import { WebCookieSessionStorage } from './sessionCookieStorage';

export type MagicAuthSendResult = {
  readonly email: string;
};

export type MagicAuthVerifyResult = {
  readonly userId: string;
  readonly setCookieHeader: string;
};

function getAuthKitConfig() {
  return getConfigurationProvider().getConfig();
}

export function sendMagicAuthCode(email: string): Effect.Effect<MagicAuthSendResult, Error> {
  return Effect.tryPromise({
    try: async () => {
      const workos = getWorkOS();
      await workos.userManagement.createMagicAuth({ email });
      return { email };
    },
    catch: (err) => (err instanceof Error ? err : new Error(String(err))),
  });
}

export function verifyMagicAuthCode(input: {
  readonly request: Request;
  readonly email: string;
  readonly code: string;
}): Effect.Effect<MagicAuthVerifyResult, Error> {
  return Effect.tryPromise({
    try: async () => {
      const config = getAuthKitConfig();
      const workos = getWorkOS();

      const userAgent = input.request.headers.get('user-agent') ?? undefined;
      const ipAddress =
        input.request.headers.get('cf-connecting-ip') ??
        input.request.headers.get('x-forwarded-for')?.split(',')[0]?.trim() ??
        undefined;

      const auth = await workos.userManagement.authenticateWithMagicAuth({
        email: input.email,
        code: input.code,
        clientId: config.clientId,
        userAgent,
        ipAddress,
      });

      const core = new AuthKitCore(config, workos, sessionEncryption);
      const sessionData = await core.encryptSession({
        accessToken: auth.accessToken,
        refreshToken: auth.refreshToken,
        user: auth.user,
        impersonator: auth.impersonator,
      });

      const storage = new WebCookieSessionStorage(config);
      const { headers } = await storage.saveSession(undefined, sessionData);
      const setCookieHeader = headers?.['Set-Cookie'];
      if (typeof setCookieHeader !== 'string') {
        throw new Error('missing Set-Cookie header from session storage');
      }

      return { userId: auth.user.id, setCookieHeader };
    },
    catch: (err) => (err instanceof Error ? err : new Error(String(err))),
  });
}

export function clearSessionCookie(): Effect.Effect<{ readonly setCookieHeader: string }, Error> {
  return Effect.tryPromise({
    try: async () => {
      const config = getAuthKitConfig();
      const storage = new WebCookieSessionStorage(config);
      const { headers } = await storage.clearSession(new Response());
      const setCookieHeader = headers?.['Set-Cookie'];
      if (typeof setCookieHeader !== 'string') {
        throw new Error('missing Set-Cookie header from session storage');
      }
      return { setCookieHeader };
    },
    catch: (err) => (err instanceof Error ? err : new Error(String(err))),
  });
}

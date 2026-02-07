import { createFileRoute } from '@tanstack/react-router';
import { createAuthService } from '@workos/authkit-session';
import { WebCookieSessionStorage } from '../auth/sessionCookieStorage';

type SessionPayload = {
  readonly ok: true;
  readonly userId: string | null;
  readonly sessionId: string | null;
  readonly token: string | null;
  readonly user: {
    readonly id: string;
    readonly email: string | null;
    readonly firstName: string | null;
    readonly lastName: string | null;
  } | null;
};

function json(body: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      ...(init?.headers ?? {}),
    },
  });
}

const authkit = createAuthService<Request, Response>({
  sessionStorageFactory: (config) => new WebCookieSessionStorage(config),
});

export const Route = createFileRoute('/api/auth/session')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        let auth: any;
        let refreshedSessionData: string | undefined;
        try {
          const result = await authkit.withAuth(request);
          auth = result.auth;
          refreshedSessionData = result.refreshedSessionData;
        } catch {
          auth = { user: null };
          refreshedSessionData = undefined;
        }

        const user = auth.user
          ? {
              id: auth.user.id,
              email: auth.user.email ?? null,
              firstName: auth.user.firstName ?? null,
              lastName: auth.user.lastName ?? null,
            }
          : null;

        const payload: SessionPayload = {
          ok: true,
          userId: user?.id ?? null,
          sessionId: auth.user ? (auth.sessionId ?? null) : null,
          token: auth.user ? auth.accessToken : null,
          user,
        };

        // If WorkOS refreshed the session, persist it back into the cookie.
        if (refreshedSessionData) {
          const { headers } = await authkit.saveSession(undefined, refreshedSessionData);
          const setCookie = headers?.['Set-Cookie'];
          if (typeof setCookie === 'string') {
            return json(payload, { status: 200, headers: { 'Set-Cookie': setCookie } });
          }
        }

        return json(payload, { status: 200 });
      },
    },
  },
});

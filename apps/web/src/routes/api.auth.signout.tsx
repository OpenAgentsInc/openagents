import { createFileRoute } from '@tanstack/react-router';
import { Effect } from 'effect';
import { clearSessionCookie } from '../auth/workosAuth';
import { getServerRuntime } from '../effect/serverRuntime';
import { TelemetryService } from '../effect/telemetry';

function json(body: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      ...(init?.headers ?? {}),
    },
  });
}

export const Route = createFileRoute('/api/auth/signout')({
  server: {
    handlers: {
      POST: async () => {
        const { runtime } = getServerRuntime();
        return runtime.runPromise(
          Effect.gen(function* () {
            const telemetry = yield* TelemetryService;
            const { setCookieHeader } = yield* clearSessionCookie();
            yield* telemetry.withNamespace('auth.session').event('session.cleared');

            return json(
              { ok: true },
              {
                status: 200,
                headers: {
                  'Set-Cookie': setCookieHeader,
                },
              },
            );
          }).pipe(
            Effect.catchAll((err) => {
              console.error('[auth.signout]', err);
              return Effect.succeed(json({ ok: false, error: 'signout_failed' }, { status: 500 }));
            }),
          ),
        );
      },
    },
  },
});


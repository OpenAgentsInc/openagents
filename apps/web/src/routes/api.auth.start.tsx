import { createFileRoute } from '@tanstack/react-router';
import { Effect } from 'effect';
import { sendMagicAuthCode } from '../auth/workosAuth';
import { getServerRuntime } from '../effect/serverRuntime';
import { TelemetryService } from '../effect/telemetry';

type StartBody = {
  readonly email?: unknown;
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

function normalizeEmail(raw: string): string {
  return raw.trim().toLowerCase();
}

export const Route = createFileRoute('/api/auth/start')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        let body: StartBody;
        try {
          body = (await request.json()) as StartBody;
        } catch {
          return json({ ok: false, error: 'invalid_json' }, { status: 400 });
        }

        const emailRaw = typeof body.email === 'string' ? body.email : '';
        const email = normalizeEmail(emailRaw);
        if (!email || !email.includes('@') || email.length > 320) {
          return json({ ok: false, error: 'invalid_email' }, { status: 400 });
        }

        const { runtime } = getServerRuntime();
        return runtime.runPromise(
          Effect.gen(function* () {
            const telemetry = yield* TelemetryService;
            yield* sendMagicAuthCode(email);
            yield* telemetry.withNamespace('auth.magic').event('magic_code.sent');
            return json({ ok: true });
          }).pipe(
            Effect.catchAll((err) => {
              console.error('[auth.start]', err);
              return Effect.succeed(json({ ok: false, error: 'send_failed' }, { status: 500 }));
            }),
          ),
        );
      },
    },
  },
});

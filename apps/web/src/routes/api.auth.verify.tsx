import { createFileRoute } from '@tanstack/react-router';
import { Effect } from 'effect';
import { verifyMagicAuthCode } from '../auth/workosAuth';
import { getServerRuntime } from '../effect/serverRuntime';
import { TelemetryService } from '../effect/telemetry';

type VerifyBody = {
  readonly email?: unknown;
  readonly code?: unknown;
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

function normalizeCode(raw: string): string {
  return raw.replace(/\s+/g, '');
}

export const Route = createFileRoute('/api/auth/verify')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        let body: VerifyBody;
        try {
          body = (await request.json()) as VerifyBody;
        } catch {
          return json({ ok: false, error: 'invalid_json' }, { status: 400 });
        }

        const emailRaw = typeof body.email === 'string' ? body.email : '';
        const codeRaw = typeof body.code === 'string' ? body.code : '';
        const email = normalizeEmail(emailRaw);
        const code = normalizeCode(codeRaw);

        if (!email || !email.includes('@') || email.length > 320) {
          return json({ ok: false, error: 'invalid_email' }, { status: 400 });
        }
        // WorkOS Magic Auth uses a 6-digit code, but accept any 4-10 digit input for flexibility.
        if (!/^[0-9]{4,10}$/.test(code)) {
          return json({ ok: false, error: 'invalid_code' }, { status: 400 });
        }

        const { runtime } = getServerRuntime();
        return runtime.runPromise(
          Effect.gen(function* () {
            const telemetry = yield* TelemetryService;
            const { userId, setCookieHeader } = yield* verifyMagicAuthCode({ request, email, code });
            yield* telemetry.withNamespace('auth.magic').event('magic_code.verified', { userId });

            return json(
              { ok: true, userId },
              {
                status: 200,
                headers: {
                  'Set-Cookie': setCookieHeader,
                },
              },
            );
          }).pipe(
            Effect.catchAll((err) => {
              console.error('[auth.verify]', err);
              return Effect.succeed(json({ ok: false, error: 'verify_failed' }, { status: 401 }));
            }),
          ),
        );
      },
    },
  },
});


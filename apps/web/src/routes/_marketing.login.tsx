import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { useMemo, useRef, useState } from 'react';
import { makeEzRegistry } from '@openagentsinc/effuse';
import { EffuseMount } from '../components/EffuseMount';
import { runLoginPage } from '../effuse-pages/login';
import { TelemetryService } from '../effect/telemetry';
import { clearRootAuthCache } from './__root';

import type { LoginPageModel, LoginStep } from '../effuse-pages/login';

export const Route = createFileRoute('/_marketing/login')({
  loader: async ({ context }) => {
    const result = await context.effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;

        const auth = yield* Effect.tryPromise({
          try: () => getAuth(),
          catch: (err) => err,
        }).pipe(Effect.catchAll(() => Effect.succeed(null)));

        const userId = auth?.user?.id ?? null;
        if (userId) {
          yield* telemetry.withNamespace('route.login').event('login.authed_redirect', { userId });
          return { kind: 'redirect' as const };
        }

        yield* telemetry.withNamespace('route.login').event('login.loaded');
        return { kind: 'ok' as const };
      }),
    );

    if (result.kind === 'redirect') {
      throw redirect({ to: '/autopilot' });
    }

    return {};
  },
  component: LoginPage,
});

function LoginPage() {
  const router = useRouter();
  const [step, setStep] = useState<LoginStep>('email');
  const [email, setEmail] = useState('');
  const [code, setCode] = useState('');
  const [isBusy, setIsBusy] = useState(false);
  const [errorText, setErrorText] = useState<string | null>(null);
  const runtime = router.options.context.effectRuntime;

  const model: LoginPageModel = useMemo(
    () => ({
      step,
      email,
      code,
      isBusy,
      errorText,
    }),
    [step, email, code, isBusy, errorText],
  );

  const ezRegistryRef = useRef(makeEzRegistry());
  const ezRegistry = ezRegistryRef.current;

  // Keep handlers in a stable Map (EffuseMount expects stable Map identity).
  ezRegistry.set('login.email.input', ({ params }) =>
    Effect.sync(() => {
      const paramsMaybe = params as Record<string, string | undefined>;
      setEmail(String(paramsMaybe.email ?? ''));
    }),
  );

  ezRegistry.set('login.code.input', ({ params }) =>
    Effect.sync(() => {
      const paramsMaybe = params as Record<string, string | undefined>;
      setCode(String(paramsMaybe.code ?? ''));
    }),
  );

  ezRegistry.set('login.email.submit', ({ params }) =>
    Effect.gen(function* () {
      if (isBusy) return;

      const paramsMaybe = params as Record<string, string | undefined>;
      const nextEmail = String(paramsMaybe.email ?? email)
        .trim()
        .toLowerCase();
      if (!nextEmail) return;

      yield* Effect.sync(() => {
        setErrorText(null);
        setIsBusy(true);
      });

      const start = Effect.tryPromise({
        try: async () => {
          const r = await fetch('/api/auth/start', {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            credentials: 'include',
            body: JSON.stringify({ email: nextEmail }),
          });
          const data = await r.json().catch(() => null);
          if (!r.ok || !data?.ok) {
            throw new Error(typeof data?.error === 'string' ? data.error : 'send_failed');
          }
        },
        catch: (err) => (err instanceof Error ? err : new Error(String(err))),
      });

      yield* start.pipe(
        Effect.tap(() =>
          Effect.sync(() => {
            setEmail(nextEmail);
            setCode('');
            setStep('code');
          }),
        ),
        Effect.catchAll((err) =>
          Effect.sync(() => {
            runtime
              .runPromise(
                Effect.gen(function* () {
                  const telemetry = yield* TelemetryService;
                  yield* telemetry.withNamespace('ui.login').event('login.start_failed', {
                    message: err.message,
                  });
                }),
              )
              .catch(() => {});

            setErrorText(
              err.message === 'invalid_email'
                ? 'Please enter a valid email.'
                : 'Failed to send code. Try again.',
            );
          }),
        ),
        Effect.ensuring(Effect.sync(() => setIsBusy(false))),
      );
    }),
  );

  ezRegistry.set('login.code.submit', ({ params }) =>
    Effect.gen(function* () {
      if (isBusy) return;
      const nextEmail = email.trim().toLowerCase();
      if (!nextEmail) return;
      const paramsMaybe = params as Record<string, string | undefined>;
      const nextCode = String(paramsMaybe.code ?? code).replace(/\s+/g, '');
      if (!nextCode) return;

      yield* Effect.sync(() => {
        setErrorText(null);
        setIsBusy(true);
      });

      const verify = Effect.tryPromise({
        try: async () => {
          const r = await fetch('/api/auth/verify', {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            credentials: 'include',
            body: JSON.stringify({ email: nextEmail, code: nextCode }),
          });
          const data = await r.json().catch(() => null);
          if (!r.ok || !data?.ok) {
            throw new Error(typeof data?.error === 'string' ? data.error : 'verify_failed');
          }
        },
        catch: (err) => (err instanceof Error ? err : new Error(String(err))),
      });

      yield* verify.pipe(
        Effect.tap(() =>
          Effect.sync(() => {
            clearRootAuthCache();
            router.navigate({ href: '/autopilot' }).catch(() => {});
          }),
        ),
        Effect.catchAll((err) =>
          Effect.sync(() => {
            runtime
              .runPromise(
                Effect.gen(function* () {
                  const telemetry = yield* TelemetryService;
                  yield* telemetry.withNamespace('ui.login').event('login.verify_failed', {
                    message: err.message,
                  });
                }),
              )
              .catch(() => {});

            setErrorText(
              err.message === 'invalid_code'
                ? 'Invalid code. Please try again.'
                : 'Verification failed. Try again.',
            );
          }),
        ),
        Effect.ensuring(Effect.sync(() => setIsBusy(false))),
      );
    }),
  );

  ezRegistry.set('login.code.back', () =>
    Effect.sync(() => {
      if (isBusy) return;
      setErrorText(null);
      setCode('');
      setStep('email');
    }),
  );

  ezRegistry.set('login.code.resend', () =>
    Effect.gen(function* () {
      if (isBusy) return;
      const nextEmail = email.trim().toLowerCase();
      if (!nextEmail) return;

      yield* Effect.sync(() => {
        setErrorText(null);
        setIsBusy(true);
      });

      const resend = Effect.tryPromise({
        try: async () => {
          const r = await fetch('/api/auth/start', {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            credentials: 'include',
            body: JSON.stringify({ email: nextEmail }),
          });
          const data = await r.json().catch(() => null);
          if (!r.ok || !data?.ok) {
            throw new Error(typeof data?.error === 'string' ? data.error : 'send_failed');
          }
        },
        catch: (err) => (err instanceof Error ? err : new Error(String(err))),
      });

      yield* resend.pipe(
        Effect.catchAll((err) =>
          Effect.sync(() => {
            runtime
              .runPromise(
                Effect.gen(function* () {
                  const telemetry = yield* TelemetryService;
                  yield* telemetry.withNamespace('ui.login').event('login.resend_failed', {
                    message: err.message,
                  });
                }),
              )
              .catch(() => {});

            setErrorText('Failed to resend code. Try again.');
          }),
        ),
        Effect.ensuring(Effect.sync(() => setIsBusy(false))),
      );
    }),
  );

  return (
    <EffuseMount
      run={(el) => runLoginPage(el, model)}
      // Avoid re-rendering the Effuse DOM on each keystroke; it causes caret/selection glitches
      // because Effuse replaces the input element. We only rerender on step/busy/error changes.
      deps={[step, isBusy, errorText]}
      ezRegistry={ezRegistry}
      className="flex min-h-0 flex-1 flex-col"
    />
  );
}

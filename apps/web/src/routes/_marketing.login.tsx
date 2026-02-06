import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { useCallback, useMemo, useState } from 'react';
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

  const run = useCallback((el: Element) => runLoginPage(el, model), [model]);

  const onRendered = useCallback(
    (container: Element) => {
      const emailInput = container.querySelector<HTMLInputElement>('input[name="email"]');
      if (emailInput) {
        emailInput.addEventListener('input', () => setEmail(emailInput.value));
        if (step === 'email') emailInput.focus();
      }

      const codeInput = container.querySelector<HTMLInputElement>('input[name="code"]');
      if (codeInput) {
        codeInput.addEventListener('input', () => setCode(codeInput.value));
        if (step === 'code') codeInput.focus();
      }

      const emailForm = container.querySelector<HTMLFormElement>('#login-email-form');
      if (emailForm) {
        emailForm.addEventListener('submit', (e) => {
          e.preventDefault();
          if (isBusy) return;
          const nextEmail = (emailInput?.value ?? email).trim().toLowerCase();
          setErrorText(null);
          setIsBusy(true);
          void fetch('/api/auth/start', {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            credentials: 'include',
            body: JSON.stringify({ email: nextEmail }),
          })
            .then(async (r) => {
              const data = await r.json().catch(() => null);
              if (!r.ok || !data?.ok) {
                throw new Error(typeof data?.error === 'string' ? data.error : 'send_failed');
              }
              setEmail(nextEmail);
              setCode('');
              setStep('code');
            })
            .catch((err) => {
              const msg = err instanceof Error ? err.message : String(err);
              setErrorText(msg === 'invalid_email' ? 'Please enter a valid email.' : 'Failed to send code. Try again.');
            })
            .finally(() => setIsBusy(false));
        });
      }

      const codeForm = container.querySelector<HTMLFormElement>('#login-code-form');
      if (codeForm) {
        codeForm.addEventListener('submit', (e) => {
          e.preventDefault();
          if (isBusy) return;
          const nextCode = (codeInput?.value ?? code).replace(/\\s+/g, '');
          const nextEmail = email.trim().toLowerCase();
          setErrorText(null);
          setIsBusy(true);
          void fetch('/api/auth/verify', {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            credentials: 'include',
            body: JSON.stringify({ email: nextEmail, code: nextCode }),
          })
            .then(async (r) => {
              const data = await r.json().catch(() => null);
              if (!r.ok || !data?.ok) {
                throw new Error(typeof data?.error === 'string' ? data.error : 'verify_failed');
              }
              clearRootAuthCache();
              return router.navigate({ href: '/autopilot' });
            })
            .catch((err) => {
              const msg = err instanceof Error ? err.message : String(err);
              setErrorText(msg === 'invalid_code' ? 'Invalid code. Please try again.' : 'Verification failed. Try again.');
            })
            .finally(() => setIsBusy(false));
        });
      }

      const backBtn = container.querySelector('[data-action="back"]');
      backBtn?.addEventListener('click', () => {
        if (isBusy) return;
        setErrorText(null);
        setCode('');
        setStep('email');
      });

      const resendBtn = container.querySelector('[data-action="resend"]');
      resendBtn?.addEventListener('click', () => {
        if (isBusy) return;
        const nextEmail = email.trim().toLowerCase();
        if (!nextEmail) return;
        setErrorText(null);
        setIsBusy(true);
        void fetch('/api/auth/start', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          credentials: 'include',
          body: JSON.stringify({ email: nextEmail }),
        })
          .then(async (r) => {
            const data = await r.json().catch(() => null);
            if (!r.ok || !data?.ok) {
              throw new Error(typeof data?.error === 'string' ? data.error : 'send_failed');
            }
          })
          .catch(() => setErrorText('Failed to resend code. Try again.'))
          .finally(() => setIsBusy(false));
      });
    },
    [router, step, email, code, isBusy],
  );

  return (
    <EffuseMount
      run={run}
      // Avoid re-rendering the Effuse DOM on each keystroke; it causes caret/selection glitches
      // because Effuse replaces the input element. We only rerender on step/busy/error changes.
      deps={[step, isBusy, errorText]}
      onRendered={onRendered}
      className="flex min-h-0 flex-1 flex-col"
    />
  );
}

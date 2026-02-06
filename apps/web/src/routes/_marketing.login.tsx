import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth, getSignInUrl } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { TelemetryService } from '../effect/telemetry';

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

        const signInUrl = yield* Effect.tryPromise({
          try: () => getSignInUrl({ data: { returnPathname: '/autopilot' } }),
          catch: (err) => err,
        }).pipe(Effect.catchAll(() => Effect.succeed('#')));

        yield* telemetry.withNamespace('route.login').event('login.loaded');
        return { kind: 'ok' as const, signInUrl };
      }),
    );

    if (result.kind === 'redirect') {
      throw redirect({ to: '/autopilot' });
    }

    return { signInUrl: result.signInUrl };
  },
  component: LoginPage,
});

function EnvelopeIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      width="48"
      height="48"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <rect x="2" y="4" width="20" height="16" rx="2" />
      <path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7" />
    </svg>
  );
}

function LoginPage() {
  const { signInUrl } = Route.useLoaderData();

  return (
    <>
      <div className="mx-auto flex flex-1 w-full max-w-5xl items-center justify-center">
        <div className="w-full max-w-[400px] rounded-2xl border border-white/10 bg-bg-secondary/95 p-8 shadow-[0_0_0_1px_rgba(255,255,255,0.06),0_8px_32px_rgba(0,0,0,0.4)]">
          <div className="flex flex-col items-center text-center">
            <div className="mb-6 text-accent">
              <EnvelopeIcon className="size-12" />
            </div>
            <h1 className="text-2xl font-bold tracking-tight text-text-primary">
              Log in to OpenAgents
            </h1>
            <p className="mt-2 text-sm text-text-muted">
              Enter your email and we'll send you a magic link.
            </p>
          </div>

          <div className="mt-8">
            <label
              htmlFor="login-email"
              className="mb-1.5 block text-sm font-medium text-text-dim"
            >
              Email address
            </label>
            <input
              id="login-email"
              type="email"
              placeholder="you@example.com"
              autoComplete="email"
              className="w-full rounded-lg border border-border-dark bg-surface-primary px-4 py-3 text-text-primary placeholder:text-text-dim outline-none transition-colors focus:border-border-focus focus:ring-2 focus:ring-border-focus/30"
            />
          </div>

          <a
            href={signInUrl}
            className="mt-6 flex w-full items-center justify-center gap-2 rounded-xl border border-accent bg-accent px-4 py-3.5 text-base font-semibold text-bg-primary transition-colors hover:bg-accent-muted hover:border-accent-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            Continue with Email
            <span aria-hidden>→</span>
          </a>
        </div>
      </div>
    </>
  );
}

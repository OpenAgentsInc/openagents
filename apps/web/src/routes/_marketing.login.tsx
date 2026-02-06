import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth, getSignInUrl } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { EffuseMount } from '../components/EffuseMount';
import { runLoginPage } from '../effuse-pages/login';
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

function LoginPage() {
  const { signInUrl } = Route.useLoaderData();

  return (
    <EffuseMount
      run={(el) => runLoginPage(el, signInUrl)}
      deps={[signInUrl]}
      className="flex min-h-0 flex-1 flex-col"
    />
  );
}

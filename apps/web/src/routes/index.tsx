import { createFileRoute, redirect } from '@tanstack/react-router';
import { DotsBackground, whitePreset } from '@openagentsinc/hud/react';
import { getAuth, getSignInUrl, getSignUpUrl } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { TelemetryService } from '../effect/telemetry';

export const Route = createFileRoute('/')({
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
          yield* telemetry.withNamespace('route.home').event('home.authed_redirect', { userId });
          return { kind: 'redirect' as const };
        }

        const [signInUrl, signUpUrl] = yield* Effect.all([
          Effect.tryPromise({
            try: () => getSignInUrl({ data: { returnPathname: '/autopilot' } }),
            catch: (err) => err,
          }),
          Effect.tryPromise({
            try: () => getSignUpUrl({ data: { returnPathname: '/autopilot' } }),
            catch: (err) => err,
          }),
        ]);

        yield* telemetry.withNamespace('route.home').event('home.loaded');

        return { kind: 'ok' as const, signInUrl, signUpUrl };
      }),
    );

    if (result.kind === 'redirect') {
      throw redirect({ to: '/autopilot' });
    }

    return { signInUrl: result.signInUrl, signUpUrl: result.signUpUrl };
  },
  component: Home,
});

function Home() {
  const { signInUrl, signUpUrl } = Route.useLoaderData();

  return (
    <div className="fixed inset-0">
      {/* Dots only (no grid), Arwes-style. White preset base + vignette. */}
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: whitePreset.backgroundColor,
          backgroundImage: [
            `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 15%, rgba(0,0,0,0.4) 55%, rgba(0,0,0,0.75) 100%)`,
            whitePreset.backgroundImage,
          ].join(', '),
        }}
      >
        <DotsBackground
          distance={whitePreset.distance}
          dotsColor="hsla(0, 0%, 100%, 0.12)"
          dotsSettings={{ type: 'circle', size: 2 }}
        />
      </div>
      <div className="absolute inset-0 z-10 flex min-h-full min-w-full flex-col p-4">
        <header className="mx-auto flex w-full max-w-5xl items-center justify-between py-2">
          <div className="select-none text-lg font-semibold text-white">OpenAgents</div>
          <div className="flex items-center gap-3">
            <a
              href={signInUrl}
              className="text-sm font-medium text-white/90 hover:text-white"
            >
              Log in
            </a>
            <a
              href={signUpUrl}
              className="inline-flex items-center justify-center rounded-md bg-white px-4 py-2 text-sm font-semibold text-black hover:bg-white/90"
            >
              Start for free
            </a>
          </div>
        </header>

        <main className="mx-auto flex w-full max-w-5xl flex-1 items-center justify-center">
          <div className="w-full max-w-3xl text-center text-white">
            <h1 className="text-balance text-4xl font-semibold tracking-tight sm:text-6xl">
              Introducing Autopilot
            </h1>
            <p className="mx-auto mt-4 max-w-2xl text-pretty text-lg text-white/80 sm:text-xl">
              Your personal agent, no Mac Mini required
            </p>

            <div className="mt-8 flex flex-col items-center justify-center gap-3 sm:flex-row">
              <a
                href={signUpUrl}
                className="inline-flex w-full max-w-xs items-center justify-center rounded-md bg-white px-5 py-3 text-sm font-semibold text-black hover:bg-white/90 sm:w-auto"
              >
                Start for free
              </a>
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}

import { DotsBackground, whitePreset } from '@openagentsinc/hud/react';
import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { HatcheryButton } from '../components/hatchery/HatcheryButton';
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

        yield* telemetry.withNamespace('route.home').event('home.loaded');
        return { kind: 'ok' as const };
      }),
    );

    if (result.kind === 'redirect') {
      throw redirect({ to: '/autopilot' });
    }

    return {};
  },
  component: Home,
});

function Home() {

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
              href="/login"
              className="pr-5 text-base font-medium text-white/90 hover:text-white"
              style={{ fontFamily: 'var(--font-square721)' }}
            >
              Log in
            </a>
            <HatcheryButton href="/login">
              Start for free
            </HatcheryButton>
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
              <HatcheryButton href="/login" className="w-full max-w-xs sm:w-auto">
                Start for free
              </HatcheryButton>
            </div>
          </div>
        </main>

        <footer className="mx-auto flex w-full max-w-5xl items-center justify-between py-4">
          <span className="text-sm text-white/70">
            © {new Date().getFullYear()} OpenAgents, Inc.
          </span>
          <div className="flex items-center gap-4">
            <a
              href="https://x.com/OpenAgentsInc"
              target="_blank"
              rel="noopener noreferrer"
              className="text-white/70 hover:text-white transition-colors"
              aria-label="OpenAgents on X"
            >
              <XIcon className="size-5" />
            </a>
            <a
              href="https://github.com/OpenAgentsInc/openagents"
              target="_blank"
              rel="noopener noreferrer"
              className="text-white/70 hover:text-white transition-colors"
              aria-label="OpenAgents on GitHub"
            >
              <GitHubIcon className="size-5" />
            </a>
          </div>
        </footer>
      </div>
    </div>
  );
}

function XIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor" aria-hidden>
      <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
    </svg>
  );
}

function GitHubIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor" aria-hidden>
      <path d="M12 2C6.477 2 2 6.477 2 12c0 4.42 2.865 8.166 6.839 9.489.5.092.682-.217.682-.482 0-.237-.008-.866-.013-1.7-2.782.604-3.369-1.34-3.369-1.34-.454-1.156-1.11-1.464-1.11-1.464-.908-.62.069-.608.069-.608 1.003.07 1.531 1.03 1.531 1.03.892 1.529 2.341 1.087 2.91.831.092-.646.35-1.086.636-1.336-2.22-.253-4.555-1.11-4.555-4.943 0-1.091.39-1.984 1.029-2.683-.103-.253-.446-1.27.098-2.647 0 0 .84-.269 2.75 1.025A9.578 9.578 0 0112 6.836c.85.004 1.705.114 2.504.336 1.909-1.294 2.747-1.025 2.747-1.025.546 1.377.203 2.394.1 2.647.64.699 1.028 1.592 1.028 2.683 0 3.842-2.339 4.687-4.566 4.935.359.309.678.919.678 1.852 0 1.336-.012 2.415-.012 2.743 0 .267.18.578.688.48C19.138 20.163 22 16.418 22 12c0-5.523-4.477-10-10-10z" />
    </svg>
  );
}

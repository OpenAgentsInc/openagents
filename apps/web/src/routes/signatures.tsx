import { useAtomValue } from '@effect-atom/atom-react';
import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { whitePreset } from '@openagentsinc/hud';
import { EffuseMount } from '../components/EffuseMount';
import { cleanupHudBackground, runHudDotsGridBackground } from '../effuse-pages/hudBackground';
import { runSignaturesPage } from '../effuse-pages/signatures';
import { SignaturesPageDataAtom } from '../effect/atoms/contracts';
import { TelemetryService } from '../effect/telemetry';

export const Route = createFileRoute('/signatures')({
  loader: async ({ context }) => {
    const result = await context.effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;

        const auth = yield* Effect.tryPromise({
          try: () => getAuth(),
          catch: (err) => err,
        }).pipe(Effect.catchAll(() => Effect.succeed(null)));

        const userId = auth?.user?.id ?? null;
        if (!userId) {
          yield* telemetry.withNamespace('route.signatures').event('signatures.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        yield* telemetry.withNamespace('route.signatures').event('signatures.open', { userId });
        return { kind: 'ok' as const, userId };
      }),
    );

    if (result.kind === 'redirect') throw redirect({ to: result.to });
    return { userId: result.userId };
  },
  component: SignaturesPage,
});

function SignaturesPage() {
  const { userId } = Route.useLoaderData();
  const pageData = useAtomValue(SignaturesPageDataAtom(userId));

  return (
    <div className="fixed inset-0 overflow-hidden text-text-primary font-mono">
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: whitePreset.backgroundColor,
          backgroundImage: [
            `radial-gradient(120% 85% at 50% 0%, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0) 55%)`,
            `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0,0,0,0.55) 60%, rgba(0,0,0,0.88) 100%)`,
            whitePreset.backgroundImage,
          ].join(', '),
        }}
      >
        <EffuseMount
          run={(el) =>
            runHudDotsGridBackground(el, {
              distance: whitePreset.distance,
              dotsColor: 'hsla(0, 0%, 100%, 0.035)',
              lineColor: 'hsla(0, 0%, 100%, 0.03)',
              dotsSettings: { type: 'circle', size: 2 },
            })
          }
          onCleanup={cleanupHudBackground}
          className="absolute inset-0 pointer-events-none"
        />
      </div>
      <EffuseMount
        run={(el) => runSignaturesPage(el, pageData)}
        deps={[pageData]}
        className="relative z-10 flex flex-col h-screen overflow-hidden"
      />
    </div>
  );
}

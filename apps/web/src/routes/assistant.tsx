import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { TelemetryService } from '../effect/telemetry';

/**
 * Single flow helper:
 * - ensure user is authenticated
 * - redirect into their single persistent chat thread
 *
 * Thread id is the WorkOS user id (one Autopilot per user).
 */
export const Route = createFileRoute('/assistant')({
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
          yield* telemetry.withNamespace('route.assistant').event('assistant.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        yield* telemetry.withNamespace('route.assistant').event('assistant.redirect', { userId });
        return { kind: 'redirect' as const, to: '/autopilot' as const };
      }),
    );

    if (result.to === '/') {
      throw redirect({ to: '/' });
    }

    throw redirect({ to: '/autopilot' });
  },
  component: () => null,
});

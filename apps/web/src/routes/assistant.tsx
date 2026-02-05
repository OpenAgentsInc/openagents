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
    const { user } = await getAuth();
    if (!user) {
      throw redirect({ to: '/' });
    }

    await context.effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;
        yield* telemetry.withNamespace('route.assistant').event('assistant.redirect', { userId: user.id });
      }),
    );

    throw redirect({
      to: '/chat/$chatId',
      params: { chatId: user.id },
    });
  },
  component: () => null,
});


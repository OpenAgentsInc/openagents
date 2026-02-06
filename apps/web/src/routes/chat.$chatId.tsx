import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { TelemetryService } from '../effect/telemetry';

// Legacy route: we now expose a single-thread Autopilot chat at `/autopilot`.
// Keep `/chat/:id` as a backwards-compatible redirect (no user id in the URL).
export const Route = createFileRoute('/chat/$chatId')({
  loader: async ({ context, params }) => {
    const result = await context.effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;

        const auth = yield* Effect.tryPromise({
          try: () => getAuth(),
          catch: (err) => err,
        }).pipe(Effect.catchAll(() => Effect.succeed(null)));

        const userId = auth?.user?.id ?? null;
        if (!userId) {
          yield* telemetry.withNamespace('route.chat').event('chat.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        yield* telemetry.withNamespace('route.chat').event('chat.legacy_redirect', {
          userId,
          legacyChatId: params.chatId,
        });

        return { kind: 'redirect' as const, to: '/autopilot' as const };
      }),
    );

    throw redirect({ to: result.to });
  },
  component: () => null,
});


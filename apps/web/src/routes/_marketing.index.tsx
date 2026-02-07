import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { useMemo } from 'react';
import { renderToString } from '@openagentsinc/effuse';
import { EffuseMount } from '../components/EffuseMount';
import { homePageTemplate, runHomePage } from '../effuse-pages/home';
import { TelemetryService } from '../effect/telemetry';

export const Route = createFileRoute('/_marketing/')({
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
  const ssrHtml = useMemo(() => renderToString(homePageTemplate()), []);
  return (
    <EffuseMount
      run={runHomePage}
      ssrHtml={ssrHtml}
      className="flex min-h-0 flex-1 flex-col"
    />
  );
}

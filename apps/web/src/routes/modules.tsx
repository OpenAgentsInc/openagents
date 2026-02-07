import { useAtomValue } from '@effect-atom/atom-react';
import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { renderToString } from '@openagentsinc/effuse';
import { useRef } from 'react';
import { EffuseMount } from '../components/EffuseMount';
import {
  authedShellTemplate,
  cleanupAuthedDotsGridBackground,
  hydrateAuthedDotsGridBackground,
  runAuthedShell,
} from '../effuse-pages/authedShell';
import { modulesPageTemplate } from '../effuse-pages/modules';
import { ModulesPageDataAtom } from '../effect/atoms/contracts';
import { TelemetryService } from '../effect/telemetry';

export const Route = createFileRoute('/modules')({
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
          yield* telemetry.withNamespace('route.modules').event('modules.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        yield* telemetry.withNamespace('route.modules').event('modules.open', { userId });
        return { kind: 'ok' as const, userId };
      }),
    );

    if (result.kind === 'redirect') throw redirect({ to: result.to });
    return { userId: result.userId };
  },
  component: ModulesPage,
});

function ModulesPage() {
  const { userId } = Route.useLoaderData();
  const pageData = useAtomValue(ModulesPageDataAtom(userId));

  const ssrHtmlRef = useRef<string | null>(null);
  if (ssrHtmlRef.current === null) {
    ssrHtmlRef.current = renderToString(authedShellTemplate(modulesPageTemplate(pageData)));
  }
  const ssrHtml = ssrHtmlRef.current;

  return (
    <EffuseMount
      run={(el) => runAuthedShell(el, modulesPageTemplate(pageData))}
      deps={[pageData]}
      ssrHtml={ssrHtml}
      hydrate={hydrateAuthedDotsGridBackground}
      onCleanup={cleanupAuthedDotsGridBackground}
      cleanupOn="unmount"
      className="h-full w-full"
    />
  );
}

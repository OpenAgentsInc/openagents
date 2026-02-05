import { HeadContent, Outlet, Scripts, createRootRouteWithContext, useRouter } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { AuthKitProvider } from '@workos/authkit-tanstack-react-start/client';
import { ConvexProviderWithAuth } from 'convex/react';
import { Effect } from 'effect';
import appCssUrl from '../app.css?url';
import { getAppConfig } from '../effect/config';
import { makeAppRuntime } from '../effect/runtime';
import { TelemetryService } from '../effect/telemetry';
import { useAuthFromWorkOS } from '../useAuthFromWorkOS';
import type { QueryClient } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import type { ConvexReactClient } from 'convex/react';
import type { ConvexQueryClient } from '@convex-dev/react-query';
import type { AppRuntime } from '../effect/runtime';

const fetchWorkosAuth = createServerFn({ method: 'GET' }).handler(async () => {
  const runtime = makeAppRuntime(getAppConfig());

  return runtime.runPromise(
    Effect.gen(function* () {
      const telemetry = yield* TelemetryService;

      const auth = yield* Effect.tryPromise({
        try: () => getAuth(),
        catch: (err) => err,
      }).pipe(Effect.catchAll(() => Effect.succeed(null)));

      const userId = auth?.user?.id ?? null;
      const token = auth?.user ? auth.accessToken : null;

      yield* telemetry.withNamespace('auth.workos').event('auth.resolved', {
        authenticated: Boolean(userId),
        userId,
      });

      return {
        userId,
        token,
      };
    }),
  );
});

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient;
  convexClient: ConvexReactClient;
  convexQueryClient: ConvexQueryClient;
  effectRuntime: AppRuntime;
}>()({
  head: () => ({
    meta: [
      {
        charSet: 'utf-8',
      },
      {
        name: 'viewport',
        content: 'width=device-width, initial-scale=1',
      },
      {
        title: 'OpenAgents Autopilot',
      },
    ],
    links: [
      { rel: 'stylesheet', href: appCssUrl },
      { rel: 'icon', href: '/favicon.ico' },
    ],
  }),
  component: RootComponent,
  notFoundComponent: () => <div>Not Found</div>,
  beforeLoad: async (ctx) => {
    const { userId, token } = await fetchWorkosAuth();

    // During SSR only (the only time serverHttpClient exists),
    // set the WorkOS auth token to make HTTP queries with.
    if (token) {
      ctx.context.convexQueryClient.serverHttpClient?.setAuth(token);
    }

    return { userId, token };
  },
});

function RootComponent() {
  const router = useRouter();
  const { convexClient } = router.options.context;

  return (
    <AuthKitProvider>
      <ConvexProviderWithAuth client={convexClient} useAuth={useAuthFromWorkOS}>
        <RootDocument>
          <Outlet />
        </RootDocument>
      </ConvexProviderWithAuth>
    </AuthKitProvider>
  );
}

function RootDocument({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  );
}

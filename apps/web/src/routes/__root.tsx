import { HeadContent, Outlet, Scripts, createRootRouteWithContext, useLoaderData, useRouter } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { AuthKitProvider } from '@workos/authkit-tanstack-react-start/client';
import { ConvexProviderWithAuth } from 'convex/react';
import { Effect } from 'effect';
import { useEffect } from 'react';
import appCssUrl from '../app.css?url';
import { PostHogLoader } from '../components/PostHogLoader';
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

/** Emits page_view to PostHog (via Telemetry) on client when pathname changes. */
function TelemetryPageView() {
  const router = useRouter();
  const pathname = router.state.location.pathname;

  useEffect(() => {
    const { effectRuntime } = router.options.context;
    effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;
        yield* telemetry.withNamespace('app').event('page_view', { path: pathname });
      }),
    ).catch(() => {});
  }, [pathname, router]);

  return null;
}

/** Identifies the user in PostHog (via Telemetry) when root loader has userId. */
function TelemetryIdentify() {
  const rootData = useLoaderData({ from: '__root__' }) as { userId?: string | null } | undefined;
  const userId = rootData?.userId ?? null;
  const router = useRouter();

  useEffect(() => {
    if (!userId) return;
    const { effectRuntime } = router.options.context;
    effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;
        yield* telemetry.withNamespace('auth.workos').identify(userId, { userId });
      }),
    ).catch(() => {});
  }, [userId, router]);

  return null;
}

function RootComponent() {
  const router = useRouter();
  const { convexClient } = router.options.context;

  return (
    <AuthKitProvider>
      <ConvexProviderWithAuth client={convexClient} useAuth={useAuthFromWorkOS}>
        <RootDocument>
          <TelemetryPageView />
          <TelemetryIdentify />
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
        <PostHogLoader />
        <Scripts />
      </body>
    </html>
  );
}

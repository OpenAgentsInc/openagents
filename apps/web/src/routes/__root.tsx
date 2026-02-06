import { HeadContent, Outlet, Scripts, createRootRouteWithContext, useRouter } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { AuthKitProvider } from '@workos/authkit-tanstack-react-start/client';
import { ConvexProviderWithAuth } from 'convex/react';
import { Hydration, Registry } from '@effect-atom/atom';
import { RegistryProvider, scheduleTask as atomScheduleTask, useAtomValue } from '@effect-atom/atom-react';
import { HydrationBoundary } from '@effect-atom/atom-react/ReactHydration';
import { Effect, Exit } from 'effect';
import { useEffect } from 'react';
import appCssUrl from '../app.css?url';
import { PostHogLoader } from '../components/PostHogLoader';
import { getServerRuntime } from '../effect/serverRuntime';
import { SessionAtom } from '../effect/atoms/session';
import { TelemetryService } from '../effect/telemetry';
import { useAuthFromWorkOS } from '../useAuthFromWorkOS';
import type { QueryClient } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import type { ConvexReactClient } from 'convex/react';
import type { ConvexQueryClient } from '@convex-dev/react-query';
import type { AppRuntime } from '../effect/runtime';

const fetchWorkosAuth = createServerFn({ method: 'GET' }).handler(async () => {
  const { runtime } = getServerRuntime();

  const exit = await runtime.runPromiseExit(
    Effect.gen(function* () {
      const telemetry = yield* TelemetryService;

      const auth = yield* Effect.tryPromise({
        try: () => getAuth(),
        catch: (err) => err,
      }).pipe(Effect.catchAll(() => Effect.succeed(null)));

      const user = auth?.user
        ? {
            id: auth.user.id,
            email: auth.user.email,
            firstName: auth.user.firstName ?? null,
            lastName: auth.user.lastName ?? null,
          }
        : null;
      const userId = user?.id ?? null;
      const token = auth?.user ? auth.accessToken : null;

      yield* telemetry.withNamespace('auth.workos').event('auth.resolved', {
        authenticated: Boolean(userId),
        userId,
      });

      const atomRegistry = Registry.make();
      atomRegistry.set(SessionAtom, { userId, user });
      const atomState = Hydration.dehydrate(atomRegistry);
      atomRegistry.dispose();

      return {
        userId,
        token,
        atomState,
      };
    }),
  );

  return Exit.match(exit, {
    onFailure: () => ({ userId: null, token: null, atomState: [] }),
    onSuccess: (value) => value,
  });
});

/** Client-only cache for root auth so we don't refetch on every client-side navigation. */
let clientAuthCache:
  | { userId: string | null; token: string | null; atomState: ReadonlyArray<Hydration.DehydratedAtom> }
  | undefined;

/** Call after sign-out so the next navigation refetches auth instead of reusing cached. */
export function clearRootAuthCache(): void {
  clientAuthCache = undefined;
}

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
    // On the client, reuse cached auth so we don't refetch on every navigation (avoids full-page feel).
    // On the server we always fetch (each request is new). Cache is cleared on full page load.
    let userId: string | null;
    let token: string | null;
    let atomState: ReadonlyArray<Hydration.DehydratedAtom>;

    if (typeof window !== 'undefined' && clientAuthCache) {
      ({ userId, token, atomState } = clientAuthCache);
    } else {
      const result = await fetchWorkosAuth();
      userId = result.userId;
      token = result.token;
      atomState = result.atomState;
      if (typeof window !== 'undefined') {
        clientAuthCache = { userId, token, atomState };
      }
    }

    // During SSR only (the only time serverHttpClient exists),
    // set the WorkOS auth token to make HTTP queries with.
    if (token) {
      ctx.context.convexQueryClient.serverHttpClient?.setAuth(token);
    }

    return { userId, token, atomState };
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
  const session = useAtomValue(SessionAtom);
  const userId = session.userId;
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
  const { atomState } = Route.useRouteContext();

  return (
    <RegistryProvider
      scheduleTask={(f) => {
        // Avoid running effectful atoms during SSR for now. (EffuseMount is client-only anyway.)
        if (typeof window === 'undefined') return;
        atomScheduleTask(f);
      }}
    >
      <HydrationBoundary state={atomState}>
        <AuthKitProvider>
          <ConvexProviderWithAuth client={convexClient} useAuth={useAuthFromWorkOS}>
            <RootDocument>
              <TelemetryPageView />
              <TelemetryIdentify />
              <Outlet />
            </RootDocument>
          </ConvexProviderWithAuth>
        </AuthKitProvider>
      </HydrationBoundary>
    </RegistryProvider>
  );
}

function RootDocument({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="en" dir="ltr">
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

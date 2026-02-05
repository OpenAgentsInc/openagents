import { createRouter } from '@tanstack/react-router';
import { ConvexQueryClient } from '@convex-dev/react-query';
import { QueryClient } from '@tanstack/react-query';
import { setupRouterSsrQueryIntegration } from '@tanstack/react-router-ssr-query';
import { ConvexReactClient } from 'convex/react';
import { routeTree } from './routeTree.gen';
import { getAppConfig } from './effect/config';
import { makeAppRuntime } from './effect/runtime';

export function getRouter() {
  const appConfig = getAppConfig();
  const convex = new ConvexReactClient(appConfig.convexUrl);
  const convexQueryClient = new ConvexQueryClient(convex);
  const effectRuntime = makeAppRuntime(appConfig);

  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        queryKeyHashFn: convexQueryClient.hashFn(),
        queryFn: convexQueryClient.queryFn(),
        gcTime: 5000,
      },
    },
  });
  convexQueryClient.connect(queryClient);

  const router = createRouter({
    routeTree,
    defaultPreload: 'intent',
    scrollRestoration: true,
    defaultPreloadStaleTime: 0, // Let React Query handle all caching
    defaultErrorComponent: (err) => <p>{err.error.stack}</p>,
    defaultNotFoundComponent: () => <p>not found</p>,
    context: { queryClient, convexClient: convex, convexQueryClient, effectRuntime },
  });
  setupRouterSsrQueryIntegration({ router, queryClient });

  return router;
}

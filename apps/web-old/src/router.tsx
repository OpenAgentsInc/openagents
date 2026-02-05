import { createRouter } from '@tanstack/react-router';
import { ConvexQueryClient } from '@convex-dev/react-query';
import { QueryClient } from '@tanstack/react-query';
import { setupRouterSsrQueryIntegration } from '@tanstack/react-router-ssr-query';
import { ConvexReactClient } from 'convex/react';
import { routeTree } from './routeTree.gen';

export function getRouter() {
  const CONVEX_URL = (import.meta as any).env.VITE_CONVEX_URL!;
  if (!CONVEX_URL) {
    throw new Error('missing VITE_CONVEX_URL env var');
  }
  const convex = new ConvexReactClient(CONVEX_URL);
  const convexQueryClient = new ConvexQueryClient(convex);

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
    context: { queryClient, convexClient: convex, convexQueryClient },
    // AuthKitProvider + ConvexProviderWithAuth live in __root.tsx so they run inside RouterProvider
  });
  setupRouterSsrQueryIntegration({ router, queryClient });

  return router;
}


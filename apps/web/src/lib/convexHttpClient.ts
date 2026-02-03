import { ConvexHttpClient } from 'convex/browser';

const KEY = '__OA_CONVEX_HTTP_CLIENT__';

/**
 * One-off Convex HTTP client for queries/mutations outside React (e.g. prefetch, loaders).
 * For reactive data in components, use useQuery(api.nostr.*) with the ConvexReactClient from context.
 */
export function getConvexHttpClient(): ConvexHttpClient {
  const scope = globalThis as typeof globalThis & { [KEY]?: ConvexHttpClient };
  if (!scope[KEY]) {
    const url = (import.meta as any).env?.VITE_CONVEX_URL;
    if (!url) throw new Error('VITE_CONVEX_URL is required for ConvexHttpClient');
    scope[KEY] = new ConvexHttpClient(url);
  }
  return scope[KEY];
}

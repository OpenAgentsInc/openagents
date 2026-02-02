import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { fetchDiscoveredSubclaws } from "@/lib/discoveredSubclaws";
import { posthogCapture } from "@/lib/posthog";

export function useDiscoveredSubclaws(options?: { limit?: number; showAll?: boolean }) {
  const { nostr } = useNostr();
  const limit = options?.limit ?? 200;
  const showAll = options?.showAll ?? false;
  const cacheLimit = 5000;

  return useQuery({
    queryKey: ["clawstr", "discovered-subclaws", limit, showAll],
    queryFn: async ({ signal }) => {
      try {
        const { data, meta } = await fetchDiscoveredSubclaws(nostr, {
          limit,
          showAll,
          cacheLimit,
          signal,
        });
        posthogCapture("nostr_subclaws_fetch", {
          source: "nostr",
          limit,
          cache_limit: meta.cacheLimit,
          cached_count: meta.cachedCount,
          combined_count: meta.combinedCount,
          show_all: showAll,
          result_count: meta.resultCount,
          duration_ms: meta.durationMs,
        });
        return data;
      } catch (err) {
        posthogCapture("nostr_subclaws_fetch_error", {
          limit,
          error: err instanceof Error ? err.message : String(err),
        });
        throw err;
      }
    },
    staleTime: 10 * 1000,
    refetchOnMount: "always",
    refetchOnWindowFocus: true,
    refetchOnReconnect: true,
  });
}

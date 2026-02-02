import { useNostr } from "@nostrify/react";
import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { DiscoveredSubclaw } from "@/lib/discoveredSubclaws";
import { fetchDiscoveredSubclaws, mergeSubclawCounts } from "@/lib/discoveredSubclaws";
import { posthogCapture } from "@/lib/posthog";

export function useDiscoveredSubclaws(options?: { limit?: number; showAll?: boolean }) {
  const { nostr } = useNostr();
  const queryClient = useQueryClient();
  const limit = options?.limit ?? 200;
  const showAll = options?.showAll ?? false;
  const cacheLimit = 5000;
  const queryKey = ["clawstr", "discovered-subclaws", limit, showAll] as const;
  const lastDataRef = useRef<DiscoveredSubclaw[] | null>(null);

  const query = useQuery<DiscoveredSubclaw[], Error>({
    queryKey,
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

  useEffect(() => {
    if (!query.data || query.data.length === 0) return;
    const previous = lastDataRef.current;
    if (!previous || previous.length === 0) {
      lastDataRef.current = query.data;
      return;
    }
    const merged = mergeSubclawCounts(previous, query.data, limit);
    const changed =
      merged.length !== query.data.length ||
      merged.some((item, idx) => {
        const next = query.data[idx];
        return !next || next.slug !== item.slug || next.count !== item.count;
      });
    if (changed) {
      queryClient.setQueryData(queryKey, merged);
      lastDataRef.current = merged;
      return;
    }
    lastDataRef.current = query.data;
  }, [limit, query.data, queryClient, queryKey]);

  return query;
}

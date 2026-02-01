import type { NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { AI_LABEL } from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { getCachedMetrics, storeMetrics } from "@/lib/nostrEventCache";

const METRIC_TTL_MS = 2 * 60 * 1000;

export function useBatchReplyCountsGlobal(
  eventIds: string[],
  showAll = false
) {
  const { nostr } = useNostr();
  const stableIds = eventIds.length > 0 ? [...eventIds].sort() : [];
  const queryKeyHash = stableIds.length > 0 ? stableIds.join(",") : "empty";

  return useQuery<Map<string, number>>({
    queryKey: ["clawstr", "batch-reply-counts-global", queryKeyHash, showAll],
    queryFn: async ({ signal }) => {
      if (eventIds.length === 0) return new Map<string, number>();

      const cacheType = showAll ? "replies-all" : "replies-ai";
      const { data: cached, missing } = await getCachedMetrics<number>(
        eventIds,
        cacheType,
        METRIC_TTL_MS
      );
      if (missing.length === 0) return cached;

      const filter: NostrFilter = {
        kinds: [1111],
        "#e": missing,
        limit: 500,
      };
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
      }

      const events = await queryWithFallback(nostr, [filter], {
        signal,
        timeoutMs: 5000,
      });

      const countMap = new Map<string, number>();
      for (const id of missing) countMap.set(id, 0);
      for (const event of events) {
        const eTag = event.tags.find(([name]) => name === "e");
        const parentId = eTag?.[1];
        if (parentId && countMap.has(parentId)) {
          countMap.set(parentId, countMap.get(parentId)! + 1);
        }
      }
      await storeMetrics(cacheType, countMap);
      const merged = new Map(cached);
      for (const [id, value] of countMap) merged.set(id, value);
      return merged;
    },
    enabled: eventIds.length > 0,
    staleTime: 60 * 1000,
  });
}

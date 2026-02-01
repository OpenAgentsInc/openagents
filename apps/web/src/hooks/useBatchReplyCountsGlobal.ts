import type { NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { AI_LABEL, hasAILabel } from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { fetchConvexEventsByParent } from "@/lib/nostrConvex";

function summarizeReplyCounts(
  eventsByParent: Map<string, { tags: string[][] }[]>,
  eventIds: string[],
  showAll: boolean
): Map<string, number> {
  const countMap = new Map<string, number>();
  for (const id of eventIds) countMap.set(id, 0);
  for (const [parentId, events] of eventsByParent) {
    if (!countMap.has(parentId)) continue;
    let count = 0;
    for (const event of events) {
      if (!showAll && !hasAILabel(event)) continue;
      count += 1;
    }
    countMap.set(parentId, count);
  }
  return countMap;
}

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

      const convexEventsByParent = await fetchConvexEventsByParent(1111, eventIds, 500);
      let convexCount = 0;
      for (const events of convexEventsByParent.values()) {
        convexCount += events.length;
      }
      if (convexCount > 0) {
        return summarizeReplyCounts(convexEventsByParent, eventIds, showAll);
      }

      const filter: NostrFilter = {
        kinds: [1111],
        "#k": ["1111"],
        "#e": eventIds,
        limit: 500,
      };
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
        filter["#L"] = [AI_LABEL.namespace];
      }

      const events = await queryWithFallback(nostr, [filter], {
        signal,
        timeoutMs: 5000,
      });

      const countMap = new Map<string, number>();
      for (const id of eventIds) countMap.set(id, 0);
      for (const event of events) {
        const eTag = event.tags.find(([name]) => name === "e");
        const parentId = eTag?.[1];
        if (parentId && countMap.has(parentId)) {
          countMap.set(parentId, countMap.get(parentId)! + 1);
        }
      }
      return countMap;
    },
    enabled: eventIds.length > 0,
    staleTime: 60 * 1000,
  });
}

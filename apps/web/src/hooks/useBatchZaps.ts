import type { NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { queryWithFallback } from "@/lib/nostrQuery";
import { fetchConvexEventsByParent } from "@/lib/nostrConvex";
import { getCachedMetrics, storeMetrics } from "@/lib/nostrEventCache";

const METRIC_TTL_MS = 2 * 60 * 1000;

export interface ZapSummary {
  count: number;
  totalSats: number;
}

function summarizeZaps(
  events: { tags: string[][] }[],
  eventIds: string[]
): Map<string, ZapSummary> {
  const summaryByPost = new Map<string, { count: number; totalSats: number }>();
  for (const id of eventIds) {
    summaryByPost.set(id, { count: 0, totalSats: 0 });
  }

  for (const ev of events) {
    const eTag = ev.tags.find(([name]) => name === "e");
    const targetId = eTag?.[1];
    if (!targetId || !summaryByPost.has(targetId)) continue;

    const amountTag = ev.tags.find(([name]) => name === "amount");
    const millisats = amountTag?.[1] ? parseInt(amountTag[1], 10) : 0;
    const sats = Number.isNaN(millisats) ? 0 : Math.floor(millisats / 1000);

    const s = summaryByPost.get(targetId)!;
    s.count += 1;
    s.totalSats += sats;
  }

  const result = new Map<string, ZapSummary>();
  for (const [id, { count, totalSats }] of summaryByPost) {
    result.set(id, { count, totalSats });
  }
  return result;
}

/**
 * Batch-fetch NIP-57 zap receipts (kind 9735) for the given post event IDs.
 * #e tag = zapped event. amount tag = millisats (optional).
 * Returns Map<eventId, { count, totalSats }>.
 */
export function useBatchZaps(eventIds: string[]) {
  const { nostr } = useNostr();
  const stableIds = eventIds.length > 0 ? [...eventIds].sort() : [];
  const queryKeyHash = stableIds.length > 0 ? stableIds.join(",") : "empty";

  return useQuery<Map<string, ZapSummary>>({
    queryKey: ["clawstr", "batch-zaps", queryKeyHash],
    queryFn: async ({ signal }) => {
      if (eventIds.length === 0) return new Map<string, ZapSummary>();

      const { data: cached, missing } = await getCachedMetrics<ZapSummary>(
        eventIds,
        "zaps",
        METRIC_TTL_MS
      );
      if (missing.length === 0) return cached;

      const convexEventsByParent = await fetchConvexEventsByParent(9735, missing, 2000);
      const convexEvents: { tags: string[][] }[] = [];
      for (const events of convexEventsByParent.values()) {
        convexEvents.push(...events);
      }
      if (convexEvents.length > 0) {
        const summary = summarizeZaps(convexEvents, missing);
        await storeMetrics("zaps", summary);
        const merged = new Map(cached);
        for (const [id, value] of summary) merged.set(id, value);
        return merged;
      }

      const filter: NostrFilter = {
        kinds: [9735],
        "#e": missing,
        limit: 2000,
      };

      const events = await queryWithFallback(nostr, [filter], {
        signal,
        timeoutMs: 5000,
      });

      const summary = summarizeZaps(events, missing);
      await storeMetrics("zaps", summary);
      const merged = new Map(cached);
      for (const [id, value] of summary) merged.set(id, value);
      return merged;
    },
    enabled: eventIds.length > 0,
    staleTime: 60 * 1000,
  });
}

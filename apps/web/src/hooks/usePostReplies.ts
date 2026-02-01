import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { AI_LABEL, WEB_KIND } from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { fetchConvexReplies } from "@/lib/nostrConvex";

export function usePostReplies(eventId: string | undefined, showAll = false) {
  const { nostr } = useNostr();

  return useQuery<NostrEvent[]>({
    queryKey: ["clawstr", "post-replies", eventId, showAll],
    queryFn: async ({ signal }) => {
      if (!eventId) return [];
      const convexReplies = await fetchConvexReplies(eventId, showAll);
      if (convexReplies.length > 0) {
        return convexReplies.sort((a, b) => a.created_at - b.created_at) as NostrEvent[];
      }
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        "#e": [eventId],
        limit: 100,
      };
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
        filter["#L"] = [AI_LABEL.namespace];
      }
      const events = await queryWithFallback(nostr, [filter], {
        signal,
        timeoutMs: 10000,
      });
      return events.sort((a, b) => a.created_at - b.created_at) as NostrEvent[];
    },
    enabled: !!eventId,
    staleTime: 30 * 1000,
  });
}

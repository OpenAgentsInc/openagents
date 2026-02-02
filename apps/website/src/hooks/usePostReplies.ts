import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { AI_LABEL } from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { posthogCapture } from "@/lib/posthog";

export function usePostReplies(eventId: string | undefined, showAll = false) {
  const { nostr } = useNostr();

  return useQuery<NostrEvent[]>({
    queryKey: ["clawstr", "post-replies", eventId, showAll],
    queryFn: async ({ signal }) => {
      if (!eventId) return [];
      const startedAt = Date.now();
      const filter: NostrFilter = {
        kinds: [1111],
        "#e": [eventId],
        limit: 100,
      };
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
      }
      try {
        const events = await queryWithFallback(nostr, [filter], {
          signal,
          timeoutMs: 10000,
        });
        const sorted = events.sort((a, b) => a.created_at - b.created_at) as NostrEvent[];
        posthogCapture("nostr_post_replies_fetch", {
          event_id: eventId,
          show_all: showAll,
          result_count: sorted.length,
          duration_ms: Date.now() - startedAt,
        });
        return sorted;
      } catch (err) {
        posthogCapture("nostr_post_replies_fetch_error", {
          event_id: eventId,
          show_all: showAll,
          duration_ms: Date.now() - startedAt,
          error: err instanceof Error ? err.message : String(err),
        });
        throw err;
      }
    },
    enabled: !!eventId,
    staleTime: 30 * 1000,
  });
}

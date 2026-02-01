import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { AI_LABEL, WEB_KIND } from "@/lib/clawstr";

export function usePostReplies(eventId: string | undefined, showAll = false) {
  const { nostr } = useNostr();

  return useQuery<NostrEvent[]>({
    queryKey: ["clawstr", "post-replies", eventId, showAll],
    queryFn: async ({ signal }) => {
      if (!eventId) return [];
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
      const events = await nostr.query([filter], {
        signal: AbortSignal.any([signal!, AbortSignal.timeout(10000)]),
      });
      return events.sort((a, b) => a.created_at - b.created_at) as NostrEvent[];
    },
    enabled: !!eventId,
    staleTime: 30 * 1000,
  });
}

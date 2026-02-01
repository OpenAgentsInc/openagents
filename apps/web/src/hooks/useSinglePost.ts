import type { NostrEvent } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { queryWithFallback } from "@/lib/nostrQuery";
import { fetchConvexPost } from "@/lib/nostrConvex";

export function useSinglePost(eventId: string | undefined) {
  const { nostr } = useNostr();

  return useQuery({
    queryKey: ["clawstr", "post", eventId],
    queryFn: async ({ signal }) => {
      if (!eventId) return null;
      const convexPost = await fetchConvexPost(eventId);
      if (convexPost) return convexPost;
      const events = await queryWithFallback(
        nostr,
        [{ kinds: [1111], ids: [eventId], limit: 1 }],
        { signal, timeoutMs: 10000 }
      );
      return (events[0] as NostrEvent) ?? null;
    },
    enabled: !!eventId,
    staleTime: 60 * 1000,
  });
}

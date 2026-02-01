import type { NostrEvent } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { queryWithFallback } from "@/lib/nostrQuery";

export function useSingleEvent(eventId: string | undefined) {
  const { nostr } = useNostr();

  return useQuery({
    queryKey: ["clawstr", "event", eventId],
    queryFn: async ({ signal }) => {
      if (!eventId) return null;
      const events = await queryWithFallback(
        nostr,
        [{ ids: [eventId], limit: 1 }],
        { signal, timeoutMs: 10000 }
      );
      return (events[0] as NostrEvent) ?? null;
    },
    enabled: !!eventId,
    staleTime: 60 * 1000,
  });
}

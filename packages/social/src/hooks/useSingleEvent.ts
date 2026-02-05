import type { NostrEvent } from '@nostrify/nostrify';
import { useNostr } from '@nostrify/react';
import { useQuery } from '@tanstack/react-query';
import { queryWithFallback } from '@/lib/nostrQuery';
import { posthogCapture } from '@/lib/posthog';

export function useSingleEvent(eventId: string | undefined) {
  const { nostr } = useNostr();

  return useQuery({
    queryKey: ['clawstr', 'event', eventId],
    queryFn: async ({ signal }) => {
      if (!eventId) return null;
      const startedAt = Date.now();
      try {
        const events = await queryWithFallback(
          nostr,
          [{ ids: [eventId], limit: 1 }],
          { signal, timeoutMs: 10000 },
        );
        const event = (events[0] as NostrEvent) ?? null;
        posthogCapture('nostr_event_fetch', {
          event_id: eventId,
          found: !!event,
          duration_ms: Date.now() - startedAt,
        });
        return event;
      } catch (err) {
        posthogCapture('nostr_event_fetch_error', {
          event_id: eventId,
          duration_ms: Date.now() - startedAt,
          error: err instanceof Error ? err.message : String(err),
        });
        throw err;
      }
    },
    enabled: !!eventId,
    staleTime: 60 * 1000,
  });
}

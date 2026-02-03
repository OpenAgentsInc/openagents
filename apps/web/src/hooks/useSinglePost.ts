import type { NostrEvent } from '@nostrify/nostrify';
import { useNostr } from '@nostrify/react';
import { useQuery } from '@tanstack/react-query';
import { queryWithFallback } from '@/lib/nostrQuery';
import { fetchConvexPost } from '@/lib/nostrConvex';
import { posthogCapture } from '@/lib/posthog';

export function useSinglePost(eventId: string | undefined) {
  const { nostr } = useNostr();

  return useQuery({
    queryKey: ['clawstr', 'post', eventId],
    queryFn: async ({ signal }) => {
      if (!eventId) return null;
      const startedAt = Date.now();
      const convexPost = await fetchConvexPost(eventId);
      if (convexPost) {
        posthogCapture('nostr_post_fetch', {
          event_id: eventId,
          source: 'convex',
          found: true,
          duration_ms: Date.now() - startedAt,
        });
        return convexPost;
      }
      try {
        const events = await queryWithFallback(
          nostr,
          [{ kinds: [1111], ids: [eventId], limit: 1 }],
          { signal, timeoutMs: 10000 },
        );
        const post = (events[0] as NostrEvent) ?? null;
        posthogCapture('nostr_post_fetch', {
          event_id: eventId,
          source: 'nostr',
          found: !!post,
          duration_ms: Date.now() - startedAt,
        });
        return post;
      } catch (err) {
        posthogCapture('nostr_post_fetch_error', {
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

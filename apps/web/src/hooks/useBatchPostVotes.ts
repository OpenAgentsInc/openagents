import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import { useNostr } from '@nostrify/react';
import { useQuery } from '@tanstack/react-query';
import { queryWithFallback } from '@/lib/nostrQuery';
import { fetchConvexEventsByParent } from '@/lib/nostrConvex';
import { getCachedMetrics, storeMetrics } from '@/lib/nostrEventCache';

const METRIC_TTL_MS = 2 * 60 * 1000;

function reactionToDelta(content: string): 1 | -1 | 0 {
  const t = content.trim();
  if (t === '+' || t === '👍' || t === '❤️' || t === '🤙' || t === '😀')
    return 1;
  if (t === '-' || t === '👎') return -1;
  return 0;
}

function summarizeVotes(
  events: NostrEvent[],
  eventIds: string[],
): Map<string, PostVoteSummary> {
  const ids = new Set(eventIds);
  const byTargetAndAuthor = new Map<string, NostrEvent>();
  for (const ev of events) {
    const eTag = ev.tags.find(([name]) => name === 'e');
    const targetId = eTag?.[1];
    if (!targetId || !ids.has(targetId)) continue;
    const key = `${targetId}:${ev.pubkey}`;
    const existing = byTargetAndAuthor.get(key);
    if (!existing || ev.created_at > existing.created_at) {
      byTargetAndAuthor.set(key, ev);
    }
  }

  const summaryByPost = new Map<string, { up: number; down: number }>();
  for (const id of eventIds) {
    summaryByPost.set(id, { up: 0, down: 0 });
  }
  for (const ev of byTargetAndAuthor.values()) {
    const eTag = ev.tags.find(([name]) => name === 'e');
    const targetId = eTag?.[1];
    if (!targetId || !summaryByPost.has(targetId)) continue;
    const delta = reactionToDelta(ev.content);
    const s = summaryByPost.get(targetId)!;
    if (delta === 1) s.up += 1;
    else if (delta === -1) s.down += 1;
  }

  const result = new Map<string, PostVoteSummary>();
  for (const [id, { up, down }] of summaryByPost) {
    result.set(id, { score: up - down, up, down });
  }
  return result;
}

export interface PostVoteSummary {
  score: number;
  up: number;
  down: number;
}

export function useBatchPostVotes(eventIds: string[]) {
  const { nostr } = useNostr();
  const stableIds = eventIds.length > 0 ? [...eventIds].sort() : [];
  const queryKeyHash = stableIds.length > 0 ? stableIds.join(',') : 'empty';

  return useQuery<Map<string, PostVoteSummary>>({
    queryKey: ['clawstr', 'batch-post-votes', queryKeyHash],
    queryFn: async ({ signal }) => {
      if (eventIds.length === 0) return new Map<string, PostVoteSummary>();

      const { data: cached, missing } = await getCachedMetrics<PostVoteSummary>(
        eventIds,
        'votes',
        METRIC_TTL_MS,
      );
      if (missing.length === 0) return cached;

      const convexEventsByParent = await fetchConvexEventsByParent(
        7,
        missing,
        2000,
      );
      const convexEvents: NostrEvent[] = [];
      for (const events of convexEventsByParent.values()) {
        convexEvents.push(...events);
      }
      if (convexEvents.length > 0) {
        const summary = summarizeVotes(convexEvents, missing);
        await storeMetrics('votes', summary);
        const merged = new Map(cached);
        for (const [id, value] of summary) merged.set(id, value);
        return merged;
      }

      const filter: NostrFilter = {
        kinds: [7],
        '#e': missing,
        limit: 5000,
      };

      const events = await queryWithFallback(nostr, [filter], {
        signal,
        timeoutMs: 5000,
      });

      const summary = summarizeVotes(events as NostrEvent[], missing);
      await storeMetrics('votes', summary);
      const merged = new Map(cached);
      for (const [id, value] of summary) merged.set(id, value);
      return merged;
    },
    enabled: eventIds.length > 0,
    staleTime: 60 * 1000,
  });
}

import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import { useNostr } from '@nostrify/react';
import { useQuery } from '@tanstack/react-query';
import { AI_LABEL } from '@/lib/clawstr';
import { queryWithFallback } from '@/lib/nostrQuery';
import { posthogCapture } from '@/lib/posthog';

export interface ThreadNode {
  event: NostrEvent;
  children: ThreadNode[];
}

function buildThread(rootId: string, events: NostrEvent[]): ThreadNode[] {
  const byParent = new Map<string, NostrEvent[]>();
  for (const ev of events) {
    const eTag = ev.tags.find(([name]) => name === 'e');
    const parentId = eTag?.[1] ?? rootId;
    if (!byParent.has(parentId)) byParent.set(parentId, []);
    byParent.get(parentId)!.push(ev);
  }
  for (const arr of byParent.values()) {
    arr.sort((a, b) => a.created_at - b.created_at);
  }

  function buildNodes(parentId: string): ThreadNode[] {
    const replies = byParent.get(parentId) ?? [];
    return replies.map((event) => ({
      event,
      children: buildNodes(event.id),
    }));
  }

  return buildNodes(rootId);
}

const MAX_THREAD_DEPTH = 20;
const LIMIT_PER_QUERY = 200;

export function usePostRepliesThread(
  rootId: string | undefined,
  showAll = false,
) {
  const { nostr } = useNostr();

  return useQuery<ThreadNode[]>({
    queryKey: ['clawstr', 'post-replies-thread', rootId, showAll],
    queryFn: async ({ signal }) => {
      if (!rootId) return [];
      const startedAt = Date.now();

      const baseFilter: Omit<NostrFilter, '#e'> = {
        kinds: [1111],
        limit: LIMIT_PER_QUERY,
      };
      if (!showAll) {
        baseFilter['#l'] = [AI_LABEL.value];
      }

      const fetched = new Map<string, NostrEvent>();
      const queriedParents = new Set<string>();
      let toQuery: string[] = [rootId];
      let depth = 0;

      try {
        while (toQuery.length > 0 && depth < MAX_THREAD_DEPTH) {
          const filter: NostrFilter = { ...baseFilter, '#e': toQuery };
          const events = await queryWithFallback(nostr, [filter], {
            signal,
            timeoutMs: 10000,
          });

          for (const id of toQuery) queriedParents.add(id);
          for (const ev of events) {
            fetched.set(ev.id, ev);
          }
          toQuery = [
            ...new Set(
              events.map((e) => e.id).filter((id) => !queriedParents.has(id)),
            ),
          ];
          depth++;
        }

        const nodes = buildThread(rootId, [...fetched.values()]);
        posthogCapture('nostr_post_thread_fetch', {
          event_id: rootId,
          show_all: showAll,
          result_count: fetched.size,
          depth,
          duration_ms: Date.now() - startedAt,
        });
        return nodes;
      } catch (err) {
        posthogCapture('nostr_post_thread_fetch_error', {
          event_id: rootId,
          show_all: showAll,
          duration_ms: Date.now() - startedAt,
          error: err instanceof Error ? err.message : String(err),
        });
        throw err;
      }
    },
    enabled: !!rootId,
    staleTime: 30 * 1000,
  });
}

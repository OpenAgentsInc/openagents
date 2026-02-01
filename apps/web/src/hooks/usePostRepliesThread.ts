import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { AI_LABEL, WEB_KIND } from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { fetchConvexThread } from "@/lib/nostrConvex";

/** One node in the reply thread: event plus its nested children (sorted by created_at). */
export interface ThreadNode {
  event: NostrEvent;
  children: ThreadNode[];
}

function buildThread(rootId: string, events: NostrEvent[]): ThreadNode[] {
  const byParent = new Map<string, NostrEvent[]>();
  for (const ev of events) {
    const eTag = ev.tags.find(([name]) => name === "e");
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

/**
 * Fetch all replies in the thread (direct + nested) and build a tree.
 * NIP-22: each reply has #e = parent event id. Root is the post; top-level nodes
 * are direct replies, each with children recursively.
 */
export function usePostRepliesThread(rootId: string | undefined, showAll = false) {
  const { nostr } = useNostr();

  return useQuery<ThreadNode[]>({
    queryKey: ["clawstr", "post-replies-thread", rootId, showAll],
    queryFn: async ({ signal }) => {
      if (!rootId) return [];

      const convexReplies = await fetchConvexThread(rootId, showAll);
      if (convexReplies.length > 0) {
        return buildThread(rootId, convexReplies);
      }

      const baseFilter: Omit<NostrFilter, "#e"> = {
        kinds: [1111],
        "#K": [WEB_KIND],
        limit: LIMIT_PER_QUERY,
      };
      if (!showAll) {
        baseFilter["#l"] = [AI_LABEL.value];
        baseFilter["#L"] = [AI_LABEL.namespace];
      }

      const fetched = new Map<string, NostrEvent>();
      const queriedParents = new Set<string>();
      let toQuery: string[] = [rootId];
      let depth = 0;

      while (toQuery.length > 0 && depth < MAX_THREAD_DEPTH) {
        const filter: NostrFilter = { ...baseFilter, "#e": toQuery };
        const events = await queryWithFallback(nostr, [filter], {
          signal,
          timeoutMs: 10000,
        });

        for (const id of toQuery) queriedParents.add(id);
        for (const ev of events) {
          fetched.set(ev.id, ev);
        }
        // Next: fetch replies to the events we just got (only ids we haven't queried as parents yet)
        toQuery = [...new Set(events.map((e) => e.id).filter((id) => !queriedParents.has(id)))];
        depth++;
      }

      return buildThread(rootId, [...fetched.values()]);
    },
    enabled: !!rootId,
    staleTime: 30 * 1000,
  });
}

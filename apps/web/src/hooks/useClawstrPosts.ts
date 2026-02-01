import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import {
  AI_LABEL,
  WEB_KIND,
  isTopLevelPost,
  isClawstrIdentifier,
} from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { fetchConvexFeed } from "@/lib/nostrConvex";

interface UseClawstrPostsOptions {
  showAll?: boolean;
  limit?: number;
  /** Optional: only events since this Unix timestamp (for "Hot" / time range). */
  since?: number;
}

export function useClawstrPosts(options: UseClawstrPostsOptions = {}) {
  const { nostr } = useNostr();
  const { showAll = false, limit = 50, since } = options;

  return useQuery({
    queryKey: ["clawstr", "posts", showAll, limit, since],
    queryFn: async ({ signal }) => {
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        limit,
      };
      if (since != null && since > 0) filter.since = since;
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
        filter["#L"] = [AI_LABEL.namespace];
      }

      const [convexPosts, nostrEvents] = await Promise.all([
        fetchConvexFeed({ limit, since, showAll }),
        queryWithFallback(nostr, [filter], {
          signal,
          timeoutMs: 10000,
          forceFallbackOnEmpty: true,
        }),
      ]);

      const byId = new Map<string, NostrEvent>();
      for (const e of convexPosts) byId.set(e.id, e);
      for (const e of nostrEvents) byId.set(e.id, e);

      const topLevel = [...byId.values()].filter((event) => {
        if (!isTopLevelPost(event)) return false;
        const identifier = event.tags.find(([name]) => name === "I")?.[1];
        return identifier && isClawstrIdentifier(identifier);
      });

      return topLevel
        .sort((a, b) => b.created_at - a.created_at)
        .slice(0, limit) as NostrEvent[];
    },
    staleTime: 30 * 1000,
  });
}

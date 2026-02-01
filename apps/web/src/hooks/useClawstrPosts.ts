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
      const convexPosts = await fetchConvexFeed({
        limit,
        since,
        showAll,
      });
      if (convexPosts.length > 0) {
        return convexPosts.sort((a, b) => b.created_at - a.created_at) as NostrEvent[];
      }

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

      const events = await queryWithFallback(nostr, [filter], {
        signal,
        timeoutMs: 10000,
      });

      const topLevel = events.filter((event) => {
        if (!isTopLevelPost(event)) return false;
        const identifier = event.tags.find(([name]) => name === "I")?.[1];
        return identifier && isClawstrIdentifier(identifier);
      });

      return topLevel.sort((a, b) => b.created_at - a.created_at) as NostrEvent[];
    },
    staleTime: 30 * 1000,
  });
}

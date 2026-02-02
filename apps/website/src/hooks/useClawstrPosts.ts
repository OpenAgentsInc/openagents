import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import {
  AI_LABEL,
  WEB_KIND,
  isTopLevelPost,
  isClawstrIdentifier,
  getPostIdentifier,
} from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { posthogCapture } from "@/lib/posthog";

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
      const startedAt = Date.now();
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        limit,
      };
      if (since != null && since > 0) filter.since = since;
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
      }

      try {
        const events = await queryWithFallback(nostr, [filter], {
          signal,
          timeoutMs: 10000,
        });

        const topLevel = events.filter((event) => {
          if (!isTopLevelPost(event)) return false;
          const identifier = getPostIdentifier(event);
          return identifier && isClawstrIdentifier(identifier);
        });

        posthogCapture("nostr_feed_fetch", {
          scope: "global",
          show_all: showAll,
          limit,
          since: since ?? null,
          result_count: topLevel.length,
          duration_ms: Date.now() - startedAt,
        });

        return topLevel
          .sort((a, b) => b.created_at - a.created_at)
          .slice(0, limit) as NostrEvent[];
      } catch (err) {
        posthogCapture("nostr_feed_fetch_error", {
          scope: "global",
          show_all: showAll,
          limit,
          since: since ?? null,
          duration_ms: Date.now() - startedAt,
          error: err instanceof Error ? err.message : String(err),
        });
        throw err;
      }
    },
    staleTime: 5 * 1000,
    refetchOnMount: "always",
    refetchOnWindowFocus: true,
    refetchOnReconnect: true,
  });
}

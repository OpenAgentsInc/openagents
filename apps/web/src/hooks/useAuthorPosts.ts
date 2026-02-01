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

interface UseAuthorPostsOptions {
  showAll?: boolean;
  limit?: number;
}

/**
 * Fetch kind 1111 posts by a single author (pubkey), Clawstr-style:
 * #K web, top-level only, optional #l for AI-only.
 */
export function useAuthorPosts(
  pubkey: string | undefined,
  options: UseAuthorPostsOptions = {}
) {
  const { nostr } = useNostr();
  const { showAll = false, limit = 50 } = options;

  return useQuery({
    queryKey: ["clawstr", "author-posts", pubkey, showAll, limit],
    queryFn: async ({ signal }) => {
      if (!pubkey) return [];

      const filter: NostrFilter = {
        kinds: [1111],
        authors: [pubkey],
        "#K": [WEB_KIND],
        limit,
      };
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
      }

      const events = await queryWithFallback(nostr, [filter], {
        signal,
        timeoutMs: 10000,
      });

      const topLevel = events.filter((event) => {
        if (!isTopLevelPost(event)) return false;
        const identifier = getPostIdentifier(event);
        return identifier && isClawstrIdentifier(identifier);
      });

      return topLevel
        .sort((a, b) => b.created_at - a.created_at)
        .slice(0, limit) as NostrEvent[];
    },
    enabled: !!pubkey,
    staleTime: 30 * 1000,
  });
}

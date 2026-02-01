import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import {
  AI_LABEL,
  WEB_KIND,
  subclawToIdentifiers,
  identifierToSubclaw,
  isTopLevelPost,
  isClawstrIdentifier,
  getPostIdentifier,
} from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";

interface UseSubclawPostsOptions {
  showAll?: boolean;
  limit?: number;
  since?: number;
}

export function useSubclawPosts(
  subclaw: string,
  options: UseSubclawPostsOptions = {}
) {
  const { nostr } = useNostr();
  const { showAll = false, limit = 50, since } = options;
  const normalizedSubclaw = subclaw.trim().toLowerCase();
  const identifiers = subclawToIdentifiers(normalizedSubclaw);

  return useQuery({
    queryKey: ["clawstr", "subclaw-posts", normalizedSubclaw, showAll, limit, since],
    queryFn: async ({ signal }) => {
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        "#I": identifiers,
        limit,
      };
      if (since != null && since > 0) filter.since = since;
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
      }

      const events = await queryWithFallback(nostr, [filter], {
        signal,
        timeoutMs: 10000,
      });

      const topLevel = events.filter((event) => {
        if (!isTopLevelPost(event)) return false;
        const id = getPostIdentifier(event);
        if (!id || !isClawstrIdentifier(id)) return false;
        const slug = identifierToSubclaw(id);
        return slug === normalizedSubclaw;
      });

      return topLevel
        .sort((a, b) => b.created_at - a.created_at)
        .slice(0, limit) as NostrEvent[];
    },
    enabled: !!subclaw.trim(),
    staleTime: 30 * 1000,
  });
}

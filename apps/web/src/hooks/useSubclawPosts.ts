import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import {
  AI_LABEL,
  WEB_KIND,
  subclawToIdentifier,
  isTopLevelPost,
  isClawstrIdentifier,
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
  const identifier = subclawToIdentifier(subclaw);

  return useQuery({
    queryKey: ["clawstr", "subclaw-posts", subclaw, showAll, limit, since],
    queryFn: async ({ signal }) => {
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        "#I": [identifier],
        "#i": [identifier],
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
        const id = event.tags.find(([name]) => name === "I")?.[1];
        return id && isClawstrIdentifier(id);
      });

      return topLevel.sort((a, b) => b.created_at - a.created_at) as NostrEvent[];
    },
    enabled: !!subclaw.trim(),
    staleTime: 30 * 1000,
  });
}

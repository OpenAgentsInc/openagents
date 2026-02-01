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

interface UseSubclawPostsOptions {
  showAll?: boolean;
  limit?: number;
}

export function useSubclawPosts(
  subclaw: string,
  options: UseSubclawPostsOptions = {}
) {
  const { nostr } = useNostr();
  const { showAll = false, limit = 50 } = options;
  const identifier = subclawToIdentifier(subclaw);

  return useQuery({
    queryKey: ["clawstr", "subclaw-posts", subclaw, showAll, limit],
    queryFn: async ({ signal }) => {
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        "#I": [identifier],
        "#i": [identifier],
        limit,
      };
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
        filter["#L"] = [AI_LABEL.namespace];
      }

      const events = await nostr.query([filter], {
        signal: AbortSignal.any([signal!, AbortSignal.timeout(10000)]),
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

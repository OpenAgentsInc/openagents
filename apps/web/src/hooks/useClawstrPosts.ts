import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import {
  AI_LABEL,
  WEB_KIND,
  isTopLevelPost,
  isClawstrIdentifier,
} from "@/lib/clawstr";

interface UseClawstrPostsOptions {
  showAll?: boolean;
  limit?: number;
}

export function useClawstrPosts(options: UseClawstrPostsOptions = {}) {
  const { nostr } = useNostr();
  const { showAll = false, limit = 50 } = options;

  return useQuery({
    queryKey: ["clawstr", "posts", showAll, limit],
    queryFn: async ({ signal }) => {
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        limit,
      };
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
        filter["#L"] = [AI_LABEL.namespace];
      }

      const events = await nostr.query([filter], {
        signal: AbortSignal.any([signal, AbortSignal.timeout(10000)]),
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

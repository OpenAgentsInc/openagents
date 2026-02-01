import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import {
  AI_LABEL,
  CLAWSTR_BASE_URL,
  OPENAGENTS_BASE_URL,
  WEB_KIND,
  subclawToIdentifier,
  identifierToSubclaw,
  isTopLevelPost,
  isClawstrIdentifier,
} from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { fetchConvexFeed } from "@/lib/nostrConvex";

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
  const identifiers = [
    subclawToIdentifier(normalizedSubclaw, OPENAGENTS_BASE_URL),
    subclawToIdentifier(normalizedSubclaw, CLAWSTR_BASE_URL),
  ];

  return useQuery({
    queryKey: ["clawstr", "subclaw-posts", normalizedSubclaw, showAll, limit, since],
    queryFn: async ({ signal }) => {
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        "#I": identifiers,
        "#i": identifiers,
        limit,
      };
      if (since != null && since > 0) filter.since = since;
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
        filter["#L"] = [AI_LABEL.namespace];
      }

      const [convexPosts, nostrEvents] = await Promise.all([
        fetchConvexFeed({ limit, since, showAll, subclaw: normalizedSubclaw }),
        queryWithFallback(nostr, [filter], { signal, timeoutMs: 10000 }),
      ]);

      const byId = new Map<string, NostrEvent>();
      for (const e of convexPosts) byId.set(e.id, e);
      for (const e of nostrEvents) byId.set(e.id, e);

      const topLevel = [...byId.values()].filter((event) => {
        if (!isTopLevelPost(event)) return false;
        const id = event.tags.find(([name]) => name === "I")?.[1];
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

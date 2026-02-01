import type { NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import {
  WEB_KIND,
  isTopLevelPost,
  isClawstrIdentifier,
  identifierToSubclaw,
} from "@/lib/clawstr";

export interface DiscoveredSubclaw {
  slug: string;
  count: number;
}

export function useDiscoveredSubclaws(options?: { limit?: number }) {
  const { nostr } = useNostr();
  const limit = options?.limit ?? 200;

  return useQuery({
    queryKey: ["clawstr", "discovered-subclaws", limit],
    queryFn: async ({ signal }) => {
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        limit,
      };
      const events = await nostr.query([filter], {
        signal: AbortSignal.any([signal!, AbortSignal.timeout(10000)]),
      });

      const topLevel = events.filter((event) => {
        if (!isTopLevelPost(event)) return false;
        const identifier = event.tags.find(([name]) => name === "I")?.[1];
        return identifier && isClawstrIdentifier(identifier);
      });

      const countBySlug = new Map<string, number>();
      for (const event of topLevel) {
        const identifier = event.tags.find(([name]) => name === "I")?.[1];
        if (!identifier) continue;
        const slug = identifierToSubclaw(identifier);
        if (slug) {
          countBySlug.set(slug, (countBySlug.get(slug) ?? 0) + 1);
        }
      }

      const result: DiscoveredSubclaw[] = [...countBySlug.entries()]
        .map(([slug, count]) => ({ slug, count }))
        .sort((a, b) => b.count - a.count);
      return result;
    },
    staleTime: 60 * 1000,
  });
}

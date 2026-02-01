import type { NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import {
  WEB_KIND,
  isTopLevelPost,
  isClawstrIdentifier,
  identifierToSubclaw,
} from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { fetchConvexSubclaws } from "@/lib/nostrConvex";
import { posthogCapture } from "@/lib/posthog";

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
      const startedAt = Date.now();
      try {
        const convexSubclaws = await fetchConvexSubclaws(limit);
        if (convexSubclaws.length > 0) {
          posthogCapture("nostr_subclaws_fetch", {
            source: "convex",
            limit,
            result_count: convexSubclaws.length,
            duration_ms: Date.now() - startedAt,
          });
          return convexSubclaws;
        }

        const filter: NostrFilter = {
          kinds: [1111],
          "#K": [WEB_KIND],
          limit,
        };
        const events = await queryWithFallback(nostr, [filter], {
          signal,
          timeoutMs: 10000,
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
        posthogCapture("nostr_subclaws_fetch", {
          source: "nostr",
          limit,
          result_count: result.length,
          duration_ms: Date.now() - startedAt,
        });
        return result;
      } catch (err) {
        posthogCapture("nostr_subclaws_fetch_error", {
          limit,
          duration_ms: Date.now() - startedAt,
          error: err instanceof Error ? err.message : String(err),
        });
        throw err;
      }
    },
    staleTime: 60 * 1000,
  });
}

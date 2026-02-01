import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import {
  AI_LABEL,
  WEB_KIND,
  getPostIdentifier,
  hasAILabel,
  isTopLevelPost,
  isClawstrIdentifier,
  identifierToSubclaw,
} from "@/lib/clawstr";
import { queryWithFallback } from "@/lib/nostrQuery";
import { queryCachedEvents } from "@/lib/nostrEventCache";
import { posthogCapture } from "@/lib/posthog";

export interface DiscoveredSubclaw {
  slug: string;
  count: number;
}

function buildSubclawCounts(events: NostrEvent[], showAll: boolean): DiscoveredSubclaw[] {
  const countBySlug = new Map<string, number>();
  for (const event of events) {
    if (!isTopLevelPost(event)) continue;
    if (!showAll && !hasAILabel(event)) continue;
    const identifier = getPostIdentifier(event);
    if (!identifier || !isClawstrIdentifier(identifier)) continue;
    const slug = identifierToSubclaw(identifier);
    if (slug) countBySlug.set(slug, (countBySlug.get(slug) ?? 0) + 1);
  }
  return [...countBySlug.entries()]
    .map(([slug, count]) => ({ slug, count }))
    .sort((a, b) => b.count - a.count);
}

function dedupeEvents(events: NostrEvent[]): NostrEvent[] {
  const seen = new Set<string>();
  const result: NostrEvent[] = [];
  for (const event of events) {
    if (seen.has(event.id)) continue;
    seen.add(event.id);
    result.push(event);
  }
  return result;
}

export function useDiscoveredSubclaws(options?: { limit?: number; showAll?: boolean }) {
  const { nostr } = useNostr();
  const limit = options?.limit ?? 200;
  const showAll = options?.showAll ?? false;
  const cacheLimit = 5000;

  return useQuery({
    queryKey: ["clawstr", "discovered-subclaws", limit, showAll],
    queryFn: async ({ signal }) => {
      const startedAt = Date.now();
      try {
        const filter: NostrFilter = {
          kinds: [1111],
          "#K": [WEB_KIND],
          limit,
        };
        if (!showAll) {
          filter["#l"] = [AI_LABEL.value];
        }
        const cachedFilter: NostrFilter = {
          kinds: [1111],
          "#K": [WEB_KIND],
          limit: cacheLimit,
        };
        if (!showAll) {
          cachedFilter["#l"] = [AI_LABEL.value];
        }
        const cachedEvents = await queryCachedEvents([cachedFilter]);
        const events = await queryWithFallback(nostr, [filter], {
          signal,
          timeoutMs: 10000,
        });
        const combined = dedupeEvents([...cachedEvents, ...events]);
        const result = buildSubclawCounts(combined, showAll).slice(0, limit);
        posthogCapture("nostr_subclaws_fetch", {
          source: "nostr",
          limit,
          cache_limit: cacheLimit,
          cached_count: cachedEvents.length,
          combined_count: combined.length,
          show_all: showAll,
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

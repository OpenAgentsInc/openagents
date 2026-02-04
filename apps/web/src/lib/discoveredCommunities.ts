import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import {
  AI_LABEL,
  WEB_KIND,
  getPostIdentifier,
  hasAILabel,
  identifierToCommunity,
  isClawstrIdentifier,
  isTopLevelPost,
} from '@/lib/clawstr';
import { isCommunityBlacklisted } from '@/lib/communityBlacklist';
import { queryCachedEvents } from '@/lib/nostrEventCache';
import { queryWithFallback } from '@/lib/nostrQuery';

export interface DiscoveredCommunity {
  slug: string;
  count: number;
}

export type DiscoveredCommunityMeta = {
  cacheLimit: number;
  cachedCount: number;
  combinedCount: number;
  resultCount: number;
  durationMs: number;
};

type NostrQueryClient = {
  query: (
    filters: Array<NostrFilter>,
    opts?: { signal?: AbortSignal; relays?: Array<string> },
  ) => Promise<Array<NostrEvent>>;
};

export function buildCommunityCounts(
  events: Array<NostrEvent>,
  showAll: boolean,
): Array<DiscoveredCommunity> {
  const countBySlug = new Map<string, number>();
  for (const event of events) {
    if (!isTopLevelPost(event)) continue;
    if (!showAll && !hasAILabel(event)) continue;
    const identifier = getPostIdentifier(event);
    if (!identifier || !isClawstrIdentifier(identifier)) continue;
    const slug = identifierToCommunity(identifier);
    if (slug && !isCommunityBlacklisted(slug))
      countBySlug.set(slug, (countBySlug.get(slug) ?? 0) + 1);
  }
  return [...countBySlug.entries()]
    .map(([slug, count]) => ({ slug, count }))
    .sort((a, b) => b.count - a.count);
}

export function dedupeEvents(events: Array<NostrEvent>): Array<NostrEvent> {
  const seen = new Set<string>();
  const result: Array<NostrEvent> = [];
  for (const event of events) {
    if (seen.has(event.id)) continue;
    seen.add(event.id);
    result.push(event);
  }
  return result;
}

export function mergeCommunityCounts(
  previous: Array<DiscoveredCommunity>,
  next: Array<DiscoveredCommunity>,
  limit: number,
): Array<DiscoveredCommunity> {
  const countBySlug = new Map<string, number>();
  for (const entry of previous) {
    countBySlug.set(entry.slug, entry.count);
  }
  for (const entry of next) {
    const current = countBySlug.get(entry.slug);
    if (current == null) {
      countBySlug.set(entry.slug, entry.count);
    } else if (entry.count > current) {
      countBySlug.set(entry.slug, entry.count);
    }
  }
  return [...countBySlug.entries()]
    .map(([slug, count]) => ({ slug, count }))
    .sort((a, b) => (b.count - a.count) || a.slug.localeCompare(b.slug))
    .slice(0, limit);
}

export async function fetchDiscoveredCommunities(
  nostr: NostrQueryClient,
  options?: {
    limit?: number;
    showAll?: boolean;
    cacheLimit?: number;
    signal?: AbortSignal;
    timeoutMs?: number;
  },
): Promise<{ data: Array<DiscoveredCommunity>; meta: DiscoveredCommunityMeta }> {
  const startedAt = Date.now();
  const limit = options?.limit ?? 200;
  const showAll = options?.showAll ?? false;
  const cacheLimit = options?.cacheLimit ?? 5000;
  const timeoutMs = options?.timeoutMs ?? 10000;

  const filter: NostrFilter = {
    kinds: [1111],
    '#K': [WEB_KIND],
    limit,
  };
  if (!showAll) {
    filter['#l'] = [AI_LABEL.value];
  }

  const cachedFilter: NostrFilter = {
    kinds: [1111],
    '#K': [WEB_KIND],
    limit: cacheLimit,
  };
  if (!showAll) {
    cachedFilter['#l'] = [AI_LABEL.value];
  }

  let cachedEvents: Array<NostrEvent> = [];
  try {
    cachedEvents = await queryCachedEvents([cachedFilter]);
  } catch {
    cachedEvents = [];
  }

  const events = await queryWithFallback(nostr, [filter], {
    signal: options?.signal,
    timeoutMs,
  });

  const combined = dedupeEvents([...cachedEvents, ...events]);
  const data = buildCommunityCounts(combined, showAll).slice(0, limit);

  return {
    data,
    meta: {
      cacheLimit,
      cachedCount: cachedEvents.length,
      combinedCount: combined.length,
      resultCount: data.length,
      durationMs: Date.now() - startedAt,
    },
  };
}

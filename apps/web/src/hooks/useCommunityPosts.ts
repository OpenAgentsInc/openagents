import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import { useNostr } from '@nostrify/react';
import { useQuery } from '@tanstack/react-query';
import {
  AI_LABEL,
  WEB_KIND,
  communityToIdentifiers,
  identifierToCommunity,
  isTopLevelPost,
  isClawstrIdentifier,
  getPostIdentifier,
} from '@/lib/clawstr';
import { queryWithFallback } from '@/lib/nostrQuery';
import { posthogCapture } from '@/lib/posthog';

interface UseCommunityPostsOptions {
  showAll?: boolean;
  limit?: number;
  since?: number;
}

export function useCommunityPosts(
  community: string,
  options: UseCommunityPostsOptions = {},
) {
  const { nostr } = useNostr();
  const { showAll = false, limit = 50, since } = options;
  const normalizedCommunity = community.trim().toLowerCase();
  const identifiers = communityToIdentifiers(normalizedCommunity);

  return useQuery({
    queryKey: [
      'clawstr',
      'community-posts',
      normalizedCommunity,
      showAll,
      limit,
      since,
    ],
    queryFn: async ({ signal }) => {
      const startedAt = Date.now();
      const filter: NostrFilter = {
        kinds: [1111],
        '#K': [WEB_KIND],
        '#I': identifiers,
        limit,
      };
      if (since != null && since > 0) filter.since = since;
      if (!showAll) {
        filter['#l'] = [AI_LABEL.value];
      }

      try {
        const events = await queryWithFallback(nostr, [filter], {
          signal,
          timeoutMs: 10000,
        });

        const topLevel = events.filter((event) => {
          if (!isTopLevelPost(event)) return false;
          const id = getPostIdentifier(event);
          if (!id || !isClawstrIdentifier(id)) return false;
          const slug = identifierToCommunity(id);
          return slug === normalizedCommunity;
        });

        posthogCapture('nostr_feed_fetch', {
          scope: 'community',
          community: normalizedCommunity,
          show_all: showAll,
          limit,
          since: since ?? null,
          result_count: topLevel.length,
          duration_ms: Date.now() - startedAt,
        });

        return topLevel
          .sort((a, b) => b.created_at - a.created_at)
          .slice(0, limit) as NostrEvent[];
      } catch (err) {
        posthogCapture('nostr_feed_fetch_error', {
          scope: 'community',
          community: normalizedCommunity,
          show_all: showAll,
          limit,
          since: since ?? null,
          duration_ms: Date.now() - startedAt,
          error: err instanceof Error ? err.message : String(err),
        });
        throw err;
      }
    },
    enabled: !!community.trim(),
    staleTime: 5 * 1000,
    refetchOnMount: 'always',
    refetchOnWindowFocus: true,
    refetchOnReconnect: true,
  });
}

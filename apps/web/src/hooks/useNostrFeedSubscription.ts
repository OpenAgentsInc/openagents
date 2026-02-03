import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import { useNostr } from '@nostrify/react';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import {
  AI_LABEL,
  WEB_KIND,
  getPostIdentifier,
  hasAILabel,
  identifierToCommunity,
  isClawstrIdentifier,
  isTopLevelPost,
  communityToIdentifiers,
} from '@/lib/clawstr';
import { getConfiguredRelays } from '@/lib/nostrPool';
import { storeEvents } from '@/lib/nostrEventCache';

const NEW_EVENT_WINDOW_SECONDS = 30;

function insertEvent(
  existing: NostrEvent[] | undefined,
  event: NostrEvent,
  limit: number,
): NostrEvent[] | undefined {
  if (!Array.isArray(existing)) return existing;
  if (existing.some((item) => item.id === event.id)) return existing;
  const merged = [event, ...existing];
  merged.sort((a, b) => b.created_at - a.created_at);
  return merged.slice(0, limit);
}

export function useNostrFeedSubscription(
  options: { showAll?: boolean; community?: string } = {},
) {
  const { nostr } = useNostr();
  const queryClient = useQueryClient();
  const { showAll = false, community } = options;
  const normalizedCommunity = community?.trim().toLowerCase();

  useEffect(() => {
    const ac = new AbortController();
    const identifiers = normalizedCommunity
      ? communityToIdentifiers(normalizedCommunity)
      : [];
    const startedAt = Math.floor(Date.now() / 1000);
    const minCreatedAt = startedAt - NEW_EVENT_WINDOW_SECONDS;
    const filter: NostrFilter = {
      kinds: [1111],
      '#K': [WEB_KIND],
      since: minCreatedAt,
      limit: 100,
    };
    if (identifiers.length > 0) {
      filter['#I'] = identifiers;
    }
    if (!showAll) {
      filter['#l'] = [AI_LABEL.value];
    }

    const pool = nostr as {
      req: (
        filters: NostrFilter[],
        opts?: { signal?: AbortSignal; relays?: string[] },
      ) => AsyncIterable<[string, string, unknown]>;
    };
    const relays = getConfiguredRelays(nostr);

    (async () => {
      try {
        for await (const msg of pool.req([filter], {
          signal: ac.signal,
          relays,
        })) {
          if (msg[0] !== 'EVENT') continue;
          const event = msg[2] as NostrEvent;
          if (!event || typeof event !== 'object') continue;
          if (event.kind !== 1111) continue;
          if (!isTopLevelPost(event)) continue;
          if (event.created_at < minCreatedAt) continue;
          if (!showAll && !hasAILabel(event)) continue;
          const identifier = getPostIdentifier(event);
          if (!identifier || !isClawstrIdentifier(identifier)) continue;
          const eventCommunity = identifierToCommunity(identifier);
          if (
            normalizedCommunity &&
            eventCommunity !== normalizedCommunity
          )
            continue;

          void storeEvents([event]);

          const postsQueries = queryClient.getQueriesData<NostrEvent[]>({
            queryKey: ['clawstr', 'posts'],
          });
          for (const [key, data] of postsQueries) {
            const [, , keyShowAll, keyLimit, keySince] = key as [
              string,
              string,
              boolean,
              number,
              number | undefined,
            ];
            if (keyShowAll === false && !hasAILabel(event)) continue;
            if (
              typeof keySince === 'number' &&
              event.created_at < keySince
            )
              continue;
            const next = insertEvent(data, event, keyLimit ?? 50);
            if (next) queryClient.setQueryData(key, next);
          }

          if (eventCommunity) {
            const communityQueries = queryClient.getQueriesData<NostrEvent[]>({
              queryKey: ['clawstr', 'community-posts', eventCommunity],
            });
            for (const [key, data] of communityQueries) {
              const [, , , keyShowAll, keyLimit, keySince] = key as [
                string,
                string,
                string,
                boolean,
                number,
                number | undefined,
              ];
              if (keyShowAll === false && !hasAILabel(event)) continue;
              if (
                typeof keySince === 'number' &&
                event.created_at < keySince
              )
                continue;
              const next = insertEvent(data, event, keyLimit ?? 50);
              if (next) queryClient.setQueryData(key, next);
            }
          }

          queryClient.invalidateQueries({
            queryKey: ['clawstr', 'discovered-communities'],
          });
        }
      } catch {
        // Aborted or relay error
      }
    })();

    return () => ac.abort();
  }, [nostr, queryClient, showAll, normalizedCommunity]);
}

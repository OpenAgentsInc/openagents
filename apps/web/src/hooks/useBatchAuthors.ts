import type { NostrEvent } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";

export interface AuthorMeta {
  name?: string;
  picture?: string;
  about?: string;
}

export function useBatchAuthors(pubkeys: string[]) {
  const { nostr } = useNostr();
  const stable = pubkeys.length > 0 ? [...new Set(pubkeys)].sort() : [];
  const key = stable.join(",");

  return useQuery({
    queryKey: ["clawstr", "batch-authors", key],
    queryFn: async ({ signal }): Promise<Map<string, AuthorMeta>> => {
      if (stable.length === 0) return new Map();

      const events = await nostr.query(
        [{ kinds: [0], authors: stable, limit: stable.length }],
        { signal: AbortSignal.any([signal, AbortSignal.timeout(5000)]) }
      );

      const map = new Map<string, AuthorMeta>();
      for (const event of events as NostrEvent[]) {
        try {
          const meta = JSON.parse(event.content) as AuthorMeta;
          map.set(event.pubkey, {
            name: meta.name,
            picture: meta.picture,
            about: meta.about,
          });
        } catch {
          map.set(event.pubkey, {});
        }
      }
      return map;
    },
    enabled: stable.length > 0,
    staleTime: 5 * 60 * 1000,
  });
}

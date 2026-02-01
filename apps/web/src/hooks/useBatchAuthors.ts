import type { NostrEvent } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";
import { queryWithFallback } from "@/lib/nostrQuery";
import { fetchConvexProfiles } from "@/lib/nostrConvex";

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

      const map = await fetchConvexProfiles(stable);
      const missing = stable.filter((pubkey) => !map.has(pubkey));
      if (missing.length === 0) return map;

      const events = await queryWithFallback(
        nostr,
        [{ kinds: [0], authors: missing, limit: missing.length }],
        { signal, timeoutMs: 5000 }
      );

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

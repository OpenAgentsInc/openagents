import type { NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { AI_LABEL, WEB_KIND } from "@/lib/clawstr";

/**
 * Subscribes to new kind-1111 feed events and invalidates the feed query when
 * they arrive so the feed refetches and shows new posts without manual refresh.
 */
export function useNostrFeedSubscription(options: { showAll?: boolean } = {}) {
  const { nostr } = useNostr();
  const queryClient = useQueryClient();
  const { showAll = false } = options;

  useEffect(() => {
    const ac = new AbortController();
    const filter: NostrFilter = {
      kinds: [1111],
      "#K": [WEB_KIND],
      since: Math.floor(Date.now() / 1000) - 120,
      limit: 100,
    };
    if (!showAll) {
      filter["#l"] = [AI_LABEL.value];
      filter["#L"] = [AI_LABEL.namespace];
    }

    const pool = nostr as { req: (filters: NostrFilter[], opts?: { signal?: AbortSignal }) => AsyncIterable<[string, string, unknown]> };
    let seenEose = false;

    (async () => {
      try {
        for await (const msg of pool.req([filter], { signal: ac.signal })) {
          if (msg[0] === "EOSE") seenEose = true;
          if (msg[0] === "EVENT" && seenEose) {
            queryClient.invalidateQueries({ queryKey: ["clawstr", "posts"] });
          }
        }
      } catch {
        // Aborted or relay error
      }
    })();

    return () => ac.abort();
  }, [nostr, queryClient, showAll]);
}

import type { NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import {
  AI_LABEL,
  CLAWSTR_BASE_URL,
  OPENAGENTS_BASE_URL,
  WEB_KIND,
  subclawToIdentifier,
} from "@/lib/clawstr";
import { getConfiguredRelays } from "@/lib/nostrPool";

/**
 * Subscribes to new kind-1111 feed events and invalidates the feed query when
 * they arrive so the feed refetches and shows new posts without manual refresh.
 */
export function useNostrFeedSubscription(
  options: { showAll?: boolean; subclaw?: string } = {}
) {
  const { nostr } = useNostr();
  const queryClient = useQueryClient();
  const { showAll = false, subclaw } = options;
  const normalizedSubclaw = subclaw?.trim().toLowerCase();

  useEffect(() => {
    const ac = new AbortController();
    const identifiers = normalizedSubclaw
      ? [
          subclawToIdentifier(normalizedSubclaw, OPENAGENTS_BASE_URL),
          subclawToIdentifier(normalizedSubclaw, CLAWSTR_BASE_URL),
        ]
      : [];
    const filter: NostrFilter = {
      kinds: [1111],
      "#K": [WEB_KIND],
      since: Math.floor(Date.now() / 1000) - 120,
      limit: 100,
    };
    if (identifiers.length > 0) {
      filter["#I"] = identifiers;
      filter["#i"] = identifiers;
    }
    if (!showAll) {
      filter["#l"] = [AI_LABEL.value];
      filter["#L"] = [AI_LABEL.namespace];
    }

    const pool = nostr as {
      req: (
        filters: NostrFilter[],
        opts?: { signal?: AbortSignal; relays?: string[] }
      ) => AsyncIterable<[string, string, unknown]>;
    };
    const relays = getConfiguredRelays(nostr);
    let seenEose = false;

    (async () => {
      try {
        for await (const msg of pool.req([filter], { signal: ac.signal, relays })) {
          if (msg[0] === "EOSE") seenEose = true;
          if (msg[0] === "EVENT" && seenEose) {
            queryClient.invalidateQueries({ queryKey: ["clawstr", "posts"] });
            if (normalizedSubclaw) {
              queryClient.invalidateQueries({
                queryKey: ["clawstr", "subclaw-posts", normalizedSubclaw],
              });
            }
          }
        }
      } catch {
        // Aborted or relay error
      }
    })();

    return () => ac.abort();
  }, [nostr, queryClient, showAll, normalizedSubclaw]);
}

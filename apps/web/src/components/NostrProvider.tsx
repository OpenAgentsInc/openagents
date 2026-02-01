import React, { useMemo } from "react";
import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { NostrContext } from "@nostrify/react";
import { NPool, NRelay1 } from "@nostrify/nostrify";
import { DEFAULT_RELAYS } from "@/lib/relayConfig";

interface NostrProviderProps {
  children: React.ReactNode;
  /** Relay URLs for read/write. Defaults to DEFAULT_RELAYS. Use relay config (localStorage) when available. */
  relayUrls?: string[];
}

export function NostrProvider({ children, relayUrls: relayUrlsProp }: NostrProviderProps) {
  const relayUrls = relayUrlsProp ?? DEFAULT_RELAYS;
  const pool = useMemo(() => {
    const relays = relayUrls.length > 0 ? relayUrls : DEFAULT_RELAYS;
    return new NPool({
      open(url: string) {
        return new NRelay1(url);
      },
      reqRouter(_filters: NostrFilter[]) {
        const routes = new Map<string, NostrFilter[]>();
        for (const url of relays) {
          routes.set(url, _filters);
        }
        return routes;
      },
      eventRouter(_event: NostrEvent) {
        return [...relays];
      },
    });
  }, [relayUrls.join(",")]);

  return (
    <NostrContext.Provider value={{ nostr: pool }}>
      {children}
    </NostrContext.Provider>
  );
}

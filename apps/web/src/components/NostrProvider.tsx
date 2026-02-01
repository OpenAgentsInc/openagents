import React, { useRef } from "react";
import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { NostrContext } from "@nostrify/react";
import { NPool, NRelay1 } from "@nostrify/nostrify";

const DEFAULT_RELAYS = [
  "wss://relay.damus.io",
  "wss://nos.lol",
  "wss://relay.nostr.band",
];

interface NostrProviderProps {
  children: React.ReactNode;
}

export function NostrProvider({ children }: NostrProviderProps) {
  const pool = useRef<NPool | null>(null);

  if (!pool.current) {
    const relays = DEFAULT_RELAYS;
    pool.current = new NPool({
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
  }

  return (
    <NostrContext.Provider value={{ nostr: pool.current }}>
      {children}
    </NostrContext.Provider>
  );
}

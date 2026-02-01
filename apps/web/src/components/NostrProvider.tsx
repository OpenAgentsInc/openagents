import React, { useMemo } from "react";
import { NostrContext } from "@nostrify/react";
import { DEFAULT_RELAYS } from "@/lib/relayConfig";
import { getNostrPool } from "@/lib/nostrPool";

interface NostrProviderProps {
  children: React.ReactNode;
  /** Relay URLs for read/write. Defaults to DEFAULT_RELAYS. Use relay config (localStorage) when available. */
  relayUrls?: string[];
}

export function NostrProvider({ children, relayUrls: relayUrlsProp }: NostrProviderProps) {
  const relayUrls = relayUrlsProp ?? DEFAULT_RELAYS;
  const pool = useMemo(() => getNostrPool(relayUrls), [relayUrls.join(",")]);

  return (
    <NostrContext.Provider value={{ nostr: pool }}>
      {children}
    </NostrContext.Provider>
  );
}

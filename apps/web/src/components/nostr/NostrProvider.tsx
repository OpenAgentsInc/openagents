import { useEffect, useMemo } from 'react';
import { NostrContext } from '@nostrify/react';
import { DEFAULT_RELAYS } from '@/lib/relayConfig';
import { getNostrPool } from '@/lib/nostrPool';
import { startNostrCacheSync } from '@/lib/nostrSync';
import type { RelayEntry, RelayMetadata } from '@/lib/relayConfig';

interface NostrProviderProps {
  children: React.ReactNode;
  relayUrls?: string[];
  relayMetadata?: RelayMetadata;
}

export function NostrProvider({
  children,
  relayUrls: relayUrlsProp,
  relayMetadata,
}: NostrProviderProps) {
  const relayConfig = useMemo<RelayEntry[]>(() => {
    if (relayMetadata?.relays?.length) return relayMetadata.relays;
    const relayUrls = relayUrlsProp ?? DEFAULT_RELAYS;
    return relayUrls.map((url) => ({
      url,
      read: true,
      write: true,
    }));
  }, [relayMetadata, relayUrlsProp]);

  const relayKey = useMemo(() => {
    const sorted = [...relayConfig].sort((a, b) => a.url.localeCompare(b.url));
    return sorted
      .map((relay) => `${relay.url}:${relay.read ? 'r' : ''}${relay.write ? 'w' : ''}`)
      .join('|');
  }, [relayConfig]);

  const pool = useMemo(() => getNostrPool(relayConfig), [relayKey]);
  useEffect(() => {
    startNostrCacheSync(pool);
  }, [pool]);

  return (
    <NostrContext.Provider value={{ nostr: pool }}>
      {children}
    </NostrContext.Provider>
  );
}

import { useEffect, useMemo } from 'react';
import { NostrContext } from '@nostrify/react';
import { DEFAULT_RELAYS } from '@/lib/relayConfig';
import { getNostrPool } from '@/lib/nostrPool';
import { startNostrCacheSync } from '@/lib/nostrSync';

interface NostrProviderProps {
  children: React.ReactNode;
  relayUrls?: string[];
}

export function NostrProvider({
  children,
  relayUrls: relayUrlsProp,
}: NostrProviderProps) {
  const relayUrls = relayUrlsProp ?? DEFAULT_RELAYS;
  const pool = useMemo(() => getNostrPool(relayUrls), [relayUrls.join(',')]);
  useEffect(() => {
    startNostrCacheSync(pool);
  }, [pool]);

  return (
    <NostrContext.Provider value={{ nostr: pool }}>
      {children}
    </NostrContext.Provider>
  );
}

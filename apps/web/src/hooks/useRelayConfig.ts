import { useState, useEffect } from 'react';
import {
  getStoredRelays,
  setStoredRelays,
  DEFAULT_RELAYS,
} from '@/lib/relayConfig';

const STORAGE_KEY = 'clawstr-relays';

export function useRelayConfig() {
  const [relayUrls, setRelayUrlsState] = useState<string[]>(DEFAULT_RELAYS);
  const [hydrated, setHydrated] = useState(false);

  useEffect(() => {
    setRelayUrlsState(getStoredRelays());
    setHydrated(true);
  }, []);

  useEffect(() => {
    if (!hydrated) return;
    const onStorage = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY && e.newValue) {
        try {
          const parsed = JSON.parse(e.newValue) as unknown;
          if (Array.isArray(parsed)) {
            setRelayUrlsState(
              parsed.filter((x): x is string => typeof x === 'string'),
            );
          }
        } catch {
          // ignore
        }
      }
    };
    window.addEventListener('storage', onStorage);
    return () => window.removeEventListener('storage', onStorage);
  }, [hydrated]);

  function setRelayUrls(urls: string[]) {
    const valid = urls.filter(
      (x) => typeof x === 'string' && x.startsWith('wss://'),
    );
    setRelayUrlsState(valid.length > 0 ? valid : DEFAULT_RELAYS);
    setStoredRelays(valid.length > 0 ? valid : DEFAULT_RELAYS);
  }

  return { relayUrls: hydrated ? relayUrls : getStoredRelays(), setRelayUrls };
}

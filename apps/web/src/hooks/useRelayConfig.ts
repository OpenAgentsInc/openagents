import { useMemo, useState, useEffect } from 'react';
import {
  buildRelayMetadataFromUrls,
  getStoredRelayMetadata,
  setStoredRelayMetadata,
  DEFAULT_RELAY_METADATA,
  RELAY_STORAGE_KEY,
  type RelayMetadata,
} from '@/lib/relayConfig';

export function useRelayConfig() {
  const [relayMetadata, setRelayMetadataState] = useState<RelayMetadata>(
    DEFAULT_RELAY_METADATA,
  );
  const [hydrated, setHydrated] = useState(false);

  useEffect(() => {
    setRelayMetadataState(getStoredRelayMetadata());
    setHydrated(true);
  }, []);

  useEffect(() => {
    if (!hydrated) return;
    const onStorage = (e: StorageEvent) => {
      if (e.key === RELAY_STORAGE_KEY && e.newValue) {
        setRelayMetadataState(getStoredRelayMetadata());
      }
    };
    window.addEventListener('storage', onStorage);
    return () => window.removeEventListener('storage', onStorage);
  }, [hydrated]);

  const relayUrls = useMemo(() => {
    const readRelays = relayMetadata.relays
      .filter((relay) => relay.read)
      .map((relay) => relay.url);
    return readRelays.length > 0 ? readRelays : DEFAULT_RELAY_METADATA.relays.map((r) => r.url);
  }, [relayMetadata]);

  function setRelayMetadata(next: RelayMetadata) {
    const updated = {
      ...next,
      updatedAt: Math.floor(Date.now() / 1000),
    };
    setRelayMetadataState(updated);
    setStoredRelayMetadata(updated);
  }

  function setRelayUrls(urls: string[]) {
    const metadata = buildRelayMetadataFromUrls(urls);
    setRelayMetadataState(metadata);
    setStoredRelayMetadata(metadata);
  }

  return {
    relayUrls: hydrated
      ? relayUrls
      : getStoredRelayMetadata().relays.filter((r) => r.read).map((r) => r.url),
    relayMetadata: hydrated ? relayMetadata : getStoredRelayMetadata(),
    setRelayUrls,
    setRelayMetadata,
  };
}

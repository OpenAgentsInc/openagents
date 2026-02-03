import { createContext, useContext, type ReactNode } from 'react';
import { useRelayConfig } from '@/hooks/useRelayConfig';
import type { RelayMetadata } from '@/lib/relayConfig';

interface RelayConfigContextType {
  relayUrls: string[];
  relayMetadata: RelayMetadata;
  setRelayUrls: (urls: string[]) => void;
  setRelayMetadata: (metadata: RelayMetadata) => void;
}

const RelayConfigContext = createContext<RelayConfigContextType | null>(null);

export function RelayConfigProvider({ children }: { children: ReactNode }) {
  const { relayUrls, setRelayUrls, relayMetadata, setRelayMetadata } =
    useRelayConfig();
  return (
    <RelayConfigContext.Provider
      value={{ relayUrls, setRelayUrls, relayMetadata, setRelayMetadata }}
    >
      {children}
    </RelayConfigContext.Provider>
  );
}

export function useRelayConfigContext(): RelayConfigContextType {
  const ctx = useContext(RelayConfigContext);
  if (!ctx)
    throw new Error(
      'useRelayConfigContext must be used within RelayConfigProvider',
    );
  return ctx;
}

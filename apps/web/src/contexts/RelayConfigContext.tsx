import { createContext, useContext, type ReactNode } from 'react';
import { useRelayConfig } from '@/hooks/useRelayConfig';

interface RelayConfigContextType {
  relayUrls: string[];
  setRelayUrls: (urls: string[]) => void;
}

const RelayConfigContext = createContext<RelayConfigContextType | null>(null);

export function RelayConfigProvider({ children }: { children: ReactNode }) {
  const { relayUrls, setRelayUrls } = useRelayConfig();
  return (
    <RelayConfigContext.Provider value={{ relayUrls, setRelayUrls }}>
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

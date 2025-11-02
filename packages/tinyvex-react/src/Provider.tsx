import React from 'react';
import type { Transport } from '@openagents/tinyvex-client';
import { TinyvexClient } from '@openagents/tinyvex-client';
import { silentLogger } from '@openagents/tinyvex-client';

export type TinyvexConfig = {
  url: string;
  token?: string;
  debug?: boolean;
};

class WsTransport implements Transport {
  private _status: 'connecting' | 'open' | 'closed' | 'error' = 'closed';
  private listeners = new Set<(evt: unknown) => void>();
  // Placeholder; Phase 2 will implement actual WS.
  async connect(): Promise<void> {
    this._status = 'open';
  }
  close(): void {
    this._status = 'closed';
  }
  send(_control: { name: string; args?: unknown }): void {
    // no-op in scaffold
  }
  onMessage(cb: (evt: unknown) => void): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }
  status() {
    return this._status;
  }
}

function makeClient(cfg: TinyvexConfig) {
  const t = new WsTransport();
  return new TinyvexClient(t, cfg.debug ? console : silentLogger);
}

export const TinyvexContext = React.createContext<TinyvexClient | null>(null);

export function TinyvexProvider({ config, children }: { config: TinyvexConfig; children: React.ReactNode }) {
  const client = React.useMemo(() => makeClient(config), [config.url, config.token, config.debug]);
  return <TinyvexContext.Provider value={client}>{children}</TinyvexContext.Provider>;
}


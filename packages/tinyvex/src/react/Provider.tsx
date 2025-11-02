import React from 'react';
import type { Transport } from '../client';
import { TinyvexClient, silentLogger } from '../client';
import { WsTransport } from '../client/WsTransport';

export type TinyvexConfig = {
  url: string;
  token?: string;
  debug?: boolean;
};

function makeClient(cfg: TinyvexConfig) {
  const t: Transport = new WsTransport({ url: cfg.url, token: cfg.token });
  return new TinyvexClient(t, cfg.debug ? console : silentLogger);
}

export const TinyvexContext = React.createContext<TinyvexClient | null>(null);

export function TinyvexProvider({ config, children }: { config: TinyvexConfig; children: React.ReactNode }) {
  const client = React.useMemo(() => makeClient(config), [config.url, config.token, config.debug]);
  return <TinyvexContext.Provider value={client}>{children}</TinyvexContext.Provider>;
}

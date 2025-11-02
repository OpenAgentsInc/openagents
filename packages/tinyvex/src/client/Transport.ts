export type TransportState = 'connecting' | 'open' | 'closed' | 'error';

export interface Transport {
  connect(): Promise<void>;
  close(): void;
  send(control: { name: string; args?: unknown }): void;
  onMessage(cb: (evt: unknown) => void): () => void;
  status(): TransportState;
}


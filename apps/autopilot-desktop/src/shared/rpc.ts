export type RPCSchema = {
  // TODO: Define bun<->webview requests/events mirroring the bridge vocabulary.
  readonly requests: Record<string, never>;
  readonly events: Record<string, never>;
};

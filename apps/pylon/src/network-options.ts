export type PylonNetworkOptions = {
  readonly agentToken?: string;
  readonly baseUrl: string;
  readonly fetch?: typeof fetch;
  readonly now?: () => Date;
};

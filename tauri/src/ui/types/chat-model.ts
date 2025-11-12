export type ChatModelAdapter = {
  run: (opts: {
    messages: any[];
    abortSignal?: AbortSignal;
  }) => AsyncGenerator<any, void, unknown>;
};


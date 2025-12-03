export type Api = "openai-completions" | "openai-responses" | "anthropic-messages" | "google-generative-ai";

export type Provider = "anthropic" | "google" | "openai" | "xai" | "groq" | "cerebras" | "openrouter" | "zai" | string;

export interface Model<TApi extends Api = Api> {
  id: string;
  name: string;
  api: TApi;
  provider: Provider;
  baseUrl: string;
  headers?: Record<string, string>;
  reasoning: boolean;
  input: Array<"text" | "image">;
  cost: {
    input: number;
    output: number;
    cacheRead: number;
    cacheWrite: number;
  };
  contextWindow: number;
  maxTokens: number;
}

export interface Usage {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  cost: {
    input: number;
    output: number;
    cacheRead: number;
    cacheWrite: number;
    total: number;
  };
}

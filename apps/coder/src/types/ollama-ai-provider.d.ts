declare module 'ollama-ai-provider' {
  import { AIProvider } from 'ai';

  export interface OllamaProviderOptions {
    baseURL?: string;
    headers?: Record<string, string>;
  }

  export function ollama(modelId: string): AIProvider;
  export function createOllama(options: OllamaProviderOptions): (modelId: string) => AIProvider;
}
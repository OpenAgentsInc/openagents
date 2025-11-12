import { useLocalRuntime } from '@assistant-ui/react';
import { createOllamaAdapter } from './ollama-adapter';

export function useAcpRuntime() {
  const adapter = createOllamaAdapter();
  return useLocalRuntime(adapter);
}


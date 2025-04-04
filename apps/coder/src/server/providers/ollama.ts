/**
 * Ollama provider implementation
 */

import { ollama, createOllama } from 'ollama-ai-provider';
import { Provider } from './index';

/**
 * Create an Ollama provider
 */
export function createOllamaProvider(
  modelId: string,
  options: Record<string, any> = {}
): Provider {
  // Get base URL from options or use default
  const baseURL = options.baseURL || options.ollamaBaseUrl || "http://localhost:11434/api";
  
  // Create the Ollama client
  const ollamaClient = createOllama({
    baseURL,
    simulateStreaming: true
  });
  
  // Log availability information - this could be enhanced with actual API calls
  console.log(`[Server] Ollama model check - baseUrl: ${baseURL}, model: ${modelId}`);
  console.log(`[Server] If experiencing issues with Ollama, check that:`);
  console.log(`[Server]   1. Ollama server is running (run 'ollama serve')`);
  console.log(`[Server]   2. The model is available (run 'ollama list')`);
  console.log(`[Server]   3. If needed, pull the model with: 'ollama pull ${modelId.split(":")[0]}'`);
  
  // Try to determine context window size based on model
  let contextWindowSize = 4096; // Default for smaller models
  
  // Adjust context window size based on recognized models
  if (modelId.includes('llama-3')) {
    contextWindowSize = 8192;
  } else if (modelId.includes('llama-2-70b')) {
    contextWindowSize = 4096;
  } else if (modelId.includes('mistral')) {
    contextWindowSize = 8192;
  } else if (modelId.includes('mixtral')) {
    contextWindowSize = 32000;
  } else if (modelId.includes('gemma')) {
    contextWindowSize = 8192;
  }
  
  // Currently, most Ollama models don't reliably support function calling
  const supportsTools = false;
  
  return {
    id: modelId,
    name: `Ollama ${modelId}`,
    model: ollamaClient(modelId),
    supportsTools,
    contextWindowSize
  };
}
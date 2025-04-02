# Issue #831: Ollama Provider Implementation

## Implementation Overview

The implementation adds support for the Ollama provider to the OpenAgents Coder app. This allows users to use both OpenRouter models and Ollama models through a unified interface.

## Changes Made

### 1. Added Dependencies

Added the `ollama-ai-provider` package to the coder app:

```bash
cd /Users/christopherdavid/code/openagents/apps/coder && yarn add ollama-ai-provider
```

### 2. Updated Server Configuration in server.ts

1. Added imports for the Ollama provider:
   ```typescript
   import { ollama, createOllama } from 'ollama-ai-provider';
   ```

2. Extended the environment interface to include Ollama configuration:
   ```typescript
   interface Env {
     OPENROUTER_API_KEY?: string;
     OLLAMA_BASE_URL?: string;
     ALLOW_COMMANDS?: string; // Shell commands whitelist for mcp-shell-server
   }
   ```

3. Initialized Ollama client with configurable base URL:
   ```typescript
   const OLLAMA_BASE_URL = process.env.OLLAMA_BASE_URL || "http://localhost:11434/api";
   
   // Create the Ollama client with custom base URL if specified
   const customOllama = createOllama({
     baseURL: OLLAMA_BASE_URL,
   });
   ```

4. Updated model validation to check for provider:
   ```typescript
   // Find model info in MODELS
   const modelInfo = MODELS.find(m => m.id === MODEL);
   if (!modelInfo) {
     return c.json({ error: `Model ${MODEL} not found in the MODELS array` }, 400);
   }
   // Get provider from model info
   const provider = modelInfo.provider;
   ```

5. Implemented provider-specific setup:
   ```typescript
   // Determine which provider to use based on model info
   let model;
   let headers = {};

   if (provider === "ollama") {
     console.log(`[Server] Using Ollama provider with base URL: ${OLLAMA_BASE_URL}`);
     // For Ollama models, use the Ollama provider
     model = customOllama(MODEL);
   } else if (provider === "openrouter") {
     console.log(`[Server] Using OpenRouter provider`);
     // For OpenRouter models, use the OpenRouter provider
     model = openrouter(MODEL);
     // Set OpenRouter specific headers
     headers = {
       'Authorization': `Bearer ${OPENROUTER_API_KEY}`,
       'HTTP-Referer': 'https://openagents.com',
       'X-Title': 'OpenAgents Coder'
     };
   } else {
     // Default to OpenRouter for unspecified providers
     // ...
   }
   ```

6. Updated stream options to include provider-specific configuration:
   ```typescript
   const streamOptions = {
     model,
     messages,
     toolCallStreaming: modelSupportsTools,
     temperature: 0.7,
     
     // Only include tools if the model supports them
     ...(modelSupportsTools && Object.keys(tools).length > 0 ? { tools } : {}),
     
     // Include headers if present
     ...(Object.keys(headers).length > 0 ? { headers } : {}),

     // Standard callbacks
     // ...
   };
   ```

## MODELS.ts Configuration

The `MODELS.ts` file already had the `provider` field in the Model interface, supporting:
```typescript
provider: "openrouter" | "anthropic" | "ollama"
```

And one Ollama model was already defined:
```typescript
{
  author: "google",
  provider: "ollama",
  id: "gemma3:12b",
  name: "Gemma 3 12B",
  created: 1742824755,
  description: "Gemma 3 12B is a large language model with 12B parameters.",
  shortDescription: "Gemma 3 12B is a large language model with 12B parameters.",
  context_length: 128000,
  supportsTools: true,
}
```

## Environment Variables

The implementation supports two key environment variables:

1. `OPENROUTER_API_KEY` - For OpenRouter models (existing)
2. `OLLAMA_BASE_URL` - For configuring the Ollama API endpoint (new, defaults to "http://localhost:11434/api")

## Benefits of the Implementation

1. **Flexible Provider Selection**: The system now dynamically selects the appropriate provider based on the model's configuration.
2. **Local Model Support**: Users can now use local Ollama models without requiring an OpenRouter API key.
3. **Remote Ollama Support**: By configuring OLLAMA_BASE_URL, users can connect to remote Ollama instances.
4. **Tool Support**: Tool integration is maintained for models that support it, regardless of provider.
5. **Backward Compatibility**: The system continues to work with existing OpenRouter models.

## Testing

The implementation has been structured to ensure compatibility with both providers. To test:

1. For OpenRouter models:
   - Set OPENROUTER_API_KEY in the environment
   - Select a model with provider="openrouter" in the UI

2. For Ollama models:
   - Run Ollama locally or set OLLAMA_BASE_URL to a remote Ollama instance
   - Select a model with provider="ollama" in the UI

The system will automatically use the correct provider based on the model's configuration.
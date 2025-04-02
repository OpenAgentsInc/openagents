# Issue #831: Implementing Ollama Provider Support

## Overview

Issue #831 involves adding support for the Ollama provider to the OpenAgents project. Ollama is a tool that allows running various large language models locally or remotely through its API. This implementation will expand the available AI model providers beyond OpenRouter, enabling users to use local or remote Ollama models with the platform.

## Current State

From examining the codebase on the `ollama` branch:

1. The `MODELS.ts` file already includes one Ollama model entry:
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

2. The `server.ts` file currently only supports the OpenRouter provider. It creates an OpenRouter client and uses it for all model requests.

3. The `Model` interface in `MODELS.ts` includes `provider` as a property that can be set to `"openrouter" | "anthropic" | "ollama"`.

4. There is no current implementation of the Ollama client or configuration for the Ollama API.

## Implementation Requirements

Based on the issue description and the Vercel AI SDK Ollama provider documentation, we need to:

1. Add the `ollama-ai-provider` package as a dependency
2. Modify the server.ts file to:
   - Import the Ollama provider
   - Check for an OLLAMA_BASE_URL environment variable (defaulting to localhost)
   - Create the Ollama client when needed based on model provider
   - Dynamically select the appropriate provider based on the model's provider field

3. Update the stream options to use the correct provider based on the model

4. Add more Ollama models to the MODELS array (optional, as models can also be dynamically added)

5. Ensure tool support works properly with Ollama models that support function calling

## Proposed Technical Approach

1. Install the `ollama-ai-provider` package to the coder app
2. Update `server.ts` to:
   - Import the Ollama provider from `ollama-ai-provider`
   - Create a function to get the appropriate provider based on model information
   - Use environment variables for configuration (OLLAMA_BASE_URL)
   - Update the stream options to use the selected provider

3. Ensure backward compatibility with the existing OpenRouter implementation

4. Test with both OpenRouter and Ollama models to verify the implementation works

## Next Steps

1. Install required dependencies
2. Implement the provider selection functionality
3. Update the server.ts file to use the dynamic provider
4. Test with both provider types
5. Document the new functionality

This implementation will make OpenAgents more versatile by supporting both cloud-based models via OpenRouter and local/remote models via Ollama, giving users more flexibility in how they deploy and use AI models.
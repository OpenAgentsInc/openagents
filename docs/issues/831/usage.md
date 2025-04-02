# Using Ollama Provider with OpenAgents

This guide explains how to use the Ollama provider integration in OpenAgents Coder.

## Prerequisites

1. Make sure you have Ollama installed locally or have access to a remote Ollama server.
   - Installation guide: https://ollama.com/download

2. For local Ollama, start the Ollama server:
   ```bash
   ollama serve
   ```

3. Pull the models you want to use:
   ```bash
   # For example, to pull the Gemma 3 12B model:
   ollama pull gemma3:12b
   ```

## Configuration

### Environment Variables

Configure OpenAgents with these environment variables:

1. **For Ollama**:
   - `OLLAMA_BASE_URL`: URL of your Ollama API (defaults to `http://localhost:11434/api` if not set)

2. **For OpenRouter**:
   - `OPENROUTER_API_KEY`: Your OpenRouter API key (required for OpenRouter models)

Example `.env` file:
```
OLLAMA_BASE_URL=http://localhost:11434/api
OPENROUTER_API_KEY=your_openrouter_api_key_here
```

## Using Ollama Models

### Local Ollama

1. Start your Ollama server locally
2. Set `OLLAMA_BASE_URL` to `http://localhost:11434/api` (default)
3. Launch OpenAgents Coder
4. Select a model with provider "ollama" from the model selection UI

### Remote Ollama

1. Ensure your remote Ollama server is running and accessible
2. Set `OLLAMA_BASE_URL` to the appropriate remote URL (e.g., `http://your-ollama-server:11434/api`)
3. Launch OpenAgents Coder
4. Select a model with provider "ollama" from the model selection UI

## Adding Custom Ollama Models

To add more Ollama models to the available selection:

1. Update the `MODELS.ts` file to include your custom Ollama models:

```typescript
// Add in packages/core/src/chat/MODELS.ts
{
  author: "your-author",
  provider: "ollama",
  id: "your-model-id", // This should match the Ollama model ID
  name: "Your Model Name",
  created: Date.now(),
  description: "Description of your model",
  shortDescription: "Short description of your model",
  context_length: 32000, // Adjust based on your model's capabilities
  supportsTools: true, // Set to true if model supports function calling
}
```

2. Ensure the model is available in your Ollama server:
```bash
ollama pull your-model-id
```

## Troubleshooting

### Common Issues

1. **Cannot connect to Ollama server**
   - Ensure Ollama is running (`ollama serve`)
   - Check if the OLLAMA_BASE_URL is correct
   - Verify network connectivity to the Ollama server

2. **Model not found**
   - Ensure the model ID in MODELS.ts matches the Ollama model ID exactly
   - Verify the model is pulled (`ollama list`)
   - Try pulling the model again: `ollama pull model-id`

3. **API timeout**
   - For larger models, initial responses might take longer
   - Check system resources (CPU, RAM, GPU) for bottlenecks

### Logging and Debugging

The server logs detailed information about the provider selection:

```
[Server] OLLAMA_BASE_URL: http://localhost:11434/api
[Server] Using Ollama provider with base URL: http://localhost:11434/api
```

Check these logs to confirm the correct provider is being used for your selected model.
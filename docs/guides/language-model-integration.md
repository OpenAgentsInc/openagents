# Language Model Integration Guide

**Audience**: Coding Agents  
**Purpose**: Complete guide for adding, configuring, and managing language models in OpenAgents  
**Last Updated**: 2025-06-23

## Overview

The OpenAgents platform supports multiple AI providers through a unified interface. Models are defined at the provider level, configured for the application, and rendered in the UI for user selection.

## Architecture

```
┌─────────────────────────┐
│   models-config.ts      │ ← UI Configuration
│ (Available models list) │
└────────────┬────────────┘
             │
┌────────────▼────────────┐
│    UI Components        │
│ (Model selector in      │
│  home.ts & chat.ts)     │
└────────────┬────────────┘
             │
┌────────────▼────────────┐
│    API Routes           │
│ (/api/cloudflare/chat)  │
│ (/api/openrouter/chat)  │
└────────────┬────────────┘
             │
┌────────────▼────────────┐
│   AI Package            │
│ Provider Implementations│
└─────────────────────────┘
```

## Model Definition Locations

### 1. Provider Level (packages/ai/src/providers/*)
Each provider defines available models as constants:

```typescript
// CloudflareLanguageModel.ts
export const models = {
  LLAMA_4_SCOUT_17B_INSTRUCT: "@cf/meta/llama-4-scout-17b-16e-instruct",
  LLAMA_3_3_70B_INSTRUCT_FP8_FAST: "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
  LLAMA_3_1_8B_INSTRUCT: "@cf/meta/llama-3.1-8b-instruct",
  DEEPSEEK_R1_DISTILL_32B: "@cf/deepseek-ai/deepseek-r1-distill-qwen-32b",
  // ... more models
} as const
```

### 2. Application Level (apps/openagents.com/src/lib/models-config.ts)
The UI uses a unified configuration that includes metadata:

```typescript
export interface ModelConfig {
  id: string                                    // Model identifier
  name: string                                  // Display name
  provider: "cloudflare" | "openrouter"        // Provider type
  requiresApiKey: boolean                       // Whether API key needed
  description?: string                          // Optional description
}

export const AVAILABLE_MODELS: Array<ModelConfig> = [
  {
    id: "@cf/meta/llama-4-scout-17b-16e-instruct",
    name: "Llama 4 Scout 17B",
    provider: "cloudflare",
    requiresApiKey: false,
    description: "Latest Llama model, optimized for efficiency"
  },
  // ... more models
]

export const DEFAULT_MODEL = "@cf/meta/llama-4-scout-17b-16e-instruct"
```

## Adding a New Model

### Step 1: Add to Provider Implementation

For Cloudflare models, edit `packages/ai/src/providers/cloudflare/CloudflareLanguageModel.ts`:

```typescript
export const models = {
  // Add your new model
  MY_NEW_MODEL: "@cf/provider/model-name",
  // ... existing models
} as const
```

For OpenRouter, models are passed dynamically via the `modelId` parameter.

### Step 2: Add to Application Config

Edit `apps/openagents.com/src/lib/models-config.ts`:

```typescript
export const AVAILABLE_MODELS: Array<ModelConfig> = [
  // Add your new model
  {
    id: "@cf/provider/model-name",
    name: "Model Display Name",
    provider: "cloudflare",
    requiresApiKey: false,
    description: "Brief description of capabilities"
  },
  // ... existing models
]
```

### Step 3: (Optional) Add Preset

For frequently used models, add a preset in the provider:

```typescript
export const presets = {
  my_model: (options?: Partial<Parameters<typeof makeLanguageModel>[0]>) =>
    makeLanguageModel({
      model: models.MY_NEW_MODEL,
      temperature: 0.7,
      maxTokens: 4096,
      ...options
    }),
  // ... existing presets
}
```

## How Models Are Used

### 1. Frontend Selection
The model selector UI is rendered in `home.ts` and `chat.ts`:
- Displays models grouped by provider
- Shows lock icon for models requiring API keys
- Stores selection in localStorage
- Passes selected model to API endpoints

### 2. API Routing
Based on the selected model's provider, requests are routed to:
- `/api/cloudflare/chat` for Cloudflare models
- `/api/openrouter/chat` for OpenRouter models
- `/api/ollama/chat` for local Ollama models

### 3. Provider Invocation
API routes use the Effect-based providers:

```typescript
// In cloudflare.ts API route
const client = yield* CloudflareClient
const aiStream = yield* client.stream({
  model,      // Model ID from frontend
  messages,   // Conversation history
  stream: true
})
```

## UI Rendering

### Model Selector Structure
The model dropdown is rendered with:
- Provider groups (Cloudflare Models, OpenRouter Models)
- Model options with name and description
- Lock icons for models requiring API keys
- Selected state highlighting

### Selection Logic
```javascript
// Frontend model selection
function selectModel(modelId) {
  const model = modelConfig.find(m => m.id === modelId);
  
  // Check API key requirement
  if (model.provider === 'openrouter' && !hasApiKey) {
    showApiKeyNotice();
    return;
  }
  
  // Save selection
  localStorage.setItem('selectedModel', modelId);
  updateUI(model);
}
```

## Model Categories

### Free Models (Cloudflare)
- No API key required
- Hosted on Cloudflare Workers AI
- Lower latency for edge deployment
- Examples: Llama models, DeepSeek

### Premium Models (OpenRouter)
- Requires API key (user or server)
- Access to 100+ models
- Includes GPT-4, Claude, etc.
- Billed per token

### Local Models (Ollama)
- Runs on user's machine
- No API costs
- Requires Ollama installation
- Full privacy

## Configuration Best Practices

### 1. Model Ordering
Place models in order of recommendation:
- Default/recommended model first
- Group by capability (general, code, vision)
- Free models before paid

### 2. Descriptions
Add helpful descriptions for:
- Model strengths ("Best for coding")
- Performance characteristics ("Fast", "High quality")
- Special capabilities ("Vision support")

### 3. Default Selection
Choose defaults based on:
- Free tier availability
- Balance of quality/speed
- General purpose capability

## Testing New Models

### 1. Provider Level
```typescript
// Test the provider implementation
const model = yield* CloudflareClient.stream({
  model: "@cf/provider/new-model",
  messages: [{ role: "user", content: "Test" }]
})
```

### 2. API Level
```bash
# Test the API endpoint
curl -X POST http://localhost:3003/api/cloudflare/chat \
  -H "Content-Type: application/json" \
  -d '{
    "model": "@cf/provider/new-model",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

### 3. UI Level
- Check model appears in dropdown
- Verify selection persistence
- Test streaming response
- Confirm error handling

## Common Issues

### Model Not Appearing in UI
- Check it's added to `AVAILABLE_MODELS`
- Verify provider is correct
- Clear localStorage/cache

### Streaming Errors
- Verify model supports streaming
- Check provider implementation
- Review SSE format conversion

### API Key Issues
- Ensure requiresApiKey is set correctly
- Check server/client key logic
- Verify key storage/retrieval

## Model Deprecation

When removing models:
1. Remove from provider constants
2. Remove from AVAILABLE_MODELS
3. Update DEFAULT_MODEL if needed
4. Add migration for stored selections
5. Update documentation

---

**Remember**: Always test new models end-to-end from UI selection through streaming response.
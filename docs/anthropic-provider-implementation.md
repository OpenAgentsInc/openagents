# Anthropic Provider Implementation Guide for OpenAgents

This document provides a detailed guide for implementing Anthropic's Claude models in the OpenAgents application, focusing on the server-side integration and API key handling.

## Table of Contents

1. [Overview](#overview)
2. [Claude Models](#claude-models)
3. [Installation](#installation)
4. [Server Integration](#server-integration)
5. [API Key Handling](#api-key-handling)
6. [Client-Side Integration](#client-side-integration)
7. [Testing and Validation](#testing-and-validation)
8. [Troubleshooting](#troubleshooting)

## Overview

Anthropic's Claude models provide powerful AI capabilities with strong reasoning, safety measures, and multimodal support. This implementation will enable OpenAgents to use Claude models directly, enhancing the platform's model options beyond the currently supported OpenRouter and local models.

Key features of this integration:
- Support for Claude's latest models
- API key management through the dedicated API Keys page
- Error handling for missing keys
- Compatibility with existing OpenAgents workflow
- Tool call support for compatible models

## Claude Models

Anthropic offers several Claude models with varying capabilities and price points. The primary models to support are:

| Model               | Context Window | Strengths                                     | Use Cases                            |
|---------------------|---------------|-------------------------------------------------|--------------------------------------|
| claude-3-7-sonnet   | 200K tokens   | Best balance of intelligence and speed         | General purpose, coding, analysis    |
| claude-3-5-sonnet   | 200K tokens   | Good balance of capabilities and cost          | Everyday tasks, document processing  |
| claude-3-opus       | 200K tokens   | Most powerful, best for complex reasoning      | Complex analysis, research, coding   |
| claude-3-haiku      | 200K tokens   | Fastest, most economical                       | Quick responses, simple tasks        |
| claude-3-sonnet     | 200K tokens   | Earlier model, balanced performance            | General purpose tasks                |
| claude-3-haiku-20240307 | 200K tokens | Earlier version of Haiku                    | Quick processing, simple interactions |

All of these models support:
- Text generation
- Tool usage (function calling)
- JSON mode
- Reasoning

## Installation

To add Anthropic support to OpenAgents, install the official AI SDK Anthropic package:

```bash
npm install @ai-sdk/anthropic
```

This package will integrate with the existing AI SDK implementation in the OpenAgents codebase.

## Server Integration

### 1. Import Required Dependencies

Update `/apps/coder/src/server/server.ts` to include Anthropic:

```typescript
// Add to existing imports
import { anthropic, createAnthropic } from '@ai-sdk/anthropic';
```

### 2. Update Environment Interface

Modify the `Env` interface to include an Anthropic API key:

```typescript
interface Env {
  OPENROUTER_API_KEY?: string;
  ANTHROPIC_API_KEY?: string;  // Add this line
  OLLAMA_BASE_URL?: string;
  ALLOW_COMMANDS?: string;
}
```

### 3. Add API Key Extraction

In the chat endpoint handler, extract the Anthropic API key:

```typescript
// Extract API keys from request if provided
const requestApiKeys = body.apiKeys || {};

// Debug the incoming request API keys
console.log("[Server] Received API keys from request:", JSON.stringify(requestApiKeys));

// Use API keys from request if available, fall back to environment variables
const OPENROUTER_API_KEY = requestApiKeys.openrouter || process.env.OPENROUTER_API_KEY || "";
const ANTHROPIC_API_KEY = requestApiKeys.anthropic || process.env.ANTHROPIC_API_KEY || "";
const OLLAMA_BASE_URL = requestApiKeys.ollama || requestApiKeys.ollamaBaseUrl || process.env.OLLAMA_BASE_URL || "http://localhost:11434/api";
```

### 4. Add Anthropic Client Creation

Add code to create the Anthropic client:

```typescript
// Log configuration for Anthropic
if (!ANTHROPIC_API_KEY) {
  console.warn("⚠️ ANTHROPIC_API_KEY is missing - Anthropic Claude models will not be available");
} else {
  console.log("✅ ANTHROPIC_API_KEY is present - Anthropic Claude models are available");
}

// Create the Anthropic client with API key
const anthropicClient = createAnthropic({
  apiKey: ANTHROPIC_API_KEY,
  // Optional: Add any custom headers if needed
  headers: {
    'Anthropic-Version': '2023-06-01'
  }
});
```

### 5. Update Provider Selection Logic

Expand the provider selection logic to include Anthropic:

```typescript
// Determine which provider to use based on model info
let model;
let headers = {};

if (provider === "lmstudio") {
  // Existing LMStudio code...
} else if (provider === "ollama") {
  // Existing Ollama code...
} else if (provider === "openrouter") {
  // Existing OpenRouter code...
} else if (provider === "anthropic") {
  console.log(`[Server] Using Anthropic provider`);
  // Check if API key is present
  if (!ANTHROPIC_API_KEY) {
    // Return error as SSE
    c.header('Content-Type', 'text/event-stream; charset=utf-8');
    c.header('Cache-Control', 'no-cache');
    c.header('Connection', 'keep-alive');
    c.header('X-Vercel-AI-Data-Stream', 'v1');

    return stream(c, async (responseStream) => {
      const errorMsg = "Anthropic API Key not configured. Please add your API key in the Settings > API Keys tab to use Claude models.";
      await responseStream.write(`data: 3:${JSON.stringify(errorMsg)}\n\n`);
    });
  }
  
  // For Anthropic models, use the Anthropic provider
  model = anthropicClient(MODEL);
  // Set Anthropic specific headers if needed
  headers = {
    'Anthropic-Version': '2023-06-01'
  };
} else {
  // Default provider handling...
}
```

## API Key Handling

### 1. Server-Side Handling

The server already has robust API key handling for OpenRouter that we can mirror for Anthropic:

1. Extract API key from request body's `apiKeys` object
2. Fall back to environment variable if no user key is provided
3. Check if key is present before attempting to use Anthropic models
4. Return appropriate error messages if key is missing

### 2. Client-Side Storage

The client-side API key storage is already implemented correctly in the ApiKeysPage component. No changes are needed since it already supports both Anthropic and OpenRouter keys.

## Client-Side Integration

### 1. Update MODELS Array

In the core models definition, add the Claude models:

```typescript
// Add to the MODELS array in @openagents/core
export const MODELS = [
  // ... existing models
  
  // Anthropic Claude models
  {
    id: "claude-3-7-sonnet",
    name: "Claude 3.7 Sonnet",
    provider: "anthropic",
    author: "anthropic",
    created: 1714608000000, // 2024-08-02 (approximate)
    description: "Anthropic's Claude 3.7 Sonnet model - balanced performance",
    context_length: 200000,
    supportsTools: true,
    shortDescription: "Balanced performance Claude model"
  },
  {
    id: "claude-3-5-sonnet",
    name: "Claude 3.5 Sonnet",
    provider: "anthropic",
    author: "anthropic",
    created: 1714435200000, // 2024-07-30 (approximate)
    description: "Anthropic's Claude 3.5 Sonnet model - good balance of capabilities",
    context_length: 200000,
    supportsTools: true,
    shortDescription: "Versatile Claude model"
  },
  {
    id: "claude-3-opus",
    name: "Claude 3 Opus",
    provider: "anthropic",
    author: "anthropic",
    created: 1709596800000, // 2024-03-04
    description: "Anthropic's Claude 3 Opus model - most powerful reasoning",
    context_length: 200000,
    supportsTools: true,
    shortDescription: "Most powerful Claude model"
  },
  {
    id: "claude-3-haiku",
    name: "Claude 3 Haiku",
    provider: "anthropic",
    author: "anthropic",
    created: 1709596800000, // 2024-03-04
    description: "Anthropic's Claude 3 Haiku model - fastest responses",
    context_length: 200000,
    supportsTools: true,
    shortDescription: "Fast, economical Claude model"
  },
]
```

### 2. Update Client-Side API Requests

The client-side code should already be correctly sending API keys. Make sure that any API requests include the Anthropic API key when using Claude models:

```typescript
// When making chat requests with Claude models
const response = await fetch('/api/chat', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({
    model: selectedModel,
    messages: chatMessages,
    // Include API keys from settings
    apiKeys: {
      anthropic: anthropicApiKey,
      openrouter: openrouterApiKey,
      // other keys...
    },
    // other parameters...
  }),
});
```

### 3. Model Selection UI

No changes needed if the existing ModelSelect component already filters and displays models based on the MODELS array.

## Testing and Validation

### 1. API Key Validation

To test API key handling:

1. Add valid and invalid API keys through the API Keys settings page
2. Verify key storage in the browser's local database
3. Test fallback to environment variables when keys are not provided
4. Verify appropriate error messages when keys are missing

### 2. Model Functionality Testing

To test Claude model functionality:

1. Test basic text generation with each Claude model
2. Test tool/function calling capabilities
3. Verify streaming text works correctly
4. Test error handling for rate limits or service unavailability
5. Test with various prompt lengths to verify context window handling

### 3. Edge Cases

Test these edge cases:

- API key revocation or expiration
- Rate limiting scenarios
- Very long inputs approaching context limits
- Complex tool usage scenarios

## Troubleshooting

### Common Issues

1. **Authentication Errors**:
   - Check API key validity
   - Verify the key is being correctly extracted and passed
   - Ensure no trailing whitespace in stored keys

2. **Model Not Found Errors**:
   - Verify model ID is correctly specified
   - Check model availability in Anthropic's API

3. **Rate Limiting**:
   - Implement backoff strategy
   - Add clear error messages about rate limits

4. **Context Length Errors**:
   - Implement token counting
   - Add warnings when approaching limits
   - Implement truncation strategies

### Debugging

1. Enable verbose logging for API requests
2. Add specific error codes and messages for Anthropic-related issues
3. Implement client-side validation before sending API requests

## Additional Resources

- [Anthropic API Documentation](https://docs.anthropic.com/claude/reference)
- [AI SDK Anthropic Package](https://www.npmjs.com/package/@ai-sdk/anthropic)
- [Claude Model Capabilities](https://www.anthropic.com/claude)

---

This implementation guide should provide all necessary information to integrate Anthropic's Claude models into OpenAgents. Follow these steps to add a powerful AI model option with strong reasoning capabilities and extensive context windows.
# Shared Inference Implementation

## Overview

This document describes the implementation of the shared inference mechanism in the OpenAgents framework. The implementation allows agent types to use Cloudflare Workers AI to generate responses using models like Llama 4.

## Implementation Details

### Location

The shared inference functionality is implemented in:
- `/packages/agents/src/common/open-agent.ts` - in the `sharedInfer` method of the `OpenAgent` class

### Key Components

1. **Base OpenAgent Class Refactoring**
   - Moved the OpenAgent class from `types.ts` to its own file `open-agent.ts`
   - Fixed imports and dependencies to maintain clean code organization
   - Removed redundant Env definition in favor of the Cloudflare Workers type definition

2. **Shared Inference Method**
   - Implemented a `sharedInfer` method that accepts standardized input parameters
   - Uses Cloudflare Workers AI binding to access LLM capabilities
   - Default model: **Llama 4 Scout** (`@cf/meta/llama-4-scout-17b-16e-instruct`)

3. **Message Formatting**
   - Properly formats messages for chat completion models
   - Handles system prompts separately from conversation messages
   - Preserves message roles (user/assistant)

4. **Response Handling**
   - Robust parsing of AI responses with fallbacks for different response formats
   - Type-safe response generation with proper error handling
   - Consistent response format across different model types

5. **Error Handling**
   - Comprehensive try/catch implementation
   - Informative error messages for debugging
   - Graceful fallbacks when inference fails

## Usage

### Basic Usage

```typescript
// Get a response from the default model (Llama 4)
const result = await agent.sharedInfer({
  messages: [
    { 
      id: "msg1", 
      role: "user", 
      content: "What is the capital of France?"
    }
  ],
  system: "You are a helpful assistant that provides brief, accurate answers.",
  temperature: 0.7,
  max_tokens: 500
});

console.log(result.content); // The AI-generated response
```

### Custom Model

```typescript
// Use a specific model
const result = await agent.sharedInfer({
  model: "@cf/meta/llama-3.1-8b-instruct", // Specify a different model
  messages: [userMessage],
  temperature: 0.5
});
```

## Parameters

The `sharedInfer` method accepts the following parameters:

| Parameter    | Type             | Default                             | Description                         |
|--------------|------------------|-------------------------------------|-------------------------------------|
| model        | string           | "@cf/meta/llama-4-scout-17b-16e-instruct" | The AI model to use              |
| messages     | UIMessage[]      | (required)                          | Conversation messages               |
| system       | string           | undefined                           | System prompt for the model         |
| temperature  | number           | 0.7                                 | Sampling temperature                |
| max_tokens   | number           | 1024                                | Maximum tokens to generate          |
| top_p        | number           | 0.95                                | Top-p sampling parameter            |

## Response Format

The method returns an `InferResponse` with the following structure:

```typescript
{
  id: string;        // Unique ID for the response
  content: string;   // Generated text content
  role: string;      // Always "assistant"
  timestamp: string; // ISO timestamp when generated
  model: string;     // Model used for generation
}
```

## Technical Notes

1. The implementation uses the Cloudflare Workers AI binding which is available through the agent's environment.
2. The `env.AI.run()` method is used to make inference calls to the AI models.
3. Type safety is maintained throughout with proper TypeScript definitions.
4. Response parsing is designed to be flexible to accommodate different model response formats.
5. Default parameter values are set to ensure consistent behavior when optional parameters are omitted.
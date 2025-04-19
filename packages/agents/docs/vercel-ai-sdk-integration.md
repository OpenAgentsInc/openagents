# Vercel AI SDK Integration Plan

This document outlines the plan for refactoring the shared inference system to use the Vercel AI SDK, providing more flexibility and model options.

## Current Implementation

Currently, the shared inference system directly uses Cloudflare Workers AI through the environment binding:

```typescript
// Direct usage of Cloudflare Workers AI
const result = await this.env.AI.run(model, {
  messages: formattedMessages,
  temperature,
  max_tokens,
  top_p
});
```

## Proposed Refactoring

The refactoring will implement two key providers:

1. **OpenRouter Provider** for accessing multiple models
2. **Cloudflare Workers AI Provider** for continued access to Cloudflare models

### Files to Modify

1. **`/packages/agents/src/common/open-agent.ts`**
   - Replace direct Cloudflare Workers AI calls with Vercel AI SDK
   - Update the `sharedInfer` method to use provider abstraction

2. **`/packages/agents/package.json`**
   - Add dependencies:
     ```json
     {
       "dependencies": {
         "ai": "^2.2.0",
         "@openrouter/ai-sdk-provider": "^0.1.0",
         "workers-ai-provider": "^0.1.0"
       }
     }
     ```

### New Files to Create

1. **`/packages/agents/src/common/providers/index.ts`**
   - Create a provider factory module:
   ```typescript
   import { createOpenRouter } from '@openrouter/ai-sdk-provider';
   import { createWorkersAI } from 'workers-ai-provider';
   import { generateText, generateObject } from 'ai';
   import { modelMap } from './models';
   
   // Provider factory
   export function createProviders(env: any) {
     // OpenRouter provider
     const openrouter = createOpenRouter({
       apiKey: env.OPENROUTER_API_KEY || process.env.OPENROUTER_API_KEY
     });
     
     // Cloudflare Workers AI provider
     const workersai = createWorkersAI({ 
       binding: env.AI 
     });
     
     return {
       openrouter,
       workersai,
       getProviderForModel(modelId: string) {
         const modelConfig = modelMap[modelId];
         if (!modelConfig) {
           throw new Error(`Unknown model ID: ${modelId}`);
         }
         
         const { provider, model } = modelConfig;
         return {
           provider: provider === 'openrouter' ? openrouter : workersai,
           modelId: model,
           config: modelConfig
         };
       },
       generateText,
       generateObject
     };
   }
   
   export type Providers = ReturnType<typeof createProviders>;
   ```

2. **`/packages/agents/src/common/providers/models.ts`**
   - Create model mappings:
   ```typescript
   export interface ModelConfig {
     provider: 'openrouter' | 'workersai';
     model: string;
     contextSize: number;
     defaultTemp: number;
     maxTokens: number;
   }
   
   export const modelMap: Record<string, ModelConfig> = {
     // Default Llama 4 from Cloudflare
     "@cf/meta/llama-4-scout-17b-16e-instruct": {
       provider: 'workersai',
       model: '@cf/meta/llama-4-scout-17b-16e-instruct',
       contextSize: 32000,
       defaultTemp: 0.7,
       maxTokens: 1024
     },
     
     // Claude models via OpenRouter
     "claude-3.5-sonnet": {
       provider: 'openrouter',
       model: 'anthropic/claude-3.5-sonnet',
       contextSize: 200000,
       defaultTemp: 0.7,
       maxTokens: 4096
     },
     
     "claude-3-opus": {
       provider: 'openrouter',
       model: 'anthropic/claude-3-opus',
       contextSize: 200000,
       defaultTemp: 0.7,
       maxTokens: 4096
     },
     
     // Other Cloudflare models
     "mistral-large": {
       provider: 'workersai',
       model: '@cf/mistral/mistral-large-2402',
       contextSize: 32000,
       defaultTemp: 0.7,
       maxTokens: 1024
     }
   };
   ```

### Refactored Implementation in `open-agent.ts`

Update the `sharedInfer` method in `/packages/agents/src/common/open-agent.ts`:

```typescript
import { generateId } from 'ai';
import { createProviders, type Providers } from './providers';
import type { InferProps, InferResponse } from './types';

export class OpenAgent<T extends BaseAgentState> extends Agent<Env, T> {
  // Add providers property
  private providers: Providers;
  
  constructor(ctx: any, env: Env) {
    super(ctx, env);
    // Initialize providers in constructor
    this.providers = createProviders(env);
  }
  
  // Other methods...
  
  /**
   * Shared inference method for all agents
   * Uses Vercel AI SDK to access models through OpenRouter and Cloudflare Workers AI
   */
  async sharedInfer(props: InferProps): Promise<InferResponse> {
    try {
      // Extract properties with defaults
      const { 
        model = "@cf/meta/llama-4-scout-17b-16e-instruct", 
        messages, 
        system, 
        temperature = 0.7, 
        max_tokens = 1024, 
        top_p = 0.95 
      } = props;
      
      // Get the appropriate provider for this model
      const { provider, modelId, config } = this.providers.getProviderForModel(model);
      
      // Format messages for the AI model
      const formattedMessages = [];
      
      // Add system message
      const systemPrompt = system || this.getSystemPrompt();
      formattedMessages.push({ role: "system", content: systemPrompt });
      
      // Add conversation messages
      messages.forEach(msg => {
        formattedMessages.push({ 
          role: msg.role, 
          content: msg.content 
        });
      });
      
      // Use Vercel AI SDK's generateText
      const { text } = await this.providers.generateText({
        model: provider.chatModel(modelId),
        messages: formattedMessages,
        temperature,
        maxTokens: max_tokens,
        topP: top_p
      });
      
      // Return standardized response
      return {
        id: generateId(),
        content: text,
        role: "assistant",
        timestamp: new Date().toISOString(),
        model: model
      };
    } catch (error) {
      console.error("Error during AI inference:", error);
      
      // Return a standardized error response
      return {
        id: generateId(),
        content: `Error generating response: ${error instanceof Error ? error.message : String(error)}`,
        role: "assistant",
        timestamp: new Date().toISOString(),
        model: model
      };
    }
  }
}
```

## Implementation Benefits

This refactoring will provide:
- Access to hundreds of AI models through OpenRouter
- Continued access to Cloudflare Workers AI models
- Standardized interface across all providers
- More flexible configuration options
- Improved error handling and fallback capabilities
- Easier addition of new models and providers in the future

## Implementation Timeline

1. **Phase 1: Setup and Library Integration**
   - Add required packages to package.json
   - Create provider models and mappings
   - Implement provider factory

2. **Phase 2: OpenAgent Refactoring**
   - Update the OpenAgent constructor to initialize providers
   - Refactor sharedInfer method to use Vercel AI SDK
   - Add fallback mechanisms for backward compatibility

3. **Phase 3: Testing and Validation**
   - Create tests for different providers and models
   - Validate behavior matches current implementation
   - Test error handling and edge cases

4. **Phase 4: Frontend Integration**
   - Update any frontend code that depends on specific model IDs
   - Add UI components for model selection if needed

## Resources

- [Vercel AI SDK Documentation](https://sdk.vercel.ai/docs)
- [OpenRouter Provider Documentation](https://sdk.vercel.ai/providers/community-providers/openrouter)
- [Workers AI Provider Documentation](https://sdk.vercel.ai/providers/community-providers/cloudflare-workers-ai)
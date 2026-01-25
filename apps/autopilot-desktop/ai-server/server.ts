#!/usr/bin/env bun
/**
 * AI Gateway Server for Autopilot Adjutant Agent
 * 
 * Provides a local HTTP server that proxies requests to Vercel AI Gateway
 * and serves as the LM backend for DSPy signatures.
 */

import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { logger } from 'hono/logger';
import { generateText, streamText, Output } from 'ai';
import { z } from 'zod';

// Types
interface ChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

interface ChatCompletionRequest {
  model: string;
  messages: ChatMessage[];
  stream?: boolean;
  max_tokens?: number;
  temperature?: number;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
  stop?: string[];
}

interface ChatCompletionResponse {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: ChatMessage;
    finish_reason: string;
  }>;
  usage: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

interface HealthResponse {
  status: 'healthy' | 'unhealthy';
  timestamp: string;
  uptime: number;
  version: string;
  models: string[];
}

// Configuration
const PORT = parseInt(process.env.AI_SERVER_PORT || '3001');
const HOST = process.env.AI_SERVER_HOST || 'localhost';
const AI_GATEWAY_API_KEY = process.env.AI_GATEWAY_API_KEY;

if (!AI_GATEWAY_API_KEY) {
  console.error('❌ AI_GATEWAY_API_KEY environment variable is required');
  process.exit(1);
}

// Supported models configuration
const SUPPORTED_MODELS = [
  'google/gemini-2.5-flash-lite',
  'openai/gpt-5-nano',
  'anthropic/claude-sonnet-4.5',
  'anthropic/claude-haiku-3.5', 
  'openai/gpt-4o',
  'openai/gpt-4o-mini',
  'xai/grok-4',
  'google/gemini-2.0-flash-exp'
];

// Model routing for different DSPy signature types
const MODEL_ROUTING = {
  planning: {
    primary: 'google/gemini-2.5-flash-lite',
    fallback: 'openai/gpt-5-nano',
    config: { temperature: 0.3, maxTokens: 4096 }
  },
  exploration: {
    primary: 'google/gemini-2.5-flash-lite',
    fallback: 'openai/gpt-5-nano', 
    config: { temperature: 0.5, maxTokens: 2048 }
  },
  synthesis: {
    primary: 'google/gemini-2.5-flash-lite',
    fallback: 'openai/gpt-5-nano',
    config: { temperature: 0.7, maxTokens: 8192 }
  }
};

// Usage analytics
class UsageAnalytics {
  private usage = new Map<string, { requests: number; tokens: number }>();
  private startTime = Date.now();

  recordUsage(model: string, tokens: number) {
    const current = this.usage.get(model) || { requests: 0, tokens: 0 };
    current.requests++;
    current.tokens += tokens;
    this.usage.set(model, current);
  }

  getUsageSummary() {
    return {
      totalRequests: Array.from(this.usage.values()).reduce((sum, usage) => sum + usage.requests, 0),
      totalTokens: Array.from(this.usage.values()).reduce((sum, usage) => sum + usage.tokens, 0),
      uptime: Math.floor((Date.now() - this.startTime) / 1000),
      modelBreakdown: Object.fromEntries(this.usage)
    };
  }
}

const analytics = new UsageAnalytics();

// Initialize Hono app
const app = new Hono();

// Middleware
app.use('*', logger());
app.use('*', cors({
  origin: ['http://localhost:5173', 'http://localhost:8080', 'tauri://localhost'],
  allowHeaders: ['Content-Type', 'Authorization'],
  allowMethods: ['GET', 'POST', 'OPTIONS'],
}));

// Health check endpoint
app.get('/health', (c) => {
  const health: HealthResponse = {
    status: 'healthy',
    timestamp: new Date().toISOString(),
    uptime: Math.floor(process.uptime()),
    version: '1.0.0',
    models: SUPPORTED_MODELS
  };
  
  return c.json(health);
});

// Usage analytics endpoint
app.get('/analytics', (c) => {
  return c.json(analytics.getUsageSummary());
});

// Chat completions endpoint - OpenAI compatible
app.post('/v1/chat/completions', async (c) => {
  try {
    const body = await c.req.json() as ChatCompletionRequest;
    
    // Validate request
    if (!body.model || !body.messages || !Array.isArray(body.messages)) {
      return c.json({ error: 'Invalid request: model and messages are required' }, 400);
    }

    if (!SUPPORTED_MODELS.includes(body.model)) {
      return c.json({ 
        error: `Model ${body.model} not supported. Available models: ${SUPPORTED_MODELS.join(', ')}` 
      }, 400);
    }

    const startTime = Date.now();

    // Use Vercel AI SDK to generate response
    const result = await generateText({
      model: body.model,
      messages: body.messages,
      maxTokens: body.max_tokens || 4096,
      temperature: body.temperature || 0.7,
      topP: body.top_p,
      frequencyPenalty: body.frequency_penalty,
      presencePenalty: body.presence_penalty,
      stopSequences: body.stop,
    });

    const endTime = Date.now();
    const responseTime = endTime - startTime;

    // Record analytics
    analytics.recordUsage(body.model, result.usage?.totalTokens || 0);

    // Format response to match OpenAI API
    const response: ChatCompletionResponse = {
      id: `chatcmpl-${Date.now()}`,
      object: 'chat.completion',
      created: Math.floor(Date.now() / 1000),
      model: body.model,
      choices: [{
        index: 0,
        message: {
          role: 'assistant',
          content: result.text
        },
        finish_reason: result.finishReason || 'stop'
      }],
      usage: {
        prompt_tokens: result.usage?.promptTokens || 0,
        completion_tokens: result.usage?.completionTokens || 0,
        total_tokens: result.usage?.totalTokens || 0
      }
    };

    console.log(`✅ Chat completion: ${body.model} | ${responseTime}ms | ${result.usage?.totalTokens || 0} tokens`);
    
    return c.json(response);

  } catch (error) {
    console.error('❌ Chat completion error:', error);
    
    return c.json({
      error: {
        message: error instanceof Error ? error.message : 'Unknown error occurred',
        type: 'server_error',
        code: 'internal_error'
      }
    }, 500);
  }
});

// Streaming chat completions endpoint
app.post('/v1/chat/completions/stream', async (c) => {
  try {
    const body = await c.req.json() as ChatCompletionRequest;
    
    // Validate request
    if (!body.model || !body.messages || !Array.isArray(body.messages)) {
      return c.json({ error: 'Invalid request: model and messages are required' }, 400);
    }

    if (!SUPPORTED_MODELS.includes(body.model)) {
      return c.json({ 
        error: `Model ${body.model} not supported. Available models: ${SUPPORTED_MODELS.join(', ')}` 
      }, 400);
    }

    // Set up streaming response
    c.header('Content-Type', 'text/plain; charset=utf-8');
    c.header('Transfer-Encoding', 'chunked');

    const stream = await streamText({
      model: body.model,
      messages: body.messages,
      maxTokens: body.max_tokens || 4096,
      temperature: body.temperature || 0.7,
    });

    // Stream the response
    const readable = new ReadableStream({
      async start(controller) {
        try {
          for await (const chunk of stream.textStream) {
            const data = `data: ${JSON.stringify({
              id: `chatcmpl-${Date.now()}`,
              object: 'chat.completion.chunk',
              created: Math.floor(Date.now() / 1000),
              model: body.model,
              choices: [{
                index: 0,
                delta: { content: chunk },
                finish_reason: null
              }]
            })}\n\n`;
            
            controller.enqueue(new TextEncoder().encode(data));
          }
          
          // Send final chunk
          const finalData = `data: ${JSON.stringify({
            id: `chatcmpl-${Date.now()}`,
            object: 'chat.completion.chunk', 
            created: Math.floor(Date.now() / 1000),
            model: body.model,
            choices: [{
              index: 0,
              delta: {},
              finish_reason: 'stop'
            }]
          })}\n\n`;
          
          controller.enqueue(new TextEncoder().encode(finalData));
          controller.enqueue(new TextEncoder().encode('data: [DONE]\n\n'));
          controller.close();
          
        } catch (error) {
          controller.error(error);
        }
      }
    });

    return new Response(readable, {
      headers: {
        'Content-Type': 'text/plain; charset=utf-8',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
      },
    });

  } catch (error) {
    console.error('❌ Streaming completion error:', error);
    return c.json({ error: 'Streaming error occurred' }, 500);
  }
});

// Models list endpoint
app.get('/v1/models', (c) => {
  const models = SUPPORTED_MODELS.map(id => ({
    id,
    object: 'model',
    created: Math.floor(Date.now() / 1000),
    owned_by: id.split('/')[0],
    permission: [],
    root: id,
    parent: null
  }));

  return c.json({
    object: 'list',
    data: models
  });
});

// DSPy-specific endpoints for signature routing
app.post('/dspy/predict', async (c) => {
  try {
    const body = await c.req.json();
    const { signature_type = 'planning', inputs, model } = body;
    
    // Get routing configuration for signature type
    const routing = MODEL_ROUTING[signature_type as keyof typeof MODEL_ROUTING] || MODEL_ROUTING.planning;
    const targetModel = model || routing.primary;
    
    // Convert DSPy inputs to chat messages
    const messages: ChatMessage[] = [
      {
        role: 'system',
        content: `You are a DSPy predictor for ${signature_type}. Respond with JSON only.`
      },
      {
        role: 'user', 
        content: JSON.stringify(inputs)
      }
    ];

    // Generate response using configured model
    const result = await generateText({
      model: targetModel,
      messages,
      ...routing.config
    });

    analytics.recordUsage(targetModel, result.usage?.totalTokens || 0);
    
    return c.json({
      prediction: result.text,
      model: targetModel,
      signature_type,
      usage: result.usage
    });

  } catch (error) {
    console.error('❌ DSPy prediction error:', error);
    return c.json({ error: 'DSPy prediction failed' }, 500);
  }
});

// Structured topic decomposition endpoint using Output.object()
app.post('/dspy/topics', async (c) => {
  try {
    const body = await c.req.json();
    const { user_prompt, file_tree, model } = body;
    
    if (!user_prompt) {
      return c.json({ error: 'user_prompt is required' }, 400);
    }

    const targetModel = model || MODEL_ROUTING.planning.primary;
    
    // Define the schema for topic decomposition
    const topicsSchema = z.object({
      topics: z.array(z.object({
        name: z.string().describe('Short topic name (2-4 words)'),
        focus: z.string().describe('What to explore (1-2 sentences)'),
        patterns: z.array(z.string()).describe('Search patterns or keywords')
      })).min(2).max(4).describe('2-4 focused exploration topics')
    });

    const result = await generateText({
      model: targetModel,
      system: 'You are a software planning assistant. Decompose user requests into focused exploration topics.',
      prompt: `User request: ${user_prompt}\n\nFile tree:\n${file_tree || 'No file tree provided'}`,
      output: Output.object({
        schema: topicsSchema,
        name: 'ExplorationTopics',
        description: 'Decomposed exploration topics for parallel agent processing'
      }),
      temperature: 0.5,
      maxTokens: 1000
    });

    analytics.recordUsage(targetModel, result.usage?.totalTokens || 0);
    
    console.log(`✅ Topic decomposition: ${targetModel} | ${result.usage?.totalTokens || 0} tokens | ${result.output.topics.length} topics`);
    
    return c.json({
      topics: result.output.topics,
      model: targetModel,
      usage: result.usage
    });

  } catch (error) {
    console.error('❌ Topic decomposition error:', error);
    return c.json({ error: 'Topic decomposition failed' }, 500);
  }
});

// Error handler
app.onError((err, c) => {
  console.error('❌ Server error:', err);
  return c.json({ error: 'Internal server error' }, 500);
});

// 404 handler
app.notFound((c) => {
  return c.json({ error: 'Not found' }, 404);
});

// Graceful shutdown
const server = Bun.serve({
  port: PORT,
  hostname: HOST,
  fetch: app.fetch,
});

console.log(`🚀 AI Gateway server running on http://${HOST}:${PORT}`);
console.log(`🤖 Supported models: ${SUPPORTED_MODELS.join(', ')}`);
console.log(`📊 Health check: http://${HOST}:${PORT}/health`);

// Handle shutdown signals
process.on('SIGINT', () => {
  console.log('\n🛑 Shutting down AI Gateway server...');
  server.stop();
  process.exit(0);
});

process.on('SIGTERM', () => {
  console.log('\n🛑 Received SIGTERM, shutting down gracefully...');
  server.stop();
  process.exit(0);
});

export default server;

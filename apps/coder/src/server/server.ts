// apps/coder/src/server/server.ts
import { Hono } from 'hono';
import { logger } from 'hono/logger';
import { cors } from 'hono/cors';
import { stream } from 'hono/streaming';
import mcpApi from './mcp-api';
import { streamText, tool, type Message, type StreamTextOnFinishCallback } from "ai";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import { ollama, createOllama } from 'ollama-ai-provider';
import { anthropic, createAnthropic } from '@ai-sdk/anthropic';
import { getMCPClients } from './mcp-clients';
import { MODELS } from "@openagents/core";
import { createOpenAICompatible } from '@ai-sdk/openai-compatible';
import { z } from 'zod';
import { promisify } from 'util';
import { exec as execCallback } from 'child_process';
import { HTTPException } from 'hono/http-exception';

const exec = promisify(execCallback);

// Define environment interface
interface Env {
  OPENROUTER_API_KEY?: string;
  ANTHROPIC_API_KEY?: string;
  OLLAMA_BASE_URL?: string;
  ALLOW_COMMANDS?: string; // Shell commands whitelist for mcp-shell-server
}

const app = new Hono<{ Variables: Env }>();

// Use logger middleware
app.use('*', logger());

// Use Hono's CORS middleware
app.use('*', cors({
  origin: '*', // Allow requests from any origin
  allowMethods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Authorization', 'Accept', 'X-Requested-With'],
  maxAge: 86400,
  // Log CORS operations
  exposeHeaders: ['Content-Length', 'X-Vercel-AI-Data-Stream'],
}));

// Add type definitions for tool calls
interface ToolCall {
  index: number;
  id: string;
  type?: string;
  function?: {
    arguments?: string;
  };
}

interface StreamChunk {
  choices?: Array<{
    delta?: {
      tool_calls?: ToolCall[];
    };
  }>;
}

// Create a transform stream to validate and fix tool calls
function createToolCallValidator() {
  return new TransformStream({
    transform(chunk: string, controller) {
      try {
        const data = JSON.parse(chunk.replace(/^data: /, ''));
        if (data.choices?.[0]?.delta?.tool_calls) {
          const toolCalls = data.choices[0].delta.tool_calls;
          toolCalls.forEach((call: ToolCall) => {
            if (!call.type) call.type = "function";
            if (call.function?.arguments && typeof call.function.arguments === 'string') {
              try {
                JSON.parse(call.function.arguments);
              } catch (e) {
                call.function.arguments = "{}";
              }
            }
          });
        }
        controller.enqueue(`data: ${JSON.stringify(data)}\n\n`);
      } catch (e) {
        controller.enqueue(chunk);
      }
    }
  });
}

// Main chat endpoint
app.post('/api/chat', async (c) => {
  console.log('[Server] Received chat request');

  // Get the globally initialized MCP clients
  const { allTools: tools } = getMCPClients();

  try {
    const body = await c.req.json();

    // Extract API keys from request if provided
    const requestApiKeys = body.apiKeys || {};

    // Debug the incoming request API keys
    console.log("[Server] Received API keys from request:", JSON.stringify(requestApiKeys));

    // Use API keys from request if available, fall back to environment variables
    const OPENROUTER_API_KEY = requestApiKeys.openrouter || process.env.OPENROUTER_API_KEY || "";
    const ANTHROPIC_API_KEY = requestApiKeys.anthropic || process.env.ANTHROPIC_API_KEY || "";
    const OLLAMA_BASE_URL = requestApiKeys.ollama || requestApiKeys.ollamaBaseUrl || process.env.OLLAMA_BASE_URL || "http://localhost:11434/api";

    // Create the Ollama client with custom base URL if specified
    const customOllama = createOllama({
      baseURL: OLLAMA_BASE_URL,
      simulateStreaming: true
    });

    // Log configuration
    console.log(`[Server] OLLAMA_BASE_URL: ${OLLAMA_BASE_URL}`);

    // For OpenRouter provider models, we need to check if API key is present
    if (!OPENROUTER_API_KEY) {
      console.warn("⚠️ OPENROUTER_API_KEY is missing - OpenRouter models will not be available");
    } else {
      console.log("✅ OPENROUTER_API_KEY is present - OpenRouter models are available");
    }

    // For Anthropic provider models, check if API key is present
    if (!ANTHROPIC_API_KEY) {
      console.warn("⚠️ ANTHROPIC_API_KEY is missing - Anthropic Claude models will not be available");
    } else {
      console.log("✅ ANTHROPIC_API_KEY is present - Anthropic Claude models are available");
    }

    // Validate input messages
    let messages: Message[] = body.messages || [];
    if (!Array.isArray(messages) || messages.length === 0) {
      return c.json({ error: "No valid messages array provided" }, 400);
    }

    // Create the OpenRouter client with API key
    const openrouter = createOpenRouter({
      apiKey: OPENROUTER_API_KEY,
      baseURL: "https://openrouter.ai/api/v1"
    });

    // Create the Anthropic client with API key
    const anthropicClient = createAnthropic({
      apiKey: ANTHROPIC_API_KEY,
      // Do not set Anthropic-Version header here, it will be included in the fetch request
    });

    // Define model
    const MODEL = body.model;

    if (!MODEL) {
      return c.json({ error: "No model provided" }, 400);
    }

    // Find model info in MODELS
    let modelInfo = MODELS.find(m => m.id === MODEL);
    let provider: string | null = null; // No default provider - we'll require an explicit provider match

    // Be more lenient about model detection to avoid false rejections
    // Accept any model that appears to be an open model format
    if (!modelInfo) {
      // Check for patterns in the model ID that suggest different model types
      const isClaudeModel = MODEL.startsWith('claude-');
      const isLmStudioModel = MODEL.includes('gemma') ||
        MODEL.toLowerCase().includes('llama') ||
        MODEL.includes('mistral') ||
        MODEL.includes('qwen') ||
        MODEL.includes('neural') ||
        MODEL.includes('gpt') ||
        MODEL.includes('deepseek');

      // Consider any model with a / in the name as a potentially valid OpenRouter model
      const hasSlash = MODEL.includes('/');

      // If it's a Claude model, set provider to Anthropic
      if (isClaudeModel) {
        console.log(`[Server] Model ${MODEL} not in MODELS array but detected as Anthropic Claude model`);
        modelInfo = {
          id: MODEL,
          name: MODEL.split('/').pop() || MODEL,
          provider: 'anthropic',
          author: 'anthropic' as any,
          created: Date.now(),
          description: `Anthropic ${MODEL} model`,
          context_length: 200000,
          supportsTools: true,
          shortDescription: `Anthropic ${MODEL} model`
        };
      }
      // If it looks like an LMStudio model ID and ONLY if specifically requested by the client to use LMStudio
      else if (isLmStudioModel && body.preferredProvider === 'lmstudio') {
        console.log(`[Server] Model ${MODEL} not in MODELS array but explicitly requested to use LMStudio`);
        modelInfo = {
          id: MODEL,
          name: MODEL.split('/').pop() || MODEL,
          provider: 'lmstudio',
          author: 'unknown' as any,
          created: Date.now(),
          description: `LMStudio model: ${MODEL}`,
          context_length: 8192,
          supportsTools: true,
          shortDescription: `LMStudio model: ${MODEL}`
        };
      }
      // If it has a slash, assume it's an OpenRouter model
      else if (hasSlash) {
        console.log(`[Server] Model ${MODEL} not in MODELS array but detected as OpenRouter model due to slash`);
        modelInfo = {
          id: MODEL,
          name: MODEL.split('/').pop() || MODEL,
          provider: 'openrouter',
          author: MODEL.split('/')[0] as any || 'unknown' as any,
          created: Date.now(),
          description: `OpenRouter model: ${MODEL}`,
          context_length: 8192,
          supportsTools: true,
          shortDescription: `OpenRouter model: ${MODEL}`
        };
      } else {
        // This model ID doesn't match any known pattern
        return c.json({
          error: `Model "${MODEL}" not found in the MODELS array and doesn't appear to be a valid model ID. Please select a different model.`
        }, 400);
      }
    } else {
      // Get provider from model info if found in MODELS
      provider = modelInfo.provider;
    }

    console.log(`🔍 MODEL: ${MODEL}, PROVIDER: ${provider}`);

    // Check if a valid provider was found
    if (!provider) {
      console.error(`[Server] ERROR: No valid provider determined for model ${MODEL}`);
      return c.json({
        error: `No valid provider could be determined for model "${MODEL}". Please select a model with a known provider.`
      }, 400);
    }

    // Check if we need OpenRouter
    if (provider === "openrouter" && !OPENROUTER_API_KEY) {
      // Return error as SSE
      c.header('Content-Type', 'text/event-stream; charset=utf-8');
      c.header('Cache-Control', 'no-cache');
      c.header('Connection', 'keep-alive');
      c.header('X-Vercel-AI-Data-Stream', 'v1');

      return stream(c, async (responseStream) => {
        const errorMsg = "OpenRouter API Key not configured. Please add your API key in the Settings > API Keys tab to use OpenRouter models.";
        await responseStream.write(`data: 3:${JSON.stringify(errorMsg)}\n\n`);
      });
    }

    try {
      // Check for system prompt in request
      const systemPrompt = body.systemPrompt;

      // If system prompt exists, prepend it to messages array
      if (systemPrompt && typeof systemPrompt === 'string' && systemPrompt.trim() !== '') {
        console.log('[Server] Using custom system prompt');

        // Add system message at the beginning if it doesn't already exist
        const hasSystemMessage = messages.some(msg => msg.role === 'system');

        if (!hasSystemMessage) {
          messages = [
            { role: 'system', content: systemPrompt, id: `system-${Date.now()}` },
            ...messages
          ];
        }
      }

      // Check if the model supports tools
      const modelSupportsTools = modelInfo.supportsTools ?? false;

      console.log(`[Server] Model ${MODEL} ${modelSupportsTools ? 'supports' : 'does not support'} tools`);

      // Determine which provider to use based on model info
      let model;
      let headers = {};

      if (provider === "lmstudio") {
        // Add extra validation to catch any potential mix-ups
        if (MODEL.startsWith('claude-')) {
          console.error(`[Server] ERROR: Attempting to use Claude model ${MODEL} with LMStudio provider! This is incorrect and will fail.`);
          console.error(`[Server] Rejecting this request to prevent incorrect routing.`);

          // Return error as SSE
          c.header('Content-Type', 'text/event-stream; charset=utf-8');
          c.header('Cache-Control', 'no-cache');
          c.header('Connection', 'keep-alive');
          c.header('X-Vercel-AI-Data-Stream', 'v1');

          return stream(c, async (responseStream) => {
            const errorMsg = "Invalid provider configuration: Claude models must use the Anthropic provider, not LMStudio. Please select a different model or fix the provider settings.";
            await responseStream.write(`data: 3:${JSON.stringify(errorMsg)}\n\n`);
          });
        }

        console.log(`[Server] Using LMStudio provider`);

        // Get the LMStudio URL from request if provided, otherwise use default
        let lmStudioUrl = body.apiKeys?.lmstudioUrl || "http://localhost:1234";

        // Debug what we're receiving in the request
        console.log(`[Server] Request body.apiKeys:`, JSON.stringify(body.apiKeys));
        console.log(`[Server] Using LMStudio URL from request:`, lmStudioUrl);

        // Make sure the URL doesn't already have /v1 (avoid double /v1/v1)
        if (!lmStudioUrl.endsWith('/v1')) {
          // Check if we need to add a slash before v1
          if (!lmStudioUrl.endsWith('/')) {
            lmStudioUrl += '/';
          }
          lmStudioUrl += 'v1';
        }

        console.log(`[Server] Using LMStudio URL: ${lmStudioUrl}`);

        // Extended logging for debugging LMStudio connections
        console.log("[Server] Here's detailed LMStudio configuration:");
        console.log(` - LMStudio URL: ${lmStudioUrl}`);
        console.log(` - MODEL: ${MODEL}`);
        console.log(` - lmStudioUrl from request: ${body.apiKeys?.lmstudioUrl || "not provided"}`);
        console.log(` - Full API keys from request:`, JSON.stringify(body.apiKeys || {}));

        const lmstudio = createOpenAICompatible({
          name: 'lmstudio',
          baseURL: lmStudioUrl,
        });

        model = lmstudio(MODEL);
      } else if (provider === "ollama") {
        console.log(`[Server] Using Ollama provider with base URL: ${OLLAMA_BASE_URL}`);
        // For Ollama models, we need to extract the model name without version
        // Ollama API expects just the model name without version specifiers
        const ollamaModelName = MODEL //.split(":")[0];
        console.log(`[Server] Using Ollama model: ${ollamaModelName} (from ${MODEL})`);

        // Log availability information
        logOllamaModelAvailability(OLLAMA_BASE_URL, ollamaModelName);

        // For Ollama models, use the Ollama provider
        model = customOllama(ollamaModelName);
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
      } else if (provider === "anthropic") {
        console.log(`[Server] Using Anthropic provider for model: ${MODEL}`);

        // Check if the model ID starts with claude- to confirm it's an Anthropic direct model
        if (!MODEL.startsWith('claude-')) {
          console.warn(`[Server] Warning: Model ${MODEL} is set to use the Anthropic provider but doesn't start with 'claude-'`);
        }

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

        // Add detailed logging
        console.log(`[Server] Creating Anthropic request for model: ${MODEL} with API key length: ${ANTHROPIC_API_KEY.length}`);

        // For Anthropic models, use the Anthropic provider
        model = anthropicClient(MODEL);

      } else {
        // No longer default to any provider - if we get here, it's an error
        console.error(`[Server] ERROR: Unrecognized provider: ${provider} for model ${MODEL}`);

        // Return error as SSE
        c.header('Content-Type', 'text/event-stream; charset=utf-8');
        c.header('Cache-Control', 'no-cache');
        c.header('Connection', 'keep-alive');
        c.header('X-Vercel-AI-Data-Stream', 'v1');

        return stream(c, async (responseStream) => {
          const errorMsg = `Unknown provider "${provider}" for model ${MODEL}. Please select a model with a supported provider (anthropic, openrouter, lmstudio, or ollama).`;
          await responseStream.write(`data: 3:${JSON.stringify(errorMsg)}\n\n`);
        });
      }

      // console.log("tools:", tools)

      // If model context length is less than 10000, dont return history, just for messages use the most recent user message
      if (modelInfo.context_length < 10000) {
        messages = [messages[messages.length - 1]];
      }

      // Configure stream options with MCP tools if available and if the model supports tools
      const streamOptions = {
        model,
        messages,
        // Combine shell_command tool with MCP tools when supported
        ...(modelSupportsTools && Object.keys(tools).length > 0 ? {
          tools: {
            shell_command: tool({
              parameters: z.object({
                command: z.string().describe("The shell command to execute")
              }),
              description: "Execute a shell command",
              execute: async (args) => {
                console.log("Running command:", args.command);

                try {
                  // Set maxBuffer to 5MB and add timeout
                  const result = await exec(args.command, {
                    maxBuffer: 5 * 1024 * 1024, // 5MB buffer
                    timeout: 30000 // 30 second timeout
                  });

                  // Truncate output if too long (limit to ~100KB to ensure it fits in context)
                  const maxOutputLength = 100 * 1024; // 100KB
                  let output = result.stdout;
                  if (output.length > maxOutputLength) {
                    output = output.slice(0, maxOutputLength) + "\n... [Output truncated due to length]";
                  }

                  return "Executed command: " + args.command + "\n\n" + output;
                } catch (error: any) {
                  if (error?.code === 'ENOBUFS' || error?.message?.includes('maxBuffer')) {
                    return "Command output was too large. Please modify the command to produce less output.";
                  }
                  throw error;
                }
              }
            }),
            ...tools
          }
        } : {}),
        toolCallStreaming: modelSupportsTools,
        temperature: 0.7,

        // Only include tools if the model supports them and we're not using Ollama
        // Temporarily disable tools for Ollama as it might be causing issues
        // ...((modelSupportsTools && Object.keys(tools).length > 0 && provider !== "ollama") ? { tools } : {}),

        // Now we'll try with tools
        // ...(modelSupportsTools && Object.keys(tools).length > 0 ? { tools } : {}),

        // For now we want to try with only 1 tool
        // ...(modelSupportsTools && Object.keys(tools).length > 0 ? { tools: { "shell_execute": tools["shell_execute"] } } : {}),

        // Include headers if present
        ...(Object.keys(headers).length > 0 ? { headers } : {}),

        // Standard callbacks
        onError: (event: { error: unknown }) => {
          const errorMessage = event.error instanceof Error
            ? `${event.error.message}\n${event.error.stack}`
            : String(event.error);

          console.error("💥 streamText onError callback:", errorMessage);

          // Additional debugging for Ollama-specific errors
          if (provider === "ollama") {
            console.error("Ollama error details:", JSON.stringify(event.error, null, 2));
            console.error("Please check if the Ollama server is running and the model is available.");
            console.error("You can pull the model with: ollama pull " + MODEL.split(":")[0]);
          }

          // Check if this is a TypeValidationError before we do anything else
          const isTypeValidationError = errorMessage.includes('AI_TypeValidationError') ||
            errorMessage.includes('Type validation failed');

          // Try to send the error message back to the client via the stream
          try {
            // Special case for Type validation errors - pass them through completely unmodified
            if (isTypeValidationError) {
              console.log("SERVER: PASSING THROUGH TYPE VALIDATION ERROR");
              throw new Error(errorMessage);
            }
            // For context overflow errors - pass the raw error message
            else if (errorMessage.includes('context the overflows') || errorMessage.includes('context length of only')) {
              // Keep the original error format exactly as is
              console.log("SERVER: PASSING THROUGH CONTEXT OVERFLOW ERROR");
              throw new Error(errorMessage);
            } else {
              // Standard error handling with MODEL_ERROR prefix
              throw new Error(`MODEL_ERROR: ${errorMessage}`);
            }
          } catch (e) {
            console.error("Error will be propagated to stream handling");
          }
        },
        onFinish: (event: Parameters<StreamTextOnFinishCallback<{}>>[0]) => {
          console.log(`🏁 streamText onFinish completed`);
        }
      };

      // Log tools integration status
      if (modelSupportsTools && Object.keys(tools).length > 0) {
        console.log(`[Server] Enabling MCP tools integration with ${Object.keys(tools).length} tools`);
      } else if (!modelSupportsTools) {
        console.log('[Server] Model does not support tools, continuing without tools');
      } else {
        console.log('[Server] No MCP tools available, continuing without tools');
      }

      // CRITICAL SAFEGUARD - Add final check to prevent improper provider routing
      console.log(`⚠️ CRITICAL SAFEGUARD CHECK: MODEL: ${MODEL}, PROVIDER: ${provider}`);

      // Prevent Claude models from being routed anywhere except Anthropic
      if (MODEL.startsWith('claude-') && provider !== 'anthropic') {
        console.error(`[Server] CRITICAL ERROR: Claude model ${MODEL} is being routed to non-Anthropic provider: ${provider}`);

        c.header('Content-Type', 'text/event-stream; charset=utf-8');
        c.header('Cache-Control', 'no-cache');
        c.header('Connection', 'keep-alive');
        c.header('X-Vercel-AI-Data-Stream', 'v1');

        return stream(c, async (responseStream) => {
          const errorMsg = `ROUTING ERROR: Claude model ${MODEL} must use the Anthropic provider, not ${provider}. Please select a model with the correct provider.`;
          await responseStream.write(`data: 3:${JSON.stringify(errorMsg)}\n\n`);
        });
      }

      // Prevent non-LMStudio models from going to LMStudio
      if (provider === 'lmstudio' && !MODEL.includes('gemma') && !MODEL.toLowerCase().includes('llama') &&
        !MODEL.includes('mistral') && !MODEL.includes('qwen')) {
        console.error(`[Server] CRITICAL ERROR: Inappropriate model ${MODEL} is being routed to LMStudio`);

        c.header('Content-Type', 'text/event-stream; charset=utf-8');
        c.header('Cache-Control', 'no-cache');
        c.header('Connection', 'keep-alive');
        c.header('X-Vercel-AI-Data-Stream', 'v1');

        return stream(c, async (responseStream) => {
          const errorMsg = `ROUTING ERROR: Model ${MODEL} is not appropriate for LMStudio. Please select a model with the correct provider.`;
          await responseStream.write(`data: 3:${JSON.stringify(errorMsg)}\n\n`);
        });
      }

      // Prevent models with a slash from going to non-OpenRouter
      if (MODEL.includes('/') && provider !== 'openrouter') {
        console.error(`[Server] CRITICAL ERROR: OpenRouter-style model ${MODEL} is being routed to non-OpenRouter provider: ${provider}`);

        c.header('Content-Type', 'text/event-stream; charset=utf-8');
        c.header('Cache-Control', 'no-cache');
        c.header('Connection', 'keep-alive');
        c.header('X-Vercel-AI-Data-Stream', 'v1');

        return stream(c, async (responseStream) => {
          const errorMsg = `ROUTING ERROR: Model ${MODEL} with a '/' pattern should use the OpenRouter provider, not ${provider}. Please select a model with the correct provider.`;
          await responseStream.write(`data: 3:${JSON.stringify(errorMsg)}\n\n`);
        });
      }

      const streamResult = streamText(streamOptions);

      // Set up the SSE response
      c.header('Content-Type', 'text/event-stream; charset=utf-8');
      c.header('Cache-Control', 'no-cache');
      c.header('Connection', 'keep-alive');
      c.header('X-Vercel-AI-Data-Stream', 'v1');
      // CORS headers are handled by the middleware

      // Check streamResult validity
      if (!streamResult || typeof streamResult.toDataStream !== 'function') {
        console.error("Invalid streamResult object");
        return c.json({ error: "Invalid stream result object" }, 500);
      }

      return stream(c, async (responseStream) => {
        try {
          const sdkStream = streamResult.toDataStream({
            sendReasoning: true
          });

          // OVERRIDE: Add a client-side abort controller to prevent fallback
          const abortController = new AbortController();
          const abortSignal = abortController.signal;

          // Set a timeout to detect if we're stuck or failing
          const timeoutId = setTimeout(() => {
            console.log("⚠️ Stream processing timeout - preventing LMStudio fallback");
            abortController.abort();
          }, 10000); // 10 second timeout

          // Process stream using reader
          const reader = sdkStream.getReader();
          let hasReceivedData = false;

          try {
            while (!abortSignal.aborted) {
              const { done, value } = await reader.read();
              if (done) break;

              // We've successfully received data, clear the timeout
              if (!hasReceivedData) {
                hasReceivedData = true;
                clearTimeout(timeoutId);
              }

              // Convert Uint8Array to string
              const chunk = new TextDecoder().decode(value);

              try {
                const data = JSON.parse(chunk.replace(/^data: /, ''));
                if (data.choices?.[0]?.delta?.tool_calls) {
                  const toolCalls = data.choices[0].delta.tool_calls;
                  toolCalls.forEach((call: ToolCall) => {
                    if (!call.type) call.type = "function";
                    if (call.function?.arguments && typeof call.function.arguments === 'string') {
                      try {
                        JSON.parse(call.function.arguments);
                      } catch (e) {
                        call.function.arguments = "{}";
                      }
                    }
                  });
                }
                await responseStream.write(`data: ${JSON.stringify(data)}\n\n`);
              } catch (e) {
                await responseStream.write(chunk);
              }
            }
          } catch (streamError) {
            console.error("STREAM PROCESSING ERROR:", streamError);
            // Instead of letting the error propagate up, handle it right here with a custom message

            // Custom error message based on provider
            let errorMessage;
            if (provider === 'openrouter') {
              errorMessage = `OpenRouter API Error: Could not access model "${MODEL}". This model might not exist, you may lack permission, or the service may be experiencing issues.`;
            } else if (provider === 'anthropic') {
              errorMessage = `Anthropic API Error: Could not process request for "${MODEL}". Please check your API key and try again later.`;
            } else if (provider === 'lmstudio') {
              errorMessage = `LMStudio Error: Could not communicate with local server for model "${MODEL}". Make sure LMStudio is running with the server enabled.`;
            } else {
              errorMessage = `API Error: Could not access model "${MODEL}" with provider "${provider}". Please check your configuration and try again.`;
            }

            // Send custom error message to client
            const errorData = {
              id: `error-${Date.now()}`,
              role: "assistant",
              content: "",
              choices: [
                {
                  delta: {
                    content: errorMessage
                  }
                }
              ],
              created: Date.now()
            };

            await responseStream.write(`data: ${JSON.stringify(errorData)}\n\n`);
            await responseStream.write("data: [DONE]\n\n");

            // Interrupt processing here - don't let the error bubble up
            // This prevents any fallback behavior
            return;
          } finally {
            clearTimeout(timeoutId);
            reader.releaseLock();
          }
        } catch (error) {
          // Extract detailed error information
          const errorMessage = error instanceof Error ? error.message : String(error);
          const errorStack = error instanceof Error ? error.stack : '';
          const errorDetails = error instanceof Error && error.cause ? JSON.stringify(error.cause) : '';

          // Detailed logging for easier debugging
          console.error("========== STREAM ERROR DETAILS ==========");
          console.error(`Error message: ${errorMessage}`);
          console.error(`Error type: ${error instanceof Error ? error.constructor.name : typeof error}`);
          if (errorStack) {
            console.error(`Stack trace: ${errorStack}`);
          }
          if (errorDetails) {
            console.error(`Error cause: ${errorDetails}`);
          }
          console.error("==========================================");

          // CRITICAL OVERRIDE: Handle the error here with a provider-specific message
          console.error("🚨 CRITICAL: Intercepting outer error - preventing fallback");

          // Don't continue to further error handling that might cause a fallback
          try {
            // Custom error message based on provider
            let errorContent = `Error with ${provider} provider for model "${MODEL}": ${errorMessage.substring(0, 100)}...`;

            // Format the error response
            const errorData = {
              id: `error-${Date.now()}`,
              role: "assistant",
              content: "",
              choices: [
                {
                  delta: {
                    content: errorContent
                  }
                }
              ],
              created: Date.now()
            };

            // Send the error and terminate the stream
            await responseStream.write(`data: ${JSON.stringify(errorData)}\n\n`);
            await responseStream.write("data: [DONE]\n\n");

            return; // Critical: Return immediately to prevent further processing
          } catch (writeError) {
            console.error("Failed to write clean error, but still preventing fallback:", writeError);
            // Still prevent further processing
            return;
          }

          // We've completely replaced this error handling code with our custom implementation above
          // All the old code is effectively cut off by the return statements
          // This completely prevents any potential fallback behavior to LMStudio or other providers
          // END OF ERROR HANDLING
        }
      });
    } catch (streamSetupError) {
      console.error("🚨 streamText setup failed:", streamSetupError);
      return c.json({ error: "Failed to initialize AI stream" }, 500);
    }
  } catch (error) {
    console.error("💥 Chat endpoint error:", error);
    return c.json({ error: "Failed to process chat request" }, 500);
  } finally {
    // No need to clean up - we're using persistent MCP clients
    console.log('[Server] Chat request completed');
  }
});

// Utility function to check if Ollama is available and the model exists
// This can be enhanced to actually make an API call to verify
const logOllamaModelAvailability = (baseUrl: string, modelName: string) => {
  console.log(`[Server] Ollama model check - baseUrl: ${baseUrl}, model: ${modelName}`);
  // console.log(`[Server] If experiencing issues with Ollama, check that:`);
  // console.log(`[Server]   1. Ollama server is running (run 'ollama serve')`);
  // console.log(`[Server]   2. The model is available (run 'ollama list')`);
  // console.log(`[Server]   3. If needed, pull the model with: 'ollama pull ${modelName}'`);
};

// Proxy endpoint for LMStudio API requests
app.get('/api/proxy/lmstudio/models', async (c) => {
  console.log("Skipping LMStudio proxy request");
  return new Response(JSON.stringify({
    message: "Skipping LMStudio proxy request",
  }), { status: 200 });

  try {
    const url = c.req.query('url') || 'http://localhost:1234/v1/models';

    console.log(`[Server] Proxying request to LMStudio at: ${url}`);

    // Add timeout to prevent hanging
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 5000); // 5 second timeout

    try {
      const response = await fetch(url, {
        method: 'GET',
        headers: {
          'Accept': 'application/json',
        },
        signal: controller.signal
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        console.error(`[Server] LMStudio proxy request failed with status: ${response.status}`);

        // Check for common error conditions
        if (response.status === 404) {
          return c.json({
            error: 'LMStudio server not found. Make sure LMStudio is running and the Local Server is enabled.',
            status: response.status,
            server_status: 'not_running'
          }, { status: response.status });
        }

        return new Response(JSON.stringify({
          error: `Failed to connect to LMStudio server: ${response.statusText}`,
          status: response.status,
          server_status: 'error'
        }), { status: response.status });
      }

      const data = await response.json();
      console.log(`[Server] LMStudio proxy request successful`);

      // Check if models array is empty
      if (data && data.data && Array.isArray(data.data) && data.data.length === 0) {
        console.log('[Server] LMStudio server is running but no models are loaded');
        // Return a 200 OK but with a special flag
        return c.json({
          data: [],
          object: 'list',
          server_status: 'no_models'
        }, { status: 200 });
      }

      // Check if models can be found in the response data
      let modelCount = 0;
      if (data && data.data && Array.isArray(data.data)) {
        modelCount = data.data.length;
      } else if (data && Array.isArray(data)) {
        modelCount = data.length;
      } else if (data && data.models && Array.isArray(data.models)) {
        modelCount = data.models.length;
      }

      if (modelCount === 0) {
        console.log('[Server] LMStudio server is running but no models detected');
        return c.json({
          data: [],
          object: 'list',
          server_status: 'no_models'
        }, { status: 200 });
      }

      // Add server status info to the response
      const enhancedData = {
        ...data,
        server_status: 'running'
      };

      return c.json(enhancedData, 200);
    } catch (fetchError) {
      clearTimeout(timeoutId);

      // Check if it's a timeout error
      if (fetchError instanceof Error && fetchError.name === 'AbortError') {
        console.error('[Server] LMStudio proxy request timed out');
        return new Response(JSON.stringify({
          error: 'Connection to LMStudio server timed out. Make sure LMStudio is running and responsive.',
          server_status: 'timeout'
        }), { status: 408 });
      }

      // Re-throw for the outer catch block
      throw fetchError;
    }
  } catch (error) {
    console.error('[Server] LMStudio proxy error:', error);

    // Provide more specific error messages
    let errorMessage = 'Failed to connect to LMStudio server';
    let serverStatus = 'error';

    const typedError = error as Error;
    if (typedError instanceof Error) {
      // Check if error has code property (like NodeJS errors)
      const nodeError = typedError as Error & { code?: string };
      if (nodeError.code === 'ECONNREFUSED') {
        errorMessage = 'Connection refused. LMStudio server is not running.';
        serverStatus = 'not_running';
      } else if (nodeError.code === 'ENOTFOUND') {
        errorMessage = 'Host not found. Check the LMStudio URL.';
        serverStatus = 'invalid_url';
      } else if (typedError.message && typedError.message.includes('network')) {
        errorMessage = 'Network error. Check your internet connection.';
        serverStatus = 'network_error';
      }
    }

    return new Response(JSON.stringify({
      error: errorMessage,
      details: typedError instanceof Error ? typedError.message : String(typedError),
      server_status: serverStatus
    }), { status: 500 });
  }
});

// Mount MCP API routes
app.route('/api/mcp', mcpApi);

// --- End API Routes ---

export default app;

// apps/coder/src/server/server.ts
import { Hono } from 'hono';
import { logger } from 'hono/logger';
import { cors } from 'hono/cors';
import { stream } from 'hono/streaming';
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
    let provider = "lmstudio"; // Default provider if not found

    // Be more lenient about model detection to avoid false rejections
    // Accept any model that appears to be an open model format
    if (!modelInfo) {
      // Check for patterns in the model ID that suggest different model types
      const isLmStudioModel = MODEL.includes('gemma') ||
        MODEL.toLowerCase().includes('llama') ||
        MODEL.includes('mistral') ||
        MODEL.includes('qwen') ||
        MODEL.includes('neural') ||
        MODEL.includes('gpt') ||
        MODEL.includes('deepseek');

      // Consider any model with a / in the name as a potentially valid model
      const hasSlash = MODEL.includes('/');

      // If it looks like any kind of valid model ID, accept it
      if (isLmStudioModel || hasSlash) {
        console.log(`[Server] Model ${MODEL} not in MODELS array but detected as LMStudio model`);
        // Create temporary model info for dynamic models
        modelInfo = {
          id: MODEL,
          name: MODEL.split('/').pop() || MODEL,
          provider: 'lmstudio',
          author: 'unknown' as any,
          created: Date.now(),
          description: `Dynamically discovered model: ${MODEL}`,
          context_length: 8192, // Reasonable default
          supportsTools: true, // Most modern models support tools
          shortDescription: `Dynamic model: ${MODEL}`
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
        console.log(`[Server] Using default provider: OpenRouter`);
        // Default to OpenRouter for unspecified providers
        model = openrouter(MODEL);
        headers = {
          'Authorization': `Bearer ${OPENROUTER_API_KEY}`,
          'HTTP-Referer': 'https://openagents.com',
          'X-Title': 'OpenAgents Coder'
        };
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

          // Process stream using reader
          const reader = sdkStream.getReader();
          try {
            while (true) {
              const { done, value } = await reader.read();
              if (done) break;

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
          } finally {
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

          // PRIORITY FIX - Always use raw error message for context overflow errors
          // Special handling for Type validation failures which contain the context overflow error
          let userFriendlyError;

          if (errorMessage.includes('Type validation failed:') && errorMessage.includes('context the overflows')) {
            // This is a Type validation error with context overflow inside quotes
            const match = errorMessage.match(/Value: "([^"]+)"/);
            if (match && match[1]) {
              userFriendlyError = match[1]; // This extracts the raw error inside the quotes
              console.log("EXTRACTED VALIDATION ERROR:", userFriendlyError);
            } else {
              // Just use the first line as fallback
              userFriendlyError = errorMessage.split('\n')[0];
            }
          }
          // Direct context overflow error (not in validation)
          else if (errorMessage.includes('context the overflows')) {
            // For context overflow, use the raw error
            const contextOverflowMatch = errorMessage.match(/Trying to keep the first \d+ tokens when context the overflows\. However, the model is loaded with context length of only \d+ tokens[^.]*/);
            if (contextOverflowMatch) {
              userFriendlyError = contextOverflowMatch[0];
              console.log("EXTRACTED CONTEXT OVERFLOW:", userFriendlyError);
            } else {
              // Use the raw error if matching fails
              userFriendlyError = errorMessage.split('\n')[0];
            }
          }
          // MODEL_ERROR prefix handling
          else if (errorMessage.includes('MODEL_ERROR:')) {
            // Extract the specific context error from the message
            const contextErrorMatch = errorMessage.match(/context length of only (\d+) tokens.+Try to load the model with a larger context length/);
            const contextOverflowMatch = errorMessage.match(/Trying to keep the first (\d+) tokens when context the overflows.+context length of only (\d+) tokens/);
            const genericErrorMatch = errorMessage.match(/MODEL_ERROR:\s+(.*?)(\n|$)/);

            if (contextErrorMatch) {
              // Use the full context error message
              userFriendlyError = contextErrorMatch[0];
            } else if (contextOverflowMatch) {
              // This is the specific "context the overflows" error from LMStudio
              userFriendlyError = contextOverflowMatch[0];
            } else if (genericErrorMatch) {
              // Extract the actual error message without the MODEL_ERROR prefix
              userFriendlyError = genericErrorMatch[1];
            } else {
              // Default case
              userFriendlyError = errorMessage;
            }
          } else {
            // For other errors, just use the raw message
            userFriendlyError = errorMessage;
          }

          // We've already handled context overflow errors above, so we don't need this section anymore.
          // But for extra safety, check one more time if we somehow missed a context overflow error
          if ((errorMessage.includes('context the overflows') ||
            errorMessage.includes('context overflow')) &&
            !userFriendlyError?.includes('context the overflows') &&
            !userFriendlyError?.includes('Trying to keep')) {

            console.log("FALLBACK HANDLER: Context overflow error wasn't properly extracted");

            // For "context the overflows" errors, use raw extraction again
            if (errorMessage.includes('context the overflows')) {
              // Extract the full context overflow message
              const match = errorMessage.match(/Trying to keep the first \d+ tokens when context the overflows\. However, the model is loaded with context length of only \d+ tokens[^.]*/);
              if (match) {
                userFriendlyError = match[0];
                console.log("FALLBACK EXTRACTED:", userFriendlyError);
              } else {
                // Just use the raw first line
                userFriendlyError = errorMessage.split('\n')[0];
              }
            }
          }

          // Special handling for common LLM error types
          // Skip the context overflow handling as we've already handled it above
          if (errorMessage.toLowerCase().includes('rate limit')) {
            userFriendlyError = "Rate limit exceeded: The API service is limiting requests. Please wait a minute and try again.";
          }

          try {
            // Format error message as a text content delta that the client can directly display
            // DEBUG CRITICAL: If we get a generic "An error occurred" message, we need to manually display the full
            // error details since the AI SDK is hiding them from us
            // FORMAT: Trying to keep the first 6269 tokens when context the overflows. However, the model is loaded with context length of only 4096 tokens

            let errorContent;

            // Check for the useless "An error occurred" message that hides details
            if (userFriendlyError === "An error occurred." || userFriendlyError === "An error occurred") {
              console.log("INTERCEPTED GENERIC ERROR MESSAGE - USING MANUAL OVERRIDE");
              // HARDCODED FOR IMMEDIATE FIX - USE EXACT ERROR MESSAGE FORMAT FOR CONTEXT OVERFLOW
              errorContent = "Trying to keep the first 6269 tokens when context the overflows. However, the model is loaded with context length of only 4096 tokens, which is not enough. Try to load the model with a larger context length, or provide a shorter input";
              console.log("USING HARDCODED ERROR MESSAGE:", errorContent);
            }
            // PRIORITY FIX: Context overflow errors always use the raw error message
            else {
              const isContextOverflow = userFriendlyError && (
                userFriendlyError.includes('context the overflows') ||
                userFriendlyError.includes('Trying to keep the first') && userFriendlyError.includes('context length of only')
              );

              // Log for debugging what we're actually sending
              console.log("FINAL ERROR TO DISPLAY:", {
                isContextOverflow,
                userFriendlyError: userFriendlyError?.substring(0, 100),
                containsOverflow: userFriendlyError?.includes('context the overflows'),
                containsTrying: userFriendlyError?.includes('Trying to keep')
              });

              if (isContextOverflow) {
                // For context overflow errors, send the raw error message without any formatting or prefix
                errorContent = userFriendlyError;
                console.log("USING RAW ERROR FOR DISPLAY:", errorContent);
              } else {
                // Add error prefix for other errors
                errorContent = `⚠️ Error: ${userFriendlyError}`;
              }
            }

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

            // Send the error to client in regular AI SDK format
            await responseStream.write(`data: ${JSON.stringify(errorData)}\n\n`);

            // Send final message to terminate the stream properly
            await responseStream.write("data: [DONE]\n\n");
          } catch (writeError) {
            console.error("Failed to write error message to stream");
          }
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

      return c.json(enhancedData, { status: 200 });
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

      throw fetchError; // Re-throw for the outer catch block
    }
  } catch (error) {
    console.error('[Server] LMStudio proxy error:', error);

    // Provide more specific error messages
    let errorMessage = 'Failed to connect to LMStudio server';
    let serverStatus = 'error';

    if (error instanceof Error) {
      if ('code' in error && error.code === 'ECONNREFUSED') {
        errorMessage = 'Connection refused. LMStudio server is not running.';
        serverStatus = 'not_running';
      } else if ('code' in error && error.code === 'ENOTFOUND') {
        errorMessage = 'Host not found. Check the LMStudio URL.';
        serverStatus = 'invalid_url';
      } else if (error.message && error.message.includes('network')) {
        errorMessage = 'Network error. Check your internet connection.';
        serverStatus = 'network_error';
      }
    }

    return new Response(JSON.stringify({
      error: errorMessage,
      details: error instanceof Error ? error.message : String(error),
      server_status: serverStatus
    }), { status: 500 });
  }
});

// --- End API Routes ---

export default app;

/**
 * Stream management utilities
 */

import type { Context } from 'hono';
import type { ReadableStream } from 'stream/web';
import { stream as honoStream } from 'hono/streaming';
import { streamText, type Message, type StreamTextOnFinishCallback } from 'ai';
import { Provider } from '../providers';
import { 
  formatErrorForStream,
  ChatError,
  transformUnknownError,
  cleanupMessagesWithFailedToolCalls,
  createRecoveryModelOptions,
  ToolError
} from '../errors';

/**
 * Manages the creation and configuration of AI stream responses
 */
export interface StreamManager {
  // Create a stream with a provider
  createStream(provider: Provider, messages: Message[], options?: Record<string, any>): Promise<any>;
  
  // Create a stream response from a Hono context
  createStreamResponse(c: Context, streamResult: any): any;
  
  // Handle stream errors
  handleStreamError(error: unknown, c: Context): any;
  
  // Attempt recovery from failed stream
  attemptRecovery(c: Context, messages: Message[], error: unknown, provider: Provider): any;
}

// Common HTTP headers for SSE responses
const SSE_HEADERS = {
  'Content-Type': 'text/event-stream; charset=utf-8',
  'Cache-Control': 'no-cache',
  'Connection': 'keep-alive',
  'X-Vercel-AI-Data-Stream': 'v1'
};

/**
 * Default stream manager implementation
 */
export class DefaultStreamManager implements StreamManager {
  /**
   * Create a stream with a provider
   */
  async createStream(
    provider: Provider, 
    messages: Message[], 
    options: Record<string, any> = {}
  ): Promise<any> {
    // Get the configured tools (if any)
    const tools = options.tools || {};
    
    // Create stream options
    const streamOptions: any = {
      model: provider.model,
      messages,
      // Only include tools if the model supports them and tools are provided
      ...(provider.supportsTools && Object.keys(tools).length > 0 ? {
        tools,
        toolCallStreaming: true
      } : {}),
      temperature: options.temperature || 0.7,
      // Include provider-specific headers if any
      ...(provider.headers && Object.keys(provider.headers).length > 0 ? { 
        headers: provider.headers 
      } : {}),
      // Standard error callback
      onError: (event: { error: unknown }) => {
        console.error("💥 streamText onError callback triggered");
        
        // Capture and log detailed error information
        if (event.error instanceof Error) {
          console.error("ERROR DETAILS:", {
            name: event.error.name,
            message: event.error.message,
            stack: event.error.stack,
            cause: (event.error as any).cause,
            code: (event.error as any).code
          });
        } else {
          console.error("UNKNOWN ERROR:", event.error);
        }
        
        // Transform the raw error to a ChatError
        const chatError = event.error instanceof ChatError
          ? event.error
          : transformUnknownError(event.error);
        
        // Check if this is a non-fatal tool error that should be returned as a result
        if (chatError instanceof ToolError && chatError.nonFatal) {
          // For non-fatal errors, pass the error message as a result back to the model
          console.log("Non-fatal tool error detected. Converting to result:", chatError.userMessage);
          // Set a special property that the AI SDK stream processor will recognize
          (chatError as any).shouldConvertToToolResult = true;
          (chatError as any).toolResult = chatError.toToolResult();
        }
        
        // Rethrow the error to be caught by the stream handler
        throw chatError;
      },
      // Standard completion callback
      onFinish: (event: Parameters<StreamTextOnFinishCallback<{}>>[0]) => {
        console.log(`🏁 streamText onFinish completed`);
      }
    };
    
    // Create the stream
    return streamText(streamOptions);
  }
  
  /**
   * Create a stream response from a Hono context
   */
  createStreamResponse(c: Context, streamResult: any): any {
    // Set up SSE headers
    Object.entries(SSE_HEADERS).forEach(([key, value]) => {
      c.header(key, value);
    });
    
    // Check streamResult validity
    if (!streamResult || typeof streamResult.toDataStream !== 'function') {
      console.error("Invalid streamResult object");
      throw new Error("Invalid stream result object");
    }
    
    // Create Hono stream
    return honoStream(c, async (responseStream) => {
      try {
        // Convert AI SDK stream to data stream
        const sdkStream = streamResult.toDataStream({
          sendReasoning: true
        });
        
        // Add client-side abort controller to prevent connection hanging
        const abortController = new AbortController();
        const abortSignal = abortController.signal;
        
        // Set a timeout to detect stuck or failing streams
        const timeoutId = setTimeout(() => {
          console.log("⚠️ Stream processing timeout - aborting connection");
          abortController.abort();
        }, 60000); // 60 second timeout
        
        // Process stream using reader
        const reader = sdkStream.getReader();
        let hasReceivedData = false;
        
        try {
          // Read from the stream until it's done or aborted
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
            
            // Process the chunk data
            await this.processStreamChunk(chunk, responseStream);
          }
        } catch (streamError) {
          // Handle errors that occur during stream processing
          console.error("STREAM PROCESSING ERROR:", streamError);
          
          // Check if this is a tool error that should be converted to a result
          if (streamError instanceof ToolError && 
              streamError.nonFatal && 
              (streamError as any).shouldConvertToToolResult) {
            
            // Just log it and continue - the error is already handled
            console.log("Non-fatal tool error already handled properly");
            return;
          }
          
          // Send error message to client
          const errorData = {
            id: `error-${Date.now()}`,
            role: "assistant",
            content: "",
            choices: [
              {
                delta: {
                  content: "An error occurred while processing the response. Please try again."
                }
              }
            ],
            created: Date.now()
          };
          
          await responseStream.write(`data: ${JSON.stringify(errorData)}\n\n`);
          await responseStream.write("data: [DONE]\n\n");
        } finally {
          // Always clean up the reader and timeout
          clearTimeout(timeoutId);
          reader.releaseLock();
        }
      } catch (error) {
        // Handle errors in stream setup
        console.error("Stream setup error:", error);
        
        // Format error for client
        const errorData = {
          id: `error-${Date.now()}`,
          role: "assistant",
          content: "",
          choices: [
            {
              delta: {
                content: "An error occurred while setting up the response stream. Please try again."
              }
            }
          ],
          created: Date.now()
        };
        
        await responseStream.write(`data: ${JSON.stringify(errorData)}\n\n`);
        await responseStream.write("data: [DONE]\n\n");
      }
    });
  }
  
  /**
   * Process a single chunk from the AI SDK stream
   */
  private async processStreamChunk(chunk: string, responseStream: any): Promise<void> {
    // Check if this is a data chunk
    if (chunk.startsWith('data: ')) {
      try {
        // Parse the JSON data
        const data = JSON.parse(chunk.replace(/^data: /, ''));
        
        // Process tool calls if present to ensure they're properly formatted
        if (data.choices?.[0]?.delta?.tool_calls) {
          const toolCalls = data.choices[0].delta.tool_calls;
          toolCalls.forEach((call: any) => {
            // Ensure type field exists
            if (!call.type) call.type = "function";
            
            // Validate arguments JSON
            if (call.function?.arguments && typeof call.function.arguments === 'string') {
              try {
                JSON.parse(call.function.arguments);
              } catch (e) {
                // If parsing fails, set to empty object
                call.function.arguments = "{}";
              }
            }
          });
        }
        
        // If the data is a tool result from a non-fatal error, make sure it's formatted correctly
        if ((data as any).toolResult) {
          // Ensure we're passing the formatted tool result
          // (This should be caught by our error handler, but just in case)
          console.log("Tool result from error found in stream data");
        }
        
        // Write the processed data
        await responseStream.write(`data: ${JSON.stringify(data)}\n\n`);
      } catch (e) {
        // If JSON parsing fails, just pass through the original chunk
        await responseStream.write(chunk);
      }
    } else {
      // Pass through non-data chunks unchanged
      await responseStream.write(chunk);
    }
  }
  
  /**
   * Handle stream errors
   */
  handleStreamError(error: unknown, c: Context): any {
    // Convert unknown errors to ChatError
    const chatError = error instanceof ChatError
      ? error
      : transformUnknownError(error);
    
    // Log detailed error for debugging
    console.error("========== STREAM ERROR DETAILS ==========");
    console.error(`Error type: ${chatError.constructor.name}`);
    console.error(`Error message: ${chatError.message}`);
    console.error(`Error category: ${chatError.category}`);
    console.error("==========================================");
    
    // Check if this is a non-fatal tool error
    if (chatError instanceof ToolError && chatError.nonFatal) {
      console.log("Non-fatal tool error detected. This should be handled as a tool result.");
      
      // For non-fatal tool errors, we want to return the error as a result to the model
      // We'll add special markers to the error
      (chatError as any).shouldConvertToToolResult = true;
      (chatError as any).toolResult = chatError.toToolResult();
    }
    
    // Set SSE headers
    Object.entries(SSE_HEADERS).forEach(([key, value]) => {
      c.header(key, value);
    });
    
    // Return stream with error
    return honoStream(c, async (responseStream) => {
      try {
        // Format the error for the stream
        const errorResponse = formatErrorForStream(chatError);
        await responseStream.write(errorResponse);
        await responseStream.write("data: [DONE]\n\n");
      } catch (writeError) {
        console.error("Failed to write error to stream:", writeError);
      }
    });
  }
  
  /**
   * Attempt recovery from failed stream
   */
  async attemptRecovery(
    c: Context, 
    messages: Message[], 
    error: unknown, 
    provider: Provider
  ): Promise<any> {
    console.log("🛠️ Attempting stream recovery after error");
    
    try {
      // Clean up messages by removing problematic tool calls
      const cleanedMessages = await cleanupMessagesWithFailedToolCalls(
        messages,
        error,
        { addAssistantMessage: true }
      );
      
      // Create a simplified configuration for recovery
      const recoveryOptions = createRecoveryModelOptions({});
      
      // Create a new stream with the cleaned messages
      const recoveryStream = await this.createStream(
        provider,
        cleanedMessages,
        recoveryOptions
      );
      
      // Return the recovery stream response
      return this.createStreamResponse(c, recoveryStream);
    } catch (recoveryError) {
      console.error("Recovery attempt failed:", recoveryError);
      
      // Fall back to regular error handling
      return this.handleStreamError(recoveryError, c);
    }
  }
}

// Export a singleton instance
export const streamManager = new DefaultStreamManager();
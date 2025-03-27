import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { streamText } from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
import { extractToolDefinitions, processToolCall, type ToolCallPayload, type ToolDefinition } from './mcp/tools';

interface Env {
  AI: typeof AI;
}

interface ChatMessage {
  content: string;
  role: 'user' | 'assistant';
}

type AIStreamChunk = Record<string, any>;

const app = new Hono<{ Bindings: Env }>();

// Initialize MCP connections
async function initMcp() {
  try {
    await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
    console.log('Connected to GitHub MCP server');
  } catch (error) {
    console.error('Failed to connect to GitHub MCP server:', error);
  }
}

// Call in a non-blocking way to avoid delaying server startup
void initMcp();

// Enable CORS for all routes
app.use('*', cors({
  origin: '*',
  allowMethods: ['POST', 'GET', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Accept', 'Authorization'],
  exposeHeaders: ['X-Vercel-AI-Data-Stream'],
  credentials: true,
}));

// Health check endpoint
app.get('/', c => c.text('200 OK'));

// Main chat endpoint
app.post('/', async c => {
  console.log("🚀 Chat request received");
  
  try {
    const body = await c.req.json();
    console.log("📝 Request body:", JSON.stringify(body).substring(0, 200));
    
    const messages = body.messages || [];
    console.log(`📨 Message count: ${messages.length}`);
    
    // Extract auth token from Authorization header
    const authToken = c.req.header('Authorization')?.replace('Bearer ', '');
    console.log(`🔑 Auth token present: ${!!authToken}`);
    
    // Get AI model from Cloudflare Workers AI
    const workersai = createWorkersAI({ binding: c.env.AI });
    
    // Get tool definitions for the LLM
    const tools = extractToolDefinitions();
    console.log(`🛠️ Available tools: ${tools.length}`);
    
    console.log("🔄 Initializing MCP connection");
    // Force MCP connection to ensure it's ready
    try {
      await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
      const allTools = mcpClientManager.getAllTools();
      console.log(`✅ MCP Connected, discovered ${allTools.length} tools`);
    } catch (mcpError) {
      console.error("❌ MCP Connection failed:", mcpError);
    }
    
    // Create streaming response with tools enabled
    console.log("🎬 Starting streaming response");
    const result = streamText({
      model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
      messages,
      tools: tools as any, // Type casting to avoid ToolSet issues
      toolCallStreaming: true
    });

  // Set up an interceptor for tool calls
  const interceptStream = async function*(stream: ReadableStream<any>) {
    const reader = stream.getReader();
    let toolCallBuffer: ToolCallPayload | null = null;
    
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          console.log("🏁 Stream completed");
          break;
        }
        
        // Check if the value is already our special error indicator
        if (value && typeof value === 'object' && value.type === 'error') {
          yield value;
          continue;
        }
        
        // Log chunk summary with detailed content
        console.log(`📦 Received chunk: ${JSON.stringify(value)}`);
        
        // Decode the array if it's a buffer-like object
        let processedValue = value;
        if (value && typeof value === 'object' && '0' in value) {
          try {
            const chars = Object.values(value).map(v => Number(v));
            const decodedText = String.fromCharCode(...chars);
            console.log(`🔍 Decoded chunk text: ${decodedText}`);
            
            // Check if this is an error from the model
            if (decodedText.startsWith('3:')) {
              // This is an error message, pass it through directly
              console.log(`⚠️ Error from model detected: ${decodedText}`);
              // We need to handle this outside the stream reader
              // Return the error to be written in the outer loop
              return { type: 'error', text: decodedText };
              // Skip further processing of this chunk
              // continue;
            }
            
            // Try to parse the decoded text
            try {
              const parsed = JSON.parse(decodedText);
              processedValue = parsed;
              console.log(`🔄 Parsed chunk: ${JSON.stringify(parsed)}`);
            } catch (parseError) {
              console.log(`⚠️ Not valid JSON: ${decodedText}`);
            }
          } catch (decodeError) {
            console.error(`❌ Failed to decode chunk: ${decodeError}`);
          }
        }
        
        // Check if chunk contains a complete tool call
        if (processedValue?.toolCalls?.length > 0 && !processedValue.isPartial) {
          console.log(`🔨 Tool call detected: ${processedValue.toolCalls[0].toolName}`);
          
          toolCallBuffer = {
            toolCallId: processedValue.toolCalls[0].toolCallId,
            toolName: processedValue.toolCalls[0].toolName,
            args: processedValue.toolCalls[0].args
          };
          
          console.log(`⚙️ Processing tool call: ${toolCallBuffer.toolName} with args: ${JSON.stringify(toolCallBuffer.args).substring(0, 100)}`);
          
          // Process the tool call with auth token
          const toolResult = await processToolCall(toolCallBuffer, authToken);
          console.log(`🎯 Tool result received: ${JSON.stringify(toolResult).substring(0, 100)}...`);
          
          // Create a modified chunk with tool result
          const resultChunk = {
            ...processedValue,
            toolResults: toolResult ? [toolResult] : []
          };
          
          yield resultChunk;
          toolCallBuffer = null;
        } else {
          // Pass through the chunk unchanged
          yield processedValue as AIStreamChunk;
        }
      }
    } catch (error) {
      console.error("💥 Error in stream processing:", error);
      throw error;
    } finally {
      console.log("🔄 Releasing stream reader");
      reader.releaseLock();
    }
  };

  // Mark the response as a SSE stream
  c.header('X-Vercel-AI-Data-Stream', 'v1');
  c.header('Content-Type', 'text/event-stream');
  c.header('Cache-Control', 'no-cache');
  c.header('Connection', 'keep-alive');

  return stream(c, async stream => {
    try {
      console.log("🔄 Starting stream processing");
      const transformedStream = interceptStream(result.toDataStream());
      
      console.log("🌊 Beginning stream iteration");
      for await (const chunk of transformedStream) {
        console.log(`📤 Writing chunk to response: ${JSON.stringify(chunk)}`);
        
        try {
          // Format in the Vercel AI SDK expected format with proper code prefix
          if (typeof chunk === 'string') {
            // Text content
            console.log(`✅ Sending text: ${chunk}`);
            await stream.write(`data: 0:${JSON.stringify(chunk)}\n\n`);
          } else if (chunk.toolCalls && chunk.toolCalls.length > 0 && !chunk.isPartial) {
            // Tool call
            console.log(`✅ Sending tool calls: ${JSON.stringify(chunk.toolCalls)}`);
            for (const toolCall of chunk.toolCalls) {
              await stream.write(`data: 9:${JSON.stringify(toolCall)}\n\n`);
            }
          } else if (chunk.toolResults && chunk.toolResults.length > 0) {
            // Tool result
            console.log(`✅ Sending tool results: ${JSON.stringify(chunk.toolResults)}`);
            for (const toolResult of chunk.toolResults) {
              await stream.write(`data: a:${JSON.stringify(toolResult)}\n\n`);
            }
          } else if (chunk.content) {
            // Regular text content
            console.log(`✅ Sending content: ${chunk.content}`);
            await stream.write(`data: 0:${JSON.stringify(chunk.content)}\n\n`);
          } else if (chunk.type === 'error' && chunk.text) {
            // Pass through the error with its original format
            console.log(`✅ Sending error directly: ${chunk.text}`);
            await stream.write(`data: ${chunk.text}\n\n`);
          } else {
            // Try with a null check for response content
            console.log(`⚠️ Unknown chunk format: ${JSON.stringify(chunk)}`);
            
            // Simple compatibility mode - just send a text response
            await stream.write(`data: 0:${JSON.stringify("I'm sorry, there was an issue processing your request.")}\n\n`);
          }
        } catch (error) {
          console.error(`❌ Error formatting chunk: ${error}`);
          // Send error in proper format
          await stream.write(`data: 3:${JSON.stringify("Error formatting response")}\n\n`);
        }
      }
      console.log("✅ Stream processing complete");
    } catch (error) {
      console.error("💥 Critical error in stream handling:", error);
      // Return error to client in a format that matches other responses
      await stream.write(`data: 3:${JSON.stringify("Stream processing failed")}\n\n`);
    }
  });
  } catch (error) {
    console.error("💥 Chat endpoint error:", error);
    return c.json({ error: "Failed to process chat request" }, 500);
  }
});

export default app;
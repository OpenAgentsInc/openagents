# Debugging MCP GitHub Tools Integration

## Issue Overview

We encountered issues with the GitHub MCP (Model Context Protocol) tools integration in the OpenAgents chat server. This document captures the debugging process and solutions for future reference.

## Background

The integration was designed to connect the OpenAgents chat server with the GitHub MCP server to provide GitHub tools functionality to the chat interface. The architecture follows a standard MCP pattern:

1. Chat server connects to MCP server via SSE (Server-Sent Events)
2. Tools are discovered and registered during connection
3. LLM generates tool calls which are routed to the appropriate MCP server
4. Tool results are streamed back to the user

## Issues Encountered

### 1. Tool Discovery Format Mismatch

**Problem**: The GitHub MCP server returns tools in a nested object format, but our code expected an array.

**Error**: `❌ Tools from github is not an array: object`

**Root Cause**: The MCP server returns tools in the format `{ tools: [...] }` instead of a direct array.

**Solution**: Modified the tool discovery code to extract the tools array from the response object:

```typescript
const toolsResponse = await Promise.race([toolsPromise, timeoutPromise]);
const tools = toolsResponse && typeof toolsResponse === 'object' && 'tools' in toolsResponse ? 
  toolsResponse.tools : toolsResponse;
```

### 2. SSE (Server-Sent Events) Streaming Format Issues

**Problem**: The client couldn't parse the data stream from the server.

**Error**: `Failed to parse stream string. Invalid code data.`

**Root Cause**: The Vercel AI SDK expects a specific data format for SSE events with proper prefixes. The server was not correctly formatting the chunks with the appropriate prefixes.

**Solution**: 
1. Fixed the SSE headers:
   ```typescript
   c.header('Content-Type', 'text/event-stream');
   c.header('Cache-Control', 'no-cache');
   c.header('Connection', 'keep-alive');
   ```

2. Properly formatted the stream chunks according to Vercel AI SDK expectations:
   ```typescript
   // Text content
   await stream.write(`data: 0:${JSON.stringify(chunk)}\n\n`);
   
   // Tool calls
   await stream.write(`data: 9:${JSON.stringify(toolCall)}\n\n`);
   
   // Tool results
   await stream.write(`data: a:${JSON.stringify(toolResult)}\n\n`);
   
   // Errors
   await stream.write(`data: 3:${JSON.stringify("An error occurred")}\n\n`);
   ```

### 3. Binary Data Handling

**Problem**: The model returned binary data instead of proper JSON.

**Root Cause**: The Cloudflare Workers AI integration sometimes returns data in a binary format that needs to be decoded.

**Solution**: Added decoder to handle binary response format:
```typescript
if (value && typeof value === 'object' && '0' in value) {
  const chars = Object.values(value).map(v => Number(v));
  const decodedText = String.fromCharCode(...chars);
  // Process the decoded text
}
```

### 4. Error Propagation

**Problem**: Model errors weren't properly propagated to the client.

**Root Cause**: The error detection and handling in the stream interceptor wasn't effectively passing errors to the client in a format it could understand.

**Solution**: Standardized error format and ensured it was properly written to the stream:
```typescript
// Standardized error format for AI SDK
return { type: 'error', text: "3:\"An error occurred\"", source: 'model' };
```

## Lessons Learned

1. **API Format Verification**: Always verify the exact format of API responses during integration. Don't assume the format will match expectations.

2. **Stream Format Standards**: When working with streaming APIs, be precise about the expected data format and include comprehensive logging.

3. **Error Handling**: Implement robust error handling at every stage of the request lifecycle.

4. **Logging Strategy**: Implement detailed logging that captures both successful operations and error conditions to aid in debugging.

5. **Protocol Compatibility**: Ensure that all components in the system are compatible with the same protocol version and format expectations.

## Next Steps

1. **Logging Improvements**: Add more detailed logging in the MCP server to better diagnose issues.

2. **Error Handling Robustness**: Strengthen error handling throughout the system.

3. **Testing Strategy**: Develop comprehensive testing strategies for streaming connections.

4. **Documentation**: Improve documentation of the expected formats and error handling.

## References

1. [Vercel AI SDK Documentation](https://sdk.vercel.ai/docs)
2. [Server-Sent Events Specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)
3. [MCP Integration Plan](/docs/issues/789/design.md)
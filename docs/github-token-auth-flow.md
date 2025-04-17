# GitHub Token Authentication Flow

This document explains the authentication flow for GitHub tokens in the OpenAgents architecture, specifically how tokens are passed from the Chat Server to the MCP GitHub Server for authenticated API calls.

## Overview

The authentication flow involves multiple components:

1. **Client/UI**: Sends the GitHub token via the `X-GitHub-Token` HTTP header
2. **Chat Server**: Receives the token and passes it through the Vercel AI SDK
3. **MCP Client Manager**: Packages the token into the MCP protocol payload
4. **MCP GitHub Server**: Extracts the token from the MCP payload and uses it for GitHub API calls

## Detailed Flow

### 1. Chat Server (`apps/chatserver/src/index.ts`)

The Chat Server receives the GitHub token as an HTTP header and passes it through to the streaming AI response:

```typescript
// Extract token from headers
const githubTokenHeader = c.req.header('X-GitHub-Token');

// Pass token to streamText via headers
const streamResult = streamText({
  model: openrouter(MODEL),
  messages: modifiedMessages,
  tools: hasTools ? tools : undefined,
  // ...other options...
  
  // Pass GitHub token via headers
  headers: {
    'Authorization': `Bearer ${c.env.OPENROUTER_API_KEY}`,
    'HTTP-Referer': 'https://openagents.com',
    'X-Title': 'OpenAgents Chat',
    'X-GitHub-Token': githubTokenHeader,
  },
  // ...callbacks...
});
```

### 2. Vercel AI SDK Tool Execution (`apps/chatserver/src/mcp/tools.ts`)

When a tool is called by the AI, the Vercel AI SDK invokes the `execute` function for the tool, passing the headers from `streamText`:

```typescript
// Inside the tool definition
execute: (async (args: any, options: any) => {
  // Extract the auth token from headers
  const authHeader = options.headers?.Authorization;
  const tokenHeader = options.headers?.['X-GitHub-Token'];
  const authToken = authHeader?.replace('Bearer ', '') || tokenHeader || null;

  console.log(`🔐 Tool ${toolName} execute() auth token info:`);
  console.log(`  - Authorization header present: ${!!authHeader}`);
  console.log(`  - X-GitHub-Token header present: ${!!tokenHeader}`);
  console.log(`  - Final token resolved: ${authToken ? 'Yes' : 'No'}`);

  // Call the MCP tool with the extracted auth token
  const result = await mcpClientManager.callTool(toolName, args, authToken);
  return result;
}) as any
```

### 3. MCP Client Manager (`apps/chatserver/src/mcp/client.ts`)

The MCP Client Manager packages the token into the `_meta` field of the MCP protocol payload:

```typescript
async callTool(toolName: string, args: Record<string, any>, token?: string): Promise<any> {
  // Generate unique ID for tracking
  const callId = crypto.randomUUID().substring(0, 8);
  
  // Prepare the arguments with token
  const callArgs = {
    name: toolName,
    arguments: args,
    // Include _meta with token and requestId
    _meta: {
      ...(token ? { token } : {}),
      requestId: callId
    }
  };

  console.log(`📤 [${callId}] Sending tool call to MCP server with args structure:`, Object.keys(callArgs));
  if (token) {
    console.log(`🔐 [${callId}] Token is being passed via _meta.token property`);
  }

  // Execute the tool call
  const result = await client.callTool(callArgs);
  return result;
}
```

### 4. MCP GitHub Server (`apps/mcp-github-server/src/index.ts`)

The MCP GitHub Server extracts the token from the `_meta` field of the incoming MCP payload:

```typescript
this.server.tool(tool.name, tool.schema.shape, async (rawPayload: Record<string, unknown>) => {
  // Extract the token from the _meta field
  const meta = rawPayload._meta as { token?: string; requestId?: string } | undefined;
  const token = meta?.token;
  const requestId = meta?.requestId || 'unknown';
  
  const context: ToolContext = { token };
  
  // Validate arguments
  if (typeof rawPayload.arguments !== 'object' || rawPayload.arguments === null) {
      console.error(`[${requestId}] ❌ Missing or invalid 'arguments' field in payload for tool ${tool.name}`);
      return { 
          content: [{ type: "text", text: JSON.stringify({ error: "Invalid tool arguments payload" }) }] 
      };
  }
  
  const validatedParams = tool.schema.parse(rawPayload.arguments);

  // Replace githubRequest with token-aware version
  const originalRequest = globalThis.githubRequest;
  try {
    console.log(`[${requestId}] 🔧 Executing GitHub tool: ${tool.name}`);
    console.log(`[${requestId}] 🔑 GitHub token present in _meta: ${!!token}`);

    globalThis.githubRequest = withToken(context.token);
    const result = await tool.handler(validatedParams as any);
    // ...rest of function...
```

### 5. GitHub API Requests (`apps/mcp-github-server/src/common/utils.ts`)

The GitHub API utility function uses the token to authenticate requests:

```typescript
export async function githubRequest(
  url: string,
  options: RequestOptions = {}
): Promise<unknown> {
  const headers: Record<string, string> = {
    "Accept": "application/vnd.github.v3+json",
    "Content-Type": "application/json",
    "User-Agent": USER_AGENT,
    ...options.headers,
  };

  if (options.token) {
    headers["Authorization"] = `Bearer ${options.token}`;
  }
  
  console.log(`🔄 GitHub API Request: ${url.substring(0, url.indexOf('?') > 0 ? url.indexOf('?') : url.length)}`);
  console.log(`🔑 Auth present: ${!!options.token}`);
  
  // ...rest of function...
}
```

## Debugging Token Flow

To debug the token flow:

1. **Chat Server Logs**:
   - Check for `X-GitHub-Token` header presence
   - Verify token extraction in `tools.ts` execute function
   - Confirm token is passed to `mcpClientManager.callTool`

2. **MCP Client Manager Logs**:
   - Look for `Token is being passed via _meta.token property` message
   - Verify `callArgs` structure includes `_meta.token`

3. **MCP GitHub Server Logs**:
   - Check for `GitHub token present in _meta: true` message
   - Verify `Auth present: true` in GitHub API request logs
   - Monitor request IDs to track specific tool calls

## Common Issues and Solutions

1. **Token Not Reaching MCP GitHub Server**:
   - Verify the token is correctly passed in the `X-GitHub-Token` header
   - Check the Chat Server logs to confirm token extraction
   - Ensure the MCP Client Manager is packaging the token correctly

2. **Authentication Errors**:
   - Verify the token has the necessary scopes for the operation
   - Check if the token is valid and not expired
   - Confirm the token format (should start with `ghp_` for personal access tokens)

3. **Request Failures**:
   - Look for rate limit errors (status 429)
   - Check for permission issues (status 401/403)
   - Verify repository accessibility (public/private)

## Security Considerations

1. The token is never logged in full, only its presence and format are logged
2. The token is passed securely through the system via headers and the `_meta` field
3. The token is only used for the specific GitHub API request and then cleaned up

## Testing the Flow

To test this authentication flow:

1. Make a request to the Chat Server with a valid GitHub token in the `X-GitHub-Token` header
2. Trigger a tool call that requires authentication (e.g., creating an issue or repository)
3. Check the logs at each stage to verify the token is being passed correctly
4. Confirm the operation succeeds with the expected result
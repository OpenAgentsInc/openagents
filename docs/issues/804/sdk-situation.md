# Understanding the Cloudflare Agents SDK Integration

## Overview

The Cloudflare Agents SDK is a framework for creating stateful AI agents that can maintain persistent connections and state. Integration with OpenAgents requires careful attention to naming conventions and API usage patterns.

## Key Components

1. **Durable Objects**: The foundation of Agents SDK, providing persistent state storage.
2. **WebSocket Connections**: Used for real-time, bidirectional communication with agents.
3. **routeAgentRequest**: The main SDK function that handles request routing to the appropriate Agent.

## Integration Requirements

For successful integration with the Agents SDK:

1. **Binding Naming Convention**:
   - The binding name in wrangler.jsonc must be lowercase (e.g., "coderagent")
   - The class_name must match the exported class (e.g., "CoderAgent")

2. **Server Implementation**:
   - Export the Agent class directly without renaming
   - Use routeAgentRequest for all routing logic
   - Avoid custom header manipulation

3. **Client Implementation**:
   - The client automatically converts agent names to lowercase
   - URLs will use this lowercase name (e.g., `/agents/coderagent/default`)

## Common Issues

1. **Case Sensitivity Mismatch**:
   When the binding name in wrangler.jsonc doesn't match the lowercase name in URLs:
   ```
   The url https://agents.openagents.com/agents/coderagent/default does not match any server namespace.
   ```

2. **Missing Headers**:
   When trying to access Durable Objects directly without proper headers:
   ```
   Missing namespace or room headers when connecting to CoderAgent.
   ```

## Best Practices

1. **Follow the Example**: Use the Cloudflare Agents starter app as a reference point.
2. **Keep It Simple**: Minimal code that delegates to routeAgentRequest is best.
3. **Consistent Naming**: Ensure naming consistency between wrangler.jsonc and exports.
4. **Let the SDK Handle Routing**: Don't try to manually route or manipulate headers.

## Updated Configuration

Our working solution uses:

1. Lowercase binding name in wrangler.jsonc:
   ```
   "name": "coderagent",
   "class_name": "CoderAgent"
   ```

2. Minimal server implementation:
   ```typescript
   export { CoderAgent };
   
   export default {
     async fetch(request: Request, env: Env, ctx: ExecutionContext) {
       return (
         (await routeAgentRequest(request, env)) ||
         new Response("Not found", { status: 404 })
       );
     },
   };
   ```
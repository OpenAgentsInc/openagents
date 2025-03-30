# Cloudflare Agents SDK Integration Guide

This document provides a guide to integrating the Cloudflare Agents SDK with OpenAgents, based on our experience fixing issue #804.

## Setup Requirements

1. **Wrangler Configuration**:
   ```jsonc
   "durable_objects": {
     "bindings": [
       {
         "name": "coderagent", // IMPORTANT: Must be lowercase
         "class_name": "CoderAgent" // Matches the exported class name
       }
     ]
   },
   "migrations": [
     {
       "tag": "v1",
       "new_sqlite_classes": [
         "CoderAgent"
       ]
     }
   ]
   ```

2. **Server Implementation**:
   ```typescript
   import { AsyncLocalStorage } from "node:async_hooks";
   import { routeAgentRequest } from "agents";
   import { CoderAgent } from "./coder-agent";
   
   // We use ALS to expose the agent context to the tools
   export const agentContext = new AsyncLocalStorage<CoderAgent>();
   
   // Export the CoderAgent class for the Durable Object
   export { CoderAgent };
   
   /**
    * Worker entry point that routes incoming requests to the appropriate handler
    */
   export default {
     async fetch(request: Request, env: Env, ctx: ExecutionContext) {
       // Check for required API keys
       if (!env.OPENROUTER_API_KEY) {
         return new Response("OPENROUTER_API_KEY is not set", { status: 500 });
       }
       
       return (
         // Route the request to our agent or return 404 if not found
         (await routeAgentRequest(request, env)) ||
         new Response("Not found", { status: 404 })
       );
     },
   };
   ```

## Important Notes

1. **Binding Names Must Be Lowercase**: The client automatically converts agent names to lowercase (e.g., "CoderAgent" → "coderagent"), so your binding name in wrangler.jsonc must also be lowercase.

2. **Class Naming**: The class_name in wrangler.jsonc must match the exported class name in your server.ts file.

3. **Simple Integration Pattern**: Keep the server.ts file simple, following the exact pattern in the starter app.

4. **No Custom Headers**: Don't try to add headers manually - let routeAgentRequest handle all the routing logic.

5. **AIChatAgent Extension**: Your agent should extend AIChatAgent from the Agents SDK:
   ```typescript
   import { AIChatAgent } from "agents/ai-chat-agent";
   
   export class CoderAgent extends AIChatAgent<Env> {
     // Agent implementation...
   }
   ```

## Client Connection

Client code should use the AgentClient from the Agents SDK:

```typescript
import { AgentClient } from "agents/client";

const client = new AgentClient({
  agent: "coderagent", // Lowercase name matching the binding
  name: "default"
});
```

## Troubleshooting

1. **Case Sensitivity**: If you see "URL does not match any server namespace" errors, check that binding names are lowercase in wrangler.jsonc.

2. **Missing Headers**: If you see "Missing namespace or room headers" errors, don't try to add headers manually - let routeAgentRequest handle it.

3. **Connection Issues**: Check browser console for WebSocket errors and ensure the URL pattern matches the binding name.

## Deployment

Deploy using Wrangler:

```bash
cd packages/agents
wrangler deploy
```
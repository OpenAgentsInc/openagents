# Issue #804: Fixing WebSocket Connections to Cloudflare Agents

## Problem

WebSocket connections to the CoderAgent Durable Object were failing with errors:

1. `The url https://agents.openagents.com/agents/coderagent/default does not match any server namespace.`
2. `Missing namespace or room headers when connecting to CoderAgent.`

This prevented the useChat hook from establishing real-time communication with the agent.

## Root Cause

A case sensitivity mismatch between different parts of the system:

1. Client-side code (agent-sdk-bridge.ts) automatically converts agent names to lowercase: `CoderAgent` → `coderagent`
2. Our wrangler.jsonc had the binding name as `CoderAgent` (uppercase)
3. The Cloudflare Agents SDK expects an exact case-sensitive match between the URL and the binding name

## Solution

1. **Binding Name Alignment**: Changed the binding name in wrangler.jsonc to lowercase to match client expectations:
   ```diff
   "durable_objects": {
     "bindings": [
       {
   -     "name": "CoderAgent",
   +     "name": "coderagent",
         "class_name": "CoderAgent"
       }
     ]
   },
   ```

2. **Simplified Implementation**: Followed the example app pattern exactly, using the SDK's routing:
   ```typescript
   export default {
     async fetch(request: Request, env: Env, ctx: ExecutionContext) {
       return (
         (await routeAgentRequest(request, env)) ||
         new Response("Not found", { status: 404 })
       );
     },
   };
   ```

## Lessons Learned

1. Cloudflare Agents SDK has specific expectations for case sensitivity in binding names
2. Simplicity wins - minimal code following the exact example pattern is most reliable
3. The client-side code makes automatic transformations (lowercase) that must be matched in server config
4. Direct Durable Object access requires precise headers; using routeAgentRequest is generally preferable

## Timeline

1. Initial errors identified when testing WebSocket connections
2. Multiple approaches tried:
   - Custom header manipulation (failed)
   - Direct Durable Object access (failed)
   - SDK routing functions with enhanced headers (failed)
3. Discovered case sensitivity mismatch by examining client logs
4. Fixed by aligning binding name case and simplifying implementation
5. Verified working solution with successful WebSocket connections
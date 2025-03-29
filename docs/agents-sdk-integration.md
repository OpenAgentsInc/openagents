# Integrating Cloudflare Agents SDK into OpenAgents

## Overview

This document explains how to integrate the Cloudflare Agents SDK into the OpenAgents platform. We've installed the [agents-starter](https://github.com/cloudflare/agents-starter) template as a base, but need to modify it to work within our monorepo structure without its own frontend, as our existing apps (chatserver, coder, etc.) will consume these agents.

## What to Remove/Modify

From the `packages/agents` directory, we should remove or modify the following:

### Files to Remove

1. **Frontend Components**:
   - `src/app.tsx` - The standalone chat UI for agents-starter
   - `src/client.tsx` - Entry point for the web client
   - `src/components/` - UI components for the standalone interface
   - `src/hooks/` - Frontend hooks 
   - `src/providers/` - React context providers
   - `src/styles.css` - Styling for the standalone UI
   - `public/` - Static assets for the frontend
   - `index.html` - HTML entry point

2. **Build Configuration for Frontend**:
   - Remove or modify `vite.config.ts` to only build the server portion

### Files to Keep and Modify

1. **Core Agent Logic**:
   - `src/server.ts` - Contains the agent implementation using Durable Objects
   - `src/tools.ts` - Contains tool definitions
   - `src/utils.ts` - Shared utility functions
   - `src/shared.ts` - Shared constants and types

2. **Configuration**:
   - `wrangler.jsonc` - Update the configuration to match our deployment needs
   - `worker-configuration.d.ts` - Keep for type definitions

3. **Package Configuration**:
   - `package.json` - Update to remove frontend dependencies and scripts

## Integration with ChatServer

The agent implementation should be kept in the agents package and made available to the chatserver through Cloudflare Workers' service bindings. This will require:

1. Configuring the agents package as a standalone Worker service
2. Setting up service bindings in the chatserver's wrangler.jsonc
3. Updating the chatserver to consume the agents through the binding
4. Ensuring the proper environment configuration (API keys, etc.)

### Service Binding Configuration

In the chatserver's `wrangler.jsonc`, we need to add a service binding section:

```jsonc
"services": [
  { 
    "binding": "AGENTS_SERVICE", 
    "service": "agents"
  }
]
```

This allows the chatserver to directly call the agents service without exposing it publicly.

## Tool Integration

The tools defined in `tools.ts` should be modified to:

1. Be more generic and reusable across our platform
2. Connect with our existing MCP implementations 
3. Support the cross-platform needs of our various clients

## Package Dependencies

Update `package.json` to:
1. Remove frontend-specific dependencies like React and UI libraries
2. Keep core dependencies related to agent functionality
3. Add proper peer dependencies for integration with our other packages

## Build Process Changes

1. Update the build scripts to only include server-side code
2. Ensure proper TypeScript configuration for our monorepo structure
3. Configure wrangler for deployment to our Cloudflare Workers infrastructure

## Using Agents in ChatServer

Once the service binding is configured, the chatserver can call the agents service using the binding:

```typescript
// In apps/chatserver/src/index.ts
interface Env {
  AGENTS_SERVICE: Fetcher; // This is the service binding
  // other environment bindings...
}

// Example of calling the agents service
app.post('/agent-request', async c => {
  // Forward the request to the agents service
  const response = await c.env.AGENTS_SERVICE.fetch(c.req);
  return response;
});
```

For more complex interactions, you can create a wrapper around the service binding that provides a more convenient API.

## Next Steps

1. Remove the identified frontend files
2. Configure the agents package as a standalone Worker service
3. Set up service bindings in the chatserver's wrangler.jsonc
4. Modify the tools to integrate with our existing MCP implementation
5. Update the build configuration to only build server components
6. Create integration tests to verify everything works correctly

## References

- [Cloudflare Service Bindings Documentation](https://developers.cloudflare.com/workers/configuration/service-bindings/)
- [Agents SDK Documentation](https://developers.cloudflare.com/workers/ai-gateway/integrations/agents-sdk/)
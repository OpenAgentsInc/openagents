# Removing Mock MCP Tools

## Problem Description

The application was displaying mock GitHub tools even when real MCP clients were configured and enabled. This created confusion because:

1. Users were seeing tools from "Remote GitHub MCP" even after disabling/deleting this client
2. The mock tools weren't functional (they were just placeholders for UI testing)
3. Real configured tools from "Local GitHub MCP" weren't being used

## Root Causes

1. **Browser Environment Mocking**: In `mcp-clients.ts`, there was code that always returned mock GitHub tools when in a browser environment
2. **Default Configuration Issue**: The default MCP client configuration enabled the Remote GitHub client by default
3. **Fallback Logic Problem**: The error recovery code attempted to initialize the Remote GitHub client instead of the Local GitHub client

## Changes Made

### 1. Removed Browser Environment Mocking

Replaced the mock client code in `mcp-clients.ts` with proper error handling:

```typescript
// OLD CODE - returning mock GitHub tools in browser
if (typeof window !== 'undefined') {
  console.log(`[MCP Clients] Browser environment detected - using mock client for: ${config.name}`);
  
  // Return a mock client with mock GitHub tools
  return {
    tools: async () => {
      return {
        'github_search': { ... },
        'github_repo': { ... }
      };
    }
  };
}

// NEW CODE - proper error handling instead of mocks
if (typeof window !== 'undefined') {
  console.log(`[MCP Clients] Browser environment detected - cannot initialize ${config.type} client: ${config.name}`);
  await updateClientStatus(config.id, 'error', 'Client cannot be initialized in browser environment');
  return null;
}
```

### 2. Updated Default Configuration

Changed the default MCP client configuration to prefer Local GitHub MCP:

```typescript
// OLD CODE - Remote GitHub enabled by default
const DEFAULT_MCP_CLIENTS: MCPClientConfig[] = [
  {
    id: 'remote-github',
    name: 'Remote GitHub MCP',
    enabled: true,
    type: 'sse',
    url: 'https://mcp-github.openagents.com/sse',
    status: 'disconnected'
  },
  {
    id: 'local-github',
    name: 'Local GitHub MCP',
    enabled: false,
    // ...
  },
];

// NEW CODE - Local GitHub enabled by default, Remote GitHub removed
const DEFAULT_MCP_CLIENTS: MCPClientConfig[] = [
  {
    id: 'local-github',
    name: 'Local GitHub MCP',
    enabled: true,
    type: 'stdio',
    // ...
  },
];
```

### 3. Fixed Fallback Logic

Updated the fallback logic to use Local GitHub MCP:

```typescript
// OLD CODE - Remote GitHub as fallback
try {
  const remoteConfig = DEFAULT_MCP_CLIENTS.find(c => c.id === 'remote-github');
  if (remoteConfig && remoteConfig.enabled) {
    // Initialize remote GitHub client
  }
}

// NEW CODE - Local GitHub as fallback
try {
  const localConfig = DEFAULT_MCP_CLIENTS.find(c => c.id === 'local-github');
  if (localConfig && localConfig.enabled) {
    // Initialize local GitHub client
  }
}
```

## Benefits of the Changes

1. **Real Tools Only**: The application now only displays real tools from actually configured and enabled MCP clients
2. **No Misleading UI**: Users will no longer see non-functional mock tools in the interface
3. **Consistent Behavior**: The tool selection now behaves consistently with the MCP client configuration in settings
4. **Better Error Handling**: Browser environments properly report that clients can't be initialized instead of showing fake tools

## Expected Behavior After Changes

When users enable Local GitHub MCP in the settings:
1. Only real tools from the configured Local GitHub MCP will appear in the tool selection dropdown
2. No mock GitHub tools will appear in the interface
3. The tool selection will properly filter to show only the tools actually available from connected clients

This ensures a consistent experience where the tools displayed match what's actually configured in the system.
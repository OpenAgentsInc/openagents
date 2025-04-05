# MCP Tool Grouping Implementation

## Overview

This document outlines the implementation of MCP (Model Context Protocol) tool grouping in the OpenAgents tool selection interface. The feature organizes tools by their source provider, making it easier for users to understand where tools come from and to enable/disable all tools from a particular provider at once.

## Key Components

### 1. Tool Definition Extension

Extended the `ToolDefinition` interface in `packages/core/src/tools/TOOLS.ts` to include provider information:

```typescript
export interface ToolDefinition {
  // Existing fields
  id: string;
  name: string;
  description: string;
  type: 'builtin' | 'mcp';
  schema: any;
  serverIdentifier: string;
  supportsModels?: string[];
  
  // New fields for provider tracking
  providerId?: string;    // ID of the provider (MCP client) that provides this tool
  providerName?: string;  // Name of the provider (MCP client) that provides this tool
}
```

### 2. MCP Client Tracking

Modified `apps/coder/src/server/mcp-clients.ts` to track which tools belong to which MCP client:

```typescript
interface MCPClients {
  clients: Record<string, Awaited<ReturnType<typeof experimental_createMCPClient>> | null>;
  allTools: Record<string, any>; // Combined tools from all clients
  clientTools: Record<string, string[]>; // NEW: Tools provided by each client
  configs: Record<string, MCPClientConfig>;
  initialized: boolean;
}
```

Enhanced the `refreshTools()` function to track tool-to-client relationships:

```typescript
// Track which tools belong to this client
mcpClients.clientTools[id] = Object.keys(clientTools);
```

### 3. Provider-Aware Tool Grouping

Updated the `extendWithMCPTools()` function to incorporate provider information:

```typescript
export function extendWithMCPTools(
  mcpTools: Record<string, any>, 
  mcpClients?: Record<string, { id: string; name: string; tools?: string[] }>
): ToolDefinition[] {
  // ...
  
  // Determine which provider this tool belongs to
  let providerId = '';
  let providerName = '';

  if (mcpClients) {
    for (const [clientId, clientInfo] of Object.entries(mcpClients)) {
      if (clientInfo.tools && clientInfo.tools.includes(toolId)) {
        providerId = clientId;
        providerName = clientInfo.name;
        break;
      }
    }
  }
  
  // Add provider info to the tool definition
  allTools.push({
    // ...other tool properties
    providerId,
    providerName
  });
  
  // ...
}
```

### 4. UI Implementation

#### 4.1 Settings Page (ToolsPage.tsx)

- Implemented collapsible provider groups in the settings page
- Added provider-level actions to enable/disable all tools from a provider at once
- Visual indicators showing which providers have enabled tools
- Search functionality that works across all tools and providers

```tsx
// Group tools by provider
const providerGroups = useMemo(() => {
  const result = {
    builtin: {
      name: "Built-in Tools",
      id: "builtin",
      tools: []
    }
  };
  
  allTools.forEach(tool => {
    if (tool.type === 'builtin') {
      result.builtin.tools.push(tool);
    } else if (tool.providerId && tool.providerName) {
      // Create or add to provider group
      // ...
    }
  });
  
  return result;
}, [allTools]);
```

#### 4.2 Tool Selection Dropdown (ToolSelect.tsx)

- Grouped tools by provider in the dropdown
- Added provider-level selection actions (Select All / Clear)
- Visual indicators for partially or fully selected provider groups
- Improved search functionality that works with provider-grouped tools

## Benefits

1. **Better Organization**: Tools are now logically grouped by their provider, making it easier to understand the source of each tool.

2. **Bulk Actions**: Users can enable/disable all tools from a provider with a single click, improving efficiency.

3. **Visual Clarity**: Clear visual indicators show which providers have enabled tools and whether all or some tools in a provider are selected.

4. **Future-Proof**: The architecture supports multiple MCP clients, each potentially providing multiple tools.

## User Experience

### Settings > Tools Page

- Shows collapsible sections for each provider (Built-in, GitHub MCP, etc.)
- Users can expand/collapse provider sections
- Provider sections show a count of tools and enabled status
- Provider-level actions for enabling/disabling all tools

### Tool Selection Dropdown

- Organized hierarchically by provider
- Expandable provider sections with tool counts
- Provider-level quick actions (Select All, Clear)
- Visual indicators for selection state at both provider and tool levels
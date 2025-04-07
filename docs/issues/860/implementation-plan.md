# Implementation Plan for MCP GitHub Tool Integration

## 1. Overview

The goal is to integrate the Model Context Protocol (MCP) GitHub tools with the Coder agent in OpenAgents. This will allow the Coder agent to perform GitHub operations through the MCP protocol.

## 2. Current Architecture Analysis

### Coder Agent Architecture
- **Agent Class**: `Coder` extends `AIChatAgent` in `packages/agents/src/server.ts`
- **Tool System**: Defined in `packages/agents/src/tools.ts`
- **Tool Processing**: Managed in `packages/agents/src/utils.ts`
- **Stream Handling**: Uses `createDataStreamResponse` and `streamText` from AI SDK

### MCP Client Implementation
- **MCP Client Manager**: Implemented in `apps/coder/src/server/mcp-clients.ts`
- **MCP Tools Integration**: Started in `apps/coder/src/server/tools/mcp-tools.ts`
- **Current Status**: MCP clients are configured but not connected to agent

## 3. Implementation Details

### 3.1 Create MCP GitHub Plugin

Create a new plugin class to handle MCP GitHub tool integration:

```typescript
// packages/agents/src/plugins/mcp-github-plugin.ts
import { agentContext } from "../server";
import { getMCPClients, refreshTools } from "apps/coder/src/server/mcp-clients";
import { wrapMCPToolsWithErrorHandling } from "apps/coder/src/server/tools/mcp-tools";

export class OpenAIAgentPlugin {
  private githubTools: Record<string, any> = {};
  
  async initialize(): Promise<void> {
    try {
      // Refresh MCP tools to make sure we have the latest
      const { allTools, clients } = getMCPClients();
      
      // Filter for GitHub-related tools only
      this.githubTools = Object.entries(allTools || {})
        .filter(([toolName]) => toolName.startsWith('github'))
        .reduce((acc, [name, tool]) => ({ ...acc, [name]: tool }), {});
      
      console.log(`Initialized MCP GitHub plugin with ${Object.keys(this.githubTools).length} tools`);
    } catch (error) {
      console.error("Failed to initialize MCP GitHub plugin:", error);
      throw error;
    }
  }
  
  getTools(): Record<string, any> {
    return wrapMCPToolsWithErrorHandling(this.githubTools);
  }
}
```

### 3.2 Update Coder Agent Class

Modify the Coder agent to use the GitHub plugin:

```typescript
// packages/agents/src/server.ts
import { OpenAIAgentPlugin } from "./plugins/mcp-github-plugin";

export class Coder extends AIChatAgent<Env> {
  private githubPlugin: OpenAIAgentPlugin;
  private combinedTools: Record<string, any>;

  constructor(state: DurableObjectState, env: Env) {
    super(state, env);
    this.githubPlugin = new OpenAIAgentPlugin();
    this.combinedTools = { ...tools };
    
    // Initialize plugin when agent is created
    this.initializePlugins().catch(err => 
      console.error("Failed to initialize agent plugins:", err)
    );
  }
  
  private async initializePlugins(): Promise<void> {
    try {
      await this.githubPlugin.initialize();
      // Merge GitHub tools with agent tools
      const githubTools = this.githubPlugin.getTools();
      this.combinedTools = { ...tools, ...githubTools };
    } catch (error) {
      console.error("Error initializing plugins:", error);
    }
  }
  
  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    return agentContext.run(this, async () => {
      const dataStreamResponse = createDataStreamResponse({
        execute: async (dataStream) => {
          // Use combinedTools instead of just tools
          const processedMessages = await processToolCalls({
            messages: this.messages,
            dataStream,
            tools: this.combinedTools,
            executions,
          });

          const result = streamText({
            model,
            system: `You are a helpful assistant that can do various tasks...
            
            ${unstable_getSchedulePrompt({ date: new Date() })}
            
            You can use GitHub tools to interact with repositories, issues, and pull requests.
            
            If the user asks to schedule a task, use the schedule tool to schedule the task.
            `,
            messages: processedMessages,
            tools: this.combinedTools,
            onFinish,
            onError: (error) => {
              console.error("Error while streaming:", error);
            },
            maxSteps: 10,
          });

          result.mergeIntoDataStream(dataStream);
        },
      });

      return dataStreamResponse;
    });
  }
}
```

### 3.3 Create Plugin Interface

```typescript
// packages/agents/src/plugins/plugin-interface.ts
export interface AgentPlugin {
  initialize(): Promise<void>;
  getTools(): Record<string, any>;
}
```

### 3.4 Error Handling

```typescript
// packages/agents/src/plugins/error-handling.ts
export class PluginError extends Error {
  constructor(message: string, public readonly pluginName: string, public readonly cause?: Error) {
    super(`Plugin ${pluginName} error: ${message}`);
    this.name = "PluginError";
  }
}

export function handlePluginError(error: unknown, pluginName: string): never {
  if (error instanceof PluginError) {
    throw error;
  }
  
  const message = error instanceof Error ? error.message : String(error);
  throw new PluginError(message, pluginName, error instanceof Error ? error : undefined);
}
```

## 4. Testing Plan

1. **Unit Tests**:
   - Test plugin initialization
   - Test tool wrapping with error handling
   - Test plugin integration with agent

2. **Integration Tests**:
   - Test end-to-end GitHub operations
   - Test error scenarios (disconnected MCP, invalid tokens)
   - Test concurrent tool usage

## 5. Implementation Steps

1. Create plugin interface and base classes
2. Implement the MCP GitHub plugin
3. Update the Coder agent to use plugins
4. Add error handling for plugin operations
5. Update system prompt to include GitHub capabilities
6. Create tests for the implementation

## 6. Refactoring Opportunities

1. Extract tool processing logic into reusable utility
2. Create a plugin manager for handling multiple plugins
3. Implement dynamic tool loading based on user requests

## 7. Completion Criteria

1. Coder agent can access GitHub tools via MCP
2. GitHub operations work end-to-end
3. Error handling is robust
4. Documentation is updated with new capabilities
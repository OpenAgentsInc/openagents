# Revised Implementation Plan for MCP GitHub Tool Integration

After thorough analysis of the repository, here is the revised implementation plan to integrate MCP GitHub tools into the Coder agent.

## 1. Overview

The goal is to integrate the Model Context Protocol (MCP) GitHub tools with the Coder agent in OpenAgents. This integration will enable the Coder agent running in Cloudflare Workers to access GitHub functionality through the MCP protocol.

## 2. Current State Analysis

### MCP Implementation
- A robust MCP client implementation exists in `apps/coder/src/server/mcp-clients.ts`
- MCP tools are exposed through `apps/coder/src/server/tools/mcp-tools.ts`
- MCP endpoints defined in `apps/coder/src/server/routes/mcp.ts`
- Chat endpoint in `apps/coder/src/server/routes/chat.ts` already integrates with MCP tools

### Coder Agent Implementation
- Agent class is defined in `packages/agents/src/server.ts` 
- Uses a simple tool system in `packages/agents/src/tools.ts`
- Tool execution logic in `packages/agents/src/utils.ts`
- Currently no integration between MCP tools and Coder agent

### Problem Statement
The current implementation has a gap: The MCP GitHub tools are available in the Node.js server environment but not in the Cloudflare Worker environment where the Coder agent runs. We need to bridge this gap.

## 3. Implementation Strategy

Since the Cloudflare Worker environment is different from the Node.js environment, we need a specialized approach:

1. **Proxy Pattern**: Instead of directly embedding the MCP client in the worker, create a proxy mechanism to forward tool calls from the worker to the MCP service.

2. **Tool Registration**: Register GitHub tools with the agent while maintaining the Worker-compatible format.

3. **Error Handling**: Implement robust error handling for network issues, token validation, etc.

## 4. Detailed Implementation Steps

### 4.1. Add Worker Plugins Support in Server.ts

```typescript
// packages/agents/src/plugins/plugin-interface.ts
export interface AgentPlugin {
  name: string;
  initialize(agent: AIChatAgent<any>): Promise<void>;
  getTools(): Record<string, any>;
}

// For GitHub plugin specifically
export interface GitHubPlugin extends AgentPlugin {
  name: 'github';
}
```

### 4.2. Create MCP GitHub Plugin for the Worker Environment

```typescript
// packages/agents/src/plugins/github-plugin.ts
import { AgentPlugin } from './plugin-interface';
import { AIChatAgent } from 'agents/ai-chat-agent';
import { tool } from 'ai';
import { z } from 'zod';

export class GitHubPlugin implements AgentPlugin {
  name = 'github';
  private agent: AIChatAgent<any> | null = null;
  private readonly gitHubTools: Record<string, any> = {};

  constructor() {
    // Define the GitHub tools that will be exposed to the agent
    this.gitHubTools = {
      // List repositories
      githubListRepos: tool({
        description: "List repositories for a GitHub user or organization",
        parameters: z.object({
          owner: z.string().describe("The GitHub username or organization name"),
          limit: z.number().optional().describe("Maximum number of repositories to return")
        }),
        execute: async ({ owner, limit = 10 }) => {
          try {
            return await this.callMCPTool('github_list_repos', { owner, limit });
          } catch (error) {
            console.error("Error listing GitHub repos:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Get repository details
      githubGetRepo: tool({
        description: "Get details about a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name")
        }),
        execute: async ({ owner, repo }) => {
          try {
            return await this.callMCPTool('github_get_repo', { owner, repo });
          } catch (error) {
            console.error("Error getting GitHub repo:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // List issues
      githubListIssues: tool({
        description: "List issues in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          state: z.enum(['open', 'closed', 'all']).optional().describe("Issue state (open, closed, all)"),
          limit: z.number().optional().describe("Maximum number of issues to return")
        }),
        execute: async ({ owner, repo, state = 'open', limit = 10 }) => {
          try {
            return await this.callMCPTool('github_list_issues', { owner, repo, state, limit });
          } catch (error) {
            console.error("Error listing GitHub issues:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Create issue
      githubCreateIssue: tool({
        description: "Create a new issue in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          title: z.string().describe("Issue title"),
          body: z.string().describe("Issue description/body")
        }),
        execute: async ({ owner, repo, title, body }) => {
          try {
            return await this.callMCPTool('github_create_issue', { owner, repo, title, body });
          } catch (error) {
            console.error("Error creating GitHub issue:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Get file contents
      githubGetFileContents: tool({
        description: "Get contents of a file from a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          path: z.string().describe("Path to the file in the repository"),
          ref: z.string().optional().describe("Branch, tag, or commit SHA")
        }),
        execute: async ({ owner, repo, path, ref }) => {
          try {
            return await this.callMCPTool('github_get_file_contents', { owner, repo, path, ref });
          } catch (error) {
            console.error("Error getting file contents:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),
    };
  }

  async initialize(agent: AIChatAgent<any>): Promise<void> {
    this.agent = agent;
    console.log("GitHub plugin initialized");
  }

  getTools(): Record<string, any> {
    return this.gitHubTools;
  }

  /**
   * Call an MCP tool by making a fetch request to the MCP API
   * This is a proxy method that bridges the Worker environment to the MCP service
   */
  private async callMCPTool(toolName: string, params: any): Promise<string> {
    if (!this.agent) {
      throw new Error("GitHub plugin not properly initialized");
    }

    try {
      // Make request to MCP bridge API
      const response = await fetch(`https://api.openagents.com/mcp/execute`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          tool: toolName,
          parameters: params
        })
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`MCP tool execution failed: ${errorText}`);
      }

      const result = await response.json();
      return result.text || JSON.stringify(result);
    } catch (error) {
      console.error(`Error executing MCP tool ${toolName}:`, error);
      throw error;
    }
  }
}
```

### 4.3. MCP Execution Bridge API

We need to create an API endpoint in the Node.js server that can receive requests from the Coder agent running in Cloudflare Workers:

```typescript
// apps/coder/src/server/routes/mcp.ts (add to existing file)

// Add a new endpoint for proxying tool execution requests
mcpRoutes.post('/execute', async (c) => {
  try {
    const body = await c.req.json();
    
    // Validate request
    if (!body.tool || !body.parameters) {
      return c.json({ error: 'Tool name and parameters are required' }, 400);
    }
    
    const toolName = body.tool;
    const parameters = body.parameters;
    
    console.log(`[MCP API] Executing tool ${toolName} with parameters:`, parameters);
    
    // Get MCP tools
    const mcpTools = getMCPTools();
    
    // Check if tool exists
    if (!mcpTools[toolName]) {
      return c.json({ error: `Tool ${toolName} not found` }, 404);
    }
    
    // Execute the tool
    try {
      const result = await mcpTools[toolName].execute(parameters);
      console.log(`[MCP API] Tool ${toolName} execution result:`, result);
      
      // Return the result
      return c.json(result);
    } catch (toolError) {
      console.error(`[MCP API] Error executing tool ${toolName}:`, toolError);
      
      // Return error
      return c.json({ 
        error: `Tool execution failed: ${toolError instanceof Error ? toolError.message : String(toolError)}` 
      }, 500);
    }
  } catch (error) {
    console.error('[MCP API] Error processing tool execution request:', error);
    
    return c.json({ 
      error: 'Failed to process tool execution request',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});
```

### 4.4. Update the Coder Agent to Support Plugins

```typescript
// packages/agents/src/server.ts

import { GitHubPlugin } from './plugins/github-plugin';
import type { AgentPlugin } from './plugins/plugin-interface';

export class Coder extends AIChatAgent<Env> {
  private plugins: AgentPlugin[] = [];
  private combinedTools: Record<string, any>;

  constructor(state: DurableObjectState, env: Env) {
    super(state, env);
    
    // Initialize with the base tools
    this.combinedTools = { ...tools };
    
    // Add the GitHub plugin
    this.plugins.push(new GitHubPlugin());
    
    // Initialize plugins
    this.initializePlugins().catch(err => 
      console.error("Failed to initialize agent plugins:", err)
    );
  }
  
  private async initializePlugins(): Promise<void> {
    try {
      // Initialize each plugin
      for (const plugin of this.plugins) {
        await plugin.initialize(this);
        
        // Get tools from the plugin
        const pluginTools = plugin.getTools();
        
        // Add tools to the combined tools
        this.combinedTools = { ...this.combinedTools, ...pluginTools };
        
        console.log(`Initialized plugin: ${plugin.name} with ${Object.keys(pluginTools).length} tools`);
      }
      
      console.log(`Total tools available: ${Object.keys(this.combinedTools).length}`);
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
            If the user asks for github information, use the github tools.
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

## 5. Testing Plan

1. **Unit Tests**:
   - Test GitHub plugin initialization
   - Test tool registration with agent
   - Test the proxy communication mechanism

2. **Integration Tests**:
   - Test end-to-end GitHub operations via the agent
   - Test error handling (network errors, GitHub API errors)
   - Test authentication failures

## 6. Deployment Considerations

1. **API Gateway**: Set up proper CORS and authentication for the execution bridge API
2. **Rate Limiting**: Implement rate limiting to prevent abuse
3. **GitHub Token Management**: Secure storage and validation of GitHub tokens
4. **Error Reporting**: Add comprehensive error reporting to diagnose issues

## 7. Future Enhancements

1. **Token Refresh**: Implement automatic refresh of expired tokens
2. **Tool Discovery**: Enable dynamic discovery of available MCP tools
3. **Caching**: Add caching for frequently used GitHub data to reduce API calls
4. **Access Control**: Add fine-grained access control for GitHub operations
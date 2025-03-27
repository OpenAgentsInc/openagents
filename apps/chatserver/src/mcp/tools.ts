import { mcpClientManager } from './client';

export interface ToolParameter {
  type: string;
  description: string;
}

export interface ToolDefinition {
  name: string;
  description: string;
  parameters: {
    type: string;
    properties: Record<string, ToolParameter>;
    required: string[];
  };
}

export interface ToolCallPayload {
  toolCallId: string;
  toolName: string;
  args: Record<string, any>;
}

export interface ToolResultPayload {
  toolCallId: string;
  toolName: string;
  args: Record<string, any>;
  result: any;
}

/**
 * Extracts tool definitions from all connected MCP servers
 * in a format compatible with LLM tool definitions.
 */
export function extractToolDefinitions(): ToolDefinition[] {
  const tools = mcpClientManager.getAllTools();
  
  // For now, return a predefined set of GitHub tools
  // In the future, we'd dynamically convert from MCP tool schemas
  return [
    {
      name: "create_issue",
      description: "Create a new issue in a GitHub repository",
      parameters: {
        type: "object",
        properties: {
          owner: { type: "string", description: "Repository owner" },
          repo: { type: "string", description: "Repository name" },
          title: { type: "string", description: "Issue title" },
          body: { type: "string", description: "Issue body" }
        },
        required: ["owner", "repo", "title"]
      }
    },
    {
      name: "get_file_contents",
      description: "Get the contents of a file from a GitHub repository",
      parameters: {
        type: "object",
        properties: {
          owner: { type: "string", description: "Repository owner" },
          repo: { type: "string", description: "Repository name" },
          path: { type: "string", description: "File path in the repository" },
          branch: { type: "string", description: "Branch name (optional)" }
        },
        required: ["owner", "repo", "path"]
      }
    },
    {
      name: "search_repositories",
      description: "Search for GitHub repositories",
      parameters: {
        type: "object",
        properties: {
          query: { type: "string", description: "Search query" },
          sort: { type: "string", description: "Sort field (stars, forks, updated, etc.)" },
          order: { type: "string", description: "Sort order (asc or desc)" },
          per_page: { type: "number", description: "Results per page" }
        },
        required: ["query"]
      }
    },
    {
      name: "list_issues",
      description: "List issues in a GitHub repository",
      parameters: {
        type: "object",
        properties: {
          owner: { type: "string", description: "Repository owner" },
          repo: { type: "string", description: "Repository name" },
          state: { type: "string", description: "Issue state (open, closed, all)" },
          labels: { type: "string", description: "Comma-separated list of label names" },
          sort: { type: "string", description: "Sort field (created, updated, comments)" },
          direction: { type: "string", description: "Sort direction (asc or desc)" }
        },
        required: ["owner", "repo"]
      }
    },
    {
      name: "get_issue",
      description: "Get details of a specific issue in a GitHub repository",
      parameters: {
        type: "object",
        properties: {
          owner: { type: "string", description: "Repository owner" },
          repo: { type: "string", description: "Repository name" },
          issue_number: { type: "number", description: "Issue number" }
        },
        required: ["owner", "repo", "issue_number"]
      }
    }
  ];
}

/**
 * Process a tool call by routing it to the appropriate MCP server.
 * @param toolCall The tool call from the LLM
 * @param authToken Optional authentication token to pass to the MCP server
 * @returns The tool result
 */
export async function processToolCall(toolCall: ToolCallPayload, authToken?: string): Promise<ToolResultPayload | null> {
  if (!toolCall) {
    console.log("⚠️ Received null tool call");
    return null;
  }
  
  console.log(`🔧 Processing tool call: ${toolCall.toolName}`);
  console.log(`📦 Tool args: ${JSON.stringify(toolCall.args).substring(0, 200)}`);
  console.log(`🔑 Auth token present: ${!!authToken}`);
  
  // Check if we know about this tool
  const toolServer = mcpClientManager.getToolServer(toolCall.toolName);
  
  if (!toolServer) {
    console.error(`❌ Unknown tool: ${toolCall.toolName}`);
    const allTools = mcpClientManager.getAllTools();
    console.log(`🧰 Available tools: ${allTools.map(t => t.name).join(', ') || 'none'}`);
    
    return {
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      args: toolCall.args,
      result: { error: `Tool "${toolCall.toolName}" not found in any connected MCP server` }
    };
  }
  
  console.log(`🔄 Routing tool call to server: ${toolServer}`);
  
  try {
    // Set a timeout to avoid hanging indefinitely
    const toolPromise = mcpClientManager.callTool(
      toolCall.toolName,
      toolCall.args,
      authToken
    );
    
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error(`Tool call ${toolCall.toolName} timed out after 15 seconds`)), 15000);
    });
    
    const result = await Promise.race([toolPromise, timeoutPromise]);
    
    console.log(`✅ Tool call successful: ${JSON.stringify(result).substring(0, 200)}`);
    
    return {
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      args: toolCall.args,
      result
    };
  } catch (error) {
    console.error(`❌ Error processing tool call ${toolCall.toolName}:`, error);
    
    return {
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      args: toolCall.args,
      result: { error: error instanceof Error ? (error as Error).message : 'Unknown error' }
    };
  }
}

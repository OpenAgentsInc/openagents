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
 * in a format compatible with LLM tool definitions (Vercel AI SDK).
 */
export function extractToolDefinitions(): Record<string, ToolDefinition> {
  const discoveredTools = mcpClientManager.getAllTools();
  console.log(`[extractToolDefinitions] Discovered ${discoveredTools.length} tools from MCP Manager.`);

  if (discoveredTools.length === 0) {
    console.warn("[extractToolDefinitions] No tools discovered. Returning empty object.");
    return {};
  }

  // Create a record of tool definitions keyed by tool name
  const toolDefinitions: Record<string, ToolDefinition> = {};

  // For debugging, only use a single tool with minimal schema
  const singleToolName = "create_issue"; // Pick a simple tool to focus on
  
  discoveredTools.forEach(mcpTool => {
    console.log(`[extractToolDefinitions] Mapping tool: ${mcpTool.name}`);

    // Skip all tools except our chosen test tool for debugging
    if (mcpTool.name !== singleToolName) {
      console.log(`[extractToolDefinitions] Skipping tool ${mcpTool.name} for debugging`);
      return;
    }

    // --- MINIMAL SCHEMA FOR DEBUGGING ---
    // Create the simplest possible valid JSON Schema for parameters:
    // An object type with minimal defined properties
    const minimalParameters: ToolDefinition['parameters'] = {
        type: "object",
        properties: {
          // Just include one property for testing
          repo: { type: "string", description: "Repository name" }
        },
        required: []  // No required fields for simplicity
    };
    // --- END MINIMAL SCHEMA ---

    // Basic validation: Ensure tool name exists
    if (!mcpTool.name) {
        console.warn(`[extractToolDefinitions] Skipping tool with missing name`);
        return; // Skip this tool
    }

    // Create and store the tool definition
    toolDefinitions[mcpTool.name] = {
      name: mcpTool.name,
      description: mcpTool.description || `Execute the ${mcpTool.name} tool.`,
      parameters: minimalParameters // Use the minimal schema
    };
    
    console.log(`[extractToolDefinitions] Added minimal schema for ${mcpTool.name}:`, 
      JSON.stringify(toolDefinitions[mcpTool.name], null, 2));
  });
  
  // Log the final tools count
  console.log(`[extractToolDefinitions] Mapped ${Object.keys(toolDefinitions).length} tools with MINIMAL schema`);
  console.log('[extractToolDefinitions] Final minimal tools structure keys:', Object.keys(toolDefinitions).join(', '));
  
  return toolDefinitions;
}

/**
 * Process a tool call by routing it to the appropriate MCP server.
 * @param toolCall The tool call from the LLM
 * @param authToken Optional authentication token to pass to the MCP server
 * @returns The tool result payload with result or error
 */
export async function processToolCall(toolCall: ToolCallPayload, authToken?: string): Promise<ToolResultPayload> {
  if (!toolCall) {
    console.log("⚠️ Received null tool call");
    return {
      toolCallId: 'unknown',
      toolName: 'unknown',
      args: {},
      result: { error: 'Received null tool call data' }
    };
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
      result: { error: error instanceof Error ? (error as Error).message : 'Unknown error during tool execution' }
    };
  }
}

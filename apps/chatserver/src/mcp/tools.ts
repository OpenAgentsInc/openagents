import { mcpClientManager } from './client';

export interface ToolParameter {
  type: string;
  description?: string; // Allow optional description
}

export interface ToolDefinition {
  name: string;
  description: string;
  parameters: {
    type: 'object'; // Must be object for properties
    properties: Record<string, ToolParameter>;
    required?: string[]; // Allow optional required array
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
 * FOR DEBUGGING: Returns only 'create_issue' with ABSOLUTE MINIMAL schema.
 */
export function extractToolDefinitions(): Record<string, ToolDefinition> {
  const discoveredToolInfos = mcpClientManager.getAllTools();
  console.log(`[extractToolDefinitions] Received ${discoveredToolInfos.length} tool infos from MCP Manager.`);

  const toolDefinitions: Record<string, ToolDefinition> = {};
  const singleToolName = "create_issue"; // Focus on this tool

  // Find the specific tool info
  const toolInfo = discoveredToolInfos.find(info => info.tool?.name === singleToolName);

  if (toolInfo && toolInfo.tool) {
    const mcpTool = toolInfo.tool;
    const toolName = mcpTool.name;
    console.log(`[extractToolDefinitions] Mapping ABSOLUTE MINIMAL tool: ${toolName}`);

    // --- ABSOLUTE MINIMAL SCHEMA ---
    const minimalParameters: ToolDefinition['parameters'] = {
        type: "object",
        properties: {}, // NO PROPERTIES
        // required: [] // OMITTING required array entirely for maximum simplicity
    };
    // --- END MINIMAL SCHEMA ---

    const toolDescription = mcpTool.description || `Executes the ${toolName} tool.`; // Ensure description

    toolDefinitions[toolName] = {
      name: toolName,
      description: toolDescription,
      parameters: minimalParameters
    };

    console.log(`[extractToolDefinitions] Added ABSOLUTE MINIMAL schema for ${toolName}:`,
      JSON.stringify(toolDefinitions[toolName], null, 2));

  } else {
      console.warn(`[extractToolDefinitions] Tool '${singleToolName}' not found among discovered tools.`);
  }

  console.log(`[extractToolDefinitions] Finished mapping ${Object.keys(toolDefinitions).length} tools with ABSOLUTE MINIMAL schema.`);
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
      result: { error: error instanceof Error ? error.message : 'Unknown error' }
    };
  }
}

import { mcpClientManager } from './client';
import { tool } from 'ai'; // Import the 'tool' helper
import { z } from 'zod'; // Import zod for parameters

// Keep ToolCallPayload and ToolResultPayload as they are useful for processToolCall
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
 * Extracts tool definitions, mapping MCP tools to the AI SDK 'tool' format.
 * FOR DEBUGGING: Returns only 'create_issue' with Zod schema.
 */
export function extractToolDefinitions(): Record<string, ReturnType<typeof tool>> {
  const discoveredToolInfos = mcpClientManager.getAllTools();
  console.log(`[extractToolDefinitions] Received ${discoveredToolInfos.length} tool infos from MCP Manager.`);

  const toolDefinitions: Record<string, ReturnType<typeof tool>> = {};
  const singleToolName = "create_issue"; // Focus on this tool

  const toolInfo = discoveredToolInfos.find(info => info.tool?.name === singleToolName);

  if (toolInfo && toolInfo.tool) {
    const mcpTool = toolInfo.tool;
    const toolName = mcpTool.name;
    console.log(`[extractToolDefinitions] Mapping tool with Zod schema: ${toolName}`);

    // --- Define Parameters using Zod ---
    // Use Zod to define the parameters schema based on your MCP tool's inputSchema
    // This provides better type safety and validation than plain JSON schema objects.
    // Adapt this Zod schema based on the *actual* required parameters for create_issue.
    const parametersSchema = z.object({
        owner: z.string().describe("Repository owner (e.g., 'OpenAgentsInc')"),
        repo: z.string().describe("Repository name (e.g., 'openagents')"),
        title: z.string().describe("The title of the new issue"),
        body: z.string().optional().describe("The body/content of the issue (optional)"),
    });
    // --- End Zod Schema ---

    const toolDescription = mcpTool.description || `Create a new issue in a GitHub repository.`;

    // --- Use the 'tool' helper ---
    // Pass the Zod schema to the 'parameters' property.
    // CRUCIALLY: Omit the 'execute' property. The SDK will emit a tool call
    //            that we handle separately via result.onToolCall.
    toolDefinitions[toolName] = tool({
      description: toolDescription,
      parameters: parametersSchema,
      // NO 'execute' function here
    });
    // --- End 'tool' helper ---

    console.log(`[extractToolDefinitions] Added Zod-based schema for ${toolName}`);

  } else {
      console.warn(`[extractToolDefinitions] Tool '${singleToolName}' not found.`);
  }

  console.log(`[extractToolDefinitions] Finished mapping ${Object.keys(toolDefinitions).length} tools.`);
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
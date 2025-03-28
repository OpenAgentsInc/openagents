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
 * Maps all discovered GitHub tools to Zod schemas.
 */
export function extractToolDefinitions(): Record<string, ReturnType<typeof tool>> {
  const discoveredToolInfos = mcpClientManager.getAllTools();
  console.log(`[extractToolDefinitions] Received ${discoveredToolInfos.length} tool infos from MCP Manager.`);

  const toolDefinitions: Record<string, ReturnType<typeof tool>> = {};

  if (discoveredToolInfos.length === 0) {
    console.warn("[extractToolDefinitions] No tool infos returned from mcpClientManager.getAllTools().");
    return {};
  }

  // Process all discovered tools
  discoveredToolInfos.forEach(toolInfo => {
    if (!toolInfo.tool || typeof toolInfo.tool !== 'object' || typeof toolInfo.tool.name !== 'string') {
      console.warn(`[extractToolDefinitions] Skipping invalid tool info:`, toolInfo);
      return;
    }

    const mcpTool = toolInfo.tool;
    const toolName = mcpTool.name;
    // console.log(`[extractToolDefinitions] Mapping tool: ${toolName}`);

    try {
      // Create dynamic Zod schema based on inputSchema
      let parametersSchema: z.ZodObject<any> = z.object({});

      const inputSchema = (mcpTool as any).inputSchema;

      if (inputSchema && typeof inputSchema === 'object' && inputSchema.properties) {
        const schemaObj: Record<string, any> = {};

        // Create Zod properties from inputSchema properties
        for (const [paramName, paramDef] of Object.entries(inputSchema.properties)) {
          if (typeof paramDef === 'object') {
            const paramType = (paramDef as any).type;
            const paramDesc = (paramDef as any).description || `Parameter ${paramName}`;

            if (paramType === 'string') {
              schemaObj[paramName] = Array.isArray(inputSchema.required) &&
                inputSchema.required.includes(paramName)
                ? z.string().describe(paramDesc)
                : z.string().optional().describe(paramDesc);
            } else if (paramType === 'number' || paramType === 'integer') {
              schemaObj[paramName] = Array.isArray(inputSchema.required) &&
                inputSchema.required.includes(paramName)
                ? z.number().describe(paramDesc)
                : z.number().optional().describe(paramDesc);
            } else if (paramType === 'boolean') {
              schemaObj[paramName] = Array.isArray(inputSchema.required) &&
                inputSchema.required.includes(paramName)
                ? z.boolean().describe(paramDesc)
                : z.boolean().optional().describe(paramDesc);
            } else if (paramType === 'array') {
              // Default to array of strings if items type is not specified
              schemaObj[paramName] = Array.isArray(inputSchema.required) &&
                inputSchema.required.includes(paramName)
                ? z.array(z.string()).describe(paramDesc)
                : z.array(z.string()).optional().describe(paramDesc);
            } else if (paramType === 'object') {
              // For nested objects, use a simpler approach
              schemaObj[paramName] = Array.isArray(inputSchema.required) &&
                inputSchema.required.includes(paramName)
                ? z.record(z.unknown()).describe(paramDesc)
                : z.record(z.unknown()).optional().describe(paramDesc);
            } else {
              // Default to string for unknown types
              schemaObj[paramName] = Array.isArray(inputSchema.required) &&
                inputSchema.required.includes(paramName)
                ? z.string().describe(paramDesc)
                : z.string().optional().describe(paramDesc);
            }
          }
        }

        // Create the schema object if we have properties
        if (Object.keys(schemaObj).length > 0) {
          parametersSchema = z.object(schemaObj);
        }
      } else {
        console.warn(`[extractToolDefinitions] Tool '${toolName}' has no valid inputSchema, using empty schema.`);
      }

      const toolDescription = mcpTool.description || `Executes the ${toolName} tool.`;

      // Create tool definition WITH execute function for automatic execution
      const toolDef = tool({
        description: toolDescription,
        parameters: parametersSchema,
        // Using type assertion to allow execute function within the expected type
        execute: (async (args: any, options: any) => {
          // Include the toolCallId in the logs
          console.log(`🧰 Executing tool ${toolName} [${options?.toolCallId}] with args:`,
            JSON.stringify(args).substring(0, 200));

          try {
            // Get the auth token - need to get this from closure since we can't pass it directly
            // through the options parameter in a production deployment
            const authHeader = options.headers?.Authorization;
            const tokenHeader = options.headers?.['X-GitHub-Token'];
            const authToken = authHeader?.replace('Bearer ', '') || tokenHeader || null;

            console.log(`🔐 Tool ${toolName} execute() auth token info:`);
            console.log(`  - Authorization header present: ${!!authHeader}`);
            console.log(`  - X-GitHub-Token header present: ${!!tokenHeader}`);
            console.log(`  - Final token resolved: ${authToken ? 'Yes' : 'No'}`);
            console.log(`  - Headers keys available: ${options.headers ? Object.keys(options.headers).join(', ') : 'none'}`);

            // Call the MCP tool via client manager with the auth token
            const result = await mcpClientManager.callTool(toolName, args, authToken);
            console.log(`✅ Tool ${toolName} execution successful`);
            return result;
          } catch (error) {
            console.error(`❌ Tool ${toolName} execution failed:`, error);
            // Return error in a format that can be shown to the user
            return { error: error instanceof Error ? error.message : String(error) };
          }
        }) as any
      }) as any;

      // Add the tool to our definitions
      toolDefinitions[toolName] = toolDef;

      // console.log(`[extractToolDefinitions] Added Zod-based schema for ${toolName}`);

    } catch (error) {
      console.error(`[extractToolDefinitions] Error creating schema for tool '${toolName}':`, error);
      // Skip this tool if there was an error
    }
  });

  const toolCount = Object.keys(toolDefinitions).length;
  console.log(`[extractToolDefinitions] Finished mapping ${toolCount} tools.`);
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

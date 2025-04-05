/**
 * MCP tools integration
 */

import { getMCPClients } from '../mcp-clients';
import { 
  ToolError, 
  ToolExecutionError,
  transformToolError
} from '@openagents/core/src/chat/errors';

/**
 * Get all available MCP tools
 */
export function getMCPTools(): Record<string, any> {
  try {
    // Get the globally initialized MCP clients
    const { allTools } = getMCPClients();
    
    // Log the available tools for diagnostic purposes
    console.log(`[MCP Tools] Retrieved ${Object.keys(allTools || {}).length} tools from MCP clients:`);
    Object.keys(allTools || {}).forEach(toolId => {
      console.log(`[MCP Tools] - Available tool: ${toolId}`);
    });
    
    // Return the raw tools directly without any mocks or modifications
    return allTools || {};
  } catch (error) {
    console.error("Error getting MCP tools:", error);
    return {};
  }
}

/**
 * Wrap MCP tools with error handling
 */
export function wrapMCPToolsWithErrorHandling(tools: Record<string, any>): Record<string, any> {
  const wrappedTools: Record<string, any> = {};
  
  // Wrap each tool with error handling
  for (const [toolName, toolImplementation] of Object.entries(tools)) {
    wrappedTools[toolName] = {
      ...toolImplementation,
      // Wrap the execute function with error handling
      execute: async (...args: any[]) => {
        try {
          // Call the original execute function
          return await toolImplementation.execute(...args);
        } catch (error) {
          // Transform the error to a proper ToolError
          const toolError = error instanceof ToolError
            ? error
            : transformToolError(error, toolName);
          
          console.error(`Error executing MCP tool ${toolName}:`, toolError);
          
          // Rethrow the transformed error
          throw toolError;
        }
      }
    };
  }
  
  return wrappedTools;
}

/**
 * Error handler for MCP tool execution failures
 */
export function handleMCPToolError(error: unknown, toolName: string): never {
  // Transform the error to a proper ToolError if needed
  const toolError = error instanceof ToolError
    ? error
    : transformToolError(error, toolName);
  
  // Log the error
  console.error(`Error executing MCP tool ${toolName}:`, toolError);
  
  // Throw the error with the proper type and formatting
  throw toolError;
}
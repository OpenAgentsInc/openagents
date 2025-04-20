import { Effect } from "effect";
import { solverTools } from "./tools";
import { solverContext } from "./tools";

// --- Utility functions for tool formatting ---

// Format our tools for Anthropic's API
export function formatToolsForAnthropic() {
  return Object.entries(solverTools).map(([name, toolObj]) => {
    // Get tool definition and cast to any to avoid TypeScript errors
    const tool = toolObj as any;
    
    // Extract parameter information from Zod schema
    const parameterProperties: Record<string, { type: string, description: string }> = {};
    const requiredParams: string[] = [];
    
    // Process each parameter in the Zod schema
    Object.entries((tool.parameters as any)._def.shape()).forEach(([paramName, paramSchema]) => {
      // Extract the type and description
      // This is a simplified approach - in a real implementation you would
      // need to handle more complex Zod schemas
      const anyParamSchema = paramSchema as any;
      const isOptional = anyParamSchema.constructor?.name === 'ZodOptional';
      const baseSchema = isOptional ? anyParamSchema._def?.innerType : anyParamSchema;
      
      let type = 'string'; // Default type
      if (baseSchema?.constructor?.name === 'ZodNumber') {
        type = 'number';
      } else if (baseSchema?.constructor?.name === 'ZodBoolean') {
        type = 'boolean';
      } else if (baseSchema?.constructor?.name === 'ZodArray') {
        type = 'array';
      } else if (baseSchema?.constructor?.name === 'ZodObject') {
        type = 'object';
      }
      
      // Get description if available
      const description = baseSchema?.description || paramName;
      
      // Add to properties
      parameterProperties[paramName] = {
        type,
        description
      };
      
      // If not optional, add to required list
      if (!isOptional) {
        requiredParams.push(paramName);
      }
    });
    
    // Format for Anthropic's API
    return {
      name,
      description: tool.description,
      input_schema: {
        type: "object",
        properties: parameterProperties,
        required: requiredParams
      }
    };
  });
}

// Execute a tool by name with params
export async function executeToolByName(
  toolName: string, 
  params: Record<string, any>
): Promise<any> {
  try {
    // Get the tool from our solverTools with proper type guard
    if (!(toolName in solverTools)) {
      return { error: `Tool '${toolName}' not found` };
    }
    
    const tool = solverTools[toolName as keyof typeof solverTools];
    
    // Execute the tool with type assertion to avoid TypeScript errors
    // This is safe because we're passing the exact parameters from Anthropic
    // which match the tool's schema
    const options = {}; // Empty options object required by Vercel AI SDK
    const result = await (tool.execute as any)(params, options);
    return result;
  } catch (error) {
    console.error(`Failed to execute tool ${toolName}:`, error);
    return { error: String(error) };
  }
}
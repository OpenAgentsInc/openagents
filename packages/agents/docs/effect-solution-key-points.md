# Key Points for Effect Integration with Vercel AI SDK

## Problem Statement
We needed to integrate Effect-based tools with the Vercel AI SDK's tool interface, which expects Promise-returning functions. The key challenge was properly managing the boundary between the Effect type system and the Promise-based interface of Vercel AI SDK.

## Solution

### 1. Create an Effect Tool Factory

We created an `effectTool` factory function that takes an Effect-returning function and wraps it in a Promise for Vercel AI SDK compatibility. It also adds an `effectBased: true` marker for optional detection.

```typescript
// effect-tool.ts
import { tool } from "ai";
import { Effect, Cause } from "effect";
import { z } from "zod";

export function effectTool<P extends z.ZodType<any, any, any>, R>(
  definition: {
    description: string;
    parameters: P;
    execute: (params: z.infer<P>, options: {}) => Effect.Effect<R, any, never>;
  }
) {
  // Create a Promise-returning execute function that wraps the Effect
  const promiseExecute = (params: any, options: {}) => {
    const effect = definition.execute(params, options);
    return Effect.runPromise(effect).catch((fiberFailure) => {
      // Extract and analyze the cause for rich error reporting
      const cause = (fiberFailure as any).cause;
      let errorMessage = "Tool execution failed.";
      let errorDetails = {};
      
      // Rich Cause analysis to extract detailed error information
      if (Cause.isFailType(cause)) {
        // Extract information from TaggedError or other errors
        // ...
      } else if (Cause.isDieType(cause)) {
        // Handle defects (programming errors)
        // ...
      }
      
      // Create an enriched error that preserves detailed information
      const enrichedError = new Error(errorMessage);
      (enrichedError as any).effectError = errorDetails;
      (enrichedError as any).causeType = cause._tag;
      
      throw enrichedError;
    });
  };

  // Create the tool using Vercel AI's tool function
  const baseTool = tool({
    description: definition.description,
    parameters: definition.parameters,
    execute: promiseExecute
  });

  // Add the effectBased marker
  return Object.assign(baseTool, { effectBased: true } as const);
}
```

### 2. Define Effect-Based Tools

Use the factory to create Effect-based tools without dealing with Promise conversion manually:

```typescript
export const fetchFileContents = effectTool({
  description: "Fetches the contents of a specific file from a GitHub repository.",
  parameters: GetFileContentsParams,
  execute: ({ owner, repo, path, branch }) => {
    // Return an Effect directly - no need to convert to Promise here
    return Effect.gen(function* () {
      // Access agent state via AsyncLocalStorage
      const agent = yield* Effect.sync(() => solverContext.getStore());
      
      // Implementation with proper Effect composition...
      
      // Return Success
      return decodedContent;
    });
  }
});
```

### 3. Simplified Tool Execution

Since our `effectTool` factory now handles the Effect-to-Promise conversion, we no longer need special handling in the `executeToolEffect` method:

```typescript
private async executeToolEffect(toolCall: ToolCallPart): Promise<ToolResultPart> {
  const { toolName, args, toolCallId } = toolCall;
  const tool = solverTools[toolName as SolverToolName];
  
  // ... tool existence check...
  
  try {
    // All tools can be executed the same way, regardless of being Effect-based or not
    // because effectTool wraps Effects in Promises
    console.log(`[executeToolEffect] Executing tool '${toolName}'...`);
    const execFn = tool.execute as (a: any, o: any) => Promise<any>;
    const resultValue = await Promise.resolve(execFn(args, {}));
    console.log(`[executeToolEffect] Tool '${toolName}' completed successfully.`);
    
    return {
      type: 'tool-result',
      toolCallId,
      toolName,
      result: resultValue
    } as ToolResultPart;
  } catch (error) {
    // Check if this is an enriched error from our effectTool
    const isEnrichedError = error instanceof Error && 
                           (error as any).effectError !== undefined;
    
    // Create the base error message
    let errorMessage = error instanceof Error 
      ? error.message 
      : `Tool '${toolName}' failed: ${String(error)}`;
    
    // Return error with detailed information if available
    return {
      type: 'tool-result',
      toolCallId,
      toolName,
      result: { 
        error: errorMessage,
        // Include additional error details from effectTool if available
        ...(isEnrichedError && { 
          errorDetails: (error as any).effectError,
          errorType: (error as any).causeType
        })
      }
    } as ToolResultPart;
  }
}
```

## Tool Design Principles
- Use `never` for the R type parameter if no explicit requirements 
- Wrap context access in `Effect.sync()`
- Use tagged errors from `Data.TaggedError` for precise type-safe error handling
- Let the effectTool factory handle the conversion to Promise
- Separate error handling logic from business logic
- Preserve detailed error information across the Effect-Promise boundary
- Return structured error details to the LLM for better error handling

## Benefits
1. **Type Safety**: Maintain strong typing within the Effect-based tools
2. **Composability**: Leverage Effect's composable async operations
3. **Rich Error Handling**: Get detailed error information with Effect's Cause analysis
4. **Error Context Preservation**: Maintain error context across the Effect-Promise boundary
5. **Clean Boundary**: The `effectTool` factory handles all the boundary complexity
6. **Maintainability**: The design pattern is consistent and easy to understand
7. **Improved LLM Responses**: Structured error details help the LLM respond more accurately to errors

## Future Considerations
- Create a dedicated tool registry/factory system for more flexibility
- Consider a more optimized approach to error handling and logging
- Add support for streaming tools that yield intermediate results
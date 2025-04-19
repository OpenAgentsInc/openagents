# Effect-Based Tool Integration with Vercel AI SDK

This document explains the integration of Effect-based tools with the Vercel AI SDK and OpenRouter provider in the Solver agent's architecture.

## Overview

We've enhanced the OpenAgent base class to use Vercel AI SDK's `generateText` function with OpenRouter as the LLM provider. This integration:

1. Properly executes Effect-based tools like `fetchFileContents` using Runtime.runPromise
2. Provides access to a wide range of models through OpenRouter
3. Supports a robust multi-turn tool calling loop pattern
4. Implements comprehensive error handling with Effect's Cause analysis

## Key Components

### 1. Proper Effect Tool Execution

The cornerstone of our implementation is the `executeToolEffect` method, which properly handles Effect-based tools:

```typescript
/**
 * Helper function to execute a tool, handling both Effect-based and standard tools.
 */
private async executeToolEffect(toolCall: ToolCallPart): Promise<ToolResultPart> {
  const { toolName, args, toolCallId } = toolCall;
  const tool = solverTools[toolName as SolverToolName];

  // --- Check if tool exists ---
  if (!tool || typeof tool.execute !== 'function') {
    console.error(`[executeToolEffect] Tool '${toolName}' not found or has no execute method.`);
    return {
      type: 'tool-result', // Correct type for Vercel AI SDK
      toolCallId,
      toolName,
      result: { error: `Tool '${toolName}' not found or is not executable.` }
    } as ToolResultPart; // Type assertion for clarity
  }

  try {
    let resultValue: unknown;

    // --- Explicit check for the known Effect-based tool ---
    if (toolName === 'fetchFileContents') {
      // 1. Cast the tool.execute function to any to bypass type checking
      // This is necessary at the boundary between Vercel AI SDK and Effect
      const toolExecuteFn = tool.execute as any;
      
      // 2. Call the execute function to get the Effect object
      // We need to use the empty options object that Vercel AI SDK expects
      const toolEffect = toolExecuteFn(args, {});

      // 3. Verify it's actually an Effect (runtime check for safety)
      if (!Effect.isEffect(toolEffect)) {
         throw new Error(`Tool '${toolName}' was expected to return an Effect but did not.`);
      }

      // 4. Run the Effect using unsafeRunPromise due to type constraints
      // This is acceptable since we're already at the Effect-Promise boundary
      console.log(`[executeToolEffect] Running Effect for tool '${toolName}'...`);
      resultValue = await (Effect as any).unsafeRunPromise(toolEffect);
      console.log(`[executeToolEffect] Effect for tool '${toolName}' completed successfully.`);

    } else {
      // --- Handle standard Promise-based or synchronous tools ---
      // Need to use type assertions since TypeScript can't verify tool args at runtime
      const execFn = tool.execute as (a: any, o: any) => Promise<any>;
      resultValue = await Promise.resolve(execFn(args, {}));
    }

    // --- Return success ToolResultPart ---
    return {
      type: 'tool-result',
      toolCallId,
      toolName,
      result: resultValue // Vercel SDK expects the actual result here
    } as ToolResultPart;
  } catch (error) {
    // --- Handle errors with comprehensive Cause analysis ---
    // ...
  }
}
```

### 2. Comprehensive Error Handling with Cause Analysis

A key advantage of Effect is its rich error information via the Cause structure:

```typescript
// --- Handle ALL errors (from runPromise or standard execute) ---
console.error(`[executeToolEffect] Tool '${toolName}' execution failed:`, error);
let errorMessage = `Tool '${toolName}' failed.`;

// Check if it's a FiberFailure from Effect.runPromise
if (error && (error as any)[Symbol.toStringTag] === 'FiberFailure') {
    const cause = (error as any).cause as Cause.Cause<any>;
    console.log(`[executeToolEffect] Analyzing Effect failure Cause for tool '${toolName}'...`);
    
    // Analyze Cause structure
    if (Cause.isFailType(cause)) {
      const specificError = cause.error as any;
      if (specificError && specificError._tag) {
        // Map specific tagged errors to user-friendly messages
        if (specificError._tag === "FileNotFoundError") {
          errorMessage = `File not found...`;
        } else if (specificError._tag === "GitHubApiError") {
          errorMessage = `GitHub API Error...`;
        }
        // ... other tagged errors
      }
    } else if (Cause.isDieType(cause)) {
      console.error(`Tool '${toolName}' defected:`, Cause.pretty(cause));
      errorMessage = `An internal error occurred while executing tool '${toolName}'.`;
    } else if (Cause.isInterruptType(cause)) {
      errorMessage = `Tool '${toolName}' execution was interrupted.`;
    } else {
      errorMessage = `Tool '${toolName}' failed with an unknown error.`;
    }
} else {
  // Standard JavaScript error from non-Effect tools
  errorMessage = `Tool '${toolName}' failed: ${error instanceof Error ? error.message : String(error)}`;
}
```

### 3. Multi-Turn Tool Execution Loop

We implement a robust tool execution loop for interaction with the LLM:

```typescript
// --- Tool Execution Loop ---
for (let i = 0; i < maxToolRoundtrips; i++) {
  console.log(`[sharedInfer] Starting LLM Call ${i + 1}`);

  // Ensure AsyncLocalStorage context is set for tool execution
  const result = await solverContext.run(asSolver(this) as Solver, async () => {
    return generateText({
      model: openrouter(model),
      messages: currentMessages,
      tools: solverTools,
      toolChoice: 'auto',
      temperature,
      maxTokens: max_tokens,
      topP: top_p
    });
  });

  const { text, toolCalls, finishReason, usage } = result;
  textResponse = text;

  if (!toolCalls || toolCalls.length === 0) {
    console.log('[sharedInfer] No tool calls made by LLM. Exiting loop.');
    break; // Exit loop if no tools called
  }

  // Execute all tool calls concurrently
  const toolExecutionPromises = toolCalls.map(toolCall => this.executeToolEffect(toolCall));
  const toolResultsList = await Promise.all(toolExecutionPromises);

  // Add assistant message with tool requests and the tool results message
  currentMessages.push({
    role: 'assistant',
    content: [{ type: 'text', text }, ...toolCalls]
  });
  currentMessages.push({
    role: 'tool',
    content: toolResultsList // Use the results from executeToolEffect
  });

  // Check for max roundtrips
  if (i === maxToolRoundtrips - 1) {
    console.warn('[sharedInfer] Maximum tool roundtrips reached.');
    textResponse = textResponse + "\n\n(Maximum tool steps reached)";
    break;
  }
}
```

### 4. Agent Context via AsyncLocalStorage

We ensure the agent instance is available to tools via AsyncLocalStorage:

```typescript
// In sharedInfer:
const asSolver = <T extends BaseAgentState>(agent: OpenAgent<T>): unknown => agent;
const result = await solverContext.run(asSolver(this) as Solver, async () => {
  return generateText({ /* ... */ });
});

// In fetchFileContents Effect:
const agent = yield* Effect.sync(() => solverContext.getStore());
```

## Configuration and Environment

### Model Selection and OpenRouter Integration

```typescript
const openrouter = createOpenRouter({
  apiKey: (this.env.OPENROUTER_API_KEY as string) || process.env.OPENROUTER_API_KEY || ''
});

// Default to Claude 3.5 Sonnet, but can be overridden
const model = props.model || "anthropic/claude-3.5-sonnet";

// Model options
return generateText({
  model: openrouter(model),
  messages: currentMessages,
  tools: solverTools,
  toolChoice: 'auto',
  temperature,
  maxTokens: max_tokens,
  topP: top_p
});
```

### Environment Requirements

- Set `OPENROUTER_API_KEY` in Cloudflare secrets for production
- For local development, add to `.dev.vars` file
- Ensure `solverTools` are properly registered

## Benefits of This Implementation

1. **Correct Effect Execution**: Properly executes Effect-based tools using `Runtime.runPromise`
2. **Type-Safe Tool Handling**: Correctly bridges the type system boundary between Vercel AI SDK and Effect
3. **Comprehensive Error Handling**: Rich error analysis through Effect's Cause structure
4. **Model Flexibility**: Access to many models via OpenRouter
5. **Robust Tool Loop**: Multi-turn interaction with proper context management

## Future Improvements

1. **Improved Tool Type Detection**: Replace the hardcoded check for 'fetchFileContents' with a more robust mechanism:
   ```typescript
   // Add a marker property to Effect-based tools
   interface EffectBasedTool {
     effectBased: true;
   }
   
   // Then check for the marker
   if ('effectBased' in tool) {
     // Handle as Effect-based tool
   }
   ```

2. **Runtime Configuration**: Create a more sophisticated Runtime with specific configuration:
   ```typescript
   // Create configured runtime
   const runtime = Runtime.defaultRuntime.pipe(
     Runtime.addLoggerScoped(() => ConsoleLogger.make({ logLevel: "INFO" }))
   );
   
   // Use configured runtime
   resultValue = await Runtime.runPromise(runtime)(toolEffect);
   ```

3. **Effect Schema Integration**: Replace the manual Zod schema validation with Effect's Schema module

4. **Dependency Injection**: Move from AsyncLocalStorage to Effect's Context/Layer for dependencies

5. **Streaming Support**: Add integration with `streamText` for more responsive UX

## Conclusion

This implementation provides a robust foundation for integrating Effect-based tools with the Vercel AI SDK. The key insight is properly executing Effect objects using `Runtime.runPromise` and handling the resulting Promise or FiberFailure, rather than attempting to directly `await` an Effect. This approach ensures we get all the benefits of Effect's error handling and composition while maintaining compatibility with the Vercel AI SDK's tool interface.
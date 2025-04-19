# Effect-Based Tool Integration with Vercel AI SDK

This document explains how we've integrated Effect-based tools with the Vercel AI SDK and OpenRouter provider in the Solver agent's architecture.

## Overview

We've enhanced the OpenAgent base class to use Vercel AI SDK's `generateText` function with OpenRouter as the LLM provider. This integration enables:

1. Seamless execution of Effect-based tools like `fetchFileContents`
2. Access to a wide range of models through OpenRouter
3. Support for a multi-turn tool calling loop pattern
4. Proper handling of Effect execution and error propagation

## Key Components

### 1. Tool Execution with Effect

The core of our implementation is the `executeToolEffect` method, which:

- Detects whether a tool uses Effect or standard async/sync patterns
- Executes Effect-based tools using `Effect.runPromise`
- Handles and formats errors from Effect operations by analyzing the `Cause`
- Returns properly formatted tool results using the Vercel AI SDK's `toolResult` helper

```typescript
private async executeToolEffect(toolCall: ToolCallPart): Promise<ToolResultPart> {
  const { toolName, args, toolCallId } = toolCall;
  const tool = solverTools[toolName as SolverToolName];

  // Execute the tool and capture the result
  let resultValue;
  if (tool.execute.toString().includes('Effect.gen')) {
    // For Effect-based tools
    const toolEffect = tool.execute(args);
    resultValue = await Runtime.runPromise(Runtime.defaultRuntime)(toolEffect);
  } else if (tool.execute.constructor.name === 'AsyncFunction') {
    // For standard async tools
    resultValue = await tool.execute(args);
  } else {
    // For synchronous tools
    resultValue = tool.execute(args);
  }

  // Handle errors and return formatted tool results
  // ...
}
```

### 2. Vercel AI SDK Integration with `generateText`

The `sharedInfer` method has been refactored to use Vercel AI SDK's `generateText` with OpenRouter:

```typescript
const openrouter = createOpenRouter({ 
  apiKey: (this.env.OPENROUTER_API_KEY as string) || process.env.OPENROUTER_API_KEY || ''
});

// Ensure agent instance is available via solverContext
const result = await solverContext.run(this, async () => {
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
```

### 3. Tool Calling Loop

We've implemented a multi-turn loop for tool calling that:

1. Calls the LLM using `generateText`
2. Checks if any tool calls were requested
3. Executes all requested tools in parallel
4. Adds the tool calls and results to the conversation history
5. Continues the loop until no more tool calls or maximum rounds reached

```typescript
for (let i = 0; i < maxToolRoundtrips; i++) {
  // Call LLM with current messages
  const { text, toolCalls } = await solverContext.run(this, async () => { /* ... */ });
  
  // Break if no tool calls
  if (!toolCalls || toolCalls.length === 0) break;
  
  // Execute tool calls
  const toolExecutionPromises = toolCalls.map(toolCall => this.executeToolEffect(toolCall));
  const results = await Promise.all(toolExecutionPromises);
  
  // Add assistant and tool messages to the conversation
  currentMessages.push({
    role: 'assistant',
    content: [{ type: 'text', text }, ...toolCalls]
  });
  
  currentMessages.push({
    role: 'tool',
    content: results
  });
}
```

### 4. Context Access via AsyncLocalStorage

A critical part of the implementation is ensuring that the Effect-based tools can access the agent instance via AsyncLocalStorage:

```typescript
// In sharedInfer
const result = await solverContext.run(this, async () => {
  return generateText({ /* ... */ });
});

// In fetchFileContents
const agent = yield* Effect.sync(() => solverContext.getStore());
```

This maintains the current pattern of accessing the agent context within tools while providing a pathway to move to more explicit dependency injection in the future.

## Error Handling

The integration includes robust error handling for Effect-based tools:

1. **FiberFailure Analysis**: We analyze the Cause structure from Effect to extract specific error information
2. **Tagged Error Handling**: Different error types (like `FileNotFoundError` or `GitHubApiError`) are identified by their `_tag` property
3. **User-Friendly Messages**: Error details are formatted into human-readable messages for the LLM
4. **Error Type Distinction**: We differentiate between expected failures (E), programming defects, and interruptions

```typescript
if (Cause.isFailType(cause)) {
  const specificError = cause.error;
  if (specificError._tag === "FileNotFoundError") {
    errorMessage = `File not found: ${specificError.path} in ${specificError.owner}/${specificError.repo}...`;
  } else if (specificError._tag === "GitHubApiError") {
    errorMessage = `GitHub API Error: ${specificError.status ? `(${specificError.status}) ` : ''}${specificError.message}`;
  }
  // ...
} else if (Cause.isDieType(cause)) {
  console.error("Tool defected:", cause.defect);
  errorMessage = "Internal error in tool execution.";
} else if (Cause.isInterruptType(cause)) {
  errorMessage = "Tool execution was interrupted.";
}
```

## Configuration

The integration supports flexible configuration through:

1. **Model Selection**: Default to `anthropic/claude-3.5-sonnet` but can be overridden
2. **Temperature Control**: Configurable temperature, max tokens, and top_p
3. **Tool Choice**: Set to `'auto'` to let the model decide when to use tools
4. **Maximum Roundtrips**: Configurable limit to prevent infinite loops (default: 5)

```typescript
// Configurable parameters
const { 
  model = "anthropic/claude-3.5-sonnet", 
  messages: initialMessages, 
  system, 
  temperature = 0.7, 
  max_tokens = 1024, 
  top_p = 0.95 
} = props;
```

## Environment Setup

The integration expects an OpenRouter API key to be available via:

```typescript
const openrouter = createOpenRouter({ 
  apiKey: (this.env.OPENROUTER_API_KEY as string) || process.env.OPENROUTER_API_KEY || ''
});
```

This key should be set in Cloudflare secrets for production and in `.dev.vars` for local development.

## Benefits

This integration provides several key benefits:

1. **Improved Model Access**: Access to cutting-edge models like Claude 3.5 Sonnet through OpenRouter
2. **Robust Error Handling**: Explicit error types and meaningful error messages
3. **Type Safety**: Strong typing throughout the implementation
4. **Flexibility**: Support for both Effect-based and traditional tools
5. **Future-Proofing**: Sets the foundation for more Effect adoption in the future

## Next Steps

Building on this integration, we can:

1. **Refactor More Tools**: Convert more tools to use Effect
2. **Dependency Management**: Move toward explicit dependencies using Effect's Context/Layer
3. **Enhanced Testing**: Leverage Effect's testing tools for more robust unit tests
4. **Stream Support**: Add support for streaming responses with `streamText` for a more responsive UI
5. **Metrics & Tracing**: Add observability features using Effect's built-in tools

## Usage Example

To use this implementation in a Solver agent, no changes are needed at the API level. The existing WebSocket message handler will continue to work with the enhanced implementation:

```typescript
// In the WebSocket message handler:
// Message type: "shared_infer"
const response = await this.sharedInfer({
  messages,
  system,
  temperature
});

// Send response back to client
ws.send(JSON.stringify({
  type: "inference_response",
  text: response.content,
  id: response.id
}));
```

Clients using the Solver agent will be able to leverage the `fetchFileContents` tool without any changes to their code, experiencing improved error messages and access to better models.

## Conclusion

This integration represents a significant step forward in robustness and flexibility for the Solver agent. By combining Effect's type-safe error handling with Vercel AI SDK's model access and tool calling capabilities, we've created a system that provides better reliability while expanding the agent's capabilities.
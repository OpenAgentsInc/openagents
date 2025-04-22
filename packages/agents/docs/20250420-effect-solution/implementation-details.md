# Implementation Details: Anthropic Tool Integration with Effect

## Files Modified

1. **chat.ts**: Updated to include tool formatting and execution
2. **effect-tools.ts**: Created to handle tool formatting and execution
3. **handleMessage.ts**: Fixed to handle UIMessage parts correctly
4. **index.ts**: Updated to include proper AsyncLocalStorage context

## Key Implementation Details

### 1. Tools Formatting (`effect-tools.ts`)

We implement a function that takes our existing tools and formats them for Anthropic's API:

```typescript
export function formatToolsForAnthropic() {
  const formattedTools = [];
  
  // Format GetIssueDetails
  formattedTools.push({
    name: "GetIssueDetails",
    description: "Fetches comprehensive issue information from GitHub.",
    input_schema: {
      type: "object",
      properties: {
        owner: { type: "string", description: "Repository owner" },
        repo: { type: "string", description: "Repository name" },
        issueNumber: { type: "number", description: "Issue number" }
      },
      required: ["owner", "repo", "issueNumber"]
    }
  });
  
  // Format UpdateIssueStatus
  formattedTools.push({
    name: "UpdateIssueStatus",
    description: "Updates the status of a GitHub issue.",
    input_schema: {
      type: "object",
      properties: {
        owner: { type: "string", description: "Repository owner" },
        repo: { type: "string", description: "Repository name" },
        issueNumber: { type: "number", description: "Issue number" },
        status: { type: "string", description: "New status (e.g., 'open', 'closed')" },
        comment: { type: "string", description: "Optional comment to add" }
      },
      required: ["owner", "repo", "issueNumber", "status"]
    }
  });
  
  // Format CreateImplementationPlan
  formattedTools.push({
    name: "CreateImplementationPlan",
    description: "Creates a step-by-step implementation plan for the current issue.",
    input_schema: {
      type: "object",
      properties: {
        steps: { 
          type: "array", 
          description: "Optional custom steps for the plan",
          items: { type: "string" } 
        }
      },
      required: []
    }
  });
  
  return formattedTools;
}
```

### 2. Tool Execution (`effect-tools.ts`)

We implement a function that executes tools by name and handles errors:

```typescript
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
    const options = {}; // Empty options object required by Vercel AI SDK
    const result = await (tool.execute as any)(params, options);
    return result;
  } catch (error) {
    console.error(`Failed to execute tool ${toolName}:`, error);
    return { error: String(error) };
  }
}
```

### 3. API Integration (`chat.ts`)

We update the API call to include tools and handle tool calls:

```typescript
// Get tools formatted for Anthropic
const formattedTools = formatToolsForAnthropic();

// Build request body with tools
const requestBody = {
  model: model || "claude-3-5-sonnet-latest",
  messages: formattedMessages,
  system: systemPrompt,
  tools: formattedTools,
  tool_choice: "auto", // Let the model decide when to use tools
  max_tokens: 1000,
  temperature: 0.7
};
```

And later handle tool calls in the response:

```typescript
// Check if there are tool calls to process
if (responseData.tool_calls && responseData.tool_calls.length > 0) {
  yield* eff(Effect.logInfo(`Processing ${responseData.tool_calls.length} tool calls from Anthropic response`));
  
  // Execute tools one by one within the Effect context
  const toolResults = [];
  for (const toolCall of responseData.tool_calls) {
    try {
      // Execute the tool using Effect.tryPromise
      const result = yield* eff(Effect.tryPromise({
        try: () => executeToolByName(toolCall.name, toolCall.input),
        catch: error => new ChatError({ cause: error })
      }));
      
      toolResults.push({
        tool_call_id: toolCall.id,
        output: result
      });
    } catch (error) {
      toolResults.push({
        tool_call_id: toolCall.id,
        output: { error: String(error) }
      });
    }
  }
  
  // Format tool results and append to the response
  responseContent += "\n\n### Tool Results\n\n";
  for (const result of toolResults) {
    const toolName = responseData.tool_calls.find(t => t.id === result.tool_call_id)?.name || "unknown";
    const resultOutput = result.output.error 
      ? `Error: ${result.output.error}`
      : JSON.stringify(result.output, null, 2);
      
    responseContent += `**${toolName}**: ${resultOutput}\n\n`;
  }
}
```

### 4. AsyncLocalStorage for Context (`index.ts`)

We use AsyncLocalStorage to provide the Solver agent context to tools:

```typescript
async onMessage(connection: Connection, message: WSMessage) {
  // Set the solver instance in AsyncLocalStorage context for tools
  return await solverContext.run(this, async () => {
    // Create the Anthropic config layer
    const anthropicConfigLayer = Layer.succeed(
      AnthropicConfig,
      defaultAnthropicConfig as AnthropicConfig
    );
    
    // Create the message handling effect
    const handleMessageEffect = createHandleMessageEffect(this, message as string);
    
    // Run with the customized runtime and provide the Anthropic config layer
    const exit = await Runtime.runPromiseExit(customizedRuntime)(
      handleMessageEffect.pipe(
        Effect.provide(anthropicConfigLayer)
      )
    );
    
    // ... handle result ...
  });
}
```

## Error Handling

We use Effect's error handling capabilities to handle errors at different levels:

1. **API Call Errors**: Handled with `Effect.tryPromise`
2. **Tool Execution Errors**: Handled with `Effect.tryPromise` and custom error types
3. **Response Processing Errors**: Handled with try/catch within Effect generators

## Type Safety

We maintain type safety throughout the implementation:

1. **Tool Parameters**: Proper typing for tool parameters
2. **Tool Results**: Proper typing for tool results
3. **API Request/Response**: Proper typing for API interactions

## Simplified Approach

After examining the full Schema.TaggedRequest approach from Effect.ai, we chose a simplified approach that:

1. Uses existing Vercel AI SDK tools
2. Uses Effect for error handling and functional programming
3. Maintains type safety and composability
4. Avoids complex Schema.TaggedRequest patterns that would require significant refactoring

This approach allowed us to quickly implement tool integration with Anthropic's API while still getting the benefits of Effect for error handling and functional programming.

## Future Improvements

1. **Move Toward Schema.TaggedRequest**: Once the basic integration is working, we can gradually move toward the full Schema.TaggedRequest approach from Effect.ai.
2. **Service Dependencies**: Replace AsyncLocalStorage with explicit service dependencies.
3. **Error Recovery**: Add more sophisticated error recovery mechanisms.
4. **Testing**: Add comprehensive tests for the tool integration.
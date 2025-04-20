# Implementing Tool Usage with Effect and Anthropic

**Date:** April 20, 2025  
**Author:** Claude Code

## Overview

This document outlines the implementation plan for integrating tool usage with our Anthropic API integration in the Solver agent. Currently, the Solver agent makes direct API calls to Anthropic's Claude model but doesn't properly pass tools to the model, resulting in hallucinated tools. 

We'll leverage the Effect library's tool integration capabilities to provide a type-safe, robust solution for tool definition, invocation, and result handling.

## Current Status

1. **API Integration**: Direct fetch calls to Anthropic API are working
2. **Tool Definitions**: Existing tools use Vercel AI's `tool()` function
3. **Issues**: Tools aren't passed to the model, causing hallucinations

## Implementation Goals

1. Convert Vercel AI tools to Effect AI tools
2. Integrate tools with Anthropic API requests
3. Handle tool execution and result reporting
4. Maintain backward compatibility with existing tool usages

## Technical Approach

### 1. Tool Definition with Effect

We'll convert our existing tools to use Effect's `Schema.TaggedRequest` pattern:

```typescript
// Current approach with Vercel AI
export const getIssueDetails = tool({
  description: "Fetches comprehensive issue information",
  parameters: z.object({
    owner: z.string().describe("Repository owner"),
    // ...
  }),
  execute: async ({ owner, repo, issueNumber }) => {
    // Implementation...
  }
});

// New approach with Effect
import { Schema } from "@effect/schema";
import { AiToolkit } from "@effect/ai";

class GetIssueDetails extends Schema.TaggedRequest("GetIssueDetails", {
  payload: Schema.struct({
    owner: Schema.string.annotations({ description: "Repository owner" }),
    repo: Schema.string.annotations({ description: "Repository name" }),
    issueNumber: Schema.number.annotations({ description: "Issue number" })
  }),
  success: Schema.instanceOf(BaseIssue),
  failure: Schema.string
})
```

### 2. Creating a Tool Toolkit

We'll organize all tools into a unified toolkit:

```typescript
const SolverTools = AiToolkit.empty
  .add(GetIssueDetails)
  .add(UpdateIssueStatus)
  .add(CreateImplementationPlan);
```

### 3. Implementing Tool Logic

The tool implementation will be separated from its definition:

```typescript
const SolverToolsImplementation = SolverTools
  .implement(GetIssueDetails, (params) => Effect.gen(function*() {
    // Get agent from context
    const agent = yield* Effect.try({
      try: () => solverContext.getStore(),
      catch: () => "Solver context not available"
    });
    
    if (!agent) {
      return yield* Effect.fail("Solver context not available");
    }
    
    // Implementation logic here...
    // Return issue details
  }))
  .implement(UpdateIssueStatus, /* implementation */)
  .implement(CreateImplementationPlan, /* implementation */);
```

### 4. Integrating with Anthropic API

We'll update our API call to include the tool definitions:

```typescript
// Format tools for Anthropic API
const formattedTools = SolverTools.toAnthropicTools();

// Update request body to include tools
const requestBody = {
  model: model || "claude-3-5-sonnet-latest",
  messages: formattedMessages,
  system: systemPrompt,
  tools: formattedTools,
  tool_choice: "auto", // Let Claude decide when to use tools
  max_tokens: 1000,
  temperature: 0.7
};
```

### 5. Processing Tool Calls from Anthropic

We'll update the response handling to process tool calls:

```typescript
// Parse response
const responseData = jsonData as { 
  content: Array<{ type: string, text: string }>,
  tool_calls?: Array<{
    id: string,
    type: "tool_call",
    name: string, 
    input: Record<string, any>
  }>
};

// If there are tool calls, process them
if (responseData.tool_calls && responseData.tool_calls.length > 0) {
  // Process each tool call
  const toolResults = await Promise.all(
    responseData.tool_calls.map(async toolCall => {
      // Execute the tool using Effect's toolkit
      return await Effect.runPromise(
        SolverToolsImplementation.execute(toolCall.name, toolCall.input)
          .pipe(
            Effect.map(result => ({
              tool_call_id: toolCall.id,
              output: result
            })),
            Effect.catchAll(error => Effect.succeed({
              tool_call_id: toolCall.id,
              output: { error: String(error) }
            }))
          )
      );
    })
  );
  
  // Send tool results back to Anthropic in a follow-up request
  // with the original conversation + tool results
}
```

## Implementation Steps

1. **Add Dependencies**
   - `@effect/schema` for schema definitions
   - Update `@effect/ai` to the latest version with tool support

2. **Create Base Files**
   - `src/agents/solver/effect-tools.ts` - Effect tool definitions
   - `src/agents/solver/effect-toolkit.ts` - Toolkit construction

3. **Convert Existing Tools**
   - Move existing tool logic to Effect-based implementation
   - Maintain backward compatibility with existing tool exports

4. **Update the Anthropic API Integration**
   - Modify the API request to include tool definitions
   - Add response handling for tool calls
   - Implement tool execution and result submission
   
5. **System Prompt Enhancement**
   - Update system prompt to include tool instructions
   - Add tool descriptions and usage guidelines

## Considerations

### 1. Single Conversation Turn Limitation

Anthropic's API isn't stateful, so for multi-turn tool conversations we'll need to:

- Keep track of the full conversation history
- Append tool results to the conversation
- Send the updated conversation history in follow-up requests

### 2. Error Handling

We'll implement comprehensive error handling for:

- Tool definition errors
- Tool execution failures
- API communication issues
- Invalid tool response formats

### 3. Backward Compatibility

To maintain backward compatibility:

- Keep existing tool exports from `tools.ts`
- Create bridge functions between Vercel AI and Effect tools
- Abstract tool execution logic behind a unified API

### 4. Performance Considerations

- Minimize unnecessary tool parsing/formatting for performance
- Batch tool calls when possible
- Use Effect's caching for repeated tool invocations

## Future Enhancements

1. **Streaming Tool Execution**: Support streaming results from long-running tools
2. **Tool Dependencies**: Add support for tools that depend on other tools
3. **Tool Permissions**: Implement a permission system for tool usage
4. **Expanded Tool Set**: Add more GitHub and project management tools

## Specific Files to Modify

1. **`packages/agents/src/agents/solver/tools.ts`**
   - Bridge between Vercel AI tools and Effect tools
   - Export compatible interfaces

2. **`packages/agents/src/agents/solver/effect-tools.ts`** (new)
   - Define tool schemas using Effect's Schema
   - Create tool implementations
   - Build toolkit

3. **`packages/agents/src/agents/solver/chat.ts`**
   - Update Anthropic API integration
   - Add tool handling

4. **`packages/agents/src/agents/solver/types.ts`**
   - Add new types for tool results
   - Update error types

5. **`packages/agents/vite.config.ts`**
   - Ensure proper bundling of Effect Schema

## Timeline

1. **Phase 1: Basic Implementation** (2 days)
   - Tool schema definitions
   - Basic Anthropic integration

2. **Phase 2: Error Handling & Robustness** (1 day)
   - Comprehensive error handling
   - Edge cases

3. **Phase 3: Testing & Refinement** (1 day)
   - Integration testing
   - Performance optimization

## Conclusion

By implementing Effect-based tools with the Anthropic API, we'll provide a more robust, type-safe tool usage experience for the Solver agent. This will eliminate hallucinated tools and enable the agent to perform actual actions to help users with their issues.

The implementation will leverage the strengths of both the Effect library for type safety and error handling, and the Anthropic API for powerful language capabilities.
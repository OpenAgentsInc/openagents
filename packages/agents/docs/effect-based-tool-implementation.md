# Effect-Based Tool Implementation & AI Integration

This document outlines the implementation of Effect-based functionality in the Solver agent:

1. Tool Implementation: `fetchFileContents` using the Effect framework
2. AI Integration: Using Effect AI for inference with Anthropic Claude models

This represents the first phases of our incremental adoption of the Effect framework for more robust and maintainable agent tools and AI inference.

## Overview

The `fetchFileContents` tool retrieves the contents of a specific file from a GitHub repository. Unlike traditional async/await-based tools, this tool leverages the Effect framework to provide:

1. Explicit error handling with typed errors
2. Composable asynchronous operations
3. Clear execution flow
4. Robust error propagation

## Key Implementation Features

### 1. Tagged Error Types

The tool defines specific error types using Effect's `Data.TaggedError` functionality:

```typescript
export class GitHubApiError extends Data.TaggedError("GitHubApiError")<{
  message: string;
  status?: number; // Optional HTTP status
  url: string; // Include URL for context
}> {}

export class FileNotFoundError extends Data.TaggedError("FileNotFoundError")<{
  path: string;
  repo: string;
  owner: string;
  branch?: string;
  url: string;
}> {}
```

These explicit error types:
- Make potential failures visible in the type signature
- Provide rich context about what went wrong
- Enable pattern matching on error types
- Improve error messages to both developers and users

### 2. Effect-Based Request Utility

The implementation includes a helper function that wraps fetch operations in Effect:

```typescript
function githubRequestEffect(
  url: string,
  options?: RequestInit
): Effect.Effect<Response, GitHubApiError> {
  return Effect.tryPromise({
    try: () => fetch(url, options),
    catch: (unknown) => new GitHubApiError({
      message: `Network error: ${unknown instanceof Error ? unknown.message : String(unknown)}`,
      url: url
    })
  });
}
```

This enables better composition of fetch operations and automatic error propagation.

### 3. Generator-Style Syntax with Effect.gen

The core logic uses `Effect.gen` to create a generator-style function that enables an async/await-like syntax while maintaining all the benefits of Effect:

```typescript
const fileContentEffect = Effect.gen(function* () {
  // Access agent state
  const agent = yield* Effect.sync(() => solverContext.getStore());
  
  // API request with error handling
  const response = yield* githubRequestEffect(urlFinal, {
    headers: { /* ... */ }
  });
  
  // Handle errors explicitly
  if (!response.ok) {
    // ...
    return yield* Effect.fail(
      new GitHubApiError({ message: errorText, status, url: urlFinal })
    );
  }
  
  // Process response
  // ...
  
  return decodedContent;
});
```

This style:
- Makes complex asynchronous flows more readable
- Preserves the explicit error handling of Effect
- Provides sequential execution with proper error short-circuiting

### 4. Integration with Vercel AI SDK

The tool integrates with the existing Vercel AI SDK pattern by running the Effect at the boundary and converting it to a Promise:

```typescript
execute: ({ owner, repo, path, branch }) => {
  const fileContentEffect = Effect.gen(/* ... */);

  // Convert Effect to Promise for Vercel AI SDK
  return Effect.runPromise(fileContentEffect).catch((fiberFailure) => {
    // Extract and analyze the cause
    const cause = (fiberFailure as any).cause;
    // Convert to user-friendly error message
    // ...
    throw new Error(errorMessage);
  });
}
```

This allows us to adopt Effect incrementally while maintaining compatibility with the existing framework.

### 5. Comprehensive Error Analysis

The boundary code analyzes the failure `Cause` to provide appropriate error messages:

```typescript
if (Cause.isFailType(cause)) {
  const error = cause.error; // This is our FetchFileContentError
  // Handle specific error types
  if (error._tag === "FileNotFoundError") {
    errorMessage = `File not found: ${error.path} in ${error.owner}/${error.repo}...`;
  } else if (error._tag === "GitHubApiError") {
    errorMessage = `GitHub API Error: ${error.status ? `(${error.status}) ` : ''}${error.message}`;
  }
  // ...
} else if (Cause.isDieType(cause)) {
  // Handle defects - indicating a bug in our code
  console.error("Tool defected:", cause.defect);
  errorMessage = "Internal error in fetchFileContents tool.";
}
```

This pattern:
- Distinguishes between expected failures and programming defects
- Provides context-appropriate error messages
- Logs internal issues properly without exposing details to users

## Usage

The `fetchFileContents` tool is registered in the Solver agent's tools object:

```typescript
export const solverTools = {
  getIssueDetails,
  updateIssueStatus,
  createImplementationPlan,
  fetchFileContents
};
```

It can be invoked by the LLM like any other tool, with the parameters:

```typescript
{
  owner: "username-or-org",
  repo: "repository-name",
  path: "path/to/file.ext",
  branch: "main" // optional
}
```

## Benefits Over Traditional Implementation

Compared to traditional async/await with try/catch implementations, this approach:

1. **Makes Errors Explicit**: The return type `Effect<string, FetchFileContentError, never>` clearly indicates that this function can fail with specific error types, forcing developers to handle these cases.

2. **Improves Error Context**: Tagged errors contain rich context about what went wrong, making debugging easier and error messages more informative.

3. **Enables Composition**: The `githubRequestEffect` utility and other Effect combinators allow operations to be easily composed while maintaining error handling.

4. **Clarifies Control Flow**: The generator syntax with `yield*` makes the execution flow clear and sequential, while still preserving all Effect benefits.

5. **Separates Error Handling Logic**: The core logic focuses on the happy path, with error handling cleanly separated and applied consistently.

## Future Enhancements

As we progress with Effect adoption, future enhancements could include:

1. **Dependency Injection**: Move from `solverContext.getStore()` to explicit dependencies using Effect's context (`R` type parameter).

2. **Retries**: Add automatic retry policies for transient failures using `Effect.retry` and `Schedule`.

3. **Additional Effect-Based Tools**: Refactor other tools to use Effect for consistent error handling patterns.

4. **Layer Management**: Create GitHub API service layers for better dependency management and testing.

## Effect AI Integration for Inference

Beyond tools, we've also implemented Effect AI for inference, allowing the agent to generate responses using Anthropic's Claude models through the Effect framework.

### Implementation Details

The implementation follows the approach outlined in the Effect AI documentation and the demo in `packages/core/src/effect/demo.ts`:

```typescript
// Import Effect AI modules
const { AnthropicClient, AnthropicCompletions } = await import("@effect/ai-anthropic");
const { Completions } = await import("@effect/ai");
const { Effect, Config } = await import("effect");

// Create the Effect for text generation
const generateDadJoke = Effect.gen(function* () {
  // Extract the Completions service from the Effect environment
  const completions = yield* Completions.Completions;
  // Use the service to generate text with our prompt
  const response = yield* completions.create("Generate a dad joke");
  return response;
});

// Create an AiModel which provides a Completions implementation
const Sonnet35 = AnthropicCompletions.model("claude-3-5-sonnet-20241022");

// Configure the Anthropic client with an API key
const Anthropic = AnthropicClient.layerConfig({
  apiKey: Config.succeed(apiKey)
});

// The main Effect that builds the model and provides it to generateDadJoke
const main = Effect.gen(function* () {
  // Build the AiModel into a Provider
  const sonnet35 = yield* Sonnet35;
  // Provide the Completions implementation to generateDadJoke
  const response = yield* sonnet35.provide(generateDadJoke);
  return response;
});

// Run the Effect with the Anthropic layer
const effectResult = await Effect.runPromise(Effect.provide(main, Anthropic));
```

### Benefits Over Standard Inference

Using Effect AI for inference provides several advantages:

1. **Explicit Error Handling**: All potential error types are part of the type signature
2. **Better Composability**: Easy to integrate pre- and post-processing steps
3. **Consistent Provider Interface**: The same pattern works across different AI providers
4. **Type-Safe Prompting**: Better type safety for prompt parameters
5. **Simplified Dependency Management**: API keys and other configuration managed through Effect's Layer system

### Testing the Effect AI Integration

You can test the Effect AI integration in the Solver agent by:

1. Connect to a Solver agent for any issue
2. Click the "Test Effect AI" button in the agent controls sidebar
3. The agent will use Effect AI with Claude 3.5 Sonnet to generate a dad joke
4. The result will be displayed in the chat panel

This button sends a specialized message type (`effect_ai_test`) to the agent, which then uses the Effect AI framework to generate a response.

### Future Enhancements for AI Integration

As we continue with the Effect adoption, we plan to enhance AI integration with:

1. **Structured Prompting**: Using Effect's schema utilities for strongly-typed prompts
2. **Tool Use with Effects**: Integrating tool execution directly into the Effect flow
3. **Streaming Responses**: Using Effect's Stream for more efficient streaming of results
4. **Multi-Model Orchestration**: Leveraging Effect's concurrency to coordinate multiple models

## Conclusion

These implementations demonstrate the first phases of adopting Effect within the Solver agent:
1. Improving tool robustness through better error handling and composability with Effect-based tools
2. Enhancing AI inference through the type-safe Effect AI framework

By incrementally adopting Effect within the existing framework, we gain many of its benefits while maintaining compatibility with the current system. The Effect framework offers a promising path toward more reliable, maintainable, and composable agent functionality.
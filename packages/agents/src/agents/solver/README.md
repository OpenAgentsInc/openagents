# Solver Agent

The Solver Agent is a specialized autonomous agent designed to help users analyze, plan, and implement solutions for project issues. It uses the Effect functional programming library to manage complexity, handle errors, and maintain a robust architecture.

## Architecture Overview

The Solver agent is built around these key components:

1. **Core Agent Class** - Extends the base Agent class and manages WebSocket communication
2. **Effect-Based Message Processing** - Uses the Effect framework for robust message handling
3. **Type-Safe Context Management** - Uses tagged services for dependency injection
4. **Tool Implementation** - Exposes a set of capabilities to the LLM
5. **Error Handling** - Uses typed, tagged errors for comprehensive error management
6. **Chat Capabilities** - Integrates with Anthropic's Claude model

## Directory Structure

```
src/agents/solver/
├── README.md           # This documentation file
├── index.ts            # Main Solver class and runtime configuration
├── types.ts            # Type definitions, tagged errors, and services
├── tools.ts            # Tool implementations using Vercel AI SDK
├── chat.ts             # Chat functionality using Anthropic
└── handleMessage.ts    # WebSocket message processing
```

## Key Components

### 1. Solver Class (`index.ts`)

This is the entry point and main implementation of the Solver agent:

```typescript
export class Solver extends Agent<Env, SolverState> {
  initialState: SolverState = {
    messages: [],
    currentIssue: undefined,
    currentProject: undefined,
    currentTeam: undefined
  };

  async onMessage(connection: Connection, message: WSMessage) {
    const handleMessageEffect = createHandleMessageEffect(this, message as string);
    
    // Create a basic default Anthropic config for use in the context
    const defaultAnthropicConfig = {
      apiKey: process.env.ANTHROPIC_API_KEY || "",
      fetch: globalThis.fetch,
      model: "claude-3-sonnet-20240229"
    };

    // Run with the customized runtime and provide the AnthropicConfig
    const exit = await Runtime.runPromiseExit(customizedRuntime)(
      handleMessageEffect.pipe(
        Effect.provideService(AnthropicConfig, defaultAnthropicConfig as AnthropicConfig)
      )
    );

    // Handle errors if needed...
  }
}
```

### 2. State Management (`types.ts`)

The agent uses a strongly-typed state interface:

```typescript
export type SolverState = {
  messages: UIMessage[];          // Chat history
  currentIssue?: BaseIssue;       // Current issue being worked on
  currentProject?: BaseProject;   // Parent project of the issue
  currentTeam?: BaseTeam;         // Team responsible for the project
  githubToken?: string;           // GitHub authentication token
};
```

### 3. Message Handling (`handleMessage.ts`)

The agent processes WebSocket messages using Effect for robust error handling and functional composition:

```typescript
export const createHandleMessageEffect = (
  agent: Agent<any, SolverState>,
  message: string
): Effect.Effect<void, HandleMessageError, AnthropicConfig> =>
  Effect.gen(function* (_) {
    // Parse the incoming message
    const parsedMessage = yield* Effect.try({
      try: () => JSON.parse(message),
      catch: (unknown) => new ParseError({ cause: unknown })
    });
    
    // Handle different message types...
    if (parsedMessage.type === 'set_context') {
      // Process context-setting messages
      // ...
      
      // Automatically start a conversation
      const chatEffect = createInitialChatEffect(currentState);
      const assistantMessage = yield* chatEffect;
      
      // Update state with new message
      // ...
    }
    // Other message types...
  });
```

### 4. Tools Implementation (`tools.ts`)

The agent implements a set of tools using the Vercel AI SDK, which are available to the LLM:

```typescript
export const getIssueDetails = tool({
  description: "Fetches comprehensive issue information",
  parameters: z.object({
    owner: z.string().describe("Repository owner"),
    repo: z.string().describe("Repository name"),
    issueNumber: z.number().describe("Issue number"),
  }),
  execute: async ({ owner, repo, issueNumber }) => {
    // Implementation...
  }
});

export const updateIssueStatus = tool({
  // ...implementation
});

export const createImplementationPlan = tool({
  // ...implementation
});

// Export all tools as a record for easy access
export const solverTools = {
  getIssueDetails,
  updateIssueStatus,
  createImplementationPlan,
};
```

### 5. Chat Integration (`chat.ts`)

The agent integrates with Anthropic's Claude model:

```typescript
export const createChatEffect = (
  currentState: SolverState,
  userMessageContent: string
): Effect.Effect<UIMessage, ChatError, AnthropicConfig> =>
  Effect.gen(function* (_) {
    // Create user message
    // ...
    
    // Get Anthropic configuration from context
    const anthropicConfig = yield* _(AnthropicConfig);
    
    // Generate response (currently simulated)
    const responseContent = `I understand you're working on ${currentState.currentIssue?.title}. How can I help?`;
    
    // Format and return response
    // ...
  });
```

## Understanding the Effect Framework

The Solver agent leverages the [Effect](https://effect.website/) framework for robust functional programming. Here's how it's used:

### 1. Effect Pattern

An `Effect<A, E, R>` represents a computation that:
- Produces a value of type `A` on success
- Might fail with an error of type `E`
- Requires an environment of type `R`

For example:
```typescript
Effect.Effect<UIMessage, ChatError, AnthropicConfig>
```
Represents a computation that:
- Returns a `UIMessage` on success
- Might fail with a `ChatError`
- Requires an `AnthropicConfig` service

### 2. Effect Generator Functions

The agent uses generator functions with the `Effect.gen` helper to create effects:

```typescript
Effect.gen(function* (_) {
  // Yield other effects with yield*
  const result1 = yield* Effect.succeed(1);
  const result2 = yield* Effect.succeed(2);
  
  // Return final result
  return result1 + result2;
})
```

### 3. Error Handling

The agent uses typed, tagged errors:

```typescript
export class ParseError extends Data.TaggedError("ParseError")<{ cause: unknown }> { }
export class StateUpdateError extends Data.TaggedError("StateUpdateError")<{ cause: unknown }> { }
export class ChatError extends Data.TaggedError("ChatError")<{ cause: unknown }> { }

export type HandleMessageError = ParseError | SetStateError | ChatError;
```

This enables type-safe error handling:

```typescript
Effect.catch({
  try: () => someOperation(),
  catch: (error) => {
    if (error instanceof ParseError) {
      // Handle parsing errors
    } else if (error instanceof StateUpdateError) {
      // Handle state errors
    }
  }
})
```

### 4. Services and Dependencies

The agent uses `Context.Tag` to define services:

```typescript
export interface AnthropicConfig {
  readonly apiKey: string;
  readonly fetch: typeof fetch;
  readonly model?: string;
}

export class AnthropicConfig extends Context.Tag("AnthropicConfig")<AnthropicConfig, AnthropicConfig>() { }
```

These services can be provided to effects:

```typescript
myEffect.pipe(
  Effect.provideService(AnthropicConfig, {
    apiKey: "...",
    fetch: globalThis.fetch,
    model: "claude-3-sonnet-20240229"
  })
)
```

### 5. Running Effects

Effects are executed using `Effect.runPromise` or `Effect.runPromiseExit`:

```typescript
const result = await Effect.runPromise(myEffect);
```

For more comprehensive execution with error handling:

```typescript
const exit = await Effect.runPromiseExit(customizedRuntime)(myEffect);

if (Exit.isFailure(exit)) {
  // Handle errors based on exit.cause
}
```

## Automatic Chat Initialization

The agent automatically starts a conversation after context is set:

1. When a `set_context` message is received, the agent updates its state with issue, project, and team info
2. It then calls `createInitialChatEffect` to generate a welcome message
3. The welcome message is added to the agent's message history
4. This provides a proactive greeting to users when they access an issue

## Tool Integration

The agent uses AsyncLocalStorage to make the agent instance available to tools:

```typescript
// Create an AsyncLocalStorage instance for solver context
export const solverContext = new AsyncLocalStorage<Solver>();

// Later, in a tool execution:
const agent = solverContext.getStore();
if (!agent) {
  throw new Error("Solver context not available");
}
```

This allows tools to access and modify the agent's state.

## Working with the Solver Agent

To extend and modify the Solver agent:

1. **Adding a New Tool**:
   ```typescript
   export const myNewTool = tool({
     description: "Description of the tool",
     parameters: z.object({
       // Define parameters with Zod
       param1: z.string().describe("Parameter description"),
     }),
     execute: async ({ param1 }) => {
       // Implementation...
       const agent = solverContext.getStore();
       // Work with agent...
       return { success: true, result: "..." };
     }
   });
   
   // Then add to solverTools:
   export const solverTools = {
     // ...existing tools
     myNewTool,
   };
   ```

2. **Adding a New Message Handler**:
   Edit `handleMessage.ts` to handle new message types:
   ```typescript
   if (parsedMessage.type === 'my_new_message_type') {
     // Handle the message...
     yield* Effect.logInfo(`Processing new message type`);
     
     // Make state updates
     yield* Effect.tryPromise({
       try: async () => {
         agent.setState({
           ...agent.state,
           // Update state properties
         });
       },
       catch: (unknown) => new SetStateError({ cause: unknown })
     });
   }
   ```

3. **Extending State**:
   Update the `SolverState` interface in `types.ts`:
   ```typescript
   export type SolverState = {
     // Existing properties
     myNewProperty?: string;
   };
   ```

## Best Practices

When working with the Solver agent and Effect:

1. **Typed Errors**: Always define specific error types for different failure modes
2. **Service Dependencies**: Make dependencies explicit in Effect signatures
3. **Error Handling**: Use `Effect.try` and `Effect.tryPromise` to convert exceptions to typed errors
4. **Logging**: Use `Effect.log*` functions for structured logging
5. **Testing**: Effects are easily testable without mocking by providing test implementations of services

## Deployment Considerations

The Solver agent is designed to run as a Cloudflare Durable Object, which provides:

- Persistence of state
- Single-threaded execution per instance
- WebSocket communication

When deploying changes, remember:
- Environment variables must be set for API keys
- Durable Object migrations may be needed for state schema changes
- Each issue gets its own solver instance

## Further Reading

- [Effect Framework Documentation](https://effect.website/)
- [Vercel AI SDK Documentation](https://sdk.vercel.ai/docs)
- [Cloudflare Durable Objects Documentation](https://developers.cloudflare.com/durable-objects/)
- Full Solver Agent documentation: `/packages/agents/docs/SOLVER-README.md`
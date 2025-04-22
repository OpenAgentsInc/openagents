# Automatic Chat Initialization with Anthropic Integration

**Date:** April 19, 2025
**Author:** Claude Code

## Overview

This document outlines the implementation of two key improvements to the Solver agent:

1. Automatic chat initialization after context is set
2. Integration with Anthropic's Claude model instead of OpenAI

These changes enhance user experience by proactively starting the conversation when a user opens an issue, while also transitioning the agent's underlying AI provider to Anthropic.

## Implementation Details

### 1. Anthropic Integration

We replaced the OpenAI client with Anthropic in the Solver agent's chat system:

#### A. Updated Service Definitions

First, we updated the service definitions in `types.ts`:

```typescript
// OLD
export interface OpenAIConfig {
  readonly apiKey: string;
  readonly fetch: typeof fetch;
}

export class OpenAIConfig extends Context.Tag("OpenAIConfig")<OpenAIConfig, OpenAIConfig>() { }

// NEW
export interface AnthropicConfig {
  readonly apiKey: string;
  readonly fetch: typeof fetch;
  readonly model?: string;
}

export class AnthropicConfig extends Context.Tag("AnthropicConfig")<AnthropicConfig, AnthropicConfig>() { }
```

The new `AnthropicConfig` has an additional `model` parameter, allowing for easy model specification when necessary.

#### B. Updated Client Layer in `chat.ts`

We replaced the OpenAI client layer with Anthropic:

```typescript
// OLD
const OpenAiClientLive = Layer.succeed(
  OpenAIConfig,
  {
    apiKey: process.env.OPENAI_API_KEY || "",
    fetch: globalThis.fetch
  } as OpenAIConfig
);

// NEW
const AnthropicClientLive = Layer.succeed(
  AnthropicConfig,
  {
    apiKey: process.env.ANTHROPIC_API_KEY || "",
    fetch: globalThis.fetch,
    model: "claude-3-sonnet-20240229"
  } as AnthropicConfig
);
```

#### C. Enhanced Effect Return Types

We updated all effect return types to correctly include the `AnthropicConfig` dependency:

```typescript
// OLD
export const createChatEffect = (
  currentState: SolverState,
  userMessageContent: string
): Effect.Effect<UIMessage, ChatError, never> =>

// NEW
export const createChatEffect = (
  currentState: SolverState,
  userMessageContent: string
): Effect.Effect<UIMessage, ChatError, AnthropicConfig> =>
```

This change propagated to other functions like `createInitialChatEffect` and `createHandleMessageEffect`.

#### D. Robust Configuration Handling

We added fallback mechanisms for when the Anthropic configuration isn't explicitly provided:

```typescript
// In chat.ts
try {
  // Get API key and client from Effect context, or use defaults for testing
  let anthropicConfig;
  try {
    anthropicConfig = yield* _(AnthropicConfig);
  } catch (e) {
    // Provide default config if it's not in the context
    anthropicConfig = {
      apiKey: process.env.ANTHROPIC_API_KEY || "",
      fetch: globalThis.fetch,
      model: "claude-3-sonnet-20240229"
    };
    yield* _(Effect.logWarning("Using default AnthropicConfig"));
  }
  
  const apiKey = anthropicConfig.apiKey;
  const model = anthropicConfig.model || "claude-3-sonnet-20240229";
  
  // Remaining implementation...
}
```

#### E. Service Provision in `index.ts`

We updated the `onMessage` handler in the Solver class to provide the Anthropic configuration to the Effect:

```typescript
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
  
  // Error handling...
}
```

### 2. Automatic Chat Initialization

We implemented automatic chat initiation when context is set, providing a more welcoming user experience:

#### A. Updated `handleMessage.ts` to Start Chat Automatically

The most significant change was to the `set_context` message handler:

```typescript
if (parsedMessage.type === 'set_context') {
  // Existing context-setting implementation...
  
  // After successful context update:
  yield* Effect.logInfo(`✓ Context Updated Successfully`);
  
  // Automatically start a chat after context is set
  yield* Effect.logInfo(`Starting initial chat conversation...`);
  
  // Get the current state after the update
  const currentState = agent.state;
  
  try {
    // Generate initial message using the createInitialChatEffect
    const chatEffect = createInitialChatEffect(currentState);
    const assistantMessage = yield* chatEffect;
    
    // Add the assistant message to the messages array
    yield* Effect.tryPromise({
      try: async () => {
        agent.setState({
          ...agent.state,
          messages: [...agent.state.messages, assistantMessage]
        });
      },
      catch: (unknown) => new SetStateError({ cause: unknown })
    });
    
    yield* Effect.logInfo(`✓ Initial welcome message sent`);
  } catch (error) {
    yield* Effect.logError(`❌ Failed to generate initial chat message`, { error });
  }
}
```

#### B. Enhanced Error Types

To handle potential errors from the chat effect, we updated the `HandleMessageError` type:

```typescript
// OLD
export type HandleMessageError = ParseError | SetStateError;

// NEW
export type HandleMessageError = ParseError | SetStateError | ChatError;
```

#### C. Initial Message Content

The initial message is generated by the `createInitialChatEffect` function:

```typescript
export const createInitialChatEffect = (
  currentState: SolverState
): Effect.Effect<UIMessage, ChatError, AnthropicConfig> => {
  const initialMessage = `Context loaded for issue #${currentState.currentIssue?.number}: "${currentState.currentIssue?.title}". How can I assist?`;
  return createChatEffect(currentState, initialMessage);
};
```

This creates a welcoming message that acknowledges the current issue context.

## Benefits

1. **Improved User Experience**
   - Users no longer need to initiate the conversation
   - The agent proactively welcomes the user with contextual information
   - Users understand immediately that the agent is ready to assist

2. **Clearer Integration Path to Anthropic**
   - Proper typing for Anthropic configuration
   - Clean separation between API providers
   - Model specification is now explicit
   - Fallback mechanisms ensure robustness

3. **Enhanced Error Handling**
   - More comprehensive error types
   - Better error logging
   - Fallback mechanisms for missing configurations

4. **Effect-based Architecture Improvements**
   - Better dependency tracking through explicit Effect types
   - Proper service provision
   - More descriptive logging
   - Clean separation of concerns

## Technical Considerations

1. **Environment Variables**
   - `ANTHROPIC_API_KEY` environment variable should be set
   - Fallbacks are in place when the key is not available, but these are for development only

2. **Type Safety**
   - All Effect types now properly declare their dependencies
   - Service tags are correctly implemented
   - Error types are comprehensive

3. **Testing**
   - The implementation includes fallback mechanisms for testing
   - Logging is enhanced for better debugging

## Future Work

1. **Implement Actual Anthropic API Call**
   - Currently, the message generation is simulated
   - Next steps would include implementing the actual API call to Claude

2. **UI Feedback During Initialization**
   - Add UI indicators that show the agent is preparing an initial message

3. **Customizable Initial Messages**
   - Allow project or team-specific customization of welcome messages
   - Consider persona-based templating

4. **Enhanced Context Extraction**
   - Improve the system prompt with more contextual information
   - Extract more relevant details from the issue to inform the initial message

## Conclusion

These changes significantly improve the Solver agent's user experience by automating the chat initialization and preparing for Anthropic integration. The code is now more robust, with better typing, error handling, and dependency management through the Effect framework.
import { Effect, Data } from "effect";
import type { Agent } from "agents";
import type { UIMessage } from "ai";
import type { SolverState } from "./index";

// Define TextUIPart for compatibility 
type TextUIPart = {
  type: 'text';
  text: string;
};

// Define potential errors for this operation
export class ParseError extends Data.TaggedError("ParseError")<{ cause: unknown }> { }
export class SetStateError extends Data.TaggedError("SetStateError")<{ cause: unknown }> { }
export type HandleMessageError = ParseError | SetStateError;

export const createHandleMessageEffect = (
  agent: Agent<any, SolverState>,
  message: string
): Effect.Effect<void, HandleMessageError, unknown> =>
  Effect.gen(function* (_) {
    const parsedMessage = yield* Effect.try({
      try: () => JSON.parse(message),
      catch: (unknown) => new ParseError({ cause: unknown })
    });

    // Log message receipt with separator for better visibility
    yield* Effect.logInfo(`━━━━━━━━━━ Incoming WebSocket Message ━━━━━━━━━━`).pipe(
      Effect.annotateLogs({
        messageType: parsedMessage.type,
        requestId: parsedMessage.requestId || 'undefined'
      })
    );

    // Format the payload with custom indentation and structure
    const formattedJson = JSON.stringify(parsedMessage, null, 2)
      .replace(/\\"/g, '"')  // Remove escaped quotes
      .replace(/"/g, "'");   // Use single quotes instead

    const prettyPayload = formattedJson
      .split('\n')
      .map(line => `│ ${line}`)
      .join('\n');

    yield* Effect.logDebug(`Payload Details:\n${prettyPayload}\n└${'─'.repeat(50)}`);

    if (parsedMessage.type === 'chat_message') {
      const userMessageContent = parsedMessage.content;
      
      if (!userMessageContent) {
        yield* Effect.logWarning(`⚠️ Missing user message in chat request`);
        return;
      }
      
      yield* Effect.logInfo(`Processing chat message: ${userMessageContent.substring(0, 100)}${userMessageContent.length > 100 ? '...' : ''}`);
      
      try {
        // Create user message object with required parts field
        const newUserMessage: UIMessage = {
          id: `user_${Date.now()}`,
          role: 'user',
          content: userMessageContent,
          createdAt: new Date(),
          parts: [{
            type: 'text',
            text: userMessageContent
          }]
        };
        
        // Create simple echo response
        const echoResponse: UIMessage = {
          id: `assistant_${Date.now()}`,
          role: 'assistant',
          content: `Echo: ${userMessageContent}`,
          createdAt: new Date(),
          parts: [{
            type: 'text',
            text: `Echo: ${userMessageContent}`
          }]
        };
        
        // Add both messages to state
        yield* Effect.tryPromise({
          try: async () => {
            agent.setState({
              ...agent.state,
              messages: [...agent.state.messages, newUserMessage, echoResponse]
            });
            return Promise.resolve();
          },
          catch: (unknown) => new SetStateError({ cause: unknown })
        });
        
        yield* Effect.logInfo(`✓ Chat response sent`).pipe(
          Effect.annotateLogs({
            messageId: echoResponse.id,
            content: echoResponse.content.substring(0, 100) + (echoResponse.content.length > 100 ? '...' : '')
          })
        );
      } catch (error) {
        yield* Effect.logError(`❌ Failed to process chat message`, { error });
      }
    } else {
      yield* Effect.logDebug(`⚠️ Unhandled Message Type: ${parsedMessage.type}`);
    }
  });
import { Agent, type Connection, type WSMessage } from "agents";
import { type UIMessage } from "ai"; // Keep for message structure
import {
    Effect, Cause, Data, Exit, Runtime, Logger, LogLevel, FiberRefs, FiberRef, FiberId
} from "effect";

// --- Minimal State ---
export type SolverState = {
  messages: UIMessage[];
};

// --- Minimal Error Types ---
class ParseError extends Data.TaggedError("ParseError")<{ cause: unknown }> {}
class StateUpdateError extends Data.TaggedError("StateUpdateError")<{ cause: unknown }> {}
type HandleMessageError = ParseError | StateUpdateError;

// --- Basic Runtime ---
const basicRuntime = Runtime.make({
    context: Runtime.defaultRuntime.context,
    runtimeFlags: Runtime.defaultRuntime.runtimeFlags,
    fiberRefs: FiberRefs.updateAs( // Set log level
        Runtime.defaultRuntime.fiberRefs, {
            fiberId: FiberId.none,
            fiberRef: FiberRef.currentMinimumLogLevel,
            value: LogLevel.Debug // Keep Debug for initial setup
        }
    )
});
// --- End Runtime ---

export class Solver extends Agent<Env, SolverState> {

  initialState: SolverState = {
      messages: []
  };

  /**
   * Minimal Message Handler using Effect
   */
  async onMessage(connection: Connection, message: WSMessage) {

    const handleMessageEffect = Effect.gen(this, function*(self) {
        // 1. Parse
        const parsedMessage = yield* Effect.try({
            try: () => JSON.parse(message as string) as { type: string; content?: string; [key: string]: any },
            catch: (unknown) => new ParseError({ cause: unknown })
        });

        yield* Effect.logInfo(`Received: ${parsedMessage.type}`);
        yield* Effect.logDebug("Payload:", parsedMessage);

        // 2. Handle ONLY simple chat messages
        if (parsedMessage.type === 'chat_message' && parsedMessage.content) {
            const userMessageContent = parsedMessage.content as string;

            yield* Effect.logInfo(`Processing chat message: "${userMessageContent.substring(0, 30)}..."`);

            // Create simple UIMessage objects for state
            const userMessage: UIMessage = {
                id: `user_${Date.now()}`,
                role: 'user',
                content: userMessageContent,
                createdAt: new Date()
            };

            const assistantResponse: UIMessage = {
                id: `asst_${Date.now()}`,
                role: "assistant",
                content: `Echo: ${userMessageContent}`, // Simple echo
                createdAt: new Date()
             };

            // 3. Update State
            yield* Effect.tryPromise({
                try: () => this.setState({
                    // Ensure messages array always exists and append
                    messages: [...(this.state.messages || []), userMessage, assistantResponse]
                }),
                catch: (unknown) => new StateUpdateError({ cause: unknown })
            });
            yield* Effect.logInfo("State updated with user message and echo response.");

            // 4. Send Response Back (Optional - state update might suffice)
            // If the client hook *doesn't* automatically update based on setState broadcast,
            // you might need to send the response explicitly.
            // connection.send(JSON.stringify({ type: 'agent_response', message: assistantResponse }));
            // yield* Effect.logDebug("Sent explicit response.");

        } else {
           yield* Effect.logDebug(`Ignoring unhandled message type: ${parsedMessage.type}`);
        }
      }).pipe(
          // Tag potential errors from this specific workflow
          Effect.mapError(error => error as HandleMessageError) // Simple cast for this minimal example
      );

    // Run the Effect
    const exit = await Runtime.runPromiseExit(basicRuntime)(handleMessageEffect);

    // Handle Failures
    if (Exit.isFailure(exit)) {
        await Runtime.runPromise(basicRuntime)(
            Effect.logError(`Message handling failed`, { failureCause: Cause.pretty(exit.cause) })
        );
        // Optionally send error back to client if needed
        // connection.send(JSON.stringify({ type: 'error', message: 'Processing failed' }));
    }
  } // End onMessage

  // Removed other methods like chat(), setGitHubToken() etc.
} // End Solver Class
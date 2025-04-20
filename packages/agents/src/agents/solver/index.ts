import { Agent, type Connection, type WSMessage } from "agents";
import type { UIMessage } from "ai";
import { Effect, Cause, Data, Exit, Runtime } from "effect";
import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";

// Define the state to hold context
type SolverState = {
  messages: UIMessage[];
  currentIssue?: BaseIssue;
  currentProject?: BaseProject;
  currentTeam?: BaseTeam;
};

// Define potential errors for this operation
class ParseError extends Data.TaggedError("ParseError")<{ cause: unknown }> { }
class SetStateError extends Data.TaggedError("SetStateError")<{ cause: unknown }> { }
// Define the union of possible errors for the handleMessageEffect
type HandleMessageError = ParseError | SetStateError;

export class Solver extends Agent<Env, SolverState> {

  // Initial state needs to align with the type
  initialState: SolverState = {
    messages: [],
    currentIssue: undefined,
    currentProject: undefined,
    currentTeam: undefined
  };

  /**
   * Handles incoming WebSocket messages using Effect
   */
  async onMessage(connection: Connection, message: WSMessage) {

    // Define the Effect workflow for handling the message
    // The Effect can fail with HandleMessageError and has no requirements (R = never)
    const handleMessageEffect: Effect.Effect<void, HandleMessageError, never> =
      Effect.gen(this, function* (self) { // Use `this` for context
        // Note: `self` here is the Effect adapter, typically named `_` or `$`

        // 1. Try parsing the message
        const parsedMessage = yield* Effect.try({
          try: () => JSON.parse(message as string),
          catch: (unknown) => new ParseError({ cause: unknown })
        });

        // Use structured logging via Effect.log* and annotations
        yield* Effect.logInfo("ON MESSAGE RECEIVED:").pipe(Effect.annotateLogs(parsedMessage));

        // 2. Check message type and update state if it's set_context
        if (parsedMessage.type === 'set_context') {
          const { issue, project, team } = parsedMessage;

          // Minimal validation (Effect Schema could enhance this later)
          if (!issue?.id || !project?.id || !team?.id) {
            yield* Effect.logWarning("Received set_context message with missing data.");
            // Returning void here, effectively skipping the update for this message.
            // Could also yield* Effect.fail(...) if this is considered an error.
            return;
          }

          // 3. Describe the state update as an Effect using `this.setState`
          yield* Effect.tryPromise({
            // Make the try function async and await setState
            try: async () => {
              // Spread current state and override specific fields for partial update
              this.setState({
                ...this.state, // Include existing state (like messages)
                currentIssue: issue,
                currentProject: project,
                currentTeam: team
                // Consider deep cloning here if BaseIssue/Project/Team are complex
                // and if setState doesn't handle it automatically.
              });
            },
            catch: (unknown) => new SetStateError({ cause: unknown })
          });

          yield* Effect.logInfo("Agent context state updated successfully.");

        } else {
          // Log other message types for debugging if needed
          yield* Effect.logDebug(`Unhandled message type: ${parsedMessage.type}`);
        }
      }); // Dependencies R = never for now

    // Run the Effect workflow
    // Using the default runtime for now. A custom runtime might be used
    // later if Layers for dependencies (like AI clients, GitHub clients) are introduced.
    const exit = await Runtime.runPromiseExit(Runtime.defaultRuntime)(handleMessageEffect);

    // Handle potential failures from the Effect workflow
    if (Exit.isFailure(exit)) {
      // Log the detailed failure cause using Effect's pretty printing
      Cause.pretty(exit.cause).split("\n").forEach(line => console.error(line));

      // Optionally, send a generic error back to the client if appropriate
      // connection.send(JSON.stringify({ type: "error", message: "Failed to process your request." }));
    }
  }
}

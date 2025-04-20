import { Agent, type Connection, type WSMessage } from "agents";
import type { UIMessage } from "ai";
import { Effect, Cause, Data, Exit, Runtime, Option } from "effect"; // Import Effect types & Data
import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core"; // Assuming these types exist

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
    const handleMessageEffect = Effect.gen(this, function* (self) { // Use `this` for context
      // Note: `self` here is the Effect adapter, typically named `_` or `$`

      const parsedMessage = yield* Effect.try({
        try: () => JSON.parse(message as string),
        catch: (unknown) => new ParseError({ cause: unknown })
      });

      yield* Effect.logInfo("ON MESSAGE RECEIVED:").pipe(Effect.annotateLogs(parsedMessage));

      if (parsedMessage.type === 'set_context') {
        const { issue, project, team } = parsedMessage;

        if (!issue?.id || !project?.id || !team?.id) {
          yield* Effect.logWarning("Received set_context message with missing data.");
          return;
        }

        // --- FIX FOR ERROR 1 & 2 ---
        yield* Effect.tryPromise({
          // Make the try function async and await setState
          try: async () => {
            // Spread current state and override specific fields
            await this.setState({
              ...this.state, // Include existing state (like messages)
              currentIssue: issue,
              currentProject: project,
              currentTeam: team
            });
          },
          catch: (unknown) => new SetStateError({ cause: unknown })
        });
        // --- END FIX ---

        yield* Effect.logInfo("Agent context state updated successfully.");
      } else {
        yield* Effect.logDebug(`Unhandled message type: ${parsedMessage.type}`);
      }
    }); // Dependencies R = never for now

    // Run the Effect workflow
    const exit = await Runtime.runPromiseExit(Runtime.defaultRuntime)(handleMessageEffect);

    // Handle potential failures from the Effect workflow
    if (Exit.isFailure(exit)) {
      Cause.pretty(exit.cause).split("\n").forEach(line => console.error(line));
      // Optionally send error back to client
    }
  }
}

import { Agent, type Connection, type WSMessage } from "agents";
import type { UIMessage } from "ai";
import {
  Effect,
  Cause,
  Data,
  Exit,
  Runtime,
  LogLevel,
  FiberRefs,
  FiberRef,
  FiberId,
} from "effect";
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
type HandleMessageError = ParseError | SetStateError;

// Configure the runtime with Debug level logging
const customizedRuntime = Runtime.make({
  context: Runtime.defaultRuntime.context,
  runtimeFlags: Runtime.defaultRuntime.runtimeFlags,
  fiberRefs: FiberRefs.updateAs(
    Runtime.defaultRuntime.fiberRefs,
    {
      fiberId: FiberId.none,
      fiberRef: FiberRef.currentMinimumLogLevel,
      value: LogLevel.Debug
    }
  )
});

export class Solver extends Agent<Env, SolverState> {

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

    const handleMessageEffect: Effect.Effect<void, HandleMessageError, never> =
      Effect.gen(this, function* (self) {
        const parsedMessage = yield* Effect.try({
          try: () => JSON.parse(message as string),
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

        if (parsedMessage.type === 'set_context') {
          const { issue, project, team } = parsedMessage;

          if (!issue?.id || !project?.id || !team?.id) {
            yield* Effect.logWarning(`⚠️  Missing Required Context Data`).pipe(
              Effect.annotateLogs({
                messageType: parsedMessage.type,
                missingFields: [
                  !issue?.id && 'issue.id',
                  !project?.id && 'project.id',
                  !team?.id && 'team.id'
                ].filter(Boolean).join(', ')
              })
            );
            return;
          }

          yield* Effect.tryPromise({
            try: async () => {
              this.setState({
                ...this.state,
                currentIssue: issue,
                currentProject: project,
                currentTeam: team
              });
            },
            catch: (unknown) => new SetStateError({ cause: unknown })
          });

          yield* Effect.logInfo(`✓ Context Updated Successfully`).pipe(
            Effect.annotateLogs({
              issueId: issue.id,
              projectName: project.name,
              teamKey: team.key
            })
          );
        } else {
          yield* Effect.logDebug(`⚠️  Unhandled Message Type: ${parsedMessage.type}`);
        }
      });

    // Run with the customized runtime
    const exit = await Runtime.runPromiseExit(customizedRuntime)(handleMessageEffect);

    if (Exit.isFailure(exit)) {
      await Runtime.runPromise(customizedRuntime)(
        Effect.logError(`❌ Message Processing Failed`).pipe(
          Effect.annotateLogs({
            failureCause: Cause.pretty(exit.cause)
              .replace(/\\"/g, '"')  // Remove escaped quotes
              .split('\n')
              .map(line => `│ ${line}`)
              .join('\n')
          })
        )
      );
    }
  }
}

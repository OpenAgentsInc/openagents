import { Effect, Data } from "effect";
import type { Agent } from "agents";
import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";

// Define potential errors for this operation
export class ParseError extends Data.TaggedError("ParseError")<{ cause: unknown }> { }
export class SetStateError extends Data.TaggedError("SetStateError")<{ cause: unknown }> { }
export type HandleMessageError = ParseError | SetStateError;

export type SolverState = {
  messages: any[]; // Using any[] since UIMessage type isn't available here
  currentIssue?: BaseIssue;
  currentProject?: BaseProject;
  currentTeam?: BaseTeam;
};

export const createHandleMessageEffect = (
  agent: Agent<any, SolverState>,
  message: string
): Effect.Effect<void, HandleMessageError, never> =>
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
          agent.setState({
            ...agent.state,
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

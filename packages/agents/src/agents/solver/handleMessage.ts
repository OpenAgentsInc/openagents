import { Effect, Data } from "effect";
import type { Agent } from "agents";
import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";
import type { SolverState } from "./index";
import { createInitialChatEffect } from "./chat";
import { ChatError, AnthropicConfig } from "./types";

// Define potential errors for this operation
export class ParseError extends Data.TaggedError("ParseError")<{ cause: unknown }> { }
export class SetStateError extends Data.TaggedError("SetStateError")<{ cause: unknown }> { }
export type HandleMessageError = ParseError | SetStateError | ChatError;

export const createHandleMessageEffect = (
  agent: Agent<any, SolverState>,
  message: string
): Effect.Effect<void, HandleMessageError, AnthropicConfig> =>
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
        
        yield* Effect.logInfo(`✓ Initial welcome message sent`).pipe(
          Effect.annotateLogs({
            messageId: assistantMessage.id,
            content: assistantMessage.content.substring(0, 100) + (assistantMessage.content.length > 100 ? '...' : '')
          })
        );
      } catch (error) {
        yield* Effect.logError(`❌ Failed to generate initial chat message`, { error });
      }
    } else {
      yield* Effect.logDebug(`⚠️  Unhandled Message Type: ${parsedMessage.type}`);
    }
  });

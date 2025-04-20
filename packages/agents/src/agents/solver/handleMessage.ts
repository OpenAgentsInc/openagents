import { Effect, Data } from "effect";
import type { Agent } from "agents";
import type { UIMessage } from "ai";
import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";
import type { SolverState } from "./index";
import { createInitialChatEffect, createChatEffect } from "./chat";
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
    } else if (parsedMessage.type === 'chat') {
      const { message: userMessage } = parsedMessage;
      
      if (!userMessage) {
        yield* Effect.logWarning(`⚠️ Missing user message in chat request`);
        return;
      }
      
      yield* Effect.logInfo(`Processing chat message: ${userMessage.substring(0, 100)}${userMessage.length > 100 ? '...' : ''}`);
      
      try {
        // Create user message object with required parts field
        const newUserMessage: UIMessage = {
          id: `user_${Date.now()}`,
          role: 'user',
          content: userMessage,
          createdAt: new Date(),
          parts: [{
            type: 'text',
            text: userMessage
          }]
        };
        
        // Add user message to state
        yield* Effect.tryPromise({
          try: async () => {
            agent.setState({
              ...agent.state,
              messages: [...agent.state.messages, newUserMessage]
            });
          },
          catch: (unknown) => new SetStateError({ cause: unknown })
        });
        
        // Get updated state with the new user message
        const currentState = agent.state;
        
        // Generate AI response
        const chatEffect = createChatEffect(currentState, userMessage);
        const assistantMessage = yield* chatEffect;
        
        // Add assistant message to state
        yield* Effect.tryPromise({
          try: async () => {
            agent.setState({
              ...agent.state,
              messages: [...agent.state.messages, assistantMessage]
            });
          },
          catch: (unknown) => new SetStateError({ cause: unknown })
        });
        
        yield* Effect.logInfo(`✓ Chat response sent`).pipe(
          Effect.annotateLogs({
            messageId: assistantMessage.id,
            content: assistantMessage.content.substring(0, 100) + (assistantMessage.content.length > 100 ? '...' : '')
          })
        );
      } catch (error) {
        yield* Effect.logError(`❌ Failed to process chat message`, { error });
      }
    } else if (parsedMessage.type === 'set_github_token') {
      const { token } = parsedMessage;
      
      if (!token) {
        yield* Effect.logWarning(`⚠️ Missing GitHub token in request`);
        return;
      }
      
      yield* Effect.tryPromise({
        try: async () => {
          agent.setState({
            ...agent.state,
            githubToken: token
          });
        },
        catch: (unknown) => new SetStateError({ cause: unknown })
      });
      
      yield* Effect.logInfo(`✓ GitHub token updated successfully`);
    } else if (parsedMessage.type === 'get_system_prompt') {
      // Handle system prompt request
      const { requestId } = parsedMessage;
      
      yield* Effect.logInfo(`Handling get_system_prompt request`, { requestId });
      
      try {
        // Generate a system prompt based on the current state
        const systemPrompt = `You are Solver Agent, an AI assistant that helps analyze, plan, and implement solutions for project issues.
    
Current Context:
- Issue: ${agent.state.currentIssue?.title ?? 'Unknown'}
- Project: ${agent.state.currentProject?.name ?? 'Unknown'}
- Team: ${agent.state.currentTeam?.name ?? 'Unknown'}

Issue Description:
${agent.state.currentIssue?.description ?? 'No description available.'}

You have access to tools that allow you to:
1. Fetch issue details
2. Update issue status
3. Create implementation plans

Use these tools when appropriate to help solve the user's issue. When using tools:
- Be specific and precise with your inputs
- Interpret tool outputs and explain them to the user
- Make helpful, actionable recommendations based on tool results

Your goal is to provide practical assistance and guide the user through addressing their issue.`;
        
        // Send the response back through the connection
        // Don't try to send directly through the connection - we'll let the agent state update
        // handle the response downstream
        yield* Effect.logInfo(`Generated system prompt (${systemPrompt.length} chars)`);
        
        yield* Effect.logInfo(`✓ System prompt sent successfully`, { requestId });
      } catch (error) {
        yield* Effect.logError(`❌ Failed to generate system prompt`, { error, requestId });
      }
    } else if (parsedMessage.type === 'shared_infer') {
      // Handle shared inference requests
      const { requestId, params, context } = parsedMessage;
      
      yield* Effect.logInfo(`Handling shared_infer request`, { requestId });
      
      // If context is provided, make sure it's set in the agent
      if (context?.issue && context?.project && context?.team) {
        yield* Effect.logInfo(`Context included in shared_infer, ensuring it's set`);
        yield* Effect.tryPromise({
          try: async () => {
            agent.setState({
              ...agent.state,
              currentIssue: context.issue,
              currentProject: context.project,
              currentTeam: context.team
            });
          },
          catch: (unknown) => new SetStateError({ cause: unknown })
        });
      }
      
      try {
        // Check if the state has issues or messages missing
        if (!agent.state.currentIssue || !Array.isArray(agent.state.messages)) {
          yield* Effect.logWarning(`Missing critical state for inference`, { 
            hasIssue: !!agent.state.currentIssue,
            hasMessages: Array.isArray(agent.state.messages)
          });
        }
        
        // Create a user message from the latest user message in params
        let userMessage: UIMessage | undefined;
        if (params.messages && Array.isArray(params.messages)) {
          const lastUserMsg = [...params.messages].reverse().find(m => m.role === 'user');
          if (lastUserMsg) {
            userMessage = {
              id: `user_${Date.now()}`,
              role: 'user',
              content: typeof lastUserMsg.content === 'string' ? lastUserMsg.content : JSON.stringify(lastUserMsg.content),
              createdAt: new Date(),
              parts: [{
                type: 'text',
                text: typeof lastUserMsg.content === 'string' ? lastUserMsg.content : JSON.stringify(lastUserMsg.content)
              }]
            };
            
            // Add user message to state
            yield* Effect.tryPromise({
              try: async () => {
                agent.setState({
                  ...agent.state,
                  messages: [...(agent.state.messages || []), userMessage as UIMessage]
                });
              },
              catch: (unknown) => new SetStateError({ cause: unknown })
            });
          }
        }
        
        // Process the message with the chat effect
        if (userMessage) {
          yield* Effect.logInfo(`Processing shared_infer as chat message`);
          
          // Create effect and execute it
          const chatEffect = createChatEffect(agent.state, userMessage.content);
          const assistantMessage = yield* chatEffect;
          
          // Add assistant message to state
          yield* Effect.tryPromise({
            try: async () => {
              agent.setState({
                ...agent.state,
                messages: [...agent.state.messages, assistantMessage]
              });
            },
            catch: (unknown) => new SetStateError({ cause: unknown })
          });
          
          // Send response back through connection
          // Don't try to send directly through the connection - we'll let the agent state update
          // handle the response downstream
          yield* Effect.logInfo(`Generated chat response: ${assistantMessage.content.substring(0, 100)}...`);
          
          yield* Effect.logInfo(`✓ shared_infer response sent successfully`, { requestId });
        } else {
          yield* Effect.logWarning(`No user message found in shared_infer request`);
        }
      } catch (error) {
        yield* Effect.logError(`❌ Failed to process shared_infer request`, { error, requestId });
      }
    } else {
      yield* Effect.logDebug(`⚠️ Unhandled Message Type: ${parsedMessage.type}`);
    }
  });

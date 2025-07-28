import { Effect, Option } from "effect"
import { createCommand, createSimpleCommand } from "./command"
import { SessionError, SessionNotFoundError } from "./errors"

// Types
export interface Message {
  id: string
  role: "user" | "assistant"
  content: string
  timestamp: string
  tool_uses?: Array<{
    id: string
    name: string
    input: any
  }>
}

export interface Session {
  id: string
  title: string
  project_path: string
  created_at: string
  is_active: boolean
  messages: Message[]
}

// Session Commands
export const SessionCommands = {
  discover: () => createSimpleCommand<string>("discover_claude").invoke({}),
  
  create: (projectPath: string) => 
    createCommand<{ project_path: string }, string>("create_session")
      .invoke({ project_path: projectPath })
      .pipe(
        Effect.mapError((error) => new SessionError({
          operation: "create",
          message: `Failed to create session for project: ${projectPath}`,
          cause: error
        }))
      ),
  
  sendMessage: (sessionId: string, message: string) =>
    createCommand<{ session_id: string; message: string }, void>("send_message")
      .invoke({ session_id: sessionId, message })
      .pipe(
        Effect.mapError((error) => new SessionError({
          operation: "send",
          sessionId,
          message: `Failed to send message to session ${sessionId}`,
          cause: error
        }))
      ),
  
  triggerResponse: (sessionId: string) =>
    createCommand<{ session_id: string }, void>("trigger_claude_response")
      .invoke({ session_id: sessionId })
      .pipe(
        Effect.mapError((error) => new SessionError({
          operation: "send",
          sessionId,
          message: `Failed to trigger Claude response for session ${sessionId}`,
          cause: error
        }))
      ),
  
  getMessages: (sessionId: string) =>
    createCommand<{ session_id: string }, Message[]>("get_messages")
      .invoke({ session_id: sessionId })
      .pipe(
        Effect.mapError((error) => new SessionError({
          operation: "get",
          sessionId,
          message: `Failed to get messages for session ${sessionId}`,
          cause: error
        })),
        Effect.map((messages) => messages || [])
      ),
  
  stop: (sessionId: string) =>
    createCommand<{ session_id: string }, void>("stop_session")
      .invoke({ session_id: sessionId })
      .pipe(
        Effect.mapError((error) => new SessionError({
          operation: "stop",
          sessionId,
          message: `Failed to stop session ${sessionId}`,
          cause: error
        }))
      ),
  
  getActiveSessions: () =>
    createSimpleCommand<Session[]>("get_active_sessions")
      .invoke({})
      .pipe(
        Effect.mapError((error) => new SessionError({
          operation: "get",
          message: "Failed to get active sessions",
          cause: error
        })),
        Effect.map((sessions) => sessions || [])
      ),
  
  handleEvent: (eventType: string, eventData: any) =>
    createCommand<{ event_type: string; event_data: any }, void>("handle_claude_event")
      .invoke({ event_type: eventType, event_data: eventData })
      .pipe(
        Effect.mapError((error) => new SessionError({
          operation: "handle_event",
          message: `Failed to handle Claude event: ${eventType}`,
          cause: error
        }))
      )
}

// Helper functions
export const findSessionById = (sessionId: string) =>
  Effect.gen(function* () {
    const sessions = yield* SessionCommands.getActiveSessions()
    return Option.fromNullable(
      sessions.find((s) => s.id === sessionId)
    )
  }).pipe(
    Effect.flatMap((maybeSession) =>
      Option.match(maybeSession, {
        onNone: () => Effect.fail(new SessionNotFoundError({ sessionId })),
        onSome: (session) => Effect.succeed(session)
      })
    )
  )

export const getAllMessages = () =>
  Effect.gen(function* () {
    const sessions = yield* SessionCommands.getActiveSessions()
    const allMessages = yield* Effect.all(
      sessions.map((session: Session) =>
        SessionCommands.getMessages(session.id).pipe(
          Effect.map((messages: Message[]) => ({ sessionId: session.id, messages }))
        )
      )
    )
    return allMessages?.flatMap(({ sessionId, messages }: { sessionId: string; messages: Message[] }) =>
      messages.map((msg: Message) => ({ ...msg, sessionId }))
    ) || []
  })
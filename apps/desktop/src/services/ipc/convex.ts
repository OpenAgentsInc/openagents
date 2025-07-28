import { Effect, Option } from "effect"
import { createCommand, createSimpleCommand } from "./command"
import { ConvexError, SessionNotFoundError, MessageNotFoundError } from "./errors"

// Types
export interface ConvexSession {
  _id: string
  _creationTime: number
  userId?: string
  title?: string
  createdAt: number
  updatedAt?: number
  deletedAt?: number
}

export interface ConvexMessage {
  _id: string
  _creationTime: number
  sessionId: string
  content: string
  role: "user" | "assistant"
  createdAt: number
  updatedAt?: number
  deletedAt?: number
}

// Convex Commands
export const ConvexCommands = {
  testConnection: () =>
    createSimpleCommand<string>("test_convex_connection")
      .invoke({})
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "test",
          message: "Failed to test Convex connection",
          cause: error
        }))
      ),
  
  getSessions: (limit?: number) =>
    createCommand<{ limit?: number }, ConvexSession[]>("get_sessions")
      .invoke({ limit })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "get_sessions",
          message: `Failed to get sessions${limit ? ` with limit ${limit}` : ""}`,
          cause: error
        })),
        Effect.map((sessions) => sessions || [])
      ),
  
  createSession: (title?: string) =>
    createCommand<{ title?: string }, string>("create_convex_session")
      .invoke({ title })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "create_session",
          message: `Failed to create session${title ? ` with title "${title}"` : ""}`,
          cause: error
        }))
      ),
  
  updateSession: (sessionId: string, updates: Partial<ConvexSession>) =>
    createCommand<{ session_id: string; updates: any }, void>("update_session")
      .invoke({ session_id: sessionId, updates })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "update_session",
          id: sessionId,
          message: `Failed to update session ${sessionId}`,
          cause: error
        }))
      ),
  
  deleteSession: (sessionId: string) =>
    createCommand<{ session_id: string }, void>("delete_session")
      .invoke({ session_id: sessionId })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "delete_session",
          id: sessionId,
          message: `Failed to delete session ${sessionId}`,
          cause: error
        }))
      ),
  
  getSessionById: (sessionId: string) =>
    createCommand<{ session_id: string }, ConvexSession | null>("get_session_by_id")
      .invoke({ session_id: sessionId })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "get_session",
          id: sessionId,
          message: `Failed to get session ${sessionId}`,
          cause: error
        })),
        Effect.flatMap((session) =>
          session 
            ? Effect.succeed(session)
            : Effect.fail(new SessionNotFoundError({ sessionId }))
        )
      ),
  
  getMessages: (sessionId: string, limit?: number) =>
    createCommand<{ session_id: string; limit?: number }, ConvexMessage[]>("get_convex_messages")
      .invoke({ session_id: sessionId, limit })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "get_messages",
          id: sessionId,
          message: `Failed to get messages for session ${sessionId}`,
          cause: error
        })),
        Effect.map((messages) => messages || [])
      ),
  
  addMessage: (sessionId: string, content: string, role: "user" | "assistant" = "user") =>
    createCommand<{ session_id: string; content: string; role: string }, string>("add_message")
      .invoke({ session_id: sessionId, content, role })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "add_message",
          id: sessionId,
          message: `Failed to add message to session ${sessionId}`,
          cause: error
        }))
      ),
  
  updateMessage: (messageId: string, updates: Partial<ConvexMessage>) =>
    createCommand<{ message_id: string; updates: any }, void>("update_message")
      .invoke({ message_id: messageId, updates })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "update_message",
          id: messageId,
          message: `Failed to update message ${messageId}`,
          cause: error
        }))
      ),
  
  deleteMessage: (messageId: string) =>
    createCommand<{ message_id: string }, void>("delete_message")
      .invoke({ message_id: messageId })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "delete_message",
          id: messageId,
          message: `Failed to delete message ${messageId}`,
          cause: error
        }))
      ),
  
  getMessageById: (messageId: string) =>
    createCommand<{ message_id: string }, ConvexMessage | null>("get_message_by_id")
      .invoke({ message_id: messageId })
      .pipe(
        Effect.mapError((error) => new ConvexError({
          operation: "get_message",
          id: messageId,
          message: `Failed to get message ${messageId}`,
          cause: error
        })),
        Effect.flatMap((message) =>
          message 
            ? Effect.succeed(message)
            : Effect.fail(new MessageNotFoundError({ messageId }))
        )
      )
}

// Helper functions
export const findSessionByTitle = (title: string) =>
  Effect.gen(function* () {
    const sessions = yield* ConvexCommands.getSessions()
    return Option.fromNullable(
      sessions.find((s) => s.title === title)
    )
  })

export const getRecentSessions = (days: number = 7) =>
  Effect.gen(function* () {
    const sessions = yield* ConvexCommands.getSessions()
    const cutoff = Date.now() - days * 24 * 60 * 60 * 1000
    return sessions.filter((s) => (s.updatedAt || s.createdAt) > cutoff)
  })

export const deleteSessionWithMessages = (sessionId: string) =>
  Effect.gen(function* () {
    // First delete all messages
    const messages = yield* ConvexCommands.getMessages(sessionId)
    yield* Effect.forEach(messages, (msg) => 
      ConvexCommands.deleteMessage(msg._id),
      { concurrency: 5 }
    )
    // Then delete the session
    yield* ConvexCommands.deleteSession(sessionId)
  })
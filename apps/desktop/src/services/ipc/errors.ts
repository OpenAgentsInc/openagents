import { Data } from "effect"

// Base error class for all IPC errors
export class IPCError extends Data.TaggedError("IPCError")<{
  command: string
  args?: unknown
  cause?: unknown
}> {}

// Domain-specific IPC errors
export class SessionError extends Data.TaggedError("SessionError")<{
  operation: "create" | "send" | "stop" | "discover" | "get" | "handle_event"
  sessionId?: string
  message?: string
  cause?: unknown
}> {}

export class APMError extends Data.TaggedError("APMError")<{
  operation: "analyze" | "get_stats" | "get_historical"
  message?: string
  cause?: unknown
}> {}

export class HistoryError extends Data.TaggedError("HistoryError")<{
  operation: "get" | "get_unified"
  limit?: number
  message?: string
  cause?: unknown
}> {}

export class SystemError extends Data.TaggedError("SystemError")<{
  operation: "greet" | "get_directory"
  message?: string
  cause?: unknown
}> {}

export class ConvexError extends Data.TaggedError("ConvexError")<{
  operation: "test" | "get_sessions" | "create_session" | "update_session" | 
             "delete_session" | "get_session" | "get_messages" | "add_message" | 
             "update_message" | "delete_message" | "get_message"
  id?: string
  message?: string
  cause?: unknown
}> {}

// Specific error types
export class SessionNotFoundError extends Data.TaggedError("SessionNotFoundError")<{
  sessionId: string
}> {}

export class MessageNotFoundError extends Data.TaggedError("MessageNotFoundError")<{
  messageId: string
}> {}

export class NetworkError extends Data.TaggedError("NetworkError")<{
  url?: string
  statusCode?: number
  cause?: unknown
}> {}

export class ValidationError extends Data.TaggedError("ValidationError")<{
  field: string
  value: unknown
  constraints: string[]
}> {}

export class TimeoutError extends Data.TaggedError("TimeoutError")<{
  operation: string
  timeout: number
}> {}
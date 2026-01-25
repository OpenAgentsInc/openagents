import { Schema } from "effect"
import type {
  ConnectUnifiedAgentResponse,
  CurrentDirectory,
  DisconnectUnifiedAgentResponse,
  SendUnifiedMessageResponse,
  StartUnifiedSessionResponse,
  UnifiedEvent,
} from "../gen/tauri-contracts"

export const CurrentDirectorySchema: Schema.Schema<CurrentDirectory> =
  Schema.String

export const ConnectUnifiedAgentResponseSchema: Schema.Schema<ConnectUnifiedAgentResponse> =
  Schema.Struct({
    success: Schema.Boolean,
    sessionId: Schema.String,
    agentId: Schema.String,
    workspaceId: Schema.String,
  })

export const DisconnectUnifiedAgentResponseSchema: Schema.Schema<DisconnectUnifiedAgentResponse> =
  Schema.Struct({
    success: Schema.Boolean,
    sessionId: Schema.String,
  })

export const StartUnifiedSessionResponseSchema: Schema.Schema<StartUnifiedSessionResponse> =
  Schema.Struct({
    success: Schema.Boolean,
    sessionId: Schema.String,
  })

export const SendUnifiedMessageResponseSchema: Schema.Schema<SendUnifiedMessageResponse> =
  Schema.Struct({
    success: Schema.Boolean,
    sessionId: Schema.String,
  })

const MessageChunkSchema = Schema.Struct({
  type: Schema.Literal("MessageChunk"),
  session_id: Schema.String,
  content: Schema.String,
  is_complete: Schema.Boolean,
})

const ThoughtChunkSchema = Schema.Struct({
  type: Schema.Literal("ThoughtChunk"),
  session_id: Schema.String,
  content: Schema.String,
  is_complete: Schema.Boolean,
})

const ToolCallSchema = Schema.Struct({
  type: Schema.Literal("ToolCall"),
  session_id: Schema.String,
  tool_id: Schema.String,
  tool_name: Schema.String,
  arguments: Schema.Unknown,
})

const ToolCallUpdateSchema = Schema.Struct({
  type: Schema.Literal("ToolCallUpdate"),
  session_id: Schema.String,
  tool_id: Schema.String,
  output: Schema.String,
  is_complete: Schema.Boolean,
})

const SessionStartedSchema = Schema.Struct({
  type: Schema.Literal("SessionStarted"),
  session_id: Schema.String,
  agent_id: Schema.Literal("Codex", "ClaudeCode", "Cursor", "Gemini", "Adjutant"),
})

const SessionCompletedSchema = Schema.Struct({
  type: Schema.Literal("SessionCompleted"),
  session_id: Schema.String,
  stop_reason: Schema.String,
})

const TokenUsageSchema = Schema.Struct({
  type: Schema.Literal("TokenUsage"),
  session_id: Schema.String,
  input_tokens: Schema.Number,
  output_tokens: Schema.Number,
  total_tokens: Schema.Number,
})

const RateLimitUpdateSchema = Schema.Struct({
  type: Schema.Literal("RateLimitUpdate"),
  agent_id: Schema.Literal("Codex", "ClaudeCode", "Cursor", "Gemini", "Adjutant"),
  used_percent: Schema.Number,
  resets_at: Schema.Union(Schema.Number, Schema.Null),
})

export const UnifiedEventSchema: Schema.Schema<UnifiedEvent> = Schema.Union(
  MessageChunkSchema,
  ThoughtChunkSchema,
  ToolCallSchema,
  ToolCallUpdateSchema,
  SessionStartedSchema,
  SessionCompletedSchema,
  TokenUsageSchema,
  RateLimitUpdateSchema
)

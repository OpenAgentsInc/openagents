// Central ACP types module
// Re-export the official ACP TypeScript SDK types so UI and Convex code stay
// aligned with the spec. Add local helpers and aliases here if we need to
// extend shapes for app concerns.

export type {
  // Top-level protocol and update shapes
  SessionNotification,
  // Content
  ContentBlock,
  // Tool call content and metadata
  ToolCallContent,
  ToolCallStatus,
  ToolKind,
  ToolCallLocation,
  // Plan and commands
  PlanEntry,
  AvailableCommand,
} from '@agentclientprotocol/sdk'

// Convenient aliases for common discriminated unions
import type { SessionNotification as _SN } from '@agentclientprotocol/sdk'

export type SessionUpdate = _SN['update']
export type AgentMessageChunk = Extract<_SN['update'], { sessionUpdate: 'agent_message_chunk' }>
export type AgentThoughtChunk = Extract<_SN['update'], { sessionUpdate: 'agent_thought_chunk' }>
export type UserMessageChunk = Extract<_SN['update'], { sessionUpdate: 'user_message_chunk' }>
export type ToolCallCreate = Extract<_SN['update'], { sessionUpdate: 'tool_call' }>
export type ToolCallUpdate = Extract<_SN['update'], { sessionUpdate: 'tool_call_update' }>
export type PlanUpdate = Extract<_SN['update'], { sessionUpdate: 'plan' }>
export type AvailableCommandsUpdate = Extract<_SN['update'], { sessionUpdate: 'available_commands_update' }>
export type CurrentModeUpdate = Extract<_SN['update'], { sessionUpdate: 'current_mode_update' }>

// Normalized props expected by some UI components
// For rendering a tool call row, we only need a subset of fields.
export type ToolCallLike = Pick<ToolCallCreate, 'title' | 'status' | 'kind' | 'content' | 'locations'>

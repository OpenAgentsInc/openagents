// Minimal ACP type shapes used by the UI components

export type Role = 'assistant' | 'user'

// Content blocks
export type TextContent = { type: 'text'; text: string }
export type ImageContent = { type: 'image'; data: string; mimeType: string; uri?: string | null }
export type AudioContent = { type: 'audio'; data: string; mimeType: string }
export type ResourceLink = {
  type: 'resource_link'
  name: string
  uri: string
  mimeType?: string | null
  description?: string | null
  title?: string | null
  size?: number | null
}
export type TextResourceContents = { text: string; uri: string; mimeType?: string | null }
export type BlobResourceContents = { blob: string; uri: string; mimeType?: string | null }
export type EmbeddedResource = {
  type: 'resource'
  resource: TextResourceContents | BlobResourceContents
}

export type ContentBlock = TextContent | ImageContent | AudioContent | ResourceLink | EmbeddedResource

// Tool call content
export type Diff = { path: string; oldText?: string | null; newText: string }
export type ToolCallContent =
  | { type: 'content'; content: ContentBlock }
  | ({ type: 'diff' } & Diff)
  | { type: 'terminal'; terminalId: string }

export type ToolKind =
  | 'read'
  | 'edit'
  | 'delete'
  | 'move'
  | 'search'
  | 'execute'
  | 'think'
  | 'fetch'
  | 'switch_mode'
  | 'other'

export type ToolCallStatus = 'pending' | 'in_progress' | 'completed' | 'failed'

export type ToolCallLocation = { path: string; line?: number | null }

export type ToolCall = {
  toolCallId?: string
  title: string
  kind: ToolKind
  status: ToolCallStatus
  content?: ToolCallContent[]
  locations?: ToolCallLocation[]
}

export type PlanEntry = {
  content: string
  priority: 'high' | 'medium' | 'low'
  status: 'pending' | 'in_progress' | 'completed'
}

export type Plan = { entries: PlanEntry[] }

// Session updates
export type SessionUpdate =
  | { sessionUpdate: 'user_message_chunk'; content: ContentBlock }
  | { sessionUpdate: 'agent_message_chunk'; content: ContentBlock }
  | { sessionUpdate: 'agent_thought_chunk'; content: ContentBlock }
  | ({ sessionUpdate: 'tool_call' } & ToolCall)
  | { sessionUpdate: 'plan'; entries: PlanEntry[] }
  | { sessionUpdate: 'available_commands_update'; available_commands: { name: string; description: string }[] }
  | { sessionUpdate: 'current_mode_update'; currentModeId: string }

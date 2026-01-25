import type { UnifiedEvent } from "../../gen/tauri-contracts"
import type { MessageRole } from "../ai-elements/message.js"

export type ConnectionPhase = "connecting" | "connected" | "ready" | "error"

export type { UnifiedEvent }

export interface RuntimeState {
  phase: ConnectionPhase
  workspacePath: string | null
  workspaceId: string | null
  sessionId: string | null
  eventCount: number
  isProcessing: boolean
}

export type FormattedMessage = {
  kind: "message"
  id: string
  role: MessageRole
  text: string
  isStreaming: boolean
}

export type FormattedReasoning = {
  kind: "reasoning"
  id: string
  summary: string
  content: string
  isStreaming: boolean
}

export type FormattedToolCall = {
  kind: "tool"
  id: string
  title: string
  detail?: string
  output?: string
  isStreaming: boolean
  status: "running" | "completed"
}

export type FormattedItem =
  | FormattedMessage
  | FormattedReasoning
  | FormattedToolCall

export type FormattedState = {
  items: FormattedItem[]
  messageIndex: number | null
  reasoningIndex: number | null
  toolIndexById: Map<string, number>
}

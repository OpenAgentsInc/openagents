/**
 * Agent Client Protocol Types
 */

export type AgentId = "Codex" | "Gemini" | "ClaudeCode" | "Cursor" | "Adjutant"

export interface AgentCapabilities {
  readonly session_new: boolean
  readonly session_load: boolean
  readonly session_list: boolean
  readonly prompt: boolean
  readonly fs_write: boolean
  readonly fs_read: boolean
  readonly terminal: boolean
}

export interface AgentCommand {
  readonly command: string
  readonly args: string[]
  readonly env: Record<string, string>
}

export interface AgentInfo {
  readonly id: AgentId
  readonly name: string
  readonly version: string
  readonly description?: string
}

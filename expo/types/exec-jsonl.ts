// Minimal TypeScript types for Exec JSONL ThreadItem variants used in UI components

export type AgentMessageItem = {
  type: 'agent_message'
  text: string
}

export type ReasoningItem = {
  type: 'reasoning'
  text: string
}

export type ThreadItem = AgentMessageItem | ReasoningItem


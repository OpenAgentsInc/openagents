export interface Message {
  id: string
  role: 'user' | 'assistant' | 'system' | 'function' | 'tool' | 'data'
  content: string
  createdAt?: string | Date
  parts?: MessagePart[]
  experimental_attachments?: Attachment[]
  toolInvocations?: ToolInvocation[]
  toolResults?: ToolResult[]
  actions?: Action[]
  isStreaming?: boolean
  isProcessingTools?: boolean
  isError?: boolean
}

export interface Attachment {
  url: string
  name?: string
  type?: string
}

export interface ToolInvocation {
  id: string
  toolName: string
  toolInput: any
  toolResponse?: any
  status?: 'running' | 'success' | 'error'
  timestamp?: Date
}

export interface ToolResult {
  id: string
  toolName: string
  result: any
  status: 'success' | 'error'
  timestamp?: Date
}

export interface Action {
  id: string
  type: string
  label: string
  onClick: () => void
}

export type MessagePart = TextUIPart | ReasoningUIPart | ToolInvocationUIPart

export interface TextUIPart {
  type: 'text'
  text: string
}

export interface ReasoningUIPart {
  type: 'reasoning'
  reasoning: string
}

export interface ToolInvocationUIPart {
  type: 'tool-invocation'
  toolInvocation: ToolInvocation
}
import React from 'react'
import { Text, FrameCorners } from '@arwes/react'
import { ChevronRight, FileText } from 'lucide-react'
import { MessagePartRenderer } from './MessagePartRenderer'
import type { Message as AIMessage } from '@ai-sdk/react'

export interface Message {
  id: string
  createdAt?: Date
  content: string
  role: 'system' | 'user' | 'assistant' | 'data'
  experimental_attachments?: AIMessage['experimental_attachments']
  toolInvocations?: Array<ToolInvocation>
  parts?: Array<TextUIPart | ReasoningUIPart | ToolInvocationUIPart | SourceUIPart | FileUIPart | StepStartUIPart>
  annotations?: JSONValue[]
  data?: JSONValue
}

export type UIMessage = Message & {
  parts: Array<TextUIPart | ReasoningUIPart | ToolInvocationUIPart | SourceUIPart | FileUIPart | StepStartUIPart>
}

export type TextUIPart = { type: 'text'; text: string }
export type ReasoningUIPart = { type: 'reasoning'; reasoning: string }
export type ToolInvocationUIPart = { type: 'tool-invocation'; toolInvocation: ToolInvocation }
export type SourceUIPart = { type: 'source'; source: any }
export type FileUIPart = { type: 'file'; mimeType: string; data: string }
export type StepStartUIPart = { type: 'step-start' }

export type ToolInvocation =
  | ({ state: 'partial-call'; step?: number } & ToolCall)
  | ({ state: 'call'; step?: number } & ToolCall)
  | ({ state: 'result'; step?: number } & ToolResult)

export interface ToolCall {
  toolCallId: string
  toolName: string
  args: Record<string, any>
}

export interface ToolResult extends ToolCall {
  result: any
}

export type Attachment = AIMessage['experimental_attachments'] extends (infer T)[] | undefined ? T : never

export type JSONValue = string | number | boolean | null | JSONValue[] | { [key: string]: JSONValue }

export interface ChatMessageProps {
  message: UIMessage | Message
  onToolResult?: ({ toolCallId, result }: { toolCallId: string; result: any }) => void
}

export const ChatMessage: React.FC<ChatMessageProps> = ({ message, onToolResult }) => {
  const isUser = message.role === 'user'
  const isSystem = message.role === 'system'

  // Don't render system messages or error messages
  if (isSystem || message.content.includes('Request not supported') || message.content.includes('not supported')) {
    return null
  }

  return (
    <div className="flex gap-3 mb-4">
      {isUser && (
        <div className="w-8 flex items-center justify-center text-cyan-300">
          <ChevronRight size={16} />
        </div>
      )}
      
      <div className="flex-1">
        <div className="py-2">
          {/* Render message content - same for both user and assistant */}
          {'parts' in message && message.parts && message.parts.length > 0 ? (
            <div className="text-cyan-300 font-sans">
              {message.parts.map((part, index) => {
                if (part.type === 'text') {
                  return (
                    <Text key={index} className="inline">
                      {part.text}
                    </Text>
                  )
                }
                if (part.type === 'step-start') {
                  return null
                }
                return (
                  <MessagePartRenderer 
                    key={index} 
                    part={part} 
                    onToolResult={onToolResult}
                  />
                )
              })}
            </div>
          ) : (
            <Text className="text-cyan-300 font-sans">
              {message.content}
            </Text>
          )}
        
          {/* Attachments */}
          {message.experimental_attachments?.map((attachment, index) => (
            <div key={index} className="mt-2 p-2 bg-cyan-800/20 rounded">
              <div className="flex items-center gap-2">
                <FileText size={14} className="text-cyan-400" />
                <span className="text-cyan-300 text-sm">{attachment.name}</span>
                {'size' in attachment && typeof attachment.size === 'number' && (
                  <span className="text-cyan-500 text-xs">
                    {(attachment.size / 1024).toFixed(1)}KB
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
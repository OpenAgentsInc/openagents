import React from 'react'
import { Text } from '@arwes/react'
import { Clock, FileText } from 'lucide-react'
import { ToolInvocationRenderer } from './ToolInvocationRenderer'
import type { 
  TextUIPart, 
  ReasoningUIPart, 
  ToolInvocationUIPart, 
  SourceUIPart, 
  FileUIPart, 
  StepStartUIPart 
} from './ChatMessage'

interface MessagePartRendererProps {
  part: TextUIPart | ReasoningUIPart | ToolInvocationUIPart | SourceUIPart | FileUIPart | StepStartUIPart
  onToolResult?: ({ toolCallId, result }: { toolCallId: string; result: any }) => void
}

export const MessagePartRenderer: React.FC<MessagePartRendererProps> = ({ part, onToolResult }) => {
  switch (part.type) {
    case 'text':
      // Text parts are handled in ChatMessage to avoid nesting issues
      return null
    
    case 'step-start':
      return (
        <div className="border-t border-cyan-500/30 my-4" />
      )
    
    case 'reasoning':
      return (
        <div className="bg-purple-900/20 border border-purple-500/30 p-3 rounded mb-2">
          <div className="flex items-center gap-2 mb-2">
            <Clock size={14} className="text-purple-400" />
            <Text className="text-purple-300 text-sm font-semibold">Reasoning</Text>
          </div>
          <div className="text-purple-200 text-sm font-mono">
            {part.reasoning}
          </div>
        </div>
      )
    
    case 'tool-invocation':
      return (
        <ToolInvocationRenderer 
          toolInvocation={part.toolInvocation} 
          onToolResult={onToolResult}
        />
      )
    
    case 'file':
      return (
        <div className="bg-cyan-900/20 border border-cyan-500/30 p-3 rounded mb-2">
          <div className="flex items-center gap-2">
            <FileText size={16} className="text-cyan-400" />
            <span className="text-cyan-300 text-sm">
              {part.mimeType} • {(part.data.length / 1024).toFixed(1)}KB
            </span>
          </div>
        </div>
      )
    
    case 'source':
      return (
        <div className="bg-yellow-900/20 border border-yellow-500/30 p-2 rounded mb-2">
          <Text className="text-yellow-300 text-xs">Source: {JSON.stringify(part.source)}</Text>
        </div>
      )
    
    default:
      return null
  }
}
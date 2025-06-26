import React, { useState, useCallback } from 'react'
import { Text } from '@arwes/react'
import { Clock, Play, CheckCircle, AlertCircle } from 'lucide-react'
import type { ToolInvocation, ToolCall } from './ChatMessage'

interface ToolInvocationRendererProps {
  toolInvocation: ToolInvocation
  onToolResult?: ({ toolCallId, result }: { toolCallId: string; result: any }) => void
}

interface ToolCallInterfaceProps {
  toolCall: ToolCall
  onResult: (result: any) => void
}

const ToolCallInterface: React.FC<ToolCallInterfaceProps> = ({ toolCall, onResult }) => {
  const [isExecuting, setIsExecuting] = useState(false)

  const handleExecute = useCallback(async () => {
    setIsExecuting(true)
    
    // Simulate tool execution
    setTimeout(() => {
      switch (toolCall.toolName) {
        case 'getWeather':
          onResult(`The weather in ${toolCall.args.city} is sunny, 24°C`)
          break
        case 'askForConfirmation':
          // Interactive confirmation - will be handled by buttons below
          break
        case 'searchDatabase':
          onResult({
            results: ['Item 1', 'Item 2', 'Item 3'],
            totalCount: 3,
            query: toolCall.args.query
          })
          break
        default:
          onResult(`Tool ${toolCall.toolName} executed successfully`)
      }
      setIsExecuting(false)
    }, 1000)
  }, [toolCall, onResult])

  if (toolCall.toolName === 'askForConfirmation') {
    return (
      <div className="space-y-2">
        <Text className="text-cyan-300 text-sm">{toolCall.args.message}</Text>
        <div className="flex gap-2">
          <button
            onClick={() => onResult('confirmed')}
            className="px-3 py-1 bg-green-600/20 text-green-300 border border-green-500/50 rounded text-sm hover:bg-green-600/30"
          >
            Confirm
          </button>
          <button
            onClick={() => onResult('cancelled')}
            className="px-3 py-1 bg-red-600/20 text-red-300 border border-red-500/50 rounded text-sm hover:bg-red-600/30"
          >
            Cancel
          </button>
        </div>
      </div>
    )
  }

  return (
    <button
      onClick={handleExecute}
      disabled={isExecuting}
      className="px-3 py-1 bg-blue-600/20 text-blue-300 border border-blue-500/50 rounded text-sm hover:bg-blue-600/30 disabled:opacity-50 flex items-center gap-2"
    >
      {isExecuting ? (
        <>
          <Clock size={12} className="animate-spin" />
          Executing...
        </>
      ) : (
        <>
          <Play size={12} />
          Execute Tool
        </>
      )}
    </button>
  )
}

export const ToolInvocationRenderer: React.FC<ToolInvocationRendererProps> = ({ 
  toolInvocation, 
  onToolResult 
}) => {
  const getStatusIcon = () => {
    switch (toolInvocation.state) {
      case 'partial-call':
        return <Clock size={14} className="text-yellow-400 animate-pulse" />
      case 'call':
        return <Play size={14} className="text-blue-400" />
      case 'result':
        return <CheckCircle size={14} className="text-green-400" />
    }
  }

  const getStatusColor = () => {
    switch (toolInvocation.state) {
      case 'partial-call':
        return 'border-yellow-500/30 bg-yellow-900/20'
      case 'call':
        return 'border-blue-500/30 bg-blue-900/20'
      case 'result':
        return 'border-green-500/30 bg-green-900/20'
    }
  }

  return (
    <div className={`border p-3 rounded mb-2 ${getStatusColor()}`}>
      <div className="flex items-center gap-2 mb-2">
        {getStatusIcon()}
        <Text className="text-cyan-300 font-semibold text-sm">
          {toolInvocation.toolName}
        </Text>
        {toolInvocation.step && (
          <span className="text-cyan-500 text-xs">Step {toolInvocation.step}</span>
        )}
      </div>
      
      {/* Tool Arguments */}
      <div className="bg-black/30 p-2 rounded text-xs font-mono mb-2">
        <Text className="text-cyan-500 mb-1">Arguments:</Text>
        <pre className="text-cyan-200 whitespace-pre-wrap">
          {JSON.stringify(toolInvocation.args, null, 2)}
        </pre>
      </div>

      {/* Tool State Handling */}
      {toolInvocation.state === 'call' && (
        <ToolCallInterface 
          toolCall={toolInvocation}
          onResult={(result) => onToolResult?.({ toolCallId: toolInvocation.toolCallId, result })}
        />
      )}

      {toolInvocation.state === 'result' && (
        <div className="bg-black/30 p-2 rounded text-xs font-mono">
          <Text className="text-green-400 mb-1">Result:</Text>
          <pre className="text-green-200 whitespace-pre-wrap">
            {typeof toolInvocation.result === 'string' 
              ? toolInvocation.result 
              : JSON.stringify(toolInvocation.result, null, 2)}
          </pre>
        </div>
      )}
    </div>
  )
}
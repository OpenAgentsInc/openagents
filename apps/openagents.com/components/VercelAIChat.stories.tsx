import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  Text,
  FrameCorners,
  FrameOctagon,
  BleepsProvider
} from '@arwes/react'
import React, { useState, useCallback, ReactNode } from 'react'
import { Send, User, Bot, CheckCircle, Clock, AlertCircle, Play, Pause, FileText } from 'lucide-react'

const meta = {
  title: 'AI Chat/Vercel AI SDK v4',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Chat interface components built with actual Vercel AI SDK v4 types and patterns. Includes message rendering, tool invocations, streaming states, and interactive tool execution.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Exact types from Vercel AI SDK v4
interface Message {
  id: string
  createdAt?: Date
  content: string
  role: 'system' | 'user' | 'assistant' | 'data'
  experimental_attachments?: Attachment[]
  toolInvocations?: Array<ToolInvocation> // deprecated
  parts?: Array<TextUIPart | ReasoningUIPart | ToolInvocationUIPart | SourceUIPart | FileUIPart | StepStartUIPart>
  annotations?: JSONValue[]
  data?: JSONValue
}

type UIMessage = Message & {
  parts: Array<TextUIPart | ReasoningUIPart | ToolInvocationUIPart | SourceUIPart | FileUIPart | StepStartUIPart>
}

type TextUIPart = { type: 'text'; text: string }
type ReasoningUIPart = { type: 'reasoning'; reasoning: string }
type ToolInvocationUIPart = { type: 'tool-invocation'; toolInvocation: ToolInvocation }
type SourceUIPart = { type: 'source'; source: any }
type FileUIPart = { type: 'file'; mimeType: string; data: string }
type StepStartUIPart = { type: 'step-start' }

type ToolInvocation =
  | ({ state: 'partial-call'; step?: number } & ToolCall)
  | ({ state: 'call'; step?: number } & ToolCall)
  | ({ state: 'result'; step?: number } & ToolResult)

interface ToolCall {
  toolCallId: string
  toolName: string
  args: Record<string, any>
}

interface ToolResult extends ToolCall {
  result: any
}

interface Attachment {
  name: string
  contentType: string
  size: number
  url: string
}

type JSONValue = string | number | boolean | null | JSONValue[] | { [key: string]: JSONValue }

// Chat hook simulation
interface UseChatHelpers {
  messages: UIMessage[]
  input: string
  error: undefined | Error
  isLoading: boolean
  status: 'submitted' | 'streaming' | 'ready' | 'error'
  data?: JSONValue[]
  metadata?: Object
  id: string
  append: (message: Message) => Promise<string | null | undefined>
  reload: () => Promise<string | null | undefined>
  stop: () => void
  setMessages: (messages: UIMessage[]) => void
  setInput: (input: string) => void
  handleInputChange: (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => void
  handleSubmit: (event?: { preventDefault?: () => void }) => void
  addToolResult: ({ toolCallId, result }: { toolCallId: string; result: any }) => void
}

// Message Part Renderer
const MessagePartRenderer = ({ part, onToolResult }: { 
  part: TextUIPart | ReasoningUIPart | ToolInvocationUIPart | SourceUIPart | FileUIPart | StepStartUIPart
  onToolResult?: ({ toolCallId, result }: { toolCallId: string; result: any }) => void 
}) => {
  switch (part.type) {
    case 'text':
      return (
        <div className="text-cyan-100">
          {part.text}
        </div>
      )
    
    case 'step-start':
      return (
        <Animated
          className="border-t border-cyan-500/30 my-4"
          animated={[['scaleX', 0, 1], ['opacity', 0, 1]]}
        />
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

// Tool Invocation Renderer
const ToolInvocationRenderer = ({ 
  toolInvocation, 
  onToolResult 
}: { 
  toolInvocation: ToolInvocation
  onToolResult?: ({ toolCallId, result }: { toolCallId: string; result: any }) => void 
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
    <Animated
      className={`border p-3 rounded mb-2 ${getStatusColor()}`}
      animated={[['scale', 0.95, 1], ['opacity', 0, 1]]}
    >
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
    </Animated>
  )
}

// Interactive Tool Call Interface
const ToolCallInterface = ({ 
  toolCall, 
  onResult 
}: { 
  toolCall: ToolCall
  onResult: (result: any) => void 
}) => {
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

// Status Indicator
const ChatStatusIndicator = ({ status }: { status: UseChatHelpers['status'] }) => {
  const getStatusInfo = () => {
    switch (status) {
      case 'submitted':
        return { icon: Clock, color: 'text-yellow-400', text: 'Submitted' }
      case 'streaming':
        return { icon: Play, color: 'text-blue-400 animate-pulse', text: 'Streaming' }
      case 'ready':
        return { icon: CheckCircle, color: 'text-green-400', text: 'Ready' }
      case 'error':
        return { icon: AlertCircle, color: 'text-red-400', text: 'Error' }
    }
  }

  const { icon: Icon, color, text } = getStatusInfo()

  return (
    <div className="flex items-center gap-2 text-xs">
      <Icon size={12} className={color} />
      <span className={color}>{text}</span>
    </div>
  )
}

// Main Message Component
const ChatMessage = ({ 
  message, 
  onToolResult 
}: { 
  message: UIMessage
  onToolResult?: ({ toolCallId, result }: { toolCallId: string; result: any }) => void 
}) => {
  const isUser = message.role === 'user'
  const Icon = isUser ? User : Bot

  return (
    <Animator>
      <Animated
        className={`flex gap-3 mb-4 ${isUser ? 'flex-row-reverse' : ''}`}
        animated={[['x', isUser ? 20 : -20, 0], ['opacity', 0, 1]]}
      >
        <div className={`w-8 h-8 rounded-full flex items-center justify-center ${
          isUser ? 'bg-cyan-600/20 text-cyan-300' : 'bg-purple-600/20 text-purple-300'
        }`}>
          <Icon size={16} />
        </div>
        
        <div className={`flex-1 max-w-2xl ${isUser ? 'text-right' : ''}`}>
          <div className={`inline-block p-4 rounded-lg ${
            isUser 
              ? 'bg-cyan-900/30 border border-cyan-500/30' 
              : 'bg-purple-900/30 border border-purple-500/30'
          }`}>
            {/* Legacy content fallback */}
            {!message.parts && message.content && (
              <div className="text-cyan-100">{message.content}</div>
            )}
            
            {/* New parts-based rendering */}
            {message.parts?.map((part, index) => (
              <MessagePartRenderer 
                key={index} 
                part={part} 
                onToolResult={onToolResult}
              />
            ))}
            
            {/* Attachments */}
            {message.experimental_attachments?.map((attachment, index) => (
              <div key={index} className="mt-2 p-2 bg-cyan-800/20 rounded">
                <div className="flex items-center gap-2">
                  <FileText size={14} className="text-cyan-400" />
                  <span className="text-cyan-300 text-sm">{attachment.name}</span>
                  <span className="text-cyan-500 text-xs">
                    {(attachment.size / 1024).toFixed(1)}KB
                  </span>
                </div>
              </div>
            ))}
          </div>
          
          <div className="mt-1 text-xs text-cyan-500 flex items-center gap-2">
            {message.createdAt?.toLocaleTimeString()}
            {message.data && <span>• has data</span>}
          </div>
        </div>
      </Animated>
    </Animator>
  )
}

// Chat Input Component
const ChatInput = ({ 
  input, 
  onInputChange, 
  onSubmit, 
  status 
}: {
  input: string
  onInputChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void
  onSubmit: () => void
  status: UseChatHelpers['status']
}) => {
  const isDisabled = status === 'streaming' || status === 'submitted'

  return (
    <div className="relative">
      <FrameCorners
        style={{
          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
        } as React.CSSProperties}
      />
      <div className="relative p-4">
        <div className="flex gap-2">
          <textarea
            value={input}
            onChange={onInputChange}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                onSubmit()
              }
            }}
            placeholder="Type your message..."
            disabled={isDisabled}
            className="flex-1 bg-transparent border border-cyan-500/30 rounded p-3 text-cyan-100 placeholder-cyan-500 resize-none focus:outline-none focus:border-cyan-400 disabled:opacity-50"
            rows={3}
          />
          <button
            onClick={onSubmit}
            disabled={!input.trim() || isDisabled}
            className="px-4 py-2 bg-cyan-600/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-600/30 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Send size={16} />
          </button>
        </div>
        
        <div className="mt-2 flex justify-between items-center">
          <ChatStatusIndicator status={status} />
          <div className="text-xs text-cyan-500">
            Press Enter to send, Shift+Enter for new line
          </div>
        </div>
      </div>
    </div>
  )
}

// Mock useChat hook data
const createMockChatHelpers = (): UseChatHelpers => {
  const [messages, setMessages] = useState<UIMessage[]>([
    {
      id: '1',
      role: 'user',
      content: 'What\'s the weather like in San Francisco?',
      createdAt: new Date(),
      parts: [
        { type: 'text', text: 'What\'s the weather like in San Francisco?' }
      ]
    },
    {
      id: '2', 
      role: 'assistant',
      content: '',
      createdAt: new Date(),
      parts: [
        { type: 'reasoning', reasoning: 'The user is asking about weather. I need to use the weather tool to get current conditions.' },
        { 
          type: 'tool-invocation', 
          toolInvocation: {
            state: 'result',
            toolCallId: 'call_1',
            toolName: 'getWeather',
            args: { city: 'San Francisco', unit: 'C' },
            result: 'The weather in San Francisco is sunny, 24°C'
          }
        },
        { type: 'text', text: 'The weather in San Francisco is currently sunny with a temperature of 24°C. It\'s a beautiful day!' }
      ]
    }
  ])
  
  const [input, setInput] = useState('')
  const [status, setStatus] = useState<UseChatHelpers['status']>('ready')

  return {
    messages,
    input,
    error: undefined,
    isLoading: status === 'streaming',
    status,
    id: 'chat-1',
    data: undefined,
    metadata: undefined,
    append: async (message) => {
      setMessages(prev => [...prev, message as UIMessage])
      return message.id
    },
    reload: async () => null,
    stop: () => setStatus('ready'),
    setMessages,
    setInput,
    handleInputChange: (e) => setInput(e.target.value),
    handleSubmit: () => {
      if (!input.trim()) return
      setStatus('submitted')
      // Simulate adding user message and AI response
      setTimeout(() => setStatus('ready'), 1000)
    },
    addToolResult: ({ toolCallId, result }) => {
      setMessages(prev => prev.map(msg => ({
        ...msg,
        parts: msg.parts?.map(part => 
          part.type === 'tool-invocation' && 
          part.toolInvocation.toolCallId === toolCallId
            ? {
                ...part,
                toolInvocation: {
                  ...part.toolInvocation,
                  state: 'result' as const,
                  result
                }
              }
            : part
        ) || []
      })))
    }
  }
}

export const MessageRendering: Story = {
  render: () => {
    const chat = createMockChatHelpers()
    
    return (
      <BleepsProvider bleeps={{}}>
        <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
          <Animator active={true}>
            <div className="min-h-screen bg-black p-8">
              <div className="max-w-4xl mx-auto">
                <div className="mb-8">
                  <Text as="h2" className="text-2xl text-cyan-300 mb-4">
                    Vercel AI SDK v4 Message Rendering
                  </Text>
                  <Text className="text-cyan-500">
                    Demonstrates the new parts-based message system with text, reasoning, tool invocations, and more
                  </Text>
                </div>
                
                <div className="space-y-4">
                  {chat.messages.map((message) => (
                    <ChatMessage 
                      key={message.id} 
                      message={message} 
                      onToolResult={chat.addToolResult}
                    />
                  ))}
                </div>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BleepsProvider>
    )
  },
}

export const ToolInvocationStates: Story = {
  render: () => {
    const [activeStates, setActiveStates] = useState<{ [key: string]: ToolInvocation['state'] }>({
      weather: 'partial-call',
      confirmation: 'call', 
      search: 'result'
    })

    const createToolInvocation = (tool: string): ToolInvocation => {
      const baseCall = {
        toolCallId: `call_${tool}`,
        toolName: tool === 'weather' ? 'getWeather' : 
                  tool === 'confirmation' ? 'askForConfirmation' : 'searchDatabase',
        args: tool === 'weather' ? { city: 'San Francisco', unit: 'C' } :
              tool === 'confirmation' ? { message: 'Do you want to proceed with deleting 5 files?' } :
              { query: 'user preferences', limit: 10 }
      }

      const state = activeStates[tool]
      
      if (state === 'result') {
        return {
          ...baseCall,
          state: 'result',
          result: tool === 'weather' ? 'The weather in San Francisco is sunny, 24°C' :
                  tool === 'confirmation' ? 'confirmed' :
                  { results: ['Setting A', 'Setting B'], totalCount: 2 }
        }
      }
      
      return {
        ...baseCall,
        state
      }
    }

    const toolInvocations = {
      weather: createToolInvocation('weather'),
      confirmation: createToolInvocation('confirmation'), 
      search: createToolInvocation('search')
    }

    const updateState = (tool: string, newState: ToolInvocation['state']) => {
      setActiveStates(prev => ({ ...prev, [tool]: newState }))
    }

    return (
      <BleepsProvider bleeps={{}}>
        <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
          <Animator active={true}>
            <div className="min-h-screen bg-black p-8">
              <div className="max-w-4xl mx-auto">
                <div className="mb-8">
                  <Text as="h2" className="text-2xl text-cyan-300 mb-4">
                    Tool Invocation States
                  </Text>
                  <Text className="text-cyan-500 mb-6">
                    Interactive demo of partial-call → call → result states
                  </Text>
                  
                  <div className="grid grid-cols-3 gap-4 mb-8">
                    {Object.entries(toolInvocations).map(([key, tool]) => (
                      <div key={key} className="space-y-2">
                        <Text className="text-cyan-300 font-semibold capitalize">{key}</Text>
                        <div className="space-y-1">
                          {(['partial-call', 'call', 'result'] as const).map(state => (
                            <button
                              key={state}
                              onClick={() => updateState(key, state)}
                              className={`block w-full px-3 py-1 text-sm rounded ${
                                tool.state === state
                                  ? 'bg-cyan-600/30 text-cyan-300 border border-cyan-500'
                                  : 'bg-gray-600/20 text-gray-400 border border-gray-600/50 hover:bg-gray-600/30'
                              }`}
                            >
                              {state}
                            </button>
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
                
                <div className="space-y-4">
                  {Object.values(toolInvocations).map((tool) => (
                    <ToolInvocationRenderer 
                      key={tool.toolCallId}
                      toolInvocation={tool}
                      onToolResult={(toolCallId, result) => {
                        console.log('Tool result:', toolCallId, result)
                      }}
                    />
                  ))}
                </div>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BleepsProvider>
    )
  },
}

export const FullChatInterface: Story = {
  render: () => {
    const chat = createMockChatHelpers()
    
    return (
      <BleepsProvider bleeps={{}}>
        <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
          <Animator active={true} manager="stagger" duration={{ stagger: 0.1 }}>
            <div className="h-screen flex flex-col bg-black">
              {/* Header */}
              <Animator>
                <Animated
                  as="header"
                  className="border-b border-cyan-500/30 p-4"
                  animated={[['y', -20, 0], ['opacity', 0, 1]]}
                >
                  <div className="flex items-center justify-between">
                    <Text as="h1" className="text-xl text-cyan-300 font-semibold">
                      AI Assistant
                    </Text>
                    <div className="flex items-center gap-4">
                      <ChatStatusIndicator status={chat.status} />
                      <button 
                        onClick={chat.stop}
                        className="p-2 text-cyan-400 hover:text-cyan-300"
                      >
                        <Pause size={16} />
                      </button>
                    </div>
                  </div>
                </Animated>
              </Animator>

              {/* Messages */}
              <Animator>
                <Animated
                  className="flex-1 overflow-y-auto p-4"
                  animated={[['opacity', 0, 1]]}
                >
                  <div className="max-w-4xl mx-auto space-y-4">
                    {chat.messages.map((message) => (
                      <ChatMessage 
                        key={message.id} 
                        message={message} 
                        onToolResult={chat.addToolResult}
                      />
                    ))}
                  </div>
                </Animated>
              </Animator>

              {/* Input */}
              <Animator>
                <Animated
                  className="border-t border-cyan-500/30 p-4"
                  animated={[['y', 20, 0], ['opacity', 0, 1]]}
                >
                  <div className="max-w-4xl mx-auto">
                    <ChatInput
                      input={chat.input}
                      onInputChange={chat.handleInputChange}
                      onSubmit={chat.handleSubmit}
                      status={chat.status}
                    />
                  </div>
                </Animated>
              </Animator>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </BleepsProvider>
    )
  },
}
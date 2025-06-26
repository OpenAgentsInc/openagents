import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  Text,
  FrameCorners
} from '@arwes/react'
import React, { useState } from 'react'
import { Code, FileText, Settings, Zap } from 'lucide-react'

const meta = {
  title: 'AI Chat/SDK Types & Utilities',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Comprehensive documentation of Vercel AI SDK v4 types, utilities, and integration patterns for building chat interfaces.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Helper to format TypeScript code
const TypeScriptCode = ({ children }: { children: string }) => (
  <pre className="bg-black/50 p-4 rounded border border-cyan-500/30 text-sm overflow-x-auto">
    <code className="text-cyan-200 font-mono whitespace-pre">{children}</code>
  </pre>
)

const CodeBlock = ({ title, children }: { title: string; children: string }) => (
  <div className="mb-6">
    <div className="flex items-center gap-2 mb-2">
      <Code size={16} className="text-cyan-400" />
      <Text className="text-cyan-300 font-semibold">{title}</Text>
    </div>
    <TypeScriptCode>{children}</TypeScriptCode>
  </div>
)

export const CoreTypes: Story = {
  render: () => {
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={true}>
          <div className="min-h-screen bg-black p-8">
            <div className="max-w-6xl mx-auto">
              <div className="mb-8">
                <Text as="h1" className="text-3xl text-cyan-300 mb-4">
                  Vercel AI SDK v4 Core Types
                </Text>
                <Text className="text-cyan-500">
                  Essential TypeScript interfaces and types for building AI chat applications
                </Text>
              </div>

              <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
                {/* Message Types */}
                <div>
                  <CodeBlock title="Message & UIMessage Types" children={`// Core message interface
interface Message {
  id: string
  createdAt?: Date
  content: string
  role: 'system' | 'user' | 'assistant' | 'data'
  experimental_attachments?: Attachment[]
  toolInvocations?: Array<ToolInvocation> // deprecated
  parts?: Array<MessagePart>
  annotations?: JSONValue[]
  data?: JSONValue
}

// UIMessage always has parts populated
type UIMessage = Message & {
  parts: Array<MessagePart>
}

// Message parts for rich content
type MessagePart = 
  | TextUIPart 
  | ReasoningUIPart 
  | ToolInvocationUIPart 
  | SourceUIPart 
  | FileUIPart 
  | StepStartUIPart`} />

                  <CodeBlock title="Tool Types" children={`// Tool invocation states
type ToolInvocation =
  | ({ state: 'partial-call' } & ToolCall)
  | ({ state: 'call' } & ToolCall)  
  | ({ state: 'result' } & ToolResult)

interface ToolCall {
  toolCallId: string
  toolName: string
  args: Record<string, any>
  step?: number
}

interface ToolResult extends ToolCall {
  result: any
}

// Tool definition
type Tool<PARAMS = any, RESULT = any> = {
  parameters: PARAMS // Zod schema
  description?: string
  execute?: (args: PARAMS) => Promise<RESULT>
}`} />
                </div>

                {/* Hook Types */}
                <div>
                  <CodeBlock title="useChat Hook Interface" children={`interface UseChatHelpers {
  // State
  messages: UIMessage[]
  input: string
  error: undefined | Error
  status: 'submitted' | 'streaming' | 'ready' | 'error'
  data?: JSONValue[]
  metadata?: Object
  id: string
  isLoading: boolean // deprecated, use status
  
  // Actions
  append: (message: Message) => Promise<string | null>
  reload: () => Promise<string | null>
  stop: () => void
  experimental_resume: () => void
  setMessages: (messages: UIMessage[]) => void
  setInput: (input: string) => void
  handleSubmit: (event?: FormEvent) => void
  addToolResult: ({ toolCallId, result }) => void
  
  // Form helpers
  handleInputChange: ChangeEventHandler
}`} />

                  <CodeBlock title="Hook Configuration" children={`interface UseChatOptions {
  api?: string // default: '/api/chat'
  id?: string // for state sharing
  initialMessages?: Message[]
  maxSteps?: number // multi-step tools
  streamProtocol?: 'data' | 'text'
  
  // Callbacks
  onToolCall?: ({ toolCall }) => Promise<unknown>
  onResponse?: (response: Response) => void
  onFinish?: (message, { usage, finishReason }) => void
  onError?: (error: Error) => void
  
  // HTTP
  headers?: Record<string, string>
  body?: object
  credentials?: RequestCredentials
}`} />
                </div>
              </div>
            </div>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const UtilityFunctions: Story = {
  render: () => {
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={true}>
          <div className="min-h-screen bg-black p-8">
            <div className="max-w-6xl mx-auto">
              <div className="mb-8">
                <Text as="h1" className="text-3xl text-cyan-300 mb-4">
                  Utility Functions & Helpers
                </Text>
                <Text className="text-cyan-500">
                  Essential helper functions from @ai-sdk/ui-utils for message processing
                </Text>
              </div>

              <div className="space-y-8">
                <CodeBlock title="Message Processing Utilities" children={`import { 
  fillMessageParts,
  getMessageParts,
  updateToolCallResult,
  shouldResubmitMessages 
} from '@ai-sdk/ui-utils'

// Convert legacy messages to parts-based format
const messagesWithParts = fillMessageParts(messages)

// Extract parts from a message
const parts = getMessageParts(message)

// Update tool call results
const updatedMessages = updateToolCallResult({
  messages,
  toolCallId: 'call_123',
  toolResult: 'Tool execution result'
})

// Check if multi-step should continue
const shouldContinue = shouldResubmitMessages(
  messages,
  experimental_onStepFinish
)`} />

                <CodeBlock title="Message Rendering Pattern" children={`// Complete message renderer
const MessageRenderer = ({ message, onToolResult }) => {
  return (
    <div className="message">
      {message.parts?.map((part, index) => {
        switch (part.type) {
          case 'text':
            return <div key={index}>{part.text}</div>
          
          case 'tool-invocation':
            return (
              <ToolRenderer 
                key={index}
                toolInvocation={part.toolInvocation}
                onResult={onToolResult}
              />
            )
          
          case 'reasoning':
            return (
              <div key={index} className="reasoning">
                {part.reasoning}
              </div>
            )
          
          case 'step-start':
            return <hr key={index} className="step-divider" />
          
          case 'file':
            return (
              <FileRenderer 
                key={index}
                mimeType={part.mimeType}
                data={part.data}
              />
            )
        }
      })}
    </div>
  )
}`} />

                <CodeBlock title="Tool Call Handling" children={`// Interactive tool execution
const handleToolCall = async ({ toolCall }) => {
  switch (toolCall.toolName) {
    case 'getWeather':
      // Automatic execution
      return \`Weather in \${toolCall.args.city}: Sunny, 24°C\`
    
    case 'askForConfirmation':
      // Let UI handle interactively
      return undefined // Don't auto-execute
    
    case 'searchDatabase':
      const results = await searchAPI(toolCall.args.query)
      return { results, totalCount: results.length }
  }
}

// Use in useChat
const { messages, addToolResult } = useChat({
  onToolCall: handleToolCall
})

// Manual result addition for interactive tools
const handleConfirm = (confirmed: boolean) => {
  addToolResult({
    toolCallId: 'call_123',
    result: confirmed ? 'confirmed' : 'cancelled'
  })
}`} />
              </div>
            </div>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const IntegrationPatterns: Story = {
  render: () => {
    const [selectedPattern, setSelectedPattern] = useState('basic')

    const patterns = {
      basic: {
        title: 'Basic Chat Implementation',
        description: 'Simple chat with automatic tool execution',
        code: `import { useChat } from 'ai/react'
import { z } from 'zod'

export default function ChatPage() {
  const { messages, input, handleInputChange, handleSubmit } = useChat({
    api: '/api/chat',
    onToolCall: async ({ toolCall }) => {
      if (toolCall.toolName === 'getWeather') {
        return \`Weather in \${toolCall.args.city}: Sunny\`
      }
    }
  })

  return (
    <div>
      {messages.map(message => (
        <MessageRenderer key={message.id} message={message} />
      ))}
      
      <form onSubmit={handleSubmit}>
        <input value={input} onChange={handleInputChange} />
        <button type="submit">Send</button>
      </form>
    </div>
  )
}`
      },
      interactive: {
        title: 'Interactive Tool Calls',
        description: 'Human-in-the-loop tool execution',
        code: `export default function InteractiveChatPage() {
  const { messages, addToolResult, ...rest } = useChat({
    api: '/api/chat'
    // Don't provide onToolCall - let UI handle
  })

  const handleConfirmation = (toolCallId: string, confirmed: boolean) => {
    addToolResult({
      toolCallId,
      result: confirmed ? 'User confirmed' : 'User declined'
    })
  }

  return (
    <div>
      {messages.map(message => (
        <div key={message.id}>
          {message.parts?.map((part, i) => {
            if (part.type === 'tool-invocation' && 
                part.toolInvocation.toolName === 'askForConfirmation' &&
                part.toolInvocation.state === 'call') {
              return (
                <div key={i}>
                  <p>{part.toolInvocation.args.message}</p>
                  <button onClick={() => 
                    handleConfirmation(part.toolInvocation.toolCallId, true)
                  }>
                    Confirm
                  </button>
                  <button onClick={() => 
                    handleConfirmation(part.toolInvocation.toolCallId, false)
                  }>
                    Decline
                  </button>
                </div>
              )
            }
            return <MessagePartRenderer key={i} part={part} />
          })}
        </div>
      ))}
    </div>
  )
}`
      },
      streaming: {
        title: 'Advanced Streaming',
        description: 'Custom streaming with status handling',
        code: `export default function StreamingChatPage() {
  const { 
    messages, 
    status, 
    error, 
    stop,
    ...rest 
  } = useChat({
    api: '/api/chat',
    streamProtocol: 'data', // Structured streaming
    experimental_throttle: 50, // Throttle updates
    onResponse: (response) => {
      console.log('Response received:', response.status)
    },
    onFinish: (message, { usage, finishReason }) => {
      console.log('Finished:', { usage, finishReason })
    },
    onError: (error) => {
      console.error('Chat error:', error)
    }
  })

  return (
    <div>
      <div className="chat-header">
        <StatusIndicator status={status} />
        {status === 'streaming' && (
          <button onClick={stop}>Stop Generation</button>
        )}
      </div>
      
      {error && (
        <div className="error">
          Error: {error.message}
        </div>
      )}
      
      <MessageList messages={messages} />
      <ChatInput {...rest} disabled={status !== 'ready'} />
    </div>
  )
}`
      },
      multistep: {
        title: 'Multi-step Tool Calls',
        description: 'Complex workflows with multiple tool steps',
        code: `export default function MultiStepChatPage() {
  const { messages, ...rest } = useChat({
    api: '/api/chat',
    maxSteps: 5, // Allow up to 5 tool steps
    onToolCall: async ({ toolCall }) => {
      // Handle different tool types
      switch (toolCall.toolName) {
        case 'analyzeData':
          return await analyzeDataAPI(toolCall.args)
        case 'generateReport':
          return await generateReportAPI(toolCall.args)
        case 'saveResults':
          return await saveResultsAPI(toolCall.args)
        default:
          // Let server handle unknown tools
          return undefined
      }
    }
  })

  return (
    <div>
      {messages.map(message => (
        <div key={message.id}>
          {message.parts?.map((part, i) => {
            // Show step indicators
            if (part.type === 'step-start') {
              return <StepDivider key={i} />
            }
            
            // Show tool progress
            if (part.type === 'tool-invocation') {
              return (
                <ToolProgressRenderer 
                  key={i}
                  toolInvocation={part.toolInvocation}
                  step={part.toolInvocation.step}
                />
              )
            }
            
            return <MessagePartRenderer key={i} part={part} />
          })}
        </div>
      ))}
    </div>
  )
}`
      }
    }

    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={true}>
          <div className="min-h-screen bg-black p-8">
            <div className="max-w-6xl mx-auto">
              <div className="mb-8">
                <Text as="h1" className="text-3xl text-cyan-300 mb-4">
                  Integration Patterns
                </Text>
                <Text className="text-cyan-500">
                  Common patterns for integrating Vercel AI SDK v4 in real applications
                </Text>
              </div>

              <div className="grid grid-cols-1 lg:grid-cols-4 gap-8">
                {/* Pattern Selection */}
                <div className="space-y-2">
                  {Object.entries(patterns).map(([key, pattern]) => (
                    <button
                      key={key}
                      onClick={() => setSelectedPattern(key)}
                      className={`w-full p-3 text-left rounded border transition-colors ${
                        selectedPattern === key
                          ? 'bg-cyan-600/20 border-cyan-500 text-cyan-300'
                          : 'bg-gray-600/10 border-gray-600/30 text-gray-300 hover:bg-gray-600/20'
                      }`}
                    >
                      <div className="font-semibold text-sm">{pattern.title}</div>
                      <div className="text-xs mt-1 opacity-70">{pattern.description}</div>
                    </button>
                  ))}
                </div>

                {/* Pattern Code */}
                <div className="lg:col-span-3">
                  <FrameCorners
                    style={{
                      '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.1)',
                      '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.3)',
                    } as React.CSSProperties}
                  />
                  <div className="relative p-6">
                    <div className="flex items-center gap-2 mb-4">
                      <Settings size={16} className="text-cyan-400" />
                      <Text className="text-cyan-300 font-semibold">
                        {patterns[selectedPattern as keyof typeof patterns].title}
                      </Text>
                    </div>
                    <Text className="text-cyan-500 mb-4">
                      {patterns[selectedPattern as keyof typeof patterns].description}
                    </Text>
                    <TypeScriptCode>
                      {patterns[selectedPattern as keyof typeof patterns].code}
                    </TypeScriptCode>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}

export const APIRouteExamples: Story = {
  render: () => {
    return (
      <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.3 }}>
        <Animator active={true}>
          <div className="min-h-screen bg-black p-8">
            <div className="max-w-6xl mx-auto">
              <div className="mb-8">
                <Text as="h1" className="text-3xl text-cyan-300 mb-4">
                  API Route Examples
                </Text>
                <Text className="text-cyan-500">
                  Server-side implementation patterns for chat API endpoints
                </Text>
              </div>

              <div className="space-y-8">
                <CodeBlock title="Basic Chat API Route" children={`// app/api/chat/route.ts
import { openai } from '@ai-sdk/openai'
import { streamText } from 'ai'
import { z } from 'zod'

export async function POST(req: Request) {
  const { messages } = await req.json()

  const result = streamText({
    model: openai('gpt-4o'),
    messages,
    tools: {
      getWeather: {
        description: 'Get weather for a city',
        parameters: z.object({
          city: z.string(),
          unit: z.enum(['C', 'F']).default('C')
        }),
        execute: async ({ city, unit }) => {
          // Server-side tool execution
          const weather = await fetchWeatherAPI(city)
          return \`Weather in \${city}: \${weather.temp}°\${unit}\`
        }
      }
    },
    maxSteps: 3 // Enable multi-step tool calling
  })

  return result.toDataStreamResponse()
}`} />

                <CodeBlock title="Tool Calling with External APIs" children={`// Advanced tool configuration
const tools = {
  searchDatabase: {
    description: 'Search the user database',
    parameters: z.object({
      query: z.string(),
      limit: z.number().default(10),
      filters: z.record(z.string()).optional()
    }),
    execute: async ({ query, limit, filters }) => {
      const results = await database.search({
        query,
        limit,
        where: filters
      })
      return {
        results: results.data,
        totalCount: results.count,
        hasMore: results.hasNextPage
      }
    }
  },
  
  sendEmail: {
    description: 'Send an email to a user',
    parameters: z.object({
      to: z.string().email(),
      subject: z.string(),
      body: z.string()
    }),
    execute: async ({ to, subject, body }) => {
      await emailService.send({ to, subject, body })
      return \`Email sent to \${to}\`
    }
  }
}`} />

                <CodeBlock title="Error Handling & Middleware" children={`// app/api/chat/route.ts with error handling
export async function POST(req: Request) {
  try {
    const { messages, userId } = await req.json()
    
    // Authentication check
    const user = await authenticateUser(req)
    if (!user) {
      return new Response('Unauthorized', { status: 401 })
    }
    
    // Rate limiting
    await checkRateLimit(user.id)
    
    const result = streamText({
      model: openai('gpt-4o'),
      messages,
      tools: {
        // Tools that require authentication
        getUserData: {
          description: 'Get user-specific data',
          parameters: z.object({ dataType: z.string() }),
          execute: async ({ dataType }) => {
            return await getUserData(user.id, dataType)
          }
        }
      },
      onFinish: async ({ usage }) => {
        // Log usage for billing
        await logUsage(user.id, usage)
      }
    })

    return result.toDataStreamResponse()
    
  } catch (error) {
    console.error('Chat API error:', error)
    return new Response('Internal Server Error', { status: 500 })
  }
}`} />

                <CodeBlock title="Streaming with Custom Headers" children={`// Custom streaming response
export async function POST(req: Request) {
  const { messages } = await req.json()

  const result = streamText({
    model: openai('gpt-4o'),
    messages,
    experimental_telemetry: {
      isEnabled: true,
      functionId: 'chat-completion'
    }
  })

  // Add custom headers
  const response = result.toDataStreamResponse()
  response.headers.set('X-Custom-Header', 'value')
  response.headers.set('Cache-Control', 'no-cache')
  
  return response
}`} />
              </div>
            </div>
          </div>
        </Animator>
      </AnimatorGeneralProvider>
    )
  },
}
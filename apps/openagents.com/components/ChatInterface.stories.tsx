import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  AnimatedX,
  Text,
  FrameCorners,
  FrameLines,
  FrameUnderline,
  FrameBase,
  GridLines,
  Dots,
  BleepsProvider,
  createBleep
} from '@arwes/react'
import React, { useState, useEffect, useRef } from 'react'
import { 
  Send, User, Bot, Code2, FileText, Terminal, 
  Copy, ThumbsUp, ThumbsDown, RotateCw, MoreVertical,
  Loader2, CheckCircle2, AlertCircle, FunctionSquare
} from 'lucide-react'

const bleepsSettings = {
  master: { volume: 0.3 },
  categories: {
    interaction: { volume: 0.5 },
    notification: { volume: 0.7 }
  },
  bleeps: {
    click: {
      category: 'interaction',
      sources: [{ src: '/sounds/click.webm', type: 'audio/webm' }]
    },
    type: {
      category: 'interaction', 
      sources: [{ src: '/sounds/type.webm', type: 'audio/webm' }]
    },
    notify: {
      category: 'notification',
      sources: [{ src: '/sounds/notify.webm', type: 'audio/webm' }]
    }
  }
}

const bleeps = createBleep(bleepsSettings)

const meta = {
  title: 'Examples/Chat Interface',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'AI chat interface components demonstrating user/agent messages, tool use, code blocks, and real-time interactions.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Message types
interface Message {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: Date
  toolCalls?: ToolCall[]
  codeBlocks?: CodeBlock[]
  status?: 'sending' | 'sent' | 'error'
}

interface ToolCall {
  id: string
  name: string
  parameters: any
  result?: any
  status: 'pending' | 'running' | 'completed' | 'error'
}

interface CodeBlock {
  language: string
  code: string
  filename?: string
}

// Message component
const ChatMessage = ({ message, isLatest }: { message: Message, isLatest: boolean }) => {
  const isUser = message.role === 'user'
  const Icon = isUser ? User : Bot
  
  return (
    <Animator>
      <Animated
        animated={[['x', isUser ? 20 : -20, 0], ['opacity', 0, 1]]}
        className={`flex gap-3 ${isUser ? 'flex-row-reverse' : ''}`}
      >
        <div className={`w-8 h-8 rounded-full flex items-center justify-center ${
          isUser ? 'bg-cyan-500/20 text-cyan-400' : 'bg-purple-500/20 text-purple-400'
        }`}>
          <Icon size={16} />
        </div>
        
        <div className={`flex-1 max-w-2xl ${isUser ? 'text-right' : ''}`}>
          <div className={`inline-block text-left relative ${isUser ? 'ml-auto' : ''}`}>
            <FrameCorners
              style={{
                // @ts-expect-error css variables
                '--arwes-frames-bg-color': isUser ? 'hsla(180, 75%, 10%, 0.3)' : 'hsla(270, 75%, 10%, 0.3)',
                '--arwes-frames-line-color': isUser ? 'hsla(180, 75%, 50%, 0.6)' : 'hsla(270, 75%, 50%, 0.6)',
              }}
            />
            <div className="relative p-4">
              <Text className={`${isUser ? 'text-cyan-300' : 'text-purple-300'}`}>
                {message.content}
              </Text>
              
              {/* Tool calls */}
              {message.toolCalls && message.toolCalls.length > 0 && (
                <div className="mt-3 space-y-2">
                  {message.toolCalls.map((tool) => (
                    <ToolCallDisplay key={tool.id} toolCall={tool} />
                  ))}
                </div>
              )}
              
              {/* Code blocks */}
              {message.codeBlocks && message.codeBlocks.length > 0 && (
                <div className="mt-3 space-y-2">
                  {message.codeBlocks.map((block, i) => (
                    <CodeBlockDisplay key={i} codeBlock={block} />
                  ))}
                </div>
              )}
            </div>
          </div>
          
          <div className={`mt-1 text-xs ${isUser ? 'text-cyan-500' : 'text-purple-500'} flex items-center gap-2 ${isUser ? 'justify-end' : ''}`}>
            <Text>{message.timestamp.toLocaleTimeString()}</Text>
            {message.status === 'sending' && <Loader2 size={12} className="animate-spin" />}
            {message.status === 'error' && <AlertCircle size={12} />}
          </div>
        </div>
      </Animated>
    </Animator>
  )
}

// Tool call display
const ToolCallDisplay = ({ toolCall }: { toolCall: ToolCall }) => {
  const [expanded, setExpanded] = useState(false)
  
  return (
    <div className="mt-2">
      <div 
        className="flex items-center gap-2 cursor-pointer bg-purple-500/10 px-3 py-2 border border-purple-500/30"
        onClick={() => setExpanded(!expanded)}
      >
        <FunctionSquare size={14} className="text-purple-400" />
        <Text className="text-purple-300 text-sm font-mono">{toolCall.name}</Text>
        <div className="ml-auto flex items-center gap-1">
          {toolCall.status === 'running' && <Loader2 size={12} className="animate-spin text-purple-400" />}
          {toolCall.status === 'completed' && <CheckCircle2 size={12} className="text-green-400" />}
          {toolCall.status === 'error' && <AlertCircle size={12} className="text-red-400" />}
        </div>
      </div>
      
      {expanded && (
        <Animator>
          <Animated animated={[['y', -10, 0], ['opacity', 0, 1]]}>
            <div className="mt-1 bg-purple-500/5 border border-purple-500/20 p-3">
              <Text className="text-purple-400 text-xs mb-2">Parameters:</Text>
              <pre className="text-purple-300 text-xs font-mono overflow-x-auto">
                {JSON.stringify(toolCall.parameters, null, 2)}
              </pre>
              
              {toolCall.result && (
                <>
                  <Text className="text-purple-400 text-xs mt-3 mb-2">Result:</Text>
                  <pre className="text-purple-300 text-xs font-mono overflow-x-auto">
                    {JSON.stringify(toolCall.result, null, 2)}
                  </pre>
                </>
              )}
            </div>
          </Animated>
        </Animator>
      )}
    </div>
  )
}

// Code block display
const CodeBlockDisplay = ({ codeBlock }: { codeBlock: CodeBlock }) => {
  const [copied, setCopied] = useState(false)
  
  const handleCopy = () => {
    navigator.clipboard.writeText(codeBlock.code)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }
  
  return (
    <div className="relative mt-2">
      <div className="flex items-center justify-between bg-black/50 px-3 py-1 border-b border-purple-500/30">
        <div className="flex items-center gap-2">
          <Code2 size={14} className="text-purple-400" />
          <Text className="text-purple-300 text-sm">
            {codeBlock.filename || codeBlock.language}
          </Text>
        </div>
        <button 
          onClick={handleCopy}
          className="text-purple-400 hover:text-purple-300"
        >
          {copied ? <CheckCircle2 size={14} /> : <Copy size={14} />}
        </button>
      </div>
      <pre className="bg-black/30 p-3 overflow-x-auto">
        <code className="text-purple-300 text-sm font-mono">
          {codeBlock.code}
        </code>
      </pre>
    </div>
  )
}

// Typing indicator
const TypingIndicator = () => {
  return (
    <Animator>
      <Animated animated={[['opacity', 0, 1]]}>
        <div className="flex gap-3">
          <div className="w-8 h-8 rounded-full flex items-center justify-center bg-purple-500/20 text-purple-400">
            <Bot size={16} />
          </div>
          <div className="bg-purple-500/10 px-4 py-3 rounded-lg border border-purple-500/30">
            <div className="flex gap-1">
              <div className="w-2 h-2 bg-purple-400 rounded-full animate-pulse" />
              <div className="w-2 h-2 bg-purple-400 rounded-full animate-pulse" style={{ animationDelay: '0.2s' }} />
              <div className="w-2 h-2 bg-purple-400 rounded-full animate-pulse" style={{ animationDelay: '0.4s' }} />
            </div>
          </div>
        </div>
      </Animated>
    </Animator>
  )
}

export const BasicChat: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [messages, setMessages] = useState<Message[]>([
      {
        id: '1',
        role: 'user',
        content: 'How can I implement a real-time chat interface with Effect?',
        timestamp: new Date('2024-01-20T10:00:00'),
        status: 'sent'
      },
      {
        id: '2',
        role: 'assistant',
        content: 'I can help you implement a real-time chat interface with Effect! Here\'s a comprehensive approach:',
        timestamp: new Date('2024-01-20T10:00:05'),
        status: 'sent',
        codeBlocks: [{
          language: 'typescript',
          filename: 'ChatService.ts',
          code: `import { Effect, Layer, Stream } from '@effect/io'

export class ChatService extends Effect.Service<ChatService>()('ChatService', {
  // Service implementation
  sendMessage: (message: string) => Effect.Effect<Message>,
  receiveMessages: () => Stream.Stream<Message>
})`
        }]
      },
      {
        id: '3',
        role: 'user',
        content: 'Can you show me how to handle WebSocket connections?',
        timestamp: new Date('2024-01-20T10:01:00'),
        status: 'sent'
      },
      {
        id: '4',
        role: 'assistant',
        content: 'Let me search for WebSocket implementation patterns in Effect...',
        timestamp: new Date('2024-01-20T10:01:05'),
        status: 'sent',
        toolCalls: [{
          id: 'tool-1',
          name: 'searchDocumentation',
          parameters: { query: 'Effect WebSocket implementation' },
          status: 'completed',
          result: { found: 3, relevantDocs: ['websocket-guide.md', 'streaming-patterns.md'] }
        }]
      }
    ])
    const [isTyping, setIsTyping] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <BleepsProvider bleeps={bleeps}>
        <div className="min-h-screen bg-black p-4">
          <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
            <Animator active={active}>
              {/* Background effects */}
              <div className="fixed inset-0">
                <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
                <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
              </div>
              
              <div className="relative z-10 max-w-4xl mx-auto">
                {/* Header */}
                <Animator>
                  <header className="mb-6">
                    <Text as="h1" className="text-3xl text-cyan-300">
                      AI Chat Interface
                    </Text>
                    <Text className="text-cyan-500">
                      Real-time conversation with code execution and tool use
                    </Text>
                  </header>
                </Animator>
                
                {/* Chat container */}
                <div className="relative">
                  <FrameLines
                    style={{
                      // @ts-expect-error css variables
                      '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.2)',
                      '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.4)',
                    }}
                  />
                  
                  <div className="relative h-[600px] overflow-y-auto p-6 space-y-4">
                    <Animator manager="stagger" duration={{ stagger: 0.1 }}>
                      {messages.map((message, i) => (
                        <ChatMessage 
                          key={message.id} 
                          message={message} 
                          isLatest={i === messages.length - 1}
                        />
                      ))}
                      {isTyping && <TypingIndicator />}
                    </Animator>
                  </div>
                  
                  {/* Input area */}
                  <div className="relative border-t border-cyan-500/30 p-4">
                    <div className="flex gap-3">
                      <input
                        type="text"
                        placeholder="Type your message..."
                        className="flex-1 bg-cyan-500/10 border border-cyan-500/30 px-4 py-3 text-cyan-300 placeholder-cyan-600"
                        onFocus={() => bleeps.click?.play()}
                      />
                      <button className="px-6 py-3 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30 flex items-center gap-2">
                        <Send size={16} />
                        <Text>Send</Text>
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </Animator>
          </AnimatorGeneralProvider>
        </div>
      </BleepsProvider>
    )
  },
}

export const StreamingChat: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [streamingMessage, setStreamingMessage] = useState('')
    const streamingContent = 'I\'m analyzing your request and generating a comprehensive response. This involves searching through documentation, analyzing code patterns, and formulating the best approach for your specific use case...'
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    useEffect(() => {
      if (active) {
        let index = 0
        const interval = setInterval(() => {
          if (index < streamingContent.length) {
            setStreamingMessage(prev => prev + streamingContent[index])
            index++
          } else {
            clearInterval(interval)
          }
        }, 30)
        return () => clearInterval(interval)
      }
    }, [active])
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h2" className="text-2xl text-cyan-300 mb-6">
                Streaming Response Demo
              </Text>
              
              <div className="relative">
                <FrameBase
                  style={{
                    // @ts-expect-error css variables
                    '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                    '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.8)',
                  }}
                />
                <div className="relative p-6">
                  <div className="flex gap-3">
                    <div className="w-8 h-8 rounded-full flex items-center justify-center bg-purple-500/20 text-purple-400">
                      <Bot size={16} />
                    </div>
                    <div className="flex-1">
                      <Text className="text-purple-300">
                        {streamingMessage}
                        {streamingMessage.length < streamingContent.length && (
                          <span className="inline-block w-2 h-4 bg-purple-400 ml-1 animate-pulse" />
                        )}
                      </Text>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const MultimodalChat: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const messages: Message[] = [
      {
        id: '1',
        role: 'user',
        content: 'Analyze this system architecture diagram and suggest improvements.',
        timestamp: new Date(),
        status: 'sent'
      },
      {
        id: '2',
        role: 'assistant',
        content: 'I\'ll analyze the architecture diagram. Let me examine the components and their relationships.',
        timestamp: new Date(),
        status: 'sent',
        toolCalls: [
          {
            id: 'tool-1',
            name: 'imageAnalysis',
            parameters: { 
              imageUrl: '/uploads/architecture.png',
              analysisType: 'technical_diagram'
            },
            status: 'completed',
            result: {
              components: ['API Gateway', 'Load Balancer', 'Microservices', 'Database Cluster'],
              issues: ['Single point of failure at gateway', 'No caching layer'],
              score: 7.5
            }
          },
          {
            id: 'tool-2',
            name: 'generateSuggestions',
            parameters: {
              context: 'system_architecture',
              currentScore: 7.5
            },
            status: 'completed',
            result: {
              suggestions: [
                'Add Redis caching layer',
                'Implement API Gateway redundancy',
                'Add monitoring service'
              ]
            }
          }
        ]
      },
      {
        id: '3',
        role: 'assistant',
        content: 'Based on my analysis, here are my recommendations for improving your architecture:',
        timestamp: new Date(),
        status: 'sent',
        codeBlocks: [{
          language: 'yaml',
          filename: 'docker-compose.yml',
          code: `version: '3.8'
services:
  redis:
    image: redis:alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data

  monitoring:
    image: prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml`
        }]
      }
    ]
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h2" className="text-2xl text-cyan-300 mb-6">
                Multimodal Chat with Tool Use
              </Text>
              
              <div className="space-y-4">
                <Animator manager="stagger" duration={{ stagger: 0.2 }}>
                  {messages.map((message) => (
                    <ChatMessage key={message.id} message={message} isLatest={false} />
                  ))}
                </Animator>
              </div>
              
              {/* File upload area */}
              <div className="mt-8">
                <Animator>
                  <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
                    <div className="relative">
                      <FrameUnderline
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.2)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
                        }}
                      />
                      <div className="relative p-6 text-center">
                        <FileText size={32} className="text-cyan-400 mx-auto mb-2" />
                        <Text className="text-cyan-300">
                          Drag and drop files or click to upload
                        </Text>
                        <Text className="text-cyan-500 text-sm">
                          Supports images, PDFs, and code files
                        </Text>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const ChatWithActions: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [selectedMessage, setSelectedMessage] = useState<string | null>(null)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const MessageActions = ({ messageId }: { messageId: string }) => (
      <div className="flex items-center gap-2 mt-2">
        <button className="text-cyan-500 hover:text-cyan-300 p-1">
          <Copy size={14} />
        </button>
        <button className="text-cyan-500 hover:text-cyan-300 p-1">
          <ThumbsUp size={14} />
        </button>
        <button className="text-cyan-500 hover:text-cyan-300 p-1">
          <ThumbsDown size={14} />
        </button>
        <button className="text-cyan-500 hover:text-cyan-300 p-1">
          <RotateCw size={14} />
        </button>
        <button className="text-cyan-500 hover:text-cyan-300 p-1">
          <MoreVertical size={14} />
        </button>
      </div>
    )
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h2" className="text-2xl text-cyan-300 mb-6">
                Interactive Chat Actions
              </Text>
              
              <div className="space-y-4">
                {/* Example message with actions */}
                <div 
                  className="group"
                  onMouseEnter={() => setSelectedMessage('1')}
                  onMouseLeave={() => setSelectedMessage(null)}
                >
                  <div className="flex gap-3">
                    <div className="w-8 h-8 rounded-full flex items-center justify-center bg-purple-500/20 text-purple-400">
                      <Bot size={16} />
                    </div>
                    <div className="flex-1">
                      <div className="relative inline-block">
                        <FrameCorners
                          style={{
                            // @ts-expect-error css variables
                            '--arwes-frames-bg-color': 'hsla(270, 75%, 10%, 0.3)',
                            '--arwes-frames-line-color': `hsla(270, 75%, 50%, ${selectedMessage === '1' ? 0.8 : 0.6})`,
                          }}
                        />
                        <div className="relative p-4">
                          <Text className="text-purple-300">
                            Here's a comprehensive solution using Effect's streaming capabilities with WebSocket integration.
                          </Text>
                        </div>
                      </div>
                      
                      {selectedMessage === '1' && (
                        <Animator>
                          <Animated animated={[['opacity', 0, 1]]}>
                            <MessageActions messageId="1" />
                          </Animated>
                        </Animator>
                      )}
                    </div>
                  </div>
                </div>
                
                {/* System actions panel */}
                <Animator>
                  <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                    <div className="mt-8 p-4 bg-cyan-500/5 border border-cyan-500/20">
                      <Text className="text-cyan-400 text-sm mb-3">Quick Actions</Text>
                      <div className="grid grid-cols-3 gap-2">
                        <button className="px-3 py-2 bg-cyan-500/10 text-cyan-300 border border-cyan-500/30 hover:bg-cyan-500/20 text-sm">
                          Clear Chat
                        </button>
                        <button className="px-3 py-2 bg-cyan-500/10 text-cyan-300 border border-cyan-500/30 hover:bg-cyan-500/20 text-sm">
                          Export
                        </button>
                        <button className="px-3 py-2 bg-cyan-500/10 text-cyan-300 border border-cyan-500/30 hover:bg-cyan-500/20 text-sm">
                          New Thread
                        </button>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const ErrorStates: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h2" className="text-2xl text-cyan-300 mb-6">
                Error States & Retry
              </Text>
              
              <div className="space-y-4">
                {/* Network error */}
                <Animator>
                  <Animated animated={[['scale', 0.95, 1], ['opacity', 0, 1]]}>
                    <div className="relative">
                      <FrameBase
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(0, 75%, 10%, 0.3)',
                          '--arwes-frames-line-color': 'hsla(0, 75%, 50%, 0.8)',
                        }}
                      />
                      <div className="relative p-6">
                        <div className="flex items-start gap-3">
                          <AlertCircle size={20} className="text-red-400 mt-1" />
                          <div className="flex-1">
                            <Text className="text-red-300 font-bold mb-1">
                              Connection Error
                            </Text>
                            <Text className="text-red-400 text-sm mb-3">
                              Failed to connect to the AI service. Please check your internet connection.
                            </Text>
                            <button className="px-4 py-2 bg-red-500/20 text-red-300 border border-red-500/50 hover:bg-red-500/30 text-sm">
                              Retry Connection
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>
                  </Animated>
                </Animator>
                
                {/* Rate limit error */}
                <Animator duration={{ delay: 0.2 }}>
                  <Animated animated={[['scale', 0.95, 1], ['opacity', 0, 1]]}>
                    <div className="relative">
                      <FrameBase
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(40, 75%, 10%, 0.3)',
                          '--arwes-frames-line-color': 'hsla(40, 75%, 50%, 0.8)',
                        }}
                      />
                      <div className="relative p-6">
                        <div className="flex items-start gap-3">
                          <Terminal size={20} className="text-yellow-400 mt-1" />
                          <div className="flex-1">
                            <Text className="text-yellow-300 font-bold mb-1">
                              Rate Limit Exceeded
                            </Text>
                            <Text className="text-yellow-400 text-sm mb-3">
                              You've reached the maximum number of requests. Please wait 60 seconds.
                            </Text>
                            <div className="flex items-center gap-3">
                              <div className="w-32 h-2 bg-yellow-500/20 rounded-full overflow-hidden">
                                <div className="h-full bg-yellow-500 animate-pulse" style={{ width: '45%' }} />
                              </div>
                              <Text className="text-yellow-500 text-sm">27s remaining</Text>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </Animated>
                </Animator>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}
import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect, useRef } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { ChatMessage } from '../molecules/ChatMessage.stories'
import { StreamingMessage } from '../molecules/StreamingMessage.stories'
import { StatusBadge } from '../atoms/StatusBadge.stories'
import { ModelBadge } from '../atoms/ModelBadge.stories'

// Icon components
const SendIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <line x1="22" y1="2" x2="11" y2="13" />
    <polygon points="22 2 15 22 11 13 2 9 22 2" />
  </svg>
)

const AttachIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48" />
  </svg>
)

const MicIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
    <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
    <line x1="12" y1="19" x2="12" y2="23" />
    <line x1="8" y1="23" x2="16" y2="23" />
  </svg>
)

const SettingsIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="3" />
    <path d="M12 1v6M12 17v6M4.22 4.22l4.24 4.24M15.54 15.54l4.24 4.24M1 12h6M17 12h6M4.22 19.78l4.24-4.24M15.54 8.46l4.24-4.24" />
  </svg>
)

// Message interface
interface Message {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: Date
  isStreaming?: boolean
  model?: string
  provider?: string
  error?: string
}

// ChatInterface component
export interface ChatInterfaceProps {
  messages?: Message[]
  isLoading?: boolean
  isStreaming?: boolean
  currentModel?: string
  currentProvider?: string
  placeholder?: string
  maxHeight?: number
  showHeader?: boolean
  showStatus?: boolean
  showModelBadge?: boolean
  animated?: boolean
  className?: string
  onSendMessage?: (message: string) => void
  onClearChat?: () => void
  onModelChange?: (model: string, provider: string) => void
}

export const ChatInterface = ({
  messages = [],
  isLoading = false,
  isStreaming = false,
  currentModel = 'claude-3-sonnet',
  currentProvider = 'anthropic',
  placeholder = 'Ask me to build something amazing...',
  maxHeight = 600,
  showHeader = true,
  showStatus = true,
  showModelBadge = true,
  animated = true,
  className = '',
  onSendMessage,
  onClearChat,
  onModelChange
}: ChatInterfaceProps) => {
  const [active, setActive] = useState(false)
  const [inputValue, setInputValue] = useState('')
  const [isComposing, setIsComposing] = useState(false)
  const messagesEndRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const handleSend = () => {
    if (inputValue.trim() && !isLoading && !isStreaming) {
      onSendMessage?.(inputValue.trim())
      setInputValue('')
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey && !isComposing) {
      e.preventDefault()
      handleSend()
    }
  }

  const chatContent = (
    <div
      className={cx(
        'flex flex-col bg-black border border-cyan-500/30 rounded-lg overflow-hidden',
        'shadow-lg shadow-cyan-500/20',
        className
      )}
      style={{ height: `${maxHeight}px` }}
    >
      {/* Header */}
      {showHeader && (
        <div className="flex items-center justify-between px-4 py-3 border-b border-cyan-500/20 bg-black/50">
          <div className="flex items-center gap-3">
            <Text as="h3" className="text-cyan-300 font-medium">
              OpenAgents Chat
            </Text>
            {showModelBadge && (
              <ModelBadge
                model={currentModel}
                provider={currentProvider}
                variant="outline"
                size="small"
                animated={false}
              />
            )}
          </div>
          
          <div className="flex items-center gap-2">
            {showStatus && (
              <StatusBadge
                status={isStreaming ? 'generating' : isLoading ? 'deploying' : 'idle'}
                size="small"
                animated={false}
              />
            )}
            
            <button
              onClick={onClearChat}
              className="p-1 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer"
              title="Clear chat"
            >
              <SettingsIcon className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center text-gray-500">
              <Text className="text-lg mb-2">Welcome to OpenAgents</Text>
              <Text className="text-sm">
                Tell me what you want to build and I'll deploy it to Cloudflare Workers
              </Text>
            </div>
          </div>
        ) : (
          messages.map((message) => (
            message.isStreaming ? (
              <StreamingMessage
                key={message.id}
                role={message.role}
                content={message.content}
                isStreaming={true}
                timestamp={message.timestamp}
                model={message.model}
                provider={message.provider}
                animated={false}
              />
            ) : (
              <ChatMessage
                key={message.id}
                role={message.role}
                content={message.content}
                timestamp={message.timestamp}
                model={message.model}
                provider={message.provider}
                status={message.error ? 'error' : 'complete'}
                error={message.error}
                animated={false}
              />
            )
          ))
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="border-t border-cyan-500/20 bg-black/30 p-4">
        <div className="relative">
          <textarea
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={handleKeyDown}
            onCompositionStart={() => setIsComposing(true)}
            onCompositionEnd={() => setIsComposing(false)}
            placeholder={placeholder}
            disabled={isLoading || isStreaming}
            className={cx(
              'w-full bg-black/50 border border-gray-600 rounded-lg px-4 py-3 pr-24',
              'text-gray-200 placeholder-gray-500 resize-none',
              'focus:border-cyan-500 focus:outline-none focus:ring-1 focus:ring-cyan-500',
              (isLoading || isStreaming) && 'opacity-50 cursor-not-allowed'
            )}
            rows={3}
            style={{ minHeight: '60px', maxHeight: '120px' }}
          />
          
          <div className="absolute right-2 bottom-2 flex items-center gap-1">
            <button
              className="p-2 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer"
              title="Attach file"
            >
              <AttachIcon className="w-4 h-4" />
            </button>
            
            <button
              className="p-2 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer"
              title="Voice input"
            >
              <MicIcon className="w-4 h-4" />
            </button>
            
            <button
              onClick={handleSend}
              disabled={!inputValue.trim() || isLoading || isStreaming}
              className={cx(
                'p-2 rounded transition-all duration-200 cursor-pointer',
                inputValue.trim() && !isLoading && !isStreaming
                  ? 'bg-cyan-500 text-black hover:bg-cyan-400'
                  : 'bg-gray-600 text-gray-400 cursor-not-allowed'
              )}
              title="Send message"
            >
              <SendIcon className="w-4 h-4" />
            </button>
          </div>
        </div>
        
        <div className="flex items-center justify-between mt-2 text-xs text-gray-500">
          <span>
            Press Enter to send, Shift+Enter for new line
          </span>
          <span>
            {inputValue.length}/4000
          </span>
        </div>
      </div>
    </div>
  )

  if (!animated) {
    return chatContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.6, exit: 0.4 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', 30, 0]]}>
          {chatContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Organisms/ChatInterface',
  component: ChatInterface,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Complete chat interface with message history, input controls, and real-time streaming support. The main UI for the chat-to-deploy experience.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    messages: {
      control: 'object',
      description: 'Array of chat messages'
    },
    isLoading: {
      control: 'boolean',
      description: 'Show loading state'
    },
    isStreaming: {
      control: 'boolean',
      description: 'Show streaming state'
    },
    currentModel: {
      control: 'text',
      description: 'Current AI model name'
    },
    currentProvider: {
      control: 'select',
      options: ['anthropic', 'openai', 'cloudflare', 'openrouter', 'custom'],
      description: 'Current AI provider'
    },
    placeholder: {
      control: 'text',
      description: 'Input placeholder text'
    },
    maxHeight: {
      control: 'number',
      description: 'Maximum height of chat interface'
    },
    showHeader: {
      control: 'boolean',
      description: 'Show chat header'
    },
    showStatus: {
      control: 'boolean',
      description: 'Show status badge'
    },
    showModelBadge: {
      control: 'boolean',
      description: 'Show model badge in header'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof ChatInterface>

export default meta
type Story = StoryObj<typeof meta>

// Mock messages for stories
const sampleMessages: Message[] = [
  {
    id: '1',
    role: 'user',
    content: 'Build me a Bitcoin puns website that makes people laugh',
    timestamp: new Date(Date.now() - 300000)
  },
  {
    id: '2',
    role: 'assistant',
    content: `I'll create a fun Bitcoin puns website for you! This will include:

- A collection of crypto-themed puns and jokes
- Interactive elements for browsing puns
- Responsive design with Bitcoin-themed styling
- Easy sharing features

Let me start building this now...`,
    timestamp: new Date(Date.now() - 250000),
    model: 'claude-3-sonnet',
    provider: 'anthropic'
  },
  {
    id: '3',
    role: 'user',
    content: 'Make it really colorful with animations',
    timestamp: new Date(Date.now() - 200000)
  },
  {
    id: '4',
    role: 'assistant',
    content: 'Perfect! I\'ll add vibrant colors and smooth animations to make it more engaging.',
    timestamp: new Date(Date.now() - 150000),
    model: 'claude-3-sonnet',
    provider: 'anthropic'
  }
]

// Stories
export const Default: Story = {
  args: {}
}

export const WithMessages: Story = {
  args: {
    messages: sampleMessages
  }
}

export const StreamingResponse: Story = {
  args: {
    messages: [
      ...sampleMessages,
      {
        id: '5',
        role: 'assistant',
        content: 'I\'m now generating your colorful Bitcoin puns website with animations...',
        timestamp: new Date(),
        isStreaming: true,
        model: 'claude-3-sonnet',
        provider: 'anthropic'
      }
    ],
    isStreaming: true
  }
}

export const LoadingState: Story = {
  args: {
    messages: sampleMessages,
    isLoading: true
  }
}

export const EmptyChat: Story = {
  args: {
    messages: []
  }
}

export const CompactView: Story = {
  args: {
    messages: sampleMessages,
    maxHeight: 400,
    showHeader: false
  }
}

export const DifferentModel: Story = {
  args: {
    messages: [
      {
        id: '1',
        role: 'user',
        content: 'Create a simple landing page',
        timestamp: new Date(Date.now() - 60000)
      },
      {
        id: '2',
        role: 'assistant',
        content: 'I\'ll create a modern landing page with clean design and responsive layout.',
        timestamp: new Date(Date.now() - 30000),
        model: 'llama-3-8b-instruct',
        provider: 'cloudflare'
      }
    ],
    currentModel: 'llama-3-8b-instruct',
    currentProvider: 'cloudflare'
  }
}

export const ErrorMessage: Story = {
  args: {
    messages: [
      {
        id: '1',
        role: 'user',
        content: 'Deploy my app',
        timestamp: new Date(Date.now() - 60000)
      },
      {
        id: '2',
        role: 'assistant',
        content: 'I encountered an error while trying to deploy your application.',
        timestamp: new Date(Date.now() - 30000),
        model: 'claude-3-sonnet',
        provider: 'anthropic',
        error: 'Failed to connect to Cloudflare Workers API'
      }
    ]
  }
}

export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    const [messages, setMessages] = useState<Message[]>([
      {
        id: '1',
        role: 'assistant',
        content: 'Hello! I\'m here to help you build and deploy applications. What would you like to create?',
        timestamp: new Date(Date.now() - 30000),
        model: 'claude-3-sonnet',
        provider: 'anthropic'
      }
    ])
    const [isStreaming, setIsStreaming] = useState(false)

    const handleSendMessage = (content: string) => {
      // Add user message
      const userMessage: Message = {
        id: Date.now().toString(),
        role: 'user',
        content,
        timestamp: new Date()
      }
      setMessages(prev => [...prev, userMessage])

      // Simulate AI response
      setIsStreaming(true)
      setTimeout(() => {
        const aiResponse: Message = {
          id: (Date.now() + 1).toString(),
          role: 'assistant',
          content: `I'll help you build "${content}". Let me start working on that for you...`,
          timestamp: new Date(),
          model: 'claude-3-sonnet',
          provider: 'anthropic'
        }
        setMessages(prev => [...prev, aiResponse])
        setIsStreaming(false)
      }, 2000)
    }

    const handleClearChat = () => {
      setMessages([{
        id: '1',
        role: 'assistant',
        content: 'Chat cleared! What would you like to build next?',
        timestamp: new Date(),
        model: 'claude-3-sonnet',
        provider: 'anthropic'
      }])
    }

    return (
      <ChatInterface
        messages={messages}
        isStreaming={isStreaming}
        onSendMessage={handleSendMessage}
        onClearChat={handleClearChat}
      />
    )
  }
}

export const LongConversation: Story = {
  args: {
    messages: [
      {
        id: '1',
        role: 'user',
        content: 'I want to build a complete e-commerce website',
        timestamp: new Date(Date.now() - 600000)
      },
      {
        id: '2',
        role: 'assistant',
        content: 'Great! I\'ll help you build a complete e-commerce website. This will include product listings, shopping cart, checkout process, and payment integration.',
        timestamp: new Date(Date.now() - 580000),
        model: 'claude-3-sonnet',
        provider: 'anthropic'
      },
      {
        id: '3',
        role: 'user',
        content: 'Add user authentication and admin panel',
        timestamp: new Date(Date.now() - 560000)
      },
      {
        id: '4',
        role: 'assistant',
        content: 'Perfect! I\'ll add secure user authentication with login/register functionality and a comprehensive admin panel for managing products, orders, and users.',
        timestamp: new Date(Date.now() - 540000),
        model: 'claude-3-sonnet',
        provider: 'anthropic'
      },
      {
        id: '5',
        role: 'user',
        content: 'Make it mobile-responsive with modern design',
        timestamp: new Date(Date.now() - 520000)
      },
      {
        id: '6',
        role: 'assistant',
        content: 'Absolutely! I\'ll ensure the entire e-commerce site is fully responsive across all devices with a modern, clean design using the latest CSS techniques and best practices.',
        timestamp: new Date(Date.now() - 500000),
        model: 'claude-3-sonnet',
        provider: 'anthropic'
      },
      {
        id: '7',
        role: 'user',
        content: 'How long will this take to deploy?',
        timestamp: new Date(Date.now() - 480000)
      },
      {
        id: '8',
        role: 'assistant',
        content: 'The deployment process typically takes 3-5 minutes for a complete e-commerce site. I\'ll handle all the setup, configuration, and deployment to Cloudflare Workers automatically.',
        timestamp: new Date(Date.now() - 460000),
        model: 'claude-3-sonnet',
        provider: 'anthropic'
      }
    ],
    maxHeight: 500
  }
}

export const CustomPlaceholder: Story = {
  args: {
    placeholder: 'Describe your dream application and I\'ll build it...',
    messages: []
  }
}

export const Playground: Story = {
  args: {
    messages: sampleMessages,
    isLoading: false,
    isStreaming: false,
    currentModel: 'claude-3-sonnet',
    currentProvider: 'anthropic',
    maxHeight: 600,
    showHeader: true,
    showStatus: true,
    showModelBadge: true,
    animated: true
  }
}
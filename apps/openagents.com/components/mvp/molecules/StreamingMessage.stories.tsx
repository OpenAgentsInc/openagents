import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { StreamingCursor } from '../atoms/StreamingCursor.stories'
import { ModelBadge } from '../atoms/ModelBadge.stories'

// Icon components
const UserIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
    <circle cx="12" cy="7" r="4" />
  </svg>
)

const AssistantIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
    <line x1="9" y1="9" x2="15" y2="9" />
    <line x1="9" y1="12" x2="15" y2="12" />
    <line x1="9" y1="15" x2="12" y2="15" />
  </svg>
)

const SystemIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="10" />
    <line x1="12" y1="8" x2="12" y2="12" />
    <line x1="12" y1="16" x2="12.01" y2="16" />
  </svg>
)

// StreamingMessage component
export interface StreamingMessageProps {
  role: 'user' | 'assistant' | 'system'
  content: string
  isStreaming?: boolean
  streamingSpeed?: number
  showTimestamp?: boolean
  timestamp?: Date
  model?: string
  provider?: 'cloudflare' | 'openrouter' | 'openai' | 'anthropic' | 'custom'
  showAvatar?: boolean
  animated?: boolean
  className?: string
  onStreamComplete?: () => void
}

export const StreamingMessage = ({
  role,
  content,
  isStreaming = false,
  streamingSpeed = 30,
  showTimestamp = true,
  timestamp = new Date(),
  model,
  provider = 'cloudflare',
  showAvatar = true,
  animated = true,
  className = '',
  onStreamComplete
}: StreamingMessageProps) => {
  const [active, setActive] = useState(false)
  const [streamingComplete, setStreamingComplete] = useState(!isStreaming)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  useEffect(() => {
    if (isStreaming && content.length > 0) {
      // Calculate when streaming should complete
      const duration = (content.length * streamingSpeed) + 500
      const timer = setTimeout(() => {
        setStreamingComplete(true)
        onStreamComplete?.()
      }, duration)
      return () => clearTimeout(timer)
    }
  }, [isStreaming, content, streamingSpeed, onStreamComplete])

  const roleConfig = {
    user: {
      icon: UserIcon,
      bgColor: 'bg-blue-500/10',
      borderColor: 'border-blue-500/30',
      textColor: 'text-blue-300',
      iconBg: 'bg-blue-500/20',
      name: 'You'
    },
    assistant: {
      icon: AssistantIcon,
      bgColor: 'bg-cyan-500/10',
      borderColor: 'border-cyan-500/30',
      textColor: 'text-cyan-300',
      iconBg: 'bg-cyan-500/20',
      name: 'Assistant'
    },
    system: {
      icon: SystemIcon,
      bgColor: 'bg-gray-500/10',
      borderColor: 'border-gray-500/30',
      textColor: 'text-gray-300',
      iconBg: 'bg-gray-500/20',
      name: 'System'
    }
  }

  const config = roleConfig[role] || roleConfig.assistant
  const Icon = config?.icon || AssistantIcon

  const formatTime = (date: Date) => {
    return date.toLocaleTimeString('en-US', {
      hour: 'numeric',
      minute: '2-digit',
      hour12: true
    })
  }

  const messageContent = (
    <div
      className={cx(
        'relative rounded-lg border p-4',
        config.bgColor,
        config.borderColor,
        className
      )}
    >
      {/* Header */}
      <div className="flex items-start gap-3 mb-3">
        {showAvatar && (
          <div
            className={cx(
              'flex items-center justify-center w-10 h-10 rounded-lg',
              config.iconBg
            )}
          >
            <Icon className={config.textColor} />
          </div>
        )}
        
        <div className="flex-1">
          <div className="flex items-center gap-2 flex-wrap">
            <span className={cx('font-medium font-sans', config.textColor)}>
              {config.name}
            </span>
            {role === 'assistant' && model && (
              <ModelBadge
                model={model}
                provider={provider}
                size="small"
                variant="outline"
                animated={false}
              />
            )}
            {showTimestamp && (
              <span className="text-gray-500 text-xs ml-auto font-sans">
                {formatTime(timestamp)}
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Message Content */}
      <div className={cx(
        'prose prose-invert max-w-none',
        showAvatar && 'ml-13'
      )}>
        {role === 'assistant' && isStreaming && !streamingComplete ? (
          <>
            <Text
              as="div"
              manager="sequence"
              className="text-gray-200 font-sans"
            >
              {content}
            </Text>
            <StreamingCursor color="cyan" blinkSpeed={500} />
          </>
        ) : (
          <Text
            as="div"
            manager={animated && role === 'assistant' ? 'decipher' : undefined}
            className="text-gray-200"
          >
            {content}
          </Text>
        )}
      </div>
    </div>
  )

  if (!animated) {
    return messageContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated
          animated={[
            ['opacity', 0, 1],
            ['y', role === 'user' ? -20 : 20, 0]
          ]}
        >
          {messageContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/StreamingMessage',
  component: StreamingMessage,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Chat message component with typewriter effect for streaming AI responses. Supports user, assistant, and system message types with proper role indicators.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    role: {
      control: 'select',
      options: ['user', 'assistant', 'system'],
      description: 'Message sender role'
    },
    content: {
      control: 'text',
      description: 'Message content'
    },
    isStreaming: {
      control: 'boolean',
      description: 'Enable streaming typewriter effect'
    },
    streamingSpeed: {
      control: { type: 'number', min: 10, max: 100, step: 10 },
      description: 'Typewriter speed in milliseconds per character'
    },
    showTimestamp: {
      control: 'boolean',
      description: 'Show message timestamp'
    },
    model: {
      control: 'text',
      description: 'AI model name (for assistant messages)'
    },
    provider: {
      control: 'select',
      options: ['cloudflare', 'openrouter', 'openai', 'anthropic', 'custom'],
      description: 'AI provider (for assistant messages)'
    },
    showAvatar: {
      control: 'boolean',
      description: 'Show role avatar'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof StreamingMessage>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    role: 'assistant',
    content: 'Hello! I\'m here to help you build and deploy applications. What would you like to create today?'
  }
}

export const StreamingAssistant: Story = {
  args: {
    role: 'assistant',
    content: 'I\'ll help you create a Bitcoin puns website. Let me generate the code for you. First, I\'ll create a simple HTML page with some Bitcoin-themed puns and styling...',
    isStreaming: true,
    model: 'llama-3-8b-instruct',
    provider: 'cloudflare'
  }
}

export const UserMessage: Story = {
  args: {
    role: 'user',
    content: 'Create a website with Bitcoin puns'
  }
}

export const SystemMessage: Story = {
  args: {
    role: 'system',
    content: 'Your deployment has been successfully created at https://bitcoin-puns.openagents.dev',
    showAvatar: true
  }
}

export const ConversationFlow: Story = {
  args: { role: 'user', content: 'Sample message' },
  render: () => {
    const messages = [
      {
        role: 'user' as const,
        content: 'Create a Bitcoin puns website',
        timestamp: new Date(Date.now() - 60000)
      },
      {
        role: 'assistant' as const,
        content: 'I\'ll help you create a fun Bitcoin puns website! Let me generate the HTML, CSS, and content for you.',
        model: 'llama-3-8b-instruct',
        timestamp: new Date(Date.now() - 45000)
      },
      {
        role: 'assistant' as const,
        content: 'Here\'s your Bitcoin puns website with several crypto-currency jokes and a modern dark theme design. The site includes responsive layout and smooth animations.',
        model: 'llama-3-8b-instruct',
        timestamp: new Date(Date.now() - 30000),
        isStreaming: true
      },
      {
        role: 'system' as const,
        content: 'Deployment successful! Your site is live at https://bitcoin-puns-xyz.openagents.dev',
        timestamp: new Date(Date.now() - 15000)
      }
    ]

    return (
      <div className="space-y-4 max-w-3xl">
        {messages.map((message, index) => (
          <div key={index} style={{ animationDelay: `${index * 200}ms` }}>
            <StreamingMessage {...message} />
          </div>
        ))}
      </div>
    )
  }
}

export const LongStreamingMessage: Story = {
  args: {
    role: 'assistant',
    content: `I'll create a comprehensive Bitcoin puns website for you. Here's what I'll include:

1. **Homepage Layout**: A modern, dark-themed design with glowing neon effects
2. **Pun Collection**: Over 20 Bitcoin and cryptocurrency puns
3. **Interactive Features**: Hover effects and animations
4. **Responsive Design**: Works perfectly on all devices
5. **Performance**: Optimized for fast loading

Let me generate the code now...`,
    isStreaming: true,
    streamingSpeed: 20,
    model: 'claude-3-opus',
    provider: 'openrouter'
  }
}

export const NoAvatar: Story = {
  args: { role: 'user', content: 'Sample message' },
  render: () => (
    <div className="space-y-4 max-w-3xl">
      <StreamingMessage
        role="user"
        content="Make it more colorful"
        showAvatar={false}
      />
      <StreamingMessage
        role="assistant"
        content="I'll add more vibrant colors to your website with gradient effects and animated backgrounds."
        showAvatar={false}
        model="llama-3-8b-instruct"
      />
    </div>
  )
}

export const SpeedComparison: Story = {
  args: { role: 'assistant', content: 'Sample message' },
  render: () => {
    const [key, setKey] = useState(0)
    const message = "This is a demonstration of different streaming speeds for the typewriter effect."
    
    return (
      <div className="space-y-4 max-w-3xl">
        <div key={`${key}-1`}>
          <p className="text-gray-400 text-sm mb-2">Fast (20ms/char)</p>
          <StreamingMessage
            role="assistant"
            content={message}
            isStreaming={true}
            streamingSpeed={20}
          />
        </div>
        <div key={`${key}-2`}>
          <p className="text-gray-400 text-sm mb-2">Normal (30ms/char)</p>
          <StreamingMessage
            role="assistant"
            content={message}
            isStreaming={true}
            streamingSpeed={30}
          />
        </div>
        <div key={`${key}-3`}>
          <p className="text-gray-400 text-sm mb-2">Slow (50ms/char)</p>
          <StreamingMessage
            role="assistant"
            content={message}
            isStreaming={true}
            streamingSpeed={50}
          />
        </div>
        <button
          onClick={() => setKey(k => k + 1)}
          className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors"
        >
          Restart Streaming
        </button>
      </div>
    )
  }
}

export const StreamingWithCallback: Story = {
  args: { role: 'assistant', content: 'Sample message' },
  render: () => {
    const [status, setStatus] = useState('Streaming...')
    
    return (
      <div className="space-y-4 max-w-3xl">
        <StreamingMessage
          role="assistant"
          content="This message will trigger a callback when streaming completes."
          isStreaming={true}
          onStreamComplete={() => setStatus('Streaming complete!')}
        />
        <p className="text-cyan-300 text-sm">{status}</p>
      </div>
    )
  }
}

export const Playground: Story = {
  args: {
    role: 'assistant',
    content: 'This is a playground message where you can test all the different props and configurations.',
    isStreaming: false,
    streamingSpeed: 30,
    showTimestamp: true,
    model: 'llama-3-8b-instruct',
    provider: 'cloudflare',
    showAvatar: true,
    animated: true
  }
}
import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { ModelBadge } from '../atoms/ModelBadge.stories'
import { CopyButton } from '../atoms/CopyButton.stories'
import { StatusBadge } from '../atoms/StatusBadge.stories'

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

const EditIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
  </svg>
)

const TrashIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="3 6 5 6 21 6" />
    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
  </svg>
)

const MoreIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="1" />
    <circle cx="19" cy="12" r="1" />
    <circle cx="5" cy="12" r="1" />
  </svg>
)

// ChatMessage component
export interface ChatMessageProps {
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp?: Date
  model?: string
  provider?: 'cloudflare' | 'openrouter' | 'openai' | 'anthropic' | 'custom'
  status?: 'idle' | 'generating' | 'complete' | 'error'
  error?: string
  showActions?: boolean
  showTimestamp?: boolean
  showAvatar?: boolean
  isEditable?: boolean
  isHighlighted?: boolean
  animated?: boolean
  className?: string
  onEdit?: () => void
  onDelete?: () => void
  onCopy?: () => void
  onRetry?: () => void
}

export const ChatMessage = ({
  role,
  content,
  timestamp = new Date(),
  model,
  provider = 'cloudflare',
  status = 'complete',
  error,
  showActions = true,
  showTimestamp = true,
  showAvatar = true,
  isEditable = false,
  isHighlighted = false,
  animated = true,
  className = '',
  onEdit,
  onDelete,
  onCopy,
  onRetry
}: ChatMessageProps) => {
  const [active, setActive] = useState(false)
  const [showMenu, setShowMenu] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

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
        'group relative rounded-lg border p-4 transition-all duration-300',
        config.bgColor,
        config.borderColor,
        isHighlighted && 'ring-2 ring-cyan-500/50',
        status === 'error' && 'border-red-500/50 bg-red-500/10',
        'hover:shadow-lg hover:shadow-cyan-500/10',
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
            <span className={cx('font-medium', config.textColor)}>
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
            {status !== 'complete' && (
              <StatusBadge
                status={status === 'generating' ? 'generating' : status === 'error' ? 'error' : 'idle'}
                size="small"
                animated={false}
              />
            )}
            {showTimestamp && (
              <span className="text-gray-500 text-xs ml-auto">
                {formatTime(timestamp)}
              </span>
            )}
          </div>
        </div>

        {/* Actions Menu */}
        {showActions && (
          <div className="relative opacity-0 group-hover:opacity-100 transition-opacity">
            <button
              onClick={() => setShowMenu(!showMenu)}
              className="p-1 rounded hover:bg-white/10 transition-colors cursor-pointer"
            >
              <MoreIcon className="text-gray-400" />
            </button>
            
            {showMenu && (
              <div className="absolute right-0 top-8 bg-gray-900 border border-gray-700 rounded-lg shadow-xl z-10">
                <div className="py-1">
                  {onCopy && (
                    <button
                      onClick={() => {
                        onCopy()
                        setShowMenu(false)
                      }}
                      className="flex items-center gap-2 px-4 py-2 text-sm text-gray-300 hover:bg-gray-800 w-full text-left cursor-pointer"
                    >
                      <CopyButton
                        text={content}
                        variant="icon"
                        size="small"
                        animated={false}
                      />
                      <span>Copy</span>
                    </button>
                  )}
                  {isEditable && onEdit && (
                    <button
                      onClick={() => {
                        onEdit()
                        setShowMenu(false)
                      }}
                      className="flex items-center gap-2 px-4 py-2 text-sm text-gray-300 hover:bg-gray-800 w-full text-left cursor-pointer"
                    >
                      <EditIcon className="w-4 h-4" />
                      <span>Edit</span>
                    </button>
                  )}
                  {status === 'error' && onRetry && (
                    <button
                      onClick={() => {
                        onRetry()
                        setShowMenu(false)
                      }}
                      className="flex items-center gap-2 px-4 py-2 text-sm text-gray-300 hover:bg-gray-800 w-full text-left cursor-pointer"
                    >
                      <span>Retry</span>
                    </button>
                  )}
                  {onDelete && (
                    <button
                      onClick={() => {
                        onDelete()
                        setShowMenu(false)
                      }}
                      className="flex items-center gap-2 px-4 py-2 text-sm text-red-400 hover:bg-gray-800 w-full text-left cursor-pointer"
                    >
                      <TrashIcon className="w-4 h-4" />
                      <span>Delete</span>
                    </button>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Message Content */}
      <div className={cx(
        'prose prose-invert max-w-none',
        showAvatar && 'ml-13'
      )}>
        {status === 'error' && error ? (
          <div className="text-red-400">
            <p className="font-medium mb-2">Error occurred:</p>
            <p className="text-sm">{error}</p>
          </div>
        ) : (
          <Text
            as="div"
            manager={animated && role === 'assistant' ? 'decipher' : undefined}
            className="text-gray-200 whitespace-pre-wrap"
          >
            {content}
          </Text>
        )}
      </div>

      {/* Click outside to close menu */}
      {showMenu && (
        <div
          className="fixed inset-0 z-0"
          onClick={() => setShowMenu(false)}
        />
      )}
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
  title: 'MVP/Molecules/ChatMessage',
  component: ChatMessage,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Complete chat message component with actions, status indicators, and role-based styling. Supports editing, copying, and deletion actions.'
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
    model: {
      control: 'text',
      description: 'AI model name (for assistant messages)'
    },
    provider: {
      control: 'select',
      options: ['cloudflare', 'openrouter', 'openai', 'anthropic', 'custom'],
      description: 'AI provider (for assistant messages)'
    },
    status: {
      control: 'select',
      options: ['idle', 'generating', 'complete', 'error'],
      description: 'Message status'
    },
    error: {
      control: 'text',
      description: 'Error message (when status is error)'
    },
    showActions: {
      control: 'boolean',
      description: 'Show action menu'
    },
    showTimestamp: {
      control: 'boolean',
      description: 'Show message timestamp'
    },
    showAvatar: {
      control: 'boolean',
      description: 'Show role avatar'
    },
    isEditable: {
      control: 'boolean',
      description: 'Enable edit action'
    },
    isHighlighted: {
      control: 'boolean',
      description: 'Highlight the message'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof ChatMessage>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    role: 'assistant',
    content: 'I can help you build and deploy web applications using AI. What would you like to create today?',
    model: 'llama-3-8b-instruct',
    provider: 'cloudflare'
  }
}

export const UserMessage: Story = {
  args: {
    role: 'user',
    content: 'Create a Bitcoin puns website with dark theme',
    isEditable: true
  }
}

export const SystemMessage: Story = {
  args: {
    role: 'system',
    content: 'Your deployment is now live at https://bitcoin-puns.openagents.dev',
    showActions: false
  }
}

export const GeneratingMessage: Story = {
  args: {
    role: 'assistant',
    content: 'Creating your Bitcoin puns website...',
    status: 'generating',
    model: 'llama-3-8b-instruct'
  }
}

export const ErrorMessage: Story = {
  args: {
    role: 'assistant',
    content: '',
    status: 'error',
    error: 'Failed to connect to the AI service. Please try again.',
    model: 'llama-3-8b-instruct'
  }
}

export const HighlightedMessage: Story = {
  args: {
    role: 'assistant',
    content: 'This message is highlighted to draw attention.',
    isHighlighted: true,
    model: 'claude-3-opus',
    provider: 'openrouter'
  }
}

export const LongMessage: Story = {
  args: {
    role: 'assistant',
    content: `I'll create a comprehensive Bitcoin puns website for you. Here's what I'll include:

**Features:**
1. Dark theme with neon accents
2. Collection of 20+ Bitcoin puns
3. Interactive hover effects
4. Responsive design
5. Smooth animations

**Some puns to include:**
- "I'm a bit coin-fused about crypto"
- "Don't go bacon my heart, Bitcoin"
- "Satoshi Nakamoto? More like Satoshi Naka-MOTTO!"
- "HODL on tight!"
- "To the moon! 🚀"

**Technical details:**
- Built with HTML5, CSS3, and JavaScript
- Optimized for performance
- SEO-friendly structure
- Cross-browser compatible

Let me generate the code for you now...`,
    model: 'gpt-4-turbo',
    provider: 'openai'
  }
}

export const ConversationThread: Story = {
  args: {
    role: 'user',
    content: 'Example message'
  },
  render: () => {
    const messages = [
      {
        role: 'user' as const,
        content: 'Build me a portfolio website',
        timestamp: new Date(Date.now() - 300000),
        isEditable: true
      },
      {
        role: 'assistant' as const,
        content: 'I\'ll help you create a professional portfolio website. What type of work do you want to showcase?',
        model: 'llama-3-8b-instruct',
        timestamp: new Date(Date.now() - 240000)
      },
      {
        role: 'user' as const,
        content: 'I\'m a web developer and designer. I want to show my projects with screenshots.',
        timestamp: new Date(Date.now() - 180000),
        isEditable: true
      },
      {
        role: 'assistant' as const,
        content: 'Perfect! I\'ll create a modern portfolio with a projects gallery, about section, and contact form. Generating the code now...',
        model: 'llama-3-8b-instruct',
        timestamp: new Date(Date.now() - 120000),
        status: 'generating' as const
      },
      {
        role: 'system' as const,
        content: 'Deployment initiated. Your portfolio will be live shortly.',
        timestamp: new Date(Date.now() - 60000),
        showActions: false
      }
    ]

    return (
      <div className="space-y-4 max-w-3xl">
        {messages.map((message, index) => (
          <div key={index} style={{ animationDelay: `${index * 100}ms` }}>
            <ChatMessage
              {...message}
              onEdit={() => console.log('Edit:', message.content)}
              onDelete={() => console.log('Delete:', message.content)}
              onCopy={() => console.log('Copy:', message.content)}
            />
          </div>
        ))}
      </div>
    )
  }
}

export const InteractiveActions: Story = {
  args: {
    role: 'assistant',
    content: 'Example message'
  },
  render: () => {
    const [message, setMessage] = useState('Click the menu to see available actions')
    
    return (
      <div className="space-y-4">
        <ChatMessage
          role="assistant"
          content={message}
          model="llama-3-8b-instruct"
          isEditable={true}
          onEdit={() => setMessage('Edit action triggered!')}
          onDelete={() => setMessage('Delete action triggered!')}
          onCopy={() => setMessage('Content copied to clipboard!')}
        />
        <p className="text-cyan-300 text-sm text-center">{message}</p>
      </div>
    )
  }
}

export const NoAvatar: Story = {
  args: {
    role: 'user',
    content: 'Example message'
  },
  render: () => (
    <div className="space-y-4 max-w-3xl">
      <ChatMessage
        role="user"
        content="Compact view without avatars"
        showAvatar={false}
      />
      <ChatMessage
        role="assistant"
        content="This is how messages look without avatars - more compact and streamlined."
        model="llama-3-8b-instruct"
        showAvatar={false}
      />
    </div>
  )
}

export const Playground: Story = {
  args: {
    role: 'assistant',
    content: 'This is a playground message where you can test all the different props and configurations.',
    model: 'llama-3-8b-instruct',
    provider: 'cloudflare',
    status: 'complete',
    showActions: true,
    showTimestamp: true,
    showAvatar: true,
    isEditable: false,
    isHighlighted: false,
    animated: true
  }
}
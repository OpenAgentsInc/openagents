import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect, useRef } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { StatusBadge } from '../atoms/StatusBadge.stories'
import { ModelBadge } from '../atoms/ModelBadge.stories'
import { StreamingCursor } from '../atoms/StreamingCursor.stories'

// Icon components
const SendIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <line x1="22" y1="2" x2="11" y2="13" />
    <polygon points="22 2 15 22 11 13 2 9 22 2" />
  </svg>
)

const AttachIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48" />
  </svg>
)

const MicIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
    <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
    <line x1="12" y1="19" x2="12" y2="23" />
    <line x1="8" y1="23" x2="16" y2="23" />
  </svg>
)

const StopIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="6" y="6" width="12" height="12" rx="1" />
  </svg>
)

const CodeIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="16 18 22 12 16 6" />
    <polyline points="8 6 2 12 8 18" />
  </svg>
)

const ImageIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
    <circle cx="8.5" cy="8.5" r="1.5" />
    <polyline points="21 15 16 10 5 21" />
  </svg>
)

// ChatInputWithStatus component
export interface ChatInputWithStatusProps {
  value?: string
  placeholder?: string
  status?: 'idle' | 'generating' | 'deploying' | 'thinking' | 'error'
  currentModel?: string
  currentProvider?: string
  isStreaming?: boolean
  isListening?: boolean
  statusMessage?: string
  maxLength?: number
  showCharCount?: boolean
  showAttachments?: boolean
  showVoice?: boolean
  showModelInfo?: boolean
  showStatusBar?: boolean
  autoFocus?: boolean
  animated?: boolean
  className?: string
  onChange?: (value: string) => void
  onSend?: (message: string) => void
  onStop?: () => void
  onAttach?: () => void
  onVoice?: () => void
  onKeyDown?: (e: React.KeyboardEvent) => void
}

export const ChatInputWithStatus = ({
  value = '',
  placeholder = 'Ask me to build something amazing...',
  status = 'idle',
  currentModel = 'claude-3-sonnet',
  currentProvider = 'anthropic',
  isStreaming = false,
  isListening = false,
  statusMessage = '',
  maxLength = 4000,
  showCharCount = true,
  showAttachments = true,
  showVoice = true,
  showModelInfo = true,
  showStatusBar = true,
  autoFocus = false,
  animated = true,
  className = '',
  onChange,
  onSend,
  onStop,
  onAttach,
  onVoice,
  onKeyDown
}: ChatInputWithStatusProps) => {
  const [active, setActive] = useState(false)
  const [internalValue, setInternalValue] = useState(value)
  const [isComposing, setIsComposing] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  useEffect(() => {
    setInternalValue(value)
  }, [value])

  useEffect(() => {
    if (autoFocus && textareaRef.current) {
      textareaRef.current.focus()
    }
  }, [autoFocus])

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = e.target.value
    if (newValue.length <= maxLength) {
      setInternalValue(newValue)
      onChange?.(newValue)
    }
  }

  const handleSend = () => {
    const trimmedValue = internalValue.trim()
    if (trimmedValue && !isStreaming && status !== 'generating') {
      onSend?.(trimmedValue)
      setInternalValue('')
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey && !isComposing) {
      e.preventDefault()
      handleSend()
    }
    onKeyDown?.(e)
  }

  const getStatusConfig = () => {
    switch (status) {
      case 'generating':
        return {
          badge: 'generating',
          message: statusMessage || 'AI is generating your response...',
          showCursor: true
        }
      case 'deploying':
        return {
          badge: 'deploying',
          message: statusMessage || 'Deploying your application...',
          showCursor: false
        }
      case 'thinking':
        return {
          badge: 'generating',
          message: statusMessage || 'AI is thinking...',
          showCursor: true
        }
      case 'error':
        return {
          badge: 'error',
          message: statusMessage || 'Something went wrong. Try again.',
          showCursor: false
        }
      default:
        return {
          badge: 'idle',
          message: statusMessage || 'Ready to help you build',
          showCursor: false
        }
    }
  }

  const statusConfig = getStatusConfig()
  const isDisabled = isStreaming || status === 'generating' || status === 'deploying'
  const canSend = internalValue.trim().length > 0 && !isDisabled

  const inputContent = (
    <div
      className={cx(
        'bg-black border border-cyan-500/30 rounded-lg overflow-hidden',
        'shadow-lg shadow-cyan-500/20',
        className
      )}
    >
      {/* Status Bar */}
      {showStatusBar && (
        <div className="flex items-center justify-between px-4 py-2 border-b border-cyan-500/20 bg-black/30">
          <div className="flex items-center gap-3">
            <StatusBadge
              status={statusConfig.badge as any}
              size="small"
              animated={false}
            />
            
            <div className="flex items-center gap-2">
              <Text className="text-sm text-gray-400">
                {statusConfig.message}
              </Text>
              {statusConfig.showCursor && (
                <StreamingCursor color="cyan" size="small" />
              )}
            </div>
          </div>
          
          {showModelInfo && (
            <ModelBadge
              model={currentModel}
              provider={currentProvider}
              variant="outline"
              size="small"
              animated={false}
            />
          )}
        </div>
      )}

      {/* Input Area */}
      <div className="relative">
        <textarea
          ref={textareaRef}
          value={internalValue}
          onChange={handleInputChange}
          onKeyDown={handleKeyDown}
          onCompositionStart={() => setIsComposing(true)}
          onCompositionEnd={() => setIsComposing(false)}
          placeholder={placeholder}
          disabled={isDisabled}
          className={cx(
            'w-full bg-black/50 px-4 py-4 pr-20 text-gray-200 placeholder-gray-500 resize-none',
            'focus:outline-none focus:ring-0 border-0',
            'min-h-[60px] max-h-[200px]',
            isDisabled && 'opacity-50 cursor-not-allowed'
          )}
          rows={3}
          style={{ 
            scrollbarWidth: 'thin',
            scrollbarColor: '#374151 transparent'
          }}
        />
        
        {/* Input Controls */}
        <div className="absolute right-2 bottom-2 flex items-center gap-1">
          {/* Attachment Button */}
          {showAttachments && (
            <button
              onClick={onAttach}
              disabled={isDisabled}
              className="p-2 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
              title="Attach file"
            >
              <AttachIcon className="w-4 h-4" />
            </button>
          )}
          
          {/* Voice Button */}
          {showVoice && (
            <button
              onClick={onVoice}
              disabled={isDisabled}
              className={cx(
                'p-2 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed',
                isListening ? 'text-red-400 hover:text-red-300' : 'text-gray-400 hover:text-cyan-300'
              )}
              title={isListening ? 'Stop recording' : 'Voice input'}
            >
              <MicIcon className="w-4 h-4" />
            </button>
          )}
          
          {/* Send/Stop Button */}
          {isStreaming || status === 'generating' ? (
            <button
              onClick={onStop}
              className="p-2 rounded bg-red-500 text-white hover:bg-red-600 transition-colors cursor-pointer"
              title="Stop generation"
            >
              <StopIcon className="w-4 h-4" />
            </button>
          ) : (
            <button
              onClick={handleSend}
              disabled={!canSend}
              className={cx(
                'p-2 rounded transition-all duration-200 cursor-pointer',
                canSend
                  ? 'bg-cyan-500 text-black hover:bg-cyan-400 transform hover:scale-105'
                  : 'bg-gray-600 text-gray-400 cursor-not-allowed'
              )}
              title="Send message"
            >
              <SendIcon className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between px-4 py-2 text-xs text-gray-500 bg-black/20">
        <div className="flex items-center gap-4">
          <span>Press Enter to send, Shift+Enter for new line</span>
          {isListening && (
            <div className="flex items-center gap-1 text-red-400">
              <div className="w-2 h-2 bg-red-400 rounded-full animate-pulse" />
              <span>Listening...</span>
            </div>
          )}
        </div>
        
        {showCharCount && (
          <span className={cx(
            'font-mono',
            internalValue.length > maxLength * 0.9 ? 'text-yellow-400' : 'text-gray-500',
            internalValue.length >= maxLength && 'text-red-400'
          )}>
            {internalValue.length}/{maxLength}
          </span>
        )}
      </div>
    </div>
  )

  if (!animated) {
    return inputContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
          {inputContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/ChatInputWithStatus',
  component: ChatInputWithStatus,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Enhanced chat input with status indicators, model information, and voice controls. Provides comprehensive input experience with real-time status updates.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    value: {
      control: 'text',
      description: 'Input value'
    },
    placeholder: {
      control: 'text',
      description: 'Placeholder text'
    },
    status: {
      control: 'select',
      options: ['idle', 'generating', 'deploying', 'thinking', 'error'],
      description: 'Current status'
    },
    currentModel: {
      control: 'text',
      description: 'Current AI model'
    },
    currentProvider: {
      control: 'select',
      options: ['anthropic', 'openai', 'cloudflare', 'openrouter', 'custom'],
      description: 'Current AI provider'
    },
    isStreaming: {
      control: 'boolean',
      description: 'Show streaming state'
    },
    isListening: {
      control: 'boolean',
      description: 'Show voice recording state'
    },
    statusMessage: {
      control: 'text',
      description: 'Custom status message'
    },
    maxLength: {
      control: 'number',
      description: 'Maximum character length'
    },
    showCharCount: {
      control: 'boolean',
      description: 'Show character counter'
    },
    showAttachments: {
      control: 'boolean',
      description: 'Show attachment button'
    },
    showVoice: {
      control: 'boolean',
      description: 'Show voice input button'
    },
    showModelInfo: {
      control: 'boolean',
      description: 'Show model badge'
    },
    showStatusBar: {
      control: 'boolean',
      description: 'Show status bar'
    },
    autoFocus: {
      control: 'boolean',
      description: 'Auto focus on mount'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof ChatInputWithStatus>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const Generating: Story = {
  args: {
    status: 'generating',
    statusMessage: 'Claude is generating your Bitcoin puns website...',
    isStreaming: true
  }
}

export const Deploying: Story = {
  args: {
    status: 'deploying',
    statusMessage: 'Deploying to Cloudflare Workers...'
  }
}

export const WithError: Story = {
  args: {
    status: 'error',
    statusMessage: 'Failed to connect to AI service. Please try again.'
  }
}

export const VoiceInput: Story = {
  args: {
    isListening: true,
    statusMessage: 'Listening for voice input...'
  }
}

export const DifferentModel: Story = {
  args: {
    currentModel: 'llama-3-8b-instruct',
    currentProvider: 'cloudflare',
    statusMessage: 'Ready to build with Cloudflare AI'
  }
}

export const LongMessage: Story = {
  args: {
    value: 'Build me a comprehensive e-commerce platform with user authentication, product catalog, shopping cart, payment processing, order management, admin dashboard, inventory tracking, email notifications, responsive design, and deployment to Cloudflare Workers with automatic scaling and global edge distribution.',
    maxLength: 500
  }
}

export const MinimalInput: Story = {
  args: {
    showStatusBar: false,
    showAttachments: false,
    showVoice: false,
    showCharCount: false,
    placeholder: 'Type your message...'
  }
}

export const CustomStatusMessage: Story = {
  args: {
    status: 'thinking',
    statusMessage: 'Analyzing your requirements and planning the architecture...',
    currentModel: 'claude-3-opus',
    currentProvider: 'anthropic'
  }
}

export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    const [value, setValue] = useState('')
    const [status, setStatus] = useState<'idle' | 'generating' | 'deploying' | 'thinking' | 'error'>('idle')
    const [isStreaming, setIsStreaming] = useState(false)
    const [isListening, setIsListening] = useState(false)
    const [message, setMessage] = useState('')

    const handleSend = (text: string) => {
      setMessage(`Sent: "${text}"`)
      setStatus('thinking')
      setIsStreaming(true)
      
      setTimeout(() => {
        setStatus('generating')
        setTimeout(() => {
          setStatus('deploying')
          setTimeout(() => {
            setStatus('idle')
            setIsStreaming(false)
            setMessage('Demo complete!')
            setTimeout(() => setMessage(''), 3000)
          }, 2000)
        }, 3000)
      }, 1000)
    }

    const handleStop = () => {
      setStatus('idle')
      setIsStreaming(false)
      setMessage('Generation stopped')
      setTimeout(() => setMessage(''), 2000)
    }

    const handleVoice = () => {
      setIsListening(!isListening)
      if (!isListening) {
        setMessage('Voice recording started')
        setTimeout(() => {
          setIsListening(false)
          setValue('Build me a Bitcoin puns website')
          setMessage('Voice input processed')
          setTimeout(() => setMessage(''), 2000)
        }, 3000)
      } else {
        setMessage('Voice recording stopped')
        setTimeout(() => setMessage(''), 2000)
      }
    }

    const handleAttach = () => {
      setMessage('File attachment clicked')
      setTimeout(() => setMessage(''), 2000)
    }

    return (
      <div className="space-y-4">
        <ChatInputWithStatus
          value={value}
          onChange={setValue}
          status={status}
          isStreaming={isStreaming}
          isListening={isListening}
          onSend={handleSend}
          onStop={handleStop}
          onVoice={handleVoice}
          onAttach={handleAttach}
          autoFocus={true}
        />
        
        {message && (
          <div className="text-center p-3 bg-cyan-500/10 border border-cyan-500/30 rounded text-cyan-300">
            {message}
          </div>
        )}
      </div>
    )
  }
}

export const AllStates: Story = {
  args: {},
  render: () => (
    <div className="space-y-6">
      <div>
        <Text className="text-sm text-gray-400 mb-2">Idle State</Text>
        <ChatInputWithStatus status="idle" />
      </div>
      
      <div>
        <Text className="text-sm text-gray-400 mb-2">Thinking State</Text>
        <ChatInputWithStatus 
          status="thinking" 
          statusMessage="Analyzing your request..."
        />
      </div>
      
      <div>
        <Text className="text-sm text-gray-400 mb-2">Generating State</Text>
        <ChatInputWithStatus 
          status="generating" 
          isStreaming={true}
          statusMessage="Generating your Bitcoin puns website..."
        />
      </div>
      
      <div>
        <Text className="text-sm text-gray-400 mb-2">Deploying State</Text>
        <ChatInputWithStatus 
          status="deploying"
          statusMessage="Deploying to Cloudflare Workers..."
        />
      </div>
      
      <div>
        <Text className="text-sm text-gray-400 mb-2">Error State</Text>
        <ChatInputWithStatus 
          status="error"
          statusMessage="Connection failed. Please try again."
        />
      </div>
      
      <div>
        <Text className="text-sm text-gray-400 mb-2">Voice Recording</Text>
        <ChatInputWithStatus 
          isListening={true}
          statusMessage="Listening for voice input..."
        />
      </div>
    </div>
  )
}

export const Playground: Story = {
  args: {
    placeholder: 'Ask me to build something amazing...',
    status: 'idle',
    currentModel: 'claude-3-sonnet',
    currentProvider: 'anthropic',
    maxLength: 4000,
    showCharCount: true,
    showAttachments: true,
    showVoice: true,
    showModelInfo: true,
    showStatusBar: true,
    animated: true
  }
}
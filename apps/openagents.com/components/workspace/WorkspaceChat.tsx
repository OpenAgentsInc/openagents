'use client'

import React, { useState, useRef, useEffect } from 'react'
import { Send, Paperclip, Mic, AlertCircleIcon, RefreshCwIcon, UserIcon, BotIcon } from 'lucide-react'
import { cx } from '@arwes/react'
import { useChat } from 'ai/react'
import { useToast } from '@/components/Toast'

interface WorkspaceChatProps {
  projectName: string
  projectId?: string
  className?: string
}

// Retry configuration for API failures
const RETRY_CONFIG = {
  maxRetries: 3,
  baseDelay: 1000,
  maxDelay: 10000
}

export function WorkspaceChat({ projectName, projectId = 'demo', className }: WorkspaceChatProps) {
  const toast = useToast()
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const [retryCount, setRetryCount] = useState(0)
  const [isRetrying, setIsRetrying] = useState(false)

  // Use Vercel AI SDK for streaming chat
  const {
    messages,
    input,
    handleInputChange,
    handleSubmit: handleChatSubmit,
    isLoading,
    error,
    reload,
    setMessages
  } = useChat({
    api: '/api/chat',
    body: {
      projectId,
      projectName,
      context: {
        framework: 'react', // TODO: Get from project settings
        deploymentUrl: undefined // TODO: Get from project data
      }
    },
    onError: (error) => {
      console.error('Chat error:', error)
      toast.error('Chat Error', 'Failed to get AI response. Please try again.', {
        action: {
          label: 'Retry',
          onClick: handleRetryMessage
        },
        persistent: true
      })
    },
    onFinish: () => {
      setRetryCount(0) // Reset retry count on successful completion
      inputRef.current?.focus()
    },
    initialMessages: [
      {
        id: 'welcome',
        role: 'assistant',
        content: `Welcome to ${projectName}! I'm here to help you build, debug, and deploy your project. Ask me anything about your code, request new features, or let me know what you'd like to work on.

Try asking me to:
• "Add a new React component"
• "Fix any TypeScript errors"  
• "Deploy this project"
• "Add styling with Tailwind CSS"
• "Create an API endpoint"`,
        createdAt: new Date(Date.now() - 5 * 60 * 1000)
      }
    ]
  })

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  // Handle retry with exponential backoff
  const handleRetryMessage = async () => {
    if (retryCount >= RETRY_CONFIG.maxRetries) {
      toast.error('Max Retries Reached', 'Please check your connection and try again later.')
      return
    }

    setIsRetrying(true)
    const delay = Math.min(
      RETRY_CONFIG.baseDelay * Math.pow(2, retryCount),
      RETRY_CONFIG.maxDelay
    )

    toast.info('Retrying...', `Attempt ${retryCount + 1} of ${RETRY_CONFIG.maxRetries}`)

    setTimeout(async () => {
      try {
        setRetryCount(prev => prev + 1)
        await reload()
        setIsRetrying(false)
        toast.success('Reconnected', 'Chat is working again!')
      } catch (error) {
        setIsRetrying(false)
        console.error('Retry failed:', error)
        // Will trigger another retry if under limit
        if (retryCount + 1 < RETRY_CONFIG.maxRetries) {
          setTimeout(handleRetryMessage, 2000)
        }
      }
    }, delay)
  }

  // Enhanced submit handler with error handling
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!input.trim() || isLoading || isRetrying) return

    try {
      await handleChatSubmit(e)
    } catch (error) {
      console.error('Submit error:', error)
      toast.error('Send Failed', 'Could not send message. Please try again.')
    }
  }

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSubmit(e as any)
    }
  }

  const formatTime = (date: Date) => {
    return date.toLocaleTimeString('en-US', { 
      hour: '2-digit', 
      minute: '2-digit', 
      hour12: false 
    })
  }

  // Show connection error state
  if (error && retryCount >= RETRY_CONFIG.maxRetries) {
    return (
      <div className={cx('h-full bg-black/50 border border-red-500/30 flex flex-col', className)}>
        <div className="h-12 bg-offblack border-b border-red-500/30 flex items-center px-4">
          <span className="text-red-500 text-sm font-mono uppercase tracking-wider">Chat Error</span>
        </div>
        <div className="flex-1 flex items-center justify-center p-8">
          <div className="text-center max-w-sm">
            <AlertCircleIcon className="w-12 h-12 text-red-400 mx-auto mb-4" />
            <div className="text-lg font-medium text-red-300 mb-2 font-sans">
              Chat Unavailable
            </div>
            <div className="text-sm text-red-400/80 mb-6 font-sans">
              Unable to connect to AI chat. Please check your connection and try again.
            </div>
            <button
              onClick={() => {
                setRetryCount(0)
                handleRetryMessage()
              }}
              className={cx(
                'flex items-center gap-2 px-4 py-2 mx-auto',
                'bg-red-500/20 hover:bg-red-500/30',
                'border border-red-500/50',
                'text-red-300 hover:text-red-200',
                'transition-all duration-200',
                'font-sans text-sm'
              )}
            >
              <RefreshCwIcon className="w-4 h-4" />
              Try Again
            </button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className={cx('h-full flex flex-col bg-black', className)}>
      {/* Header */}
      <div className="h-12 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
        <span className="text-cyan-500 text-sm font-mono uppercase tracking-wider">OpenAgents Chat</span>
        <div className="ml-auto">
          <span className="text-cyan-300/60 text-xs" style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>
            AI Assistant
          </span>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.map((message) => (
          <div
            key={message.id}
            className={cx(
              'flex items-start gap-3',
              message.role === 'user' ? 'justify-end' : 'justify-start'
            )}
          >
            {/* Avatar for assistant messages */}
            {message.role === 'assistant' && (
              <div className="w-8 h-8 rounded-full bg-cyan-500/20 border border-cyan-500/30 flex items-center justify-center flex-shrink-0 mt-1">
                <BotIcon className="w-4 h-4 text-cyan-400" />
              </div>
            )}
            
            <div
              className={cx(
                'max-w-[75%] rounded-lg p-3',
                message.role === 'user'
                  ? 'bg-cyan-600/20 border border-cyan-500/30'
                  : 'bg-gray-800/50 border border-gray-700/30'
              )}
            >
              <div
                className={cx(
                  'text-sm leading-relaxed whitespace-pre-wrap',
                  message.role === 'user' ? 'text-cyan-100' : 'text-gray-100'
                )}
                style={{ fontFamily: 'var(--font-titillium), sans-serif' }}
              >
                {message.content}
              </div>
              <div className="text-xs text-gray-400 mt-2" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
                {formatTime(message.createdAt || new Date())}
              </div>
            </div>

            {/* Avatar for user messages */}
            {message.role === 'user' && (
              <div className="w-8 h-8 rounded-full bg-cyan-600/20 border border-cyan-600/30 flex items-center justify-center flex-shrink-0 mt-1">
                <UserIcon className="w-4 h-4 text-cyan-300" />
              </div>
            )}
          </div>
        ))}

        {/* Typing indicator for streaming */}
        {isLoading && (
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 rounded-full bg-cyan-500/20 border border-cyan-500/30 flex items-center justify-center flex-shrink-0">
              <BotIcon className="w-4 h-4 text-cyan-400" />
            </div>
            <div className="bg-gray-800/50 border border-gray-700/30 rounded-lg p-3">
              <div className="flex items-center space-x-1">
                <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse"></div>
                <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse" style={{ animationDelay: '0.2s' }}></div>
                <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse" style={{ animationDelay: '0.4s' }}></div>
              </div>
            </div>
          </div>
        )}

        {/* Retry indicator */}
        {isRetrying && (
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 rounded-full bg-yellow-500/20 border border-yellow-500/30 flex items-center justify-center flex-shrink-0">
              <RefreshCwIcon className="w-4 h-4 text-yellow-400 animate-spin" />
            </div>
            <div className="bg-yellow-800/30 border border-yellow-700/30 rounded-lg p-3">
              <div className="text-sm text-yellow-300 font-sans">
                Reconnecting... (Attempt {retryCount + 1})
              </div>
            </div>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="border-t border-cyan-900/30 p-4">
        <form onSubmit={handleSubmit} className="flex items-end space-x-2">
          <button 
            type="button"
            className="p-2 text-cyan-500 hover:text-cyan-300 transition-colors"
            title="Attach files (coming soon)"
          >
            <Paperclip size={18} />
          </button>
          
          <div className="flex-1 relative">
            <textarea
              ref={inputRef}
              value={input}
              onChange={handleInputChange}
              onKeyPress={handleKeyPress}
              placeholder="Ask me to help with your project..."
              rows={1}
              disabled={isLoading || isRetrying}
              className={cx(
                'w-full bg-gray-900/50 border border-gray-700/50 rounded-lg',
                'px-4 py-3 pr-12 text-gray-100 text-sm resize-none',
                'focus:outline-none focus:border-cyan-500/50 focus:ring-1 focus:ring-cyan-500/50',
                'placeholder-gray-500 transition-colors',
                (isLoading || isRetrying) && 'opacity-50 cursor-not-allowed'
              )}
              style={{ 
                fontFamily: 'var(--font-titillium), sans-serif',
                minHeight: '44px',
                maxHeight: '120px'
              }}
            />
            
            <button
              type="submit"
              disabled={!input.trim() || isLoading || isRetrying}
              className={cx(
                'absolute right-2 top-1/2 -translate-y-1/2 p-2 rounded-md transition-colors',
                input.trim() && !isLoading && !isRetrying
                  ? 'text-cyan-500 hover:text-cyan-300 hover:bg-cyan-500/10'
                  : 'text-gray-600 cursor-not-allowed'
              )}
              title={isLoading ? 'Generating response...' : isRetrying ? 'Retrying...' : 'Send message'}
            >
              <Send size={16} className={isLoading ? 'animate-pulse' : ''} />
            </button>
          </div>

          <button 
            type="button"
            className="p-2 text-cyan-500 hover:text-cyan-300 transition-colors"
            title="Voice input (coming soon)"
          >
            <Mic size={18} />
          </button>
        </form>
        
        <div className="mt-2 flex items-center justify-between text-xs text-gray-500" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
          <span>Press Enter to send • Shift+Enter for new line</span>
          {isLoading && (
            <span className="text-cyan-400 animate-pulse">AI is typing...</span>
          )}
          {isRetrying && (
            <span className="text-yellow-400">Reconnecting...</span>
          )}
        </div>
      </div>
    </div>
  )
}
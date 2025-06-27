'use client'

import React, { useState, useRef, useEffect } from 'react'
import { Send, Paperclip, Mic } from 'lucide-react'
import { cx } from '@arwes/react'

interface Message {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date
}

interface WorkspaceChatProps {
  projectName: string
  className?: string
}

export function WorkspaceChat({ projectName, className }: WorkspaceChatProps) {
  const [messages, setMessages] = useState<Message[]>([
    {
      id: '1',
      role: 'assistant',
      content: `Welcome to ${projectName}! I'm here to help you build, debug, and deploy your project. What would you like to work on?`,
      timestamp: new Date(Date.now() - 5 * 60 * 1000)
    },
    {
      id: '2',
      role: 'user',
      content: 'Can you help me add a new feature to fetch real-time Bitcoin prices?',
      timestamp: new Date(Date.now() - 4 * 60 * 1000)
    },
    {
      id: '3',
      role: 'assistant',
      content: `Absolutely! I can help you integrate a Bitcoin price API. Let me suggest using the CoinGecko API for real-time data. Here's what we'll need to do:

1. Add an API endpoint to fetch Bitcoin prices
2. Update the frontend to poll for new data
3. Add error handling for API failures

Would you like me to start by creating the API integration in the code editor?`,
      timestamp: new Date(Date.now() - 3 * 60 * 1000)
    }
  ])
  
  const [inputValue, setInputValue] = useState('')
  const [isTyping, setIsTyping] = useState(false)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }

  useEffect(() => {
    scrollToBottom()
  }, [messages])

  const handleSendMessage = async () => {
    if (!inputValue.trim()) return

    const userMessage: Message = {
      id: Date.now().toString(),
      role: 'user',
      content: inputValue.trim(),
      timestamp: new Date()
    }

    setMessages(prev => [...prev, userMessage])
    setInputValue('')
    setIsTyping(true)

    // Simulate AI response
    setTimeout(() => {
      const responses = [
        "I'll help you implement that. Let me update the code to add that functionality.",
        "Great idea! I can see a few ways to approach this. Let me show you the most efficient solution.",
        "That's a good question. Let me analyze the current code and suggest some improvements.",
        "I'll help you debug this issue. Let me examine the code and identify the problem.",
        "Perfect! I'll implement that feature for you. This will make the application much more robust."
      ]
      
      const assistantMessage: Message = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: responses[Math.floor(Math.random() * responses.length)],
        timestamp: new Date()
      }

      setMessages(prev => [...prev, assistantMessage])
      setIsTyping(false)
    }, 1500 + Math.random() * 1000)
  }

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSendMessage()
    }
  }

  const formatTime = (date: Date) => {
    return date.toLocaleTimeString('en-US', { 
      hour: '2-digit', 
      minute: '2-digit', 
      hour12: false 
    })
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
              'flex',
              message.role === 'user' ? 'justify-end' : 'justify-start'
            )}
          >
            <div
              className={cx(
                'max-w-[80%] rounded-lg p-3',
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
                {formatTime(message.timestamp)}
              </div>
            </div>
          </div>
        ))}

        {isTyping && (
          <div className="flex justify-start">
            <div className="bg-gray-800/50 border border-gray-700/30 rounded-lg p-3">
              <div className="flex items-center space-x-1">
                <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse"></div>
                <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse" style={{ animationDelay: '0.2s' }}></div>
                <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse" style={{ animationDelay: '0.4s' }}></div>
              </div>
            </div>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="border-t border-cyan-900/30 p-4">
        <div className="flex items-end space-x-2">
          <button className="p-2 text-cyan-500 hover:text-cyan-300 transition-colors">
            <Paperclip size={18} />
          </button>
          
          <div className="flex-1 relative">
            <textarea
              ref={inputRef}
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              onKeyPress={handleKeyPress}
              placeholder="Ask me to help with your project..."
              rows={1}
              className={cx(
                'w-full bg-gray-900/50 border border-gray-700/50 rounded-lg',
                'px-4 py-3 pr-12 text-gray-100 text-sm resize-none',
                'focus:outline-none focus:border-cyan-500/50 focus:ring-1 focus:ring-cyan-500/50',
                'placeholder-gray-500'
              )}
              style={{ 
                fontFamily: 'var(--font-titillium), sans-serif',
                minHeight: '44px',
                maxHeight: '120px'
              }}
            />
            
            <button
              onClick={handleSendMessage}
              disabled={!inputValue.trim() || isTyping}
              className={cx(
                'absolute right-2 top-1/2 -translate-y-1/2 p-2 rounded-md transition-colors',
                inputValue.trim() && !isTyping
                  ? 'text-cyan-500 hover:text-cyan-300 hover:bg-cyan-500/10'
                  : 'text-gray-600 cursor-not-allowed'
              )}
            >
              <Send size={16} />
            </button>
          </div>

          <button className="p-2 text-cyan-500 hover:text-cyan-300 transition-colors">
            <Mic size={18} />
          </button>
        </div>
        
        <div className="mt-2 text-xs text-gray-500" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
          Press Enter to send • Shift+Enter for new line
        </div>
      </div>
    </div>
  )
}
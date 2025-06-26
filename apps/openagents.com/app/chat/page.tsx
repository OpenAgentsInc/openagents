'use client'

import React from 'react'
import { useChat } from '@ai-sdk/react'
import { Animator, Animated, cx, Text } from '@arwes/react'
import { AppLayout } from '@/components/AppLayout'
import { ButtonSimple } from '@/components/ButtonSimple'
import { Send, Bot, User } from 'lucide-react'

const ChatPage = (): React.ReactElement => {
  const { messages, input, handleInputChange, handleSubmit, isLoading } = useChat()

  const onSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    handleSubmit(e)
  }

  return (
    <AppLayout>
      <div className="flex flex-col h-full">
        <div className="mb-6">
          <h1 className="text-2xl font-bold font-mono text-cyan-300">
            <Text>OpenAgents Chat</Text>
          </h1>
          <div className="text-sm text-cyan-500/60 font-mono mt-1">
            <Text>Interact with AI agents powered by OpenRouter</Text>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto mb-4 border border-cyan-500/20 bg-black/30 p-4">
          {messages.length === 0 ? (
            <div className="text-center text-cyan-500/40 py-16 font-mono text-sm">
              <Text>Start a conversation...</Text>
            </div>
          ) : (
            <div className="space-y-3">
              {messages.map((message) => (
                <div
                  key={message.id}
                  className={cx(
                    'flex gap-3 p-3 rounded border',
                    message.role === 'user' 
                      ? 'bg-cyan-500/5 border-cyan-500/20' 
                      : 'bg-purple-500/5 border-purple-500/20'
                  )}
                >
                  <div className="flex-shrink-0 w-6 h-6">
                    {message.role === 'user' ? (
                      <User className="text-cyan-500" size={20} />
                    ) : (
                      <Bot className="text-purple-500" size={20} />
                    )}
                  </div>
                  <div className="flex-1 font-mono text-sm">
                    <div className={cx(
                      'font-semibold mb-1 text-xs uppercase',
                      message.role === 'user' ? 'text-cyan-400' : 'text-purple-400'
                    )}>
                      {message.role === 'user' ? 'You' : 'Agent'}
                    </div>
                    <div className="whitespace-pre-wrap break-words text-cyan-300/80">
                      {message.content}
                    </div>
                  </div>
                </div>
              ))}
              {isLoading && (
                <div className="flex gap-3 p-3 rounded border bg-purple-500/5 border-purple-500/20">
                  <Bot className="text-purple-500 animate-pulse" size={20} />
                  <div className="flex-1 font-mono text-sm">
                    <div className="font-semibold mb-1 text-xs uppercase text-purple-400">
                      Agent
                    </div>
                    <div className="text-purple-300/80">
                      <span className="inline-block">Thinking</span>
                      <span className="inline-block animate-pulse">...</span>
                    </div>
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        <form
          onSubmit={onSubmit}
          className="flex gap-2"
        >
          <input
            type="text"
            value={input}
            onChange={handleInputChange}
            placeholder="Type your message..."
            className={cx(
              'flex-1 px-4 py-3',
              'bg-black/50 border border-cyan-500/30',
              'text-cyan-300 font-mono text-sm',
              'placeholder-cyan-500/30',
              'focus:outline-none focus:border-cyan-500/50',
              'focus:bg-black/70',
              'transition-all duration-200'
            )}
            disabled={isLoading}
          />
          <ButtonSimple
            type="submit"
            title="Send message"
            disabled={!input.trim() || isLoading}
            className={cx(
              'px-6',
              !input.trim() || isLoading ? 'opacity-50 cursor-not-allowed' : ''
            )}
          >
            <Send size={16} />
            <span>Send</span>
          </ButtonSimple>
        </form>
      </div>
    </AppLayout>
  )
}

export default ChatPage
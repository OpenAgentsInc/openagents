'use client'

import React from 'react'
import { useChat } from '@ai-sdk/react'
import { Animator, Animated, cx } from '@arwes/react'
import { PageLayout } from '@/components/PageLayout'
import { ButtonSimple } from '@/components/ButtonSimple'
import { Send, Bot, User } from 'lucide-react'

const ChatPage = (): React.ReactElement => {
  const { messages, input, handleInputChange, handleSubmit, isLoading } = useChat()

  const onSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    handleSubmit(e)
  }


  return (
    <PageLayout>
      <Animator combine manager="sequence">

        <Animated
          as="main"
          className={cx('flex flex-col h-full w-full p-6 max-w-4xl mx-auto')}
          animated={[['y', 24, 0, 0]]}
        >
          <Animator>
            <Animated as="h1" className="text-3xl md:text-4xl font-bold mb-6 text-center font-sans">
              OpenAgents Chat
            </Animated>
          </Animator>

          <Animator>
            <Animated
              className="flex-1 overflow-y-auto mb-6 border border-cyan-500/30 bg-black/50 backdrop-blur p-4 font-mono text-sm"
              animated={['flicker']}
            >
              {messages.length === 0 ? (
                <div className="text-center text-cyan-500/60 py-8">
                  Start a conversation...
                </div>
              ) : (
                <div className="space-y-4">
                  {messages.map((message) => (
                    <div
                      key={message.id}
                      className={cx(
                        'flex gap-3 p-3 rounded',
                        message.role === 'user' ? 'bg-cyan-500/10' : 'bg-purple-500/10'
                      )}
                    >
                      <div className="flex-shrink-0 w-6 h-6">
                        {message.role === 'user' ? (
                          <User className="text-cyan-500" size={20} />
                        ) : (
                          <Bot className="text-purple-500" size={20} />
                        )}
                      </div>
                      <div className="flex-1 whitespace-pre-wrap break-words">
                        {message.content}
                      </div>
                    </div>
                  ))}
                  {isLoading && (
                    <div className="flex gap-3 p-3 rounded bg-purple-500/10">
                      <Bot className="text-purple-500" size={20} />
                      <div className="flex-1">
                        <span className="inline-block animate-pulse">Thinking...</span>
                      </div>
                    </div>
                  )}
                </div>
              )}
            </Animated>
          </Animator>

          <Animator>
            <form
              onSubmit={onSubmit}
              className="flex gap-2"
            >
              <input
                type="text"
                value={input}
                onChange={handleInputChange}
                placeholder="Type your message..."
                className="flex-1 px-4 py-3 bg-black/80 border border-cyan-500/50 text-cyan-500 font-mono text-sm placeholder-cyan-500/30 focus:outline-none focus:border-cyan-500 focus:shadow-[0_0_10px_rgba(0,255,255,0.3)]"
                disabled={isLoading}
              />
              <ButtonSimple
                type="submit"
                title="Send message"
                disabled={!input.trim() || isLoading}
              >
                <Send size={16} />
                <span>Send</span>
              </ButtonSimple>
            </form>
          </Animator>

        </Animated>
      </Animator>
    </PageLayout>
  )
}

export default ChatPage

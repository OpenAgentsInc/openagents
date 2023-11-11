// Inspired by Chatbot-UI and modified to fit the needs of this project
// @see https://github.com/mckaywrigley/chatbot-ui/blob/main/components/Chat/ChatMessage.tsx

import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'
import { ChatMessageActions } from './chat-message-actions'
import { MemoizedReactMarkdown } from './markdown'
import { CodeBlock } from '@/Components/ui/codeblock'
import { IconGPUtopia, IconOpenAI, IconUser } from '@/Components/ui/icons'
import { cn } from '@/lib/utils'

export interface ChatMessageProps {
  message: any // Message
}

export function ChatMessage({ message, ...props }: ChatMessageProps) {
  return (
    <div className={cn('group relative mb-4 flex items-start text-left')} {...props}>
      <div
        className={cn(
          'flex h-8 w-8 shrink-0 select-none items-center justify-center rounded-md shadow',
          message.role === 'user' ? 'bg-background' : 'bg-primary text-primary-foreground'
        )}
      >
        {message.role === 'user' ? <IconUser /> : <IconGPUtopia />}
      </div>
      <div className="flex-1 px-1 ml-4 space-y-2">
        {message.content === '...' ? (
          <div className="ml-1 mt-1 loader" />
        ) : (
          <MemoizedReactMarkdown
            className="prose-invert prose-p:leading-relaxed prose-pre:p-0 pr-2"
            remarkPlugins={[remarkGfm, remarkMath]}
            components={{
              p({ children }) {
                return <p className="mb-2 last:mb-0">{children}</p>
              },
              code({ node, inline, className, children, ...props }) {
                if (children.length) {
                  if (children[0] == '▍') {
                    return <span className="mt-1 cursor-default animate-pulse">▍</span>
                  }

                  children[0] = (children[0] as string).replace('`▍`', '▍')
                }

                const match = /language-(\w+)/.exec(className || '')

                if (inline) {
                  return (
                    <code className={className} {...props}>
                      {children}
                    </code>
                  )
                }

                return (
                  <CodeBlock
                    key={Math.random()}
                    language={(match && match[1]) || ''}
                    value={String(children).replace(/\n$/, '')}
                    {...props}
                  />
                )
              }
            }}
          >
            {message.content}
          </MemoizedReactMarkdown>
        )}
        {message.content !== '...' && <ChatMessageActions message={message} />}
      </div>
    </div>
  )
}

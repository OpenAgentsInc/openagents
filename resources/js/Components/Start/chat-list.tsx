import { ChatMessage } from './chat-message'
import { Separator } from '@/Components/ui/separator'

export interface ChatList {
  messages: any // Message[]
}

export function ChatList({ messages }: ChatList) {
  if (!messages.length) {
    return null
  }

  return (
    <div className="relative mx-auto max-w-3xl px-4 overflow-hidden">
      {messages.map((message, index) => (
        <div key={index}>
          <ChatMessage message={message} />
          {index < messages.length - 1 && <Separator className="bg-transparent my-4 md:my-6" />}
        </div>
      ))}
    </div>
  )
}

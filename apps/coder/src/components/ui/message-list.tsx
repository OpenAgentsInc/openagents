import React from "react"
import {
  ChatMessage,
  type ChatMessageProps,
  type Message,
} from "@/components/ui/chat-message"
import { TypingIndicator } from "@/components/ui/typing-indicator"
import { UIMessage } from "@openagents/core"

type AdditionalMessageOptions = Omit<ChatMessageProps, keyof Message>

interface MessageListProps {
  messages: UIMessage[]
  showTimeStamps?: boolean
  isTyping?: boolean
  messageOptions?:
  | AdditionalMessageOptions
  | ((message: Message) => AdditionalMessageOptions)
}

export function MessageList({
  messages,
  showTimeStamps = false,
  isTyping = false,
  messageOptions,
}: MessageListProps) {
  // Create ref for the messages container
  const messagesEndRef = React.useRef<HTMLDivElement>(null);
  
  // Sort messages by timestamp - most recent last
  // If timestamps are the same, maintain original order
  const sortedMessages = React.useMemo(() => {
    // Create array of [message, originalIndex] pairs
    const messagesWithIndex = messages.map((message, index) => [message, index]);
    
    return messagesWithIndex
      .sort(([a, aIndex], [b, bIndex]) => {
        const aTime = new Date(a.timestamp).getTime();
        const bTime = new Date(b.timestamp).getTime();
        
        // First sort by timestamp
        if (aTime !== bTime) {
          return aTime - bTime;
        }
        
        // If timestamps are equal, preserve original order
        return aIndex - bIndex;
      })
      .map(([message]) => message);
  }, [messages]);
  
  // Scroll to bottom when messages change
  React.useEffect(() => {
    if (messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({ behavior: 'auto' });
    }
  }, [sortedMessages.length]);

  return (
    <div className="space-y-4 overflow-visible min-h-0">
      {sortedMessages.map((message, index) => {
        const additionalOptions =
          typeof messageOptions === "function"
            ? messageOptions(message as Message)
            : messageOptions

        return (
          <ChatMessage
            key={message.id || index}
            showTimeStamp={showTimeStamps}
            {...(message as Message)}
            {...additionalOptions}
          />
        )
      })}
      {isTyping && <TypingIndicator />}
      <div ref={messagesEndRef} />
    </div>
  )
}

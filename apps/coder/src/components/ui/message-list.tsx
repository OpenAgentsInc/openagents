import React from "react"
import {
  ChatMessage,
  type ChatMessageProps,
  type Message,
} from "@/components/ui/chat-message"
import { TypingIndicator } from "@/components/ui/typing-indicator"
import { UIMessage } from "@openagents/core"
import { useAutoScroll } from "@/hooks/use-auto-scroll"

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
  // Use the messages directly without sorting again - let parent component control sort
  const sortedMessages = React.useMemo(() => messages, [messages]);

  // Import and use the useAutoScroll hook
  const {
    containerRef,
    scrollToBottom,
    handleScroll,
    shouldAutoScroll,
    handleTouchStart
  } = useAutoScroll([sortedMessages, isTyping]);

  return (
    <div
      ref={containerRef}
      onScroll={handleScroll}
      onTouchStart={handleTouchStart}
      className="space-y-4 overflow-y-auto min-h-0 max-h-full h-full pb-6"
    >
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
    </div>
  )
}

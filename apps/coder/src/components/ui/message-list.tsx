import React, { useEffect, useRef } from "react"
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
  // Use the messages directly without sorting again - let parent component control sort
  const sortedMessages = React.useMemo(() => messages, [messages]);
  
  // Create ref for the container
  const containerRef = useRef<HTMLDivElement>(null);
  
  // Track user scroll position
  const userScrolledUp = useRef(false);
  
  // Track the total number of parts for scroll detection
  const totalParts = useRef<number>(0);
  const currentTotalParts = sortedMessages.reduce((count, message) => {
    return count + (message.parts?.length || 1);
  }, 0) + (isTyping ? 1 : 0);
  
  // Scroll to bottom function
  const scrollToBottom = () => {
    if (containerRef.current && !userScrolledUp.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  };
  
  // Handle scroll event
  const handleScroll = () => {
    if (containerRef.current) {
      const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
      const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;
      
      // If user scrolls up, mark as user-controlled
      if (!isAtBottom) {
        userScrolledUp.current = true;
      } else {
        // If user scrolls back to bottom, resume auto-scrolling
        userScrolledUp.current = false;
      }
    }
  };
  
  // Auto-scroll when messages or parts change
  useEffect(() => {
    // Check if content has been added
    if (currentTotalParts > totalParts.current) {
      // Short delay to ensure DOM has updated
      setTimeout(scrollToBottom, 0);
    }
    
    // Update the total parts count
    totalParts.current = currentTotalParts;
  }, [sortedMessages, currentTotalParts, isTyping]);
  
  // Initial scroll on mount
  useEffect(() => {
    scrollToBottom();
    
    // Set up MutationObserver to detect DOM changes in chat content
    if (containerRef.current) {
      const observer = new MutationObserver(() => {
        if (!userScrolledUp.current) {
          scrollToBottom();
        }
      });
      
      observer.observe(containerRef.current, {
        childList: true,
        subtree: true,
        characterData: true
      });
      
      return () => observer.disconnect();
    }
  }, []);

  return (
    <div 
      ref={containerRef}
      onScroll={handleScroll}
      className="space-y-4 overflow-y-auto min-h-0 max-h-full h-full"
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

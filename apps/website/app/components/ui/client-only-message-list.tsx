import { useEffect, useState } from "react";
import type { Message } from "./chat-message";

type ClientOnlyMessageListProps = {
  messages: Message[];
  showTimeStamps?: boolean;
  isTyping?: boolean;
};

export function ClientOnlyMessageList({ messages, showTimeStamps = false, isTyping = false }: ClientOnlyMessageListProps) {
  const [isMounted, setIsMounted] = useState(false);

  useEffect(() => {
    setIsMounted(true);
  }, []);

  if (!isMounted) {
    // Return a placeholder with the same dimensions to avoid layout shifts
    return (
      <div className="space-y-4 overflow-visible min-h-[200px] flex items-center justify-center text-muted-foreground">
        Loading messages...
      </div>
    );
  }

  // Simplified message list that doesn't use the complex ChatMessage component
  return (
    <div className="space-y-4 overflow-visible">
      {messages.map((message, index) => (
        <div 
          key={message.id || index}
          className={`p-4 rounded-lg ${message.role === 'user' ? 'bg-primary text-primary-foreground ml-12' : 'bg-muted text-foreground mr-12'}`}
        >
          <div className="text-sm mb-1 opacity-70">
            {message.role === 'user' ? 'You' : 'Assistant'}
          </div>
          <div className="whitespace-pre-wrap">
            {message.content}
          </div>
          {showTimeStamps && message.createdAt && (
            <div className="mt-1 text-xs opacity-50">
              {message.createdAt.toLocaleTimeString()}
            </div>
          )}
        </div>
      ))}
      {isTyping && (
        <div className="flex items-center space-x-2 text-muted-foreground">
          <div className="animate-bounce">•</div>
          <div className="animate-bounce animation-delay-150">•</div>
          <div className="animate-bounce animation-delay-300">•</div>
        </div>
      )}
    </div>
  );
}
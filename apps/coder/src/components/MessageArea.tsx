import React, { memo, useEffect, useRef } from 'react';
import { MessageList } from '@/components/ui/message-list';
import { useStreamingMessages } from '@/providers/StreamingMessageProvider';

export const MessageArea = memo(function MessageArea() {
  // Use the streaming message context instead of the main context
  // This creates a complete isolation boundary for streaming changes
  const { messages, isGenerating } = useStreamingMessages();
  
  // Reference to the scrollable container
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  
  // Handle autoscrolling at this level
  useEffect(() => {
    if (scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = scrollContainerRef.current.scrollHeight;
    }
  }, [messages, isGenerating]);
  
  return (
    <div 
      ref={scrollContainerRef}
      className="overflow-y-auto relative flex-1"
      data-autoscroll="true"
    >
      <div className="absolute inset-0 p-4 pt-8">
        <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
          <MessageList
            messages={messages}
            isTyping={false}
          />
        </div>
      </div>
    </div>
  );
});
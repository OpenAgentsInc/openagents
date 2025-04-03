import React, { memo } from 'react';
import { MessageList } from '@/components/ui/message-list';
import { useStreamingMessages } from '@/providers/StreamingMessageProvider';

export const MessageArea = memo(function MessageArea() {
  // Use the streaming message context instead of the main context
  // This creates a complete isolation boundary for streaming changes
  const { messages, isGenerating } = useStreamingMessages();
  
  return (
    <div className="flex-1 overflow-hidden relative">
      <div className="absolute inset-0 p-4 pt-8 flex flex-col h-full">
        <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem] flex-1 flex flex-col h-full">
          <MessageList
            messages={messages}
            isTyping={isGenerating}
          />
        </div>
      </div>
    </div>
  );
});
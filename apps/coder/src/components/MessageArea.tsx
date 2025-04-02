import React, { memo } from 'react';
import { MessageList } from '@/components/ui/message-list';
import { useChatState } from '@/providers/ChatStateProvider';

export const MessageArea = memo(function MessageArea() {
  const { messages, isGenerating } = useChatState();
  
  return (
    <div className="overflow-y-auto relative">
      <div className="absolute inset-0 p-4 pt-8">
        <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
          <MessageList
            messages={messages}
            isTyping={isGenerating}
          />
        </div>
      </div>
    </div>
  );
});
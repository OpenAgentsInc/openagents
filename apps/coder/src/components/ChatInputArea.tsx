import React, { memo, useCallback, useMemo } from 'react';
import { ChatForm } from '@/components/ui/chat';
import { MessageInput } from '@/components/ui/message-input';
import { ModelWarningBanner } from './ModelWarningBanner';
import { useModelContext } from '@/providers/ModelProvider';
import { useInputContext, useMessageContext } from '@/providers/ChatStateProvider';

export const ChatInputArea = memo(function ChatInputArea() {
  const { isModelAvailable } = useModelContext();
  
  // Get input state from InputContext (won't rerender during streaming)
  const { 
    input, 
    handleInputChange, 
    handleSubmit,
    stop 
  } = useInputContext();
  
  // Only get isGenerating from MessageContext
  const { isGenerating } = useMessageContext();
  
  // Memoize the onChange handler to prevent recreation on every render
  const handleOnChange = useCallback((e: string | React.ChangeEvent<HTMLTextAreaElement>) => {
    // Handle both string and event types
    if (typeof e === 'string') {
      handleInputChange(e);
    } else if (e && e.target) {
      handleInputChange(e.target.value);
    }
  }, [handleInputChange]);

  // Memoize the submit handler to prevent recreation on every render
  const memoizedHandleSubmit = useCallback((event?: { preventDefault?: () => void }) => {
    if (!isModelAvailable && event?.preventDefault) {
      event.preventDefault();
      return;
    }
    handleSubmit(event);
  }, [isModelAvailable, handleSubmit]);

  // Memoize the placeholder value
  const placeholderText = useMemo(() => 
    !isModelAvailable ? "API key required for this model" : "Message...", 
  [isModelAvailable]);
  
  // Wrap the MessageInput render function in useMemo to prevent rerenders during streaming
  const renderMessageInput = useCallback(({ files, setFiles }: { files: File[] | null, setFiles: React.Dispatch<React.SetStateAction<File[] | null>> }) => (
    <MessageInput
      value={input}
      onChange={handleOnChange}
      allowAttachments={false}
      stop={stop}
      isGenerating={isGenerating}
      disabled={!isModelAvailable}
      placeholder={placeholderText}
    />
  ), [input, handleOnChange, stop, isGenerating, isModelAvailable, placeholderText]);
  
  return (
    <div className="border-t bg-background p-4">
      <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
        <ModelWarningBanner />
        
        <ChatForm
          isPending={isGenerating}
          handleSubmit={memoizedHandleSubmit}
          className="relative"
        >
          {renderMessageInput}
        </ChatForm>
      </div>
    </div>
  );
});
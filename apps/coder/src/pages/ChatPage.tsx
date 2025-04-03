import React from 'react';
import { MessageArea } from '@/components/MessageArea';
import { ChatInputArea } from '@/components/ChatInputArea';
import { useMessageContext, useInputContext } from '@/providers/ChatStateProvider';
import { StreamingMessageProvider } from '@/providers/StreamingMessageProvider';
import { IsolatedInputProvider } from '@/providers/IsolatedInputProvider';

// Special component that completely isolates the input area
// It gets the input handlers only once on mount and never rerenders
const IsolatedInputWrapper = React.memo(function IsolatedInputWrapper({
  children
}: {
  children: React.ReactNode
}) {
  // Get the input context only once on mount and store in refs
  const { input, handleInputChange, handleSubmit, stop, isGenerating } = useInputContext();

  // Store everything in refs to prevent any updates from props
  const inputRef = React.useRef(input);
  const handleInputChangeRef = React.useRef(handleInputChange);
  const handleSubmitRef = React.useRef(handleSubmit);
  const stopRef = React.useRef(stop);
  const isGeneratingRef = React.useRef(isGenerating);

  // Update refs when values change but don't rerender
  React.useEffect(() => {
    inputRef.current = input;
    handleInputChangeRef.current = handleInputChange;
    handleSubmitRef.current = handleSubmit;
    stopRef.current = stop;
    isGeneratingRef.current = isGenerating;
  }, [input, handleInputChange, handleSubmit, stop, isGenerating]);

  return (
    <IsolatedInputProvider
      inputRef={inputRef}
      handleInputChangeRef={handleInputChangeRef}
      handleSubmitRef={handleSubmitRef}
      stopRef={stopRef}
      isGeneratingRef={isGeneratingRef}
    >
      {children}
    </IsolatedInputProvider>
  );
});

export default function ChatPage() {
  const { messages, isGenerating } = useMessageContext();

  return (
    <>
      {/* Wrap just the MessageArea in the streaming provider */}
      <StreamingMessageProvider messages={messages} isGenerating={isGenerating}>
        <MessageArea />
      </StreamingMessageProvider>

      {/* Create the most isolated possible input area */}
      <IsolatedInputWrapper>
        <ChatInputArea />
      </IsolatedInputWrapper>
    </>
  );
}

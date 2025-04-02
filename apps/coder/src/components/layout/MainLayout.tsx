import React, { memo, useRef, useEffect } from 'react';
import {
  SidebarProvider,
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarInset
} from '@/components/ui/sidebar';
import { AppHeader } from '@/components/AppHeader';
import { ModelHeader } from '@/components/ModelHeader';
import { ThreadList } from '@/components/ThreadList';
import { MessageArea } from '@/components/MessageArea';
import { ChatInputArea } from '@/components/ChatInputArea';
import { useThreadContext, useMessageContext, useInputContext } from '@/providers/ChatStateProvider';
import { StreamingMessageProvider } from '@/providers/StreamingMessageProvider';
import { StableInputProvider } from '@/providers/StableInputProvider';
import { StableHeaderProvider } from '@/providers/StableHeaderProvider';
import { IsolatedInputProvider } from '@/providers/IsolatedInputProvider';

// Special component that completely isolates the input area
// It gets the input handlers only once on mount and never rerenders
const IsolatedInputWrapper = memo(function IsolatedInputWrapper({
  children
}: {
  children: React.ReactNode
}) {
  // Get the input context only once on mount and store in refs
  const { input, handleInputChange, handleSubmit, stop, isGenerating } = useInputContext();

  // Store everything in refs to prevent any updates from props
  const inputRef = useRef(input);
  const handleInputChangeRef = useRef(handleInputChange);
  const handleSubmitRef = useRef(handleSubmit);
  const stopRef = useRef(stop);
  const isGeneratingRef = useRef(isGenerating);

  // Update refs when values change but don't rerender
  useEffect(() => {
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

export const MainLayout = memo(function MainLayout() {
  // Use thread context instead of the full chat state
  // This ensures this component won't rerender during message streaming
  const {
    currentThreadId,
    handleSelectThread,
    handleCreateThread,
    handleDeleteThread,
    handleRenameThread,
    threadListKey
  } = useThreadContext();

  // Get message data for streaming provider
  const { messages, isGenerating } = useMessageContext();

  // Get input data for stable input provider - still needed for other parts
  const inputContext = useInputContext();

  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-screen w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden z-0">
          <div className="mt-[30px] relative flex h-full w-full flex-row overflow-hidden">
            <Sidebar>
              <SidebarHeader className="border-y h-14 mt-[30px]">
                <StableHeaderProvider onCreateThread={handleCreateThread}>
                  <AppHeader />
                </StableHeaderProvider>
              </SidebarHeader>

              <SidebarContent>
                <ThreadList
                  key={`thread-list-${threadListKey}`}
                  currentThreadId={currentThreadId ?? ''}
                  onSelectThread={handleSelectThread}
                  onDeleteThread={handleDeleteThread}
                  onRenameThread={handleRenameThread}
                  onCreateThread={handleCreateThread}
                />
              </SidebarContent>

              {/* SidebarFooter removed */}
            </Sidebar>

            <SidebarInset>
              <div className="grid grid-rows-[auto_minmax(0,1fr)_auto] h-[calc(100vh-30px)]">
                <div className="absolute top-0 left-0 right-0 border-t p-3 flex items-center justify-between z-20">
                  <ModelHeader />
                </div>

                {/* Add a spacer div to maintain grid layout */}
                <div className="" />

                {/* Wrap just the MessageArea in the streaming provider */}
                <StreamingMessageProvider messages={messages} isGenerating={isGenerating}>
                  <MessageArea />
                </StreamingMessageProvider>

                {/* Create the most isolated possible input area */}
                <IsolatedInputWrapper>
                  <ChatInputArea />
                </IsolatedInputWrapper>
              </div>
            </SidebarInset>
          </div>
        </div>
      </div>
    </SidebarProvider>
  );
});

import React, { memo, useCallback, useMemo, useState } from 'react';
import { ChatForm } from '@/components/ui/chat';
import { MessageInput } from '@/components/ui/message-input';
import { ModelWarningBanner } from './ModelWarningBanner';
import { useModelContext } from '@/providers/ModelProvider';
import { useIsolatedInput } from '@/providers/IsolatedInputProvider';
import { cn } from "@/utils/tailwind";

export const ChatInputArea = memo(function ChatInputArea() {
  const { isModelAvailable, selectedModelId, handleModelChange } = useModelContext();
  // Track selected tools for this chat request
  const [selectedToolIds, setSelectedToolIds] = useState<string[]>([]);

  // Now use the completely isolated input provider
  // This completely disconnects this component from the streaming context
  // and prevents sidebar rerenders
  const {
    input,
    handleInputChange,
    handleSubmit,
    stop,
    isGenerating
  } = useIsolatedInput();

  // Memoize the onChange handler to prevent recreation on every render
  const handleOnChange = useCallback((e: string | React.ChangeEvent<HTMLTextAreaElement>) => {
    // Handle both string and event types
    if (typeof e === 'string') {
      handleInputChange(e);
    } else if (e && e.target) {
      handleInputChange(e.target.value);
    }
  }, [handleInputChange]);

  // Handle tool selection changes - enhanced with better logging and state updates
  const handleToolsChange = useCallback((toolIds: string[]) => {
    console.log('[ChatInputArea] Tool selection changed:', toolIds);
    // Make sure we update the local state
    setSelectedToolIds(toolIds);
    // Store in session storage for debugging
    try {
      sessionStorage.setItem('selected_tool_ids', JSON.stringify(toolIds));
    } catch (e) {
      console.warn('[ChatInputArea] Failed to store selected tools in session storage:', e);
    }
  }, []);

  // Memoize the submit handler to prevent recreation on every render
  const memoizedHandleSubmit = useCallback((event?: { preventDefault?: () => void }) => {
    if (!isModelAvailable && event?.preventDefault) {
      event.preventDefault();
      return;
    }
    
    // Get tools from session storage as a fallback
    let toolsToUse = selectedToolIds;
    try {
      const storedTools = sessionStorage.getItem('selected_tool_ids');
      if (storedTools && (!toolsToUse || toolsToUse.length === 0)) {
        toolsToUse = JSON.parse(storedTools);
        console.log('[ChatInputArea] Retrieved tools from session storage:', toolsToUse);
      }
    } catch (e) {
      console.warn('[ChatInputArea] Error retrieving tools from session storage:', e);
    }
    
    // Include the selected tools in the submission
    const submissionOptions = {
      // Create a proper body object for the API request
      body: {
        // Pass the tools that have been explicitly selected in the UI
        // The server should use these specific tool IDs rather than all available tools
        selectedToolIds: toolsToUse,
        // Add a debug flag to trace the tool selection issues
        debug_tool_selection: true
      }
    };
    
    console.log('[ChatInputArea] Submitting with explicitly selected tools:', toolsToUse);
    
    handleSubmit(event, submissionOptions);
  }, [isModelAvailable, handleSubmit, selectedToolIds]);

  // Memoize the placeholder value
  const placeholderText = useMemo(() =>
    !isModelAvailable ? "API key required for this model" : "Ask Coder",
    [isModelAvailable]);

  // Create a stable onChange that won't recreate during streaming
  // This is critical - creating a new stable handler with a ref
  const stableOnChangeRef = React.useRef<((e: any) => void) | null>(null);

  // Update the ref to point to the current handleOnChange function
  React.useEffect(() => {
    stableOnChangeRef.current = (e: any) => {
      handleOnChange(e);
    };
  }, [handleOnChange]);

  // This is the stable onChange handler we'll pass to MessageInput - it never changes
  const stableOnChange = React.useCallback((e: any) => {
    if (stableOnChangeRef.current) {
      stableOnChangeRef.current(e);
    }
  }, []);

  // Make all handlers stable
  const stopRef = React.useRef(stop);

  // Update the stop ref when it changes
  React.useEffect(() => {
    stopRef.current = stop;
  }, [stop]);

  // Create a stable stop handler that never changes
  const stableStop = React.useCallback(() => {
    if (stopRef.current) {
      stopRef.current();
    }
  }, []);

  // Stable input value & model state tracking
  const inputRef = React.useRef(input);
  const isGeneratingRef = React.useRef(isGenerating);
  const isModelAvailableRef = React.useRef(isModelAvailable);

  // Update refs when values change
  React.useEffect(() => {
    inputRef.current = input;
    isGeneratingRef.current = isGenerating;
    isModelAvailableRef.current = isModelAvailable;
  }, [input, isGenerating, isModelAvailable]);

  // Create completely stable props for MessageInput
  const messageInputProps = useMemo(() => {
    return {
      value: input,
      onChange: stableOnChange,
      allowAttachments: false,
      files: null,
      setFiles: () => { },
      stop: stableStop,
      selectedToolIds,
      handleToolsChange,
      isGenerating,
      disabled: !isModelAvailable,
      placeholder: placeholderText,
      selectedModelId,
      handleModelChange,
      isModelAvailable
    };
  }, [
    input, isGenerating, isModelAvailable, placeholderText, 
    selectedModelId, handleModelChange, stableOnChange, stableStop,
    // Include selectedToolIds in the dependency array to ensure props update when tools change
    selectedToolIds, handleToolsChange
  ]);

  // Wrap the MessageInput render function in useCallback 
  // It will now update whenever messageInputProps change, including tool selection changes
  const renderMessageInput = useCallback(({ files, setFiles }: { files: File[] | null, setFiles: React.Dispatch<React.SetStateAction<File[] | null>> }) => {
    console.log('[ChatInputArea] Rendering MessageInput with tools:', selectedToolIds);
    return <MessageInput {...messageInputProps} />;
  }, [messageInputProps]);

  return (
    <div className="">
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

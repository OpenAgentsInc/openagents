import React, { createContext, useContext, useMemo, useRef, useCallback, useEffect, useState } from 'react';

// Input state context isolated from the rest of the app
interface InputContextType {
  input: string;
  handleInputChange: (e: string | React.ChangeEvent<HTMLTextAreaElement>) => void;
  handleSubmit: (event?: { preventDefault?: () => void }, options?: any) => void;
  stop: () => void;
  isGenerating: boolean;
}

const IsolatedInputContext = createContext<InputContextType | null>(null);

export const useIsolatedInput = () => {
  const context = useContext(IsolatedInputContext);
  if (!context) throw new Error('useIsolatedInput must be used within an IsolatedInputProvider');
  return context;
};

interface IsolatedInputProviderProps {
  // We only need an initial reference to these handlers at mount time
  inputRef: React.MutableRefObject<string>;
  handleInputChangeRef: React.MutableRefObject<(value: string) => void>;
  handleSubmitRef: React.MutableRefObject<(event?: { preventDefault?: () => void }, options?: any) => void>;
  stopRef: React.MutableRefObject<() => void>;
  isGeneratingRef: React.MutableRefObject<boolean>;
  children: React.ReactNode;
}

export const IsolatedInputProvider: React.FC<IsolatedInputProviderProps> = ({
  inputRef,
  handleInputChangeRef,
  handleSubmitRef,
  stopRef,
  isGeneratingRef,
  children
}) => {
  // Complete isolation: maintain internal state
  const [input, setInput] = useState(inputRef.current);
  const [isGenerating, setIsGenerating] = useState(isGeneratingRef.current);
  const isTyping = useRef<boolean>(false);
  const typingTimeout = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // Debug logging for state changes
  useEffect(() => {
    console.log('[IsolatedInputProvider] Input state:', input);
    console.log('[IsolatedInputProvider] IsGenerating state:', isGenerating);
  }, [input, isGenerating]);

  // Only sync the generating state from outside
  useEffect(() => {
    const syncGeneratingState = () => {
      if (isGeneratingRef.current !== isGenerating) {
        console.log('[IsolatedInputProvider] Syncing generating state:', isGeneratingRef.current);
        setIsGenerating(isGeneratingRef.current);
      }
    };

    const intervalId = setInterval(syncGeneratingState, 200);
    return () => clearInterval(intervalId);
  }, [isGenerating]);

  // Create stable handler functions that affect our local state
  // but also propagate to the real handlers
  const handleInputChange = useCallback((e: string | React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = typeof e === 'string' ? e : e.target.value;
    console.log('[IsolatedInputProvider] handleInputChange called with:', value);

    // Clear any existing typing timeout
    if (typingTimeout.current) {
      clearTimeout(typingTimeout.current);
    }

    // Set typing flag
    isTyping.current = true;

    // Update our internal state
    setInput(value);

    // Propagate to the real handler
    console.log('[IsolatedInputProvider] Propagating input change to real handler');
    handleInputChangeRef.current(value);

    // Set a timeout to clear the typing flag
    typingTimeout.current = setTimeout(() => {
      isTyping.current = false;
    }, 1000);
  }, []);

  const handleSubmit = useCallback((event?: { preventDefault?: () => void }, options?: any) => {
    console.log('[IsolatedInputProvider] handleSubmit called with input:', input);
    console.log('[IsolatedInputProvider] Submit options:', options);

    event?.preventDefault?.();
    if (!input.trim()) {
      console.log('[IsolatedInputProvider] Preventing empty submission');
      return;
    }

    // Update the external ref before submitting
    console.log('[IsolatedInputProvider] Updating external ref before submit');
    inputRef.current = input;

    // Call the real submit handler
    console.log('[IsolatedInputProvider] Calling real submit handler');
    handleSubmitRef.current(event, options);

    // Clear input after submission
    console.log('[IsolatedInputProvider] Clearing input after submission');
    setInput('');
  }, [input]);

  const stop = useCallback(() => {
    console.log('[IsolatedInputProvider] Stop called');
    stopRef.current();
  }, []);

  // Memoize the context value to prevent context updates during streaming
  const contextValue = useMemo(() => ({
    input,
    handleInputChange,
    handleSubmit,
    stop,
    isGenerating
  }), [input, handleInputChange, handleSubmit, stop, isGenerating]);

  return (
    <IsolatedInputContext.Provider value={contextValue}>
      {children}
    </IsolatedInputContext.Provider>
  );
};

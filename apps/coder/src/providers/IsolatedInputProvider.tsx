import React, { createContext, useContext, useMemo, useRef, useCallback, useEffect, useState } from 'react';

// Input state context isolated from the rest of the app
type IsolatedInputContextType = {
  input: string;
  handleInputChange: (value: string) => void;
  handleSubmit: (event?: { preventDefault?: () => void }, options?: any) => void;
  stop: () => void;
  isGenerating: boolean;
};

const IsolatedInputContext = createContext<IsolatedInputContextType | null>(null);

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

  // Sync with source of truth when it changes (but only from outside edits, not our own)
  useEffect(() => {
    // Create a function to sync state from the outside references
    const syncState = () => {
      setInput(inputRef.current);
      setIsGenerating(isGeneratingRef.current);
    };

    // Set up an interval to check for external changes
    const intervalId = setInterval(syncState, 200);

    // Clean up the interval on unmount
    return () => clearInterval(intervalId);
  }, []);

  // Create stable handler functions that affect our local state
  // but also propagate to the real handlers
  const handleInputChange = useCallback((value: string) => {
    // Update our internal state
    setInput(value);
    
    // Propagate to the real handler
    handleInputChangeRef.current(value);
  }, []);
  
  const handleSubmit = useCallback((event?: { preventDefault?: () => void }, options?: any) => {
    handleSubmitRef.current(event, options);
  }, []);
  
  const stop = useCallback(() => {
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
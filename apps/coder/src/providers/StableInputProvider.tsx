import React, { createContext, useContext, useMemo, useRef, useCallback, useEffect } from 'react';

// Input context that ensures stability during rendering
type StableInputContextType = {
  input: string;
  handleInputChange: (value: string) => void;
  handleSubmit: (event?: { preventDefault?: () => void }, options?: any) => void;
  stop: () => void;
  isGenerating: boolean; // Needed for input behavior but rendered in a stable way
};

const StableInputContext = createContext<StableInputContextType | null>(null);

export const useStableInput = () => {
  const context = useContext(StableInputContext);
  if (!context) throw new Error('useStableInput must be used within a StableInputProvider');
  return context;
};

interface StableInputProviderProps {
  input: string;
  handleInputChange: (value: string) => void;
  handleSubmit: (event?: { preventDefault?: () => void }, options?: any) => void;
  stop: () => void;
  isGenerating: boolean;
  children: React.ReactNode;
}

export const StableInputProvider: React.FC<StableInputProviderProps> = ({
  input,
  handleInputChange,
  handleSubmit,
  stop,
  isGenerating,
  children
}) => {
  // Store all handlers in refs to maintain stable identities
  const inputChangeRef = useRef(handleInputChange);
  const submitRef = useRef(handleSubmit);
  const stopRef = useRef(stop);
  const isGeneratingRef = useRef(isGenerating);
  
  // Update refs when props change
  useEffect(() => {
    inputChangeRef.current = handleInputChange;
    submitRef.current = handleSubmit;
    stopRef.current = stop;
    isGeneratingRef.current = isGenerating;
  }, [handleInputChange, handleSubmit, stop, isGenerating]);
  
  // Create stable handler functions that never change identity
  const stableInputChange = useCallback((value: string) => {
    inputChangeRef.current(value);
  }, []);
  
  const stableSubmit = useCallback((event?: { preventDefault?: () => void }, options?: any) => {
    submitRef.current(event, options);
  }, []);
  
  const stableStop = useCallback(() => {
    stopRef.current();
  }, []);

  // Memoize the context value to prevent context updates during streaming
  const contextValue = useMemo(() => ({
    input, // Input value needs to update as user types
    handleInputChange: stableInputChange,
    handleSubmit: stableSubmit,
    stop: stableStop,
    isGenerating // isGenerating needs to update for UI feedback
  }), [input, stableInputChange, stableSubmit, stableStop, isGenerating]);

  return (
    <StableInputContext.Provider value={contextValue}>
      {children}
    </StableInputContext.Provider>
  );
};
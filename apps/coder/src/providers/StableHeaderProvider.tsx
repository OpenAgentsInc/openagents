import React, { createContext, useContext, useCallback, useRef, useMemo, useEffect } from 'react';

// Define context type
interface StableHeaderContextType {
  handleCreateThread: () => Promise<void>;
}

// Create the context
const StableHeaderContext = createContext<StableHeaderContextType | null>(null);

// Hook for components to use the context
export const useStableHeader = (): StableHeaderContextType => {
  const context = useContext(StableHeaderContext);
  if (!context) {
    throw new Error('useStableHeader must be used within a StableHeaderProvider');
  }
  return context;
};

// Props for the provider component
interface StableHeaderProviderProps {
  onCreateThread: () => Promise<void>;
  children: React.ReactNode;
}

// The provider component
export const StableHeaderProvider: React.FC<StableHeaderProviderProps> = ({
  onCreateThread,
  children
}) => {
  // Store handler reference to maintain stable identity
  const createThreadRef = useRef(onCreateThread);
  
  // Update ref when prop changes
  useEffect(() => {
    createThreadRef.current = onCreateThread;
  }, [onCreateThread]);
  
  // Create stable handler function that never changes identity
  const handleCreateThread = useCallback(async (): Promise<void> => {
    return createThreadRef.current();
  }, []);
  
  // Create context value
  const contextValue = useMemo(() => ({
    handleCreateThread
  }), [handleCreateThread]);
  
  return (
    <StableHeaderContext.Provider value={contextValue}>
      {children}
    </StableHeaderContext.Provider>
  );
};
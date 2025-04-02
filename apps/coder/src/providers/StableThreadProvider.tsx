import React, { useContext, useCallback, useRef, useEffect, createContext, useMemo } from 'react';

// Define the context type
interface StableThreadContextType {
  currentThreadId: string;
  handleSelectThread: (threadId: string) => void;
  handleDeleteThread: (e: React.MouseEvent, threadId: string, threadTitle: string) => void;
  deletingThreadIds: Set<string>;
}

// Create the context
const StableThreadContext = createContext<StableThreadContextType | null>(null);

// Hook for components to use the context
export const useStableThread = (): StableThreadContextType => {
  const context = useContext(StableThreadContext);
  if (!context) {
    throw new Error('useStableThread must be used within a StableThreadProvider');
  }
  return context;
};

// Props for the provider component
interface StableThreadProviderProps {
  currentThreadId: string;
  onSelectThread: (threadId: string) => void;
  onDeleteThread: (e: React.MouseEvent, threadId: string, threadTitle: string) => void;
  deletingThreadIds: Set<string>;
  children: React.ReactNode;
}

// The provider component
export const StableThreadProvider: React.FC<StableThreadProviderProps> = ({
  currentThreadId,
  onSelectThread,
  onDeleteThread,
  deletingThreadIds,
  children
}) => {
  // Store handler references to maintain stable identities
  const selectThreadRef = useRef(onSelectThread);
  const deleteThreadRef = useRef(onDeleteThread);
  
  // Update refs when props change
  useEffect(() => {
    selectThreadRef.current = onSelectThread;
    deleteThreadRef.current = onDeleteThread;
  }, [onSelectThread, onDeleteThread]);
  
  // Create stable handler functions that never change identity
  const handleSelectThread = useCallback((threadId: string) => {
    selectThreadRef.current(threadId);
  }, []);
  
  const handleDeleteThread = useCallback((e: React.MouseEvent, threadId: string, threadTitle: string) => {
    deleteThreadRef.current(e, threadId, threadTitle);
  }, []);
  
  // Create context value
  const contextValue = useMemo(() => ({
    currentThreadId,
    handleSelectThread,
    handleDeleteThread,
    deletingThreadIds
  }), [currentThreadId, handleSelectThread, handleDeleteThread, deletingThreadIds]);
  
  return (
    <StableThreadContext.Provider value={contextValue}>
      {children}
    </StableThreadContext.Provider>
  );
};
import React, { createContext, useContext, useEffect, useState } from 'react';
import { showDatabaseErrorToast } from '@/components/ui/database-error-notification';
import { logger } from '@openagents/core';

// Context types
interface DatabaseErrorContextType {
  databaseError: Error | null;
  setDatabaseError: (error: Error | null) => void;
  clearDatabaseError: () => void;
  retryDatabaseOperation: () => void;
}

// Create the context
const DatabaseErrorContext = createContext<DatabaseErrorContextType | null>(null);

/**
 * Hook to use the database error context
 */
export const useDatabaseError = () => {
  const context = useContext(DatabaseErrorContext);
  if (!context) {
    throw new Error('useDatabaseError must be used within a DatabaseErrorProvider');
  }
  return context;
};

interface DatabaseErrorProviderProps {
  children: React.ReactNode;
}

/**
 * Provider component for database errors
 */
export const DatabaseErrorProvider: React.FC<DatabaseErrorProviderProps> = ({ children }) => {
  const [databaseError, setDatabaseError] = useState<Error | null>(null);
  const [retryCount, setRetryCount] = useState(0);

  // Listen for database errors from window events
  useEffect(() => {
    // Handler for database errors
    const handleDatabaseError = (event: CustomEvent<{ error: Error }>) => {
      logger.error('Database error event received', event.detail.error);
      setDatabaseError(event.detail.error);
      
      // Show toast notification for the error
      showDatabaseErrorToast(event.detail.error);
    };

    // Add event listener
    window.addEventListener('database-error' as any, handleDatabaseError as EventListener);

    // Clean up
    return () => {
      window.removeEventListener('database-error' as any, handleDatabaseError as EventListener);
    };
  }, []);

  // Clear the database error
  const clearDatabaseError = () => {
    setDatabaseError(null);
  };

  // Retry the database operation
  const retryDatabaseOperation = () => {
    // Increment retry count to trigger the retry effect
    setRetryCount(count => count + 1);
    // Clear the current error
    clearDatabaseError();
    
    // Dispatch an event to trigger database retry
    window.dispatchEvent(new CustomEvent('database-retry'));
    
    logger.info('Database retry requested', { retryCount: retryCount + 1 });
  };

  // Context value
  const contextValue: DatabaseErrorContextType = {
    databaseError,
    setDatabaseError,
    clearDatabaseError,
    retryDatabaseOperation,
  };

  return (
    <DatabaseErrorContext.Provider value={contextValue}>
      {children}
    </DatabaseErrorContext.Provider>
  );
};
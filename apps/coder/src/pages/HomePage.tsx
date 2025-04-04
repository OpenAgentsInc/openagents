import React, { useEffect, useState } from 'react';
import ChatPage from './ChatPage';
import { logger } from '@openagents/core';

// Extend window type to include our context bridge API
declare global {
  interface Window {
    dbStatusContext: {
      getDbStatus: () => Promise<{ ready: boolean; error: string | null }>;
    };
  }
}

export default function HomePage() {
  // State to track database status - in dev mode, start with ready: true for faster loading
  const [dbStatus, setDbStatus] = useState<{ ready: boolean; error: string | null }>({ 
    ready: process.env.NODE_ENV !== 'production', // Start ready in dev mode
    error: null 
  });
  const [isLoading, setIsLoading] = useState(process.env.NODE_ENV === 'production'); // Only show loading in production

  // Create a logger
  const pageLogger = logger.createLogger ? logger.createLogger('HomePage') : {
    info: console.log,
    warn: console.warn,
    error: console.error
  };

  // Check database status on mount and potentially poll
  useEffect(() => {
    let isMounted = true;
    let pollInterval: NodeJS.Timeout | null = null;

    const checkDb = async () => {
      pageLogger.info('Checking DB status via IPC...');
      try {
        // In development mode, just proceed immediately without waiting
        if (process.env.NODE_ENV !== 'production') {
          pageLogger.info('Development mode: Bypassing DB status check');
          if (isMounted) {
            setDbStatus({ ready: true, error: null });
            setIsLoading(false);
          }
          return;
        }
        
        // Continue with normal DB check for production
        if (!window.dbStatusContext) {
          pageLogger.warn('Database status context not available yet, will retry...');
          
          // Set a default loading state but don't throw an error immediately
          if (isMounted) {
            setDbStatus(prevState => ({ ...prevState, error: null }));
            
            // If we're still loading and don't have a poll interval, start one
            if (!pollInterval) {
              pollInterval = setInterval(checkDb, 1000); // Poll more frequently during initial load
            }
          }
          return; // Exit this attempt, will retry via interval
        }
        
        // We have the context, try to get the status
        const status = await window.dbStatusContext.getDbStatus();
        pageLogger.info('Received DB status:', status);
        
        if (isMounted) {
          // In dev mode, always set ready to true
          if (process.env.NODE_ENV !== 'production') {
            setDbStatus({ ready: true, error: null });
          } else {
            setDbStatus(status);
          }
          setIsLoading(false); // Stop loading indicator
          
          // If DB becomes ready or has an error, or if we're in dev mode, stop polling
          if ((status.ready || status.error || process.env.NODE_ENV !== 'production') && pollInterval) {
            pageLogger.info('DB ready, error occurred, or in dev mode - stopping polling.');
            clearInterval(pollInterval);
            pollInterval = null;
          }
        }
      } catch (error) {
        pageLogger.error('Error checking DB status:', error);
        
        if (isMounted) {
          // In dev mode, ignore errors and proceed
          if (process.env.NODE_ENV !== 'production') {
            setDbStatus({ ready: true, error: null });
          } else {
            setDbStatus({ ready: false, error: error.message || 'Failed to check database status' });
          }
          setIsLoading(false);
        }
      }
    };

    // No delay in dev mode - check immediately to speed up startup
    checkDb();
    
    // In development mode, set a fallback to proceed after a short delay regardless of DB status
    const devModeFallback = setTimeout(() => {
      if (isMounted && isLoading) {
        pageLogger.info('Development mode fallback: proceeding without waiting for DB');
        setDbStatus({ ready: true, error: null });
        setIsLoading(false);
        
        if (pollInterval) {
          clearInterval(pollInterval);
          pollInterval = null;
        }
      }
    }, 2000); // After 2 seconds, proceed anyway in dev mode
    
    // Cleanup function
    return () => {
      isMounted = false;
      clearTimeout(devModeFallback);
      if (pollInterval) {
        clearInterval(pollInterval);
      }
    };
  }, []); // Run only on mount

  // Show loading/error state while database is not ready
  if (isLoading || !dbStatus.ready) {
    const errorMessage = dbStatus.error;
    // Check for specific error cases (like lock errors reported from main process)
    const isDuplicateInstance = errorMessage?.includes('locked') || errorMessage?.includes('lock') || errorMessage?.includes('in use');

    return (
      <div className="h-screen w-screen flex items-center justify-center bg-background">
        <div className="flex flex-col items-center p-6 max-w-md text-center">
          {isDuplicateInstance ? (
            // Duplicate instance / Lock Error
            <>
              <svg xmlns="http://www.w3.org/2000/svg" className="h-16 w-16 text-amber-500 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              <h2 className="text-xl font-bold mb-2">Database Access Issue</h2>
              <p className="mb-4 text-muted-foreground">
                {errorMessage || 'Could not access the database. It might be locked by another instance of Coder.'}
              </p>
              <p className="mb-4 text-sm">
                Please close all other instances of Coder and try again.
              </p>
              {/* Note: Renderer process cannot directly close the app, main process handles critical errors */}
              {/* We could add an IPC call to request quit if needed */}
              <button
                  className="py-2 px-4 bg-primary text-primary-foreground rounded hover:bg-primary/90"
                  onClick={() => window.location.reload()} // Offer reload
                >
                  Retry
                </button>
            </>
          ) : errorMessage ? (
             // Generic Initialization Error
             <div className="mt-4 p-4 border border-red-200 bg-red-50 dark:bg-red-900/20 dark:border-red-800 rounded-md">
                <p className="text-red-600 dark:text-red-400 font-medium">Initialization Error:</p>
                <p className="text-red-500 mt-2 text-sm">
                  {errorMessage}
                </p>
                <button
                  className="mt-4 py-1 px-3 bg-primary text-primary-foreground rounded hover:bg-primary/90 text-sm"
                  onClick={() => window.location.reload()} // Offer reload
                >
                  Retry
                </button>
              </div>
          ) : (
            // Normal loading state
            <>
              <div className="animate-spin mb-4 h-8 w-8 border-4 border-primary border-t-transparent rounded-full"></div>
              <p className="text-muted-foreground">Waiting for services...</p>
            </>
          )}
        </div>
      </div>
    );
  }

  // Render ChatPage only after database is ready
  return <ChatPage />;
}

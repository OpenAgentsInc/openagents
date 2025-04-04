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
  // State to track database status
  const [dbStatus, setDbStatus] = useState<{ ready: boolean; error: string | null }>({ ready: false, error: null });
  const [isLoading, setIsLoading] = useState(true);

  // Create a logger
  const pageLogger = logger.createLogger ? logger.createLogger('HomePage') : {
    info: console.log,
    error: console.error
  };

  // Check database status on mount and potentially poll
  useEffect(() => {
    let isMounted = true;
    let pollInterval: NodeJS.Timeout | null = null;

    const checkDb = async () => {
      pageLogger.info('Checking DB status via IPC...');
      try {
        if (!window.dbStatusContext) {
            throw new Error('Database status context not available on window object.');
        }
        const status = await window.dbStatusContext.getDbStatus();
        pageLogger.info('Received DB status:', status);
        if (isMounted) {
          setDbStatus(status);
          setIsLoading(false); // Stop initial loading indicator once we get a status

          // If DB is not ready and there's no error yet, poll for status updates
          // This handles the case where the main process is still initializing the DB
          if (!status.ready && !status.error && !pollInterval) {
            pageLogger.info('DB not ready, starting polling...');
            pollInterval = setInterval(checkDb, 2000); // Poll every 2 seconds
          }
          // If DB becomes ready or an error occurs, stop polling
          if ((status.ready || status.error) && pollInterval) {
            pageLogger.info('DB ready or error occurred, stopping polling.');
            clearInterval(pollInterval);
            pollInterval = null;
          }
        }
      } catch (error) {
        pageLogger.error('Error checking DB status:', error);
        if (isMounted) {
          setDbStatus({ ready: false, error: error.message || 'Failed to check database status' });
          setIsLoading(false);
          if (pollInterval) {
            clearInterval(pollInterval);
            pollInterval = null;
          }
        }
      }
    };

    checkDb(); // Initial check

    // Cleanup function
    return () => {
      isMounted = false;
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

import React, { useEffect, useState } from 'react';
import { useSettings } from '@openagents/core';
import ChatPage from './ChatPage';
import { logger } from '@openagents/core';

export default function HomePage() {
  // State to track if database is initialized
  const [isDbInitialized, setIsDbInitialized] = useState(false);
  const [initError, setInitError] = useState<Error | null>(null);
  
  // Create a database initialization logger
  const dbLogger = logger.createLogger ? logger.createLogger('database-init') : {
    info: console.log,
    error: console.error
  };

  // Initialize database before rendering the ChatPage
  useEffect(() => {
    // Flag to handle component unmount
    let isMounted = true;
    let initTimeout: NodeJS.Timeout | null = null;
    let initAttempts = 0;
    const MAX_ATTEMPTS = 3;
    
    dbLogger.info('HomePage mounted - initializing database');
    
    // Initialize database early and track completion
    const initializeDatabase = async () => {
      try {
        // Increment attempt counter
        initAttempts++;
        
        // Import directly here to avoid circular dependencies
        const db = await import('@openagents/core/src/db/database');
        
        // Get database and wait for it to fully initialize
        // Add a timeout to prevent hanging indefinitely
        const timeoutPromise = new Promise<never>((_, reject) => {
          initTimeout = setTimeout(() => {
            reject(new Error(`Database initialization timed out after 10 seconds (attempt ${initAttempts}/${MAX_ATTEMPTS})`));
          }, 10000); // 10 second timeout
        });
        
        // Race the database initialization against the timeout
        const database = await Promise.race([
          db.getDatabase(),
          timeoutPromise
        ]);
        
        // Clear timeout if initialization succeeded
        if (initTimeout) {
          clearTimeout(initTimeout);
          initTimeout = null;
        }
        
        // Additional verification that database has collections
        if (database && database.collections) {
          const collectionNames = Object.keys(database.collections);
          dbLogger.info(`Database initialized with collections: ${collectionNames.join(', ')}`);
          
          // Also initialize repositories explicitly
          const { threadRepository, messageRepository, settingsRepository } = await import('@openagents/core/src/db/repositories');
          await threadRepository.initialize(database);
          await messageRepository.initialize(database);
          await settingsRepository.initialize(database);
          
          dbLogger.info("Database and repositories initialized on startup successfully");
          
          // Only update state if component is still mounted
          if (isMounted) {
            setIsDbInitialized(true);
          }
        } else {
          throw new Error('Database initialized but collections are missing');
        }
      } catch (error) {
        // Clear any pending timeout
        if (initTimeout) {
          clearTimeout(initTimeout);
          initTimeout = null;
        }
        
        dbLogger.error(`Database initialization attempt ${initAttempts}/${MAX_ATTEMPTS} failed:`, error);
        
        // Check for lock-related errors (another instance running)
        const errorStr = String(error).toLowerCase();
        const isLockError = errorStr.includes('lock') || 
                           errorStr.includes('in use') || 
                           errorStr.includes('another instance') ||
                           errorStr.includes('access denied');
        
        if (isLockError) {
          dbLogger.warn('Database appears to be locked by another instance of the application');
          if (isMounted) {
            setInitError(new Error('Database is locked by another instance of Coder. Please close other instances before continuing.'));
          }
          return; // Don't retry for lock errors
        }
        
        // For other errors, retry up to MAX_ATTEMPTS
        if (initAttempts < MAX_ATTEMPTS && isMounted) {
          dbLogger.info(`Retrying database initialization (attempt ${initAttempts+1}/${MAX_ATTEMPTS})...`);
          
          // Set error message to show the retry is happening
          setInitError(new Error(`Initialization failed, retrying... (${initAttempts}/${MAX_ATTEMPTS})`));
          
          // Retry after a delay
          setTimeout(initializeDatabase, 2000);
        } else if (isMounted) {
          // Final error after all attempts
          setInitError(error instanceof Error ? error : new Error(String(error)));
        }
      }
    };
    
    // Start initialization
    initializeDatabase();
    
    // Cleanup function
    return () => {
      isMounted = false;
      if (initTimeout) {
        clearTimeout(initTimeout);
      }
    };
  }, []);
  
  // Show loading state while database is initializing
  if (!isDbInitialized) {
    // Check for specific error cases
    const isDuplicateInstance = initError && 
      (initError.message.includes('locked by another instance') || 
       initError.message.includes('already in use'));
    
    return (
      <div className="h-screen w-screen flex items-center justify-center bg-background">
        <div className="flex flex-col items-center p-6 max-w-md text-center">
          {isDuplicateInstance ? (
            // Duplicate instance detected
            <>
              <svg xmlns="http://www.w3.org/2000/svg" className="h-16 w-16 text-amber-500 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              <h2 className="text-xl font-bold mb-2">Another Instance Is Running</h2>
              <p className="mb-4 text-muted-foreground">
                It appears that another instance of Coder is already running and has locked the database.
              </p>
              <p className="mb-4 text-sm">
                Please close all other instances of Coder before continuing, or use the existing instance.
              </p>
              <div className="grid grid-cols-2 gap-2 w-full">
                <button 
                  className="py-2 px-4 bg-primary text-primary-foreground rounded hover:bg-primary/90"
                  onClick={() => window.location.reload()}
                >
                  Retry
                </button>
                <button 
                  className="py-2 px-4 bg-destructive text-destructive-foreground rounded hover:bg-destructive/90"
                  onClick={() => window.close()}
                >
                  Close App
                </button>
              </div>
            </>
          ) : (
            // Normal loading state
            <>
              <div className="animate-spin mb-4 h-8 w-8 border-4 border-primary border-t-transparent rounded-full"></div>
              <p className="text-muted-foreground">Initializing database...</p>
              {initError && (
                <div className="mt-4 p-4 border border-red-200 bg-red-50 dark:bg-red-900/20 dark:border-red-800 rounded-md">
                  <p className="text-red-600 dark:text-red-400 font-medium">Error initializing database:</p>
                  <p className="text-red-500 mt-2 text-sm">
                    {initError.message}
                  </p>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    );
  }
  
  // Render ChatPage only after database is initialized
  return <ChatPage />;
}
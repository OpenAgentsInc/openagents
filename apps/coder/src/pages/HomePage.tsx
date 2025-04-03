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
    
    dbLogger.info('HomePage mounted - initializing database');
    
    // Initialize database early and track completion
    const initializeDatabase = async () => {
      try {
        // Import directly here to avoid circular dependencies
        const db = await import('@openagents/core/src/db/database');
        
        // Get database and wait for it to fully initialize
        const database = await db.getDatabase();
        
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
        dbLogger.error("Failed to initialize database on startup:", error);
        if (isMounted) {
          setInitError(error instanceof Error ? error : new Error(String(error)));
        }
      }
    };
    
    // Start initialization
    initializeDatabase();
    
    // Cleanup function
    return () => {
      isMounted = false;
    };
  }, []);
  
  // Show loading state while database is initializing
  if (!isDbInitialized) {
    return (
      <div className="h-screen w-screen flex items-center justify-center bg-background">
        <div className="flex flex-col items-center">
          <div className="animate-spin mb-4 h-8 w-8 border-4 border-primary border-t-transparent rounded-full"></div>
          <p className="text-muted-foreground">Initializing database...</p>
          {initError && (
            <p className="text-red-500 mt-2 text-sm max-w-md text-center">
              Error: {initError.message}
            </p>
          )}
        </div>
      </div>
    );
  }
  
  // Render ChatPage only after database is initialized
  return <ChatPage />;
}
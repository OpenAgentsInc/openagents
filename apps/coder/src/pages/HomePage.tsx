import React, { useEffect } from 'react';
import { useSettings } from '@openagents/core';
import ChatPage from './ChatPage';
import { logger } from '@openagents/core';

export default function HomePage() {
  // Create a database initialization logger
  const dbLogger = logger.createLogger ? logger.createLogger('database-init') : {
    info: console.log,
    error: console.error
  };

  // Initialize database early
  useEffect(() => {
    dbLogger.info('HomePage mounted - initializing database');
    
    // Initialize database early
    (async () => {
      try {
        // Import directly here to avoid circular dependencies
        const db = await import('@openagents/core/src/db/database');
        await db.getDatabase();
        dbLogger.info("Database initialized on startup successfully");
      } catch (error) {
        dbLogger.error("Failed to initialize database on startup:", error);
      }
    })();
  }, []);
  
  // Simply render the ChatPage - all providers are already set up in the router
  return <ChatPage />;
}
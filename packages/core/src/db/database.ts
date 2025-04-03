import {
  RxCollection,
  createRxDatabase,
  addRxPlugin,
  RxDocument,
  RxJsonSchema,
  RxStorage,
  RxDatabase,
  RxDatabaseCreator
} from 'rxdb/plugins/core';
import { getRxStorageDexie } from 'rxdb/plugins/storage-dexie';
import { wrappedValidateZSchemaStorage } from 'rxdb/plugins/validate-z-schema';
import { RxDBQueryBuilderPlugin } from 'rxdb/plugins/query-builder';
import { RxDBUpdatePlugin } from 'rxdb/plugins/update';
import { RxDBMigrationSchemaPlugin } from 'rxdb/plugins/migration-schema';
import { Thread, StoredMessage, Settings, DatabaseCollections, Database } from './types';
import { threadSchema, messageSchema, settingsSchema } from './schema';

// Add required plugins
addRxPlugin(RxDBQueryBuilderPlugin);
addRxPlugin(RxDBUpdatePlugin);
addRxPlugin(RxDBMigrationSchemaPlugin);

// Initialize storage with validation
const storage = wrappedValidateZSchemaStorage({
  storage: getRxStorageDexie()
});

// Database instance (singleton)
let dbInstance: Database | null = null;

// Using consistent database names to maintain data between version changes
// RxDB supports schema migrations, so we don't need to change the name for each schema change
const PROD_DB_NAME = 'openagents_prod';

// Store a static database name for development to prevent double-init issues with strict mode
// Using a reproducible name helps with development and prevents creating multiple databases
let DEV_DB_NAME = 'openagents_dev';

// Track database creation attempts to handle Strict Mode double-mounting
let dbCreationInProgress = false;
let dbCreationPromise: Promise<Database> | null = null;

/**
 * Creates and initializes the RxDB database with all collections
 */
export async function createDatabase(): Promise<Database> {
  // Import logger dynamically to avoid circular dependencies
  const { createLogger } = await import('../utils/logManager');
  const dbLogger = createLogger('database');
  
  dbLogger.info('Creating database', { 
    environment: process.env.NODE_ENV,
    existingInstance: !!dbInstance,
    creationInProgress: dbCreationInProgress,
    databaseName: process.env.NODE_ENV === 'production' ? PROD_DB_NAME : DEV_DB_NAME
  });

  // If database already exists, return it
  if (dbInstance) {
    dbLogger.info('Using existing database instance');
    return dbInstance;
  }

  // If database creation is already in progress, return the promise to prevent double creation
  if (dbCreationInProgress && dbCreationPromise) {
    dbLogger.info('Database creation already in progress, returning existing promise');
    return dbCreationPromise;
  }

  // Set flag to indicate we're creating the database
  dbCreationInProgress = true;
  dbLogger.info('Starting database creation process');

  // Create a promise to handle concurrent calls
  dbCreationPromise = (async () => {
    try {
      // Import dev mode plugins in development
      if (process.env.NODE_ENV === 'development') {
        dbLogger.info('Loading development plugins for RxDB');
        const devModeModule = await import('rxdb/plugins/dev-mode');
        addRxPlugin(devModeModule.RxDBDevModePlugin);

        // Disable dev-mode warnings
        if (devModeModule.disableWarnings) {
          devModeModule.disableWarnings();
        }
      }

      // Use a static database name per session in development to handle Strict Mode
      // Use fixed name in production for persistence
      const dbName = process.env.NODE_ENV === 'production'
        ? PROD_DB_NAME
        : DEV_DB_NAME;
      
      dbLogger.info(`Creating RxDB database with name: ${dbName}`);

      // Create database with more resilient options
      dbLogger.info('Creating RxDatabase with configuration', {
        name: dbName,
        multiInstance: false,
        ignoreDuplicate: true,
        eventReduce: true
      });
      
      try {
        const db = await createRxDatabase<DatabaseCollections>({
          name: dbName,
          storage,
          multiInstance: false, // Single instance mode for better reliability
          ignoreDuplicate: true, // Ignore duplicate db creation for strict mode
          eventReduce: true, // Reduce event load
          cleanupPolicy: {
            // Automatically clean up old revisions to prevent storage issues
            minimumCollectionAge: 1000 * 60 * 60 * 24, // 1 day
            minimumDeletedTime: 1000 * 60 * 60 * 24, // 1 day
            runEach: 1000 * 60 * 60 // every hour
          }
        });
        
        dbLogger.info('RxDatabase created successfully');

        // Create collections with schema validation and migrations
        dbLogger.info('Adding collections to database');
        try {
          await db.addCollections({
          threads: {
            schema: threadSchema,
            migrationStrategies: {
              // Migrate from version 0 to 1 - keep document as is
              1: function (oldDoc) {
                return oldDoc;
              },
              // Version 2 - no changes needed for threads, just keep the document
              2: function (oldDoc) {
                return oldDoc;
              }
            }
          },
          messages: {
            schema: messageSchema,
            migrationStrategies: {
              // Migrate from version 0 to 1 - keep document as is
              1: function (oldDoc) {
                return oldDoc;
              },
              // Version 2 - no changes needed for messages, just keep the document
              2: function (oldDoc) {
                return oldDoc;
              }
            }
          },
          settings: {
            schema: settingsSchema,
            migrationStrategies: {
              // Migrate from version 0 to 1 - keep document as is
              1: function (oldDoc) {
                return oldDoc;
              },
              // Migrate from version 1 to 2 - add the new fields
              2: function (oldDoc) {
                return {
                  ...oldDoc,
                  // Add the new fields with sensible defaults
                  selectedModelId: oldDoc.defaultModel || 'anthropic/claude-3.7-sonnet',
                  visibleModelIds: [
                    'anthropic/claude-3.7-sonnet',
                    'anthropic/claude-3.5-sonnet',
                    'openai/gpt-4o-mini', 
                    'openai/gpt-4o-2024-11-20',
                    'google/gemini-2.0-flash-001'
                  ],
                  // Add empty array for MCP clients
                  mcpClients: []
                };
              }
            }
          }
        });
        dbLogger.info('Successfully added all collections');
      } catch (error) {
        dbLogger.error('Error adding collections to database', error);
        throw error;
      }

        // Check that collections were created successfully
        const collectionNames = Object.keys(db.collections);
        dbLogger.info(`Database collections created: ${collectionNames.join(', ')}`);
        
        // Database successfully created
        dbInstance = db;
        dbLogger.info('Database successfully created and initialized');
        return db;
      } catch (dbCreateError) {
        dbLogger.error('Error creating database or collections', dbCreateError);
        throw dbCreateError;
      }

    } catch (error) {
      // Import logger dynamically to avoid circular dependencies
      const { createLogger } = await import('../utils/logManager');
      const dbLogger = createLogger('database');
      
      dbLogger.error('Failed to create RxDB database:', error);

      // If we hit the collection limit, try to clean up and regenerate the database name
      if (error && typeof error === 'object' && 'code' in error && error.code === 'COL23') {
        dbLogger.warn('RxDB collection limit reached - generating new database name');

        // Generate a new database name for the next attempt
        DEV_DB_NAME = `openagents_${Date.now().toString(36)}_${Math.random().toString(36).substring(2)}`;

        // Clear the instance on error
        await cleanupDatabase();
        dbInstance = null;
      }

      throw error;
    } finally {
      // Clear the creation flags regardless of outcome
      dbCreationInProgress = false;
      dbCreationPromise = null;
    }
  })();

  return dbCreationPromise;
}

/**
 * Gets the database instance, creating it if it doesn't exist
 */
export async function getDatabase(): Promise<Database> {
  try {
    // REMOVED: Force cleanup on each app start
    // Only uncomment this line when you need to wipe the database for schema changes:
    // await cleanupDatabase();

    if (!dbInstance) {
      return createDatabase();
    }
    return dbInstance;
  } catch (error) {
    // Import logger dynamically to avoid circular dependencies
    const { createLogger } = await import('../utils/logManager');
    const dbLogger = createLogger('database');
    
    dbLogger.error('Error getting database:', error);
    throw error;
  }
}

/**
 * Cleanup database instance
 */
export async function cleanupDatabase() {
  // Import logger dynamically to avoid circular dependencies
  const { createLogger } = await import('../utils/logManager');
  const dbLogger = createLogger('database');
  
  dbLogger.info('Starting database cleanup');
  
  if (dbInstance) {
    try {
      dbLogger.info('Cleaning up existing database instance');
      
      // Try to destroy the database if the method exists
      if (typeof (dbInstance as any).destroy === 'function') {
        dbLogger.info('Calling database destroy() method');
        await (dbInstance as any).destroy();
      }

      // Also try to remove database by name
      if (typeof (dbInstance as any).name === 'string' && typeof window !== 'undefined' && window.indexedDB) {
        try {
          const dbName = (dbInstance as any).name;
          dbLogger.info(`Explicitly removing database by name: ${dbName}`);
          window.indexedDB.deleteDatabase(dbName);
        } catch (nameErr) {
          dbLogger.warn('Error removing database by name:', nameErr);
        }
      }
    } catch (err) {
      dbLogger.warn('Error during database instance cleanup:', err);
    }
    dbInstance = null;
  }

  // Clean up all matching indexedDB databases
  if (typeof window !== 'undefined' && window.indexedDB) {
    try {
      dbLogger.info('Cleaning up all IndexedDB databases');
      
      // Try to list and delete existing databases
      if (typeof window.indexedDB.databases === 'function') {
        const dbs = await window.indexedDB.databases() || [];
        dbLogger.info(`Found ${dbs.length} IndexedDB databases, checking for openagents databases`);
        
        for (const db of dbs) {
          if (db.name && db.name.includes('openagents')) {
            dbLogger.info(`Removing database: ${db.name}`);
            await window.indexedDB.deleteDatabase(db.name);
          }
        }
      } else {
        // Safari and some browsers don't support databases() method
        // Try to delete the known database names we use
        dbLogger.info('IndexedDB.databases() not supported, trying known database names');
        await window.indexedDB.deleteDatabase(PROD_DB_NAME);
        await window.indexedDB.deleteDatabase(DEV_DB_NAME);

        // Also try timestamp-based names that might have been created
        for (let i = 0; i < 5; i++) {
          const legacyName = `openagents_${Date.now().toString(36)}_cleanup${i}`;
          await window.indexedDB.deleteDatabase(legacyName);
        }

        // Also clean up older versions
        await window.indexedDB.deleteDatabase('openagents');
        await window.indexedDB.deleteDatabase('openagents_dev');
        await window.indexedDB.deleteDatabase('openagents_v2');
        await window.indexedDB.deleteDatabase('openagents_dev_v2');
      }

      dbLogger.info('Database cleanup completed successfully');
    } catch (err) {
      dbLogger.error('Error cleaning up IndexedDB:', err);
    }
  }
}

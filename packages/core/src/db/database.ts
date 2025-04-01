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
import { Thread, StoredMessage, Settings, DatabaseCollections, Database } from './types';
import { threadSchema, messageSchema, settingsSchema } from './schema';

// Add required plugins
addRxPlugin(RxDBQueryBuilderPlugin);
addRxPlugin(RxDBUpdatePlugin);

// Initialize storage with validation
const storage = wrappedValidateZSchemaStorage({
  storage: getRxStorageDexie()
});

// Database instance (singleton)
let dbInstance: Database | null = null;

// Database name constants
const PROD_DB_NAME = 'openagents';

// Store a static database name for development to prevent double-init issues with strict mode
let DEV_DB_NAME = `openagents_${Date.now().toString(36)}`;

// Track database creation attempts to handle Strict Mode double-mounting
let dbCreationInProgress = false;
let dbCreationPromise: Promise<Database> | null = null;

/**
 * Creates and initializes the RxDB database with all collections
 */
export async function createDatabase(): Promise<Database> {
  // If database already exists, return it
  if (dbInstance) {
    return dbInstance;
  }
  
  // If database creation is already in progress, return the promise to prevent double creation
  if (dbCreationInProgress && dbCreationPromise) {
    return dbCreationPromise;
  }
  
  // Set flag to indicate we're creating the database
  dbCreationInProgress = true;
  
  // Create a promise to handle concurrent calls
  dbCreationPromise = (async () => {
    try {
      // Import dev mode plugins in development
      if (process.env.NODE_ENV === 'development') {
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

      // Create database
      const db = await createRxDatabase<DatabaseCollections>({
        name: dbName,
        storage,
        multiInstance: false,
        ignoreDuplicate: true
      });

      // Create collections with schema validation
      await db.addCollections({
        threads: {
          schema: threadSchema
        },
        messages: {
          schema: messageSchema
        },
        settings: {
          schema: settingsSchema
        }
      });

      // Database successfully created
      dbInstance = db;
      return db;

    } catch (error) {
      console.error('Failed to create RxDB database:', error);
      
      // If we hit the collection limit, try to clean up and regenerate the database name
      if (error && typeof error === 'object' && 'code' in error && error.code === 'COL23') {
        console.warn('RxDB collection limit reached - generating new database name');
        
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
    if (!dbInstance) {
      return createDatabase();
    }
    return dbInstance;
  } catch (error) {
    console.error('Error getting database:', error);
    throw error;
  }
}

/**
 * Cleanup database instance
 */
export async function cleanupDatabase() {
  if (dbInstance) {
    try {
      // Try to destroy the database if the method exists
      if (typeof (dbInstance as any).destroy === 'function') {
        await (dbInstance as any).destroy();
      }
    } catch (err) {
      console.warn('Error during database cleanup:', err);
    }
    dbInstance = null;
  }
  
  // Also clean up any indexedDB databases if we're in collection limit trouble
  if (process.env.NODE_ENV === 'development' && typeof window !== 'undefined' && window.indexedDB) {
    try {
      // Try to list and delete existing databases
      if (typeof window.indexedDB.databases === 'function') {
        const dbs = await window.indexedDB.databases() || [];
        for (const db of dbs) {
          if (db.name && db.name.includes('openagents')) {
            await window.indexedDB.deleteDatabase(db.name);
          }
        }
      }
    } catch (err) {
      console.warn('Error cleaning up IndexedDB:', err);
    }
  }
}
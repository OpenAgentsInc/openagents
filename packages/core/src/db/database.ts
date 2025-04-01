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

/**
 * Creates and initializes the RxDB database with all collections
 */
export async function createDatabase(): Promise<Database> {
  // If database already exists, return it
  if (dbInstance) {
    return dbInstance;
  }

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

    // Use unique database name in development to avoid collection limit issues
    // Use fixed name in production for persistence
    const dbName = process.env.NODE_ENV === 'production' 
      ? PROD_DB_NAME 
      : `openagents_${Date.now().toString(36)}`;

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
    
    // Clear the instance on error
    dbInstance = null;
    
    throw error;
  }
}

/**
 * Gets the database instance, creating it if it doesn't exist
 */
export async function getDatabase(): Promise<Database> {
  if (!dbInstance) {
    return createDatabase();
  }
  return dbInstance;
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
}
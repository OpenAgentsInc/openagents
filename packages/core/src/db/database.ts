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

    // Create database
    const db = await createRxDatabase<DatabaseCollections>({
      name: 'openagents',
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
    
    // If this is a collection limit error, log it but don't try to auto-recover
    if (error && typeof error === 'object' && 'code' in error && error.code === 'COL23') {
      console.warn('RxDB collection limit reached - please refresh the page to reset connections');
    }
    
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
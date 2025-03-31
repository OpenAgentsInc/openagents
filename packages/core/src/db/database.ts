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

// Flag to indicate if we've already disabled warnings
let warningsDisabled = false;

/**
 * Creates and initializes the RxDB database with all collections
 */
export async function createDatabase(): Promise<Database> {
  // If database already exists, return it
  if (dbInstance) {
    return dbInstance;
  }

  try {
    // Import and disable dev mode warnings if not already done
    if (!warningsDisabled) {
      try {
        // Import dev mode plugin
        const devModeModule = await import('rxdb/plugins/dev-mode');
        addRxPlugin(devModeModule.RxDBDevModePlugin);
        
        // Disable warnings to prevent console spam
        if (devModeModule.disableWarnings) {
          devModeModule.disableWarnings();
          warningsDisabled = true;
          console.log('RxDB warnings disabled successfully');
        }
      } catch (err) {
        console.warn('Could not disable RxDB warnings:', err);
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
    
    // If this is a collection limit error, try to recover
    if (error && typeof error === 'object' && 'code' in error && error.code === 'COL23') {
      console.warn('RxDB collection limit reached - attempting cleanup');
      
      // Try to clean up existing database
      await cleanupDatabase();
      
      // Clear IndexedDB databases if possible
      if (typeof window !== 'undefined' && window.indexedDB) {
        try {
          await window.indexedDB.deleteDatabase('rxdb-dexie-openagents');
          await window.indexedDB.deleteDatabase('rxdb-dexie-openagents--0--threads');
          await window.indexedDB.deleteDatabase('rxdb-dexie-openagents--0--messages');
          await window.indexedDB.deleteDatabase('rxdb-dexie-openagents--0--settings');
          await window.indexedDB.deleteDatabase('rxdb-dexie-openagents--0--_rxdb_internal');
        } catch (err) {
          console.warn('Error cleaning up IndexedDB databases:', err);
        }
      }
      
      // Wait a bit for cleanup
      await new Promise(resolve => setTimeout(resolve, 500));
      
      // Try again with a fresh start
      dbInstance = null;
      return createDatabase();
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
      // Use any type to access destroy method
      await (dbInstance as any).destroy();
    } catch (err) {
      console.warn('Error during database cleanup:', err);
    }
    dbInstance = null;
  }
}
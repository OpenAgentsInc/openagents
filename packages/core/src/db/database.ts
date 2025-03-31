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
import { getRxStorageMemory } from 'rxdb/plugins/storage-memory';
import { wrappedValidateZSchemaStorage } from 'rxdb/plugins/validate-z-schema';
import { RxDBQueryBuilderPlugin } from 'rxdb/plugins/query-builder';
import { RxDBUpdatePlugin } from 'rxdb/plugins/update';
import { Thread, StoredMessage, Settings, DatabaseCollections, Database } from './types';
import { threadSchema, messageSchema, settingsSchema } from './schema';

// CONFIGURATION:
// Force in-memory database for development to avoid collection limit issues
const USE_MEMORY_STORAGE = process.env.NODE_ENV !== 'production';

// Database name
const DB_NAME = 'openagents';

// Add required plugins
addRxPlugin(RxDBQueryBuilderPlugin);
addRxPlugin(RxDBUpdatePlugin);

// Database instance (singleton)
let dbInstance: any = null;  // Use any type to have access to destroy method

// Flag to indicate if we've already disabled warnings
let warningsDisabled = false;

// Setup appropriate storage based on environment
const getStorage = () => {
  if (USE_MEMORY_STORAGE) {
    console.log('Using in-memory storage for development');
    return getRxStorageMemory();
  } else {
    // Use persistent IndexedDB storage for production
    return wrappedValidateZSchemaStorage({
      storage: getRxStorageDexie()
    });
  }
};

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

    // Base database config
    const dbCreator: RxDatabaseCreator = {
      name: DB_NAME,
      storage: getStorage(),
      multiInstance: !USE_MEMORY_STORAGE, // Disable multi-instance for memory storage
      ignoreDuplicate: true
    };

    // Create database
    const db = await createRxDatabase<DatabaseCollections>(dbCreator);

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
    
    // Clean up on error
    await cleanupDatabase();
    
    // If we're in development, force in-memory regardless of settings
    if (process.env.NODE_ENV !== 'production' && !USE_MEMORY_STORAGE) {
      console.warn('Switching to memory storage after persistent storage error');
      
      // Try to create a memory-only database as a fallback
      try {
        const memoryDb = await createRxDatabase<DatabaseCollections>({
          name: DB_NAME,
          storage: getRxStorageMemory(),
          multiInstance: false,
          ignoreDuplicate: true
        });
        
        await memoryDb.addCollections({
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
        
        dbInstance = memoryDb;
        return memoryDb;
      } catch (memError) {
        console.error('Failed to create memory database:', memError);
      }
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
      // Check if destroy exists before calling
      if (typeof dbInstance.destroy === 'function') {
        await dbInstance.destroy();
      } else if (typeof dbInstance.remove === 'function') {
        await dbInstance.remove();
      }
    } catch (err) {
      console.warn('Error during database cleanup:', err);
    }
    dbInstance = null;
  }
  
  // Clean up IndexedDB if we're not using memory storage
  if (!USE_MEMORY_STORAGE && typeof window !== 'undefined' && window.indexedDB) {
    try {
      await window.indexedDB.deleteDatabase(`rxdb-dexie-${DB_NAME}`);
      await window.indexedDB.deleteDatabase(`rxdb-dexie-${DB_NAME}--0--threads`);
      await window.indexedDB.deleteDatabase(`rxdb-dexie-${DB_NAME}--0--messages`);
      await window.indexedDB.deleteDatabase(`rxdb-dexie-${DB_NAME}--0--settings`);
      await window.indexedDB.deleteDatabase(`rxdb-dexie-${DB_NAME}--0--_rxdb_internal`);
    } catch (err) {
      console.warn('Error cleaning up IndexedDB:', err);
    }
  }
}

/**
 * Returns true if the database is using in-memory storage
 */
export function isUsingMemoryStorage(): boolean {
  return USE_MEMORY_STORAGE;
}
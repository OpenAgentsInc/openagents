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
let dbInstance: any = null;  // Use any type to have access to destroy method

// Flag to indicate if we've already disabled warnings
let warningsDisabled = false;

// Keep track of instances for cleanup
let instanceCounter = 0;
const MAX_INSTANCES = 5;

/**
 * Creates and initializes the RxDB database with all collections
 */
export async function createDatabase(): Promise<Database> {
  // If database already exists and it's not too old, return it
  if (dbInstance) {
    return dbInstance;
  }

  // Increment counter to track recreation
  instanceCounter++;
  
  // Force cleanup of old databases if we're recreating too many times
  if (instanceCounter > MAX_INSTANCES) {
    console.warn(`Too many database instances created (${instanceCounter}), forcing cleanup`);
    await forceCleanupAllDatabases();
    instanceCounter = 1;
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

    // Use a unique database name for each instance in development
    // to prevent collection limit issues
    const uniqueId = process.env.NODE_ENV === 'production' ? '' : `_${Date.now().toString(36)}`;
    const dbName = `openagents${uniqueId}`;

    // Create database
    const db = await createRxDatabase<DatabaseCollections>({
      name: dbName,
      storage,
      multiInstance: false,
      ignoreDuplicate: true,
      options: {
        // Shorter cleanup intervals for development
        cleanupInterval: 10000 // 10 seconds
      }
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
      console.warn('RxDB collection limit reached - forcing complete cleanup');
      
      // Force cleanup of everything
      await forceCleanupAllDatabases();
      
      // Wait a bit for cleanup
      await new Promise(resolve => setTimeout(resolve, 500));
      
      // Try again with a fresh start
      dbInstance = null;
      instanceCounter = 1;
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
 * Cleanup database instance - safely destroys current database
 */
export async function cleanupDatabase() {
  if (dbInstance) {
    try {
      // Check if destroy exists before calling it
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
}

/**
 * Force cleanup of all databases - used when collection limit is reached
 */
async function forceCleanupAllDatabases() {
  // First cleanup current instance
  await cleanupDatabase();
  
  // Force deletion of all databases in IndexedDB
  if (typeof window !== 'undefined' && window.indexedDB) {
    try {
      // Try to get all database names if possible
      if (typeof window.indexedDB.databases === 'function') {
        const dbs = await window.indexedDB.databases() || [];
        console.log(`Found ${dbs.length} databases to clean up`);
        
        for (const db of dbs) {
          if (db.name && (db.name.includes('openagents') || db.name.includes('rxdb'))) {
            try {
              console.log(`Deleting database: ${db.name}`);
              await window.indexedDB.deleteDatabase(db.name);
            } catch (e) {
              // Ignore errors during cleanup
            }
          }
        }
      } else {
        // Fallback to deleting known database patterns
        console.log('Using fallback database cleanup');
        
        // Delete any database that might be related to RxDB/openagents
        const baseNames = [
          'rxdb-dexie-openagents',
          'openagents',
          'rxdb'
        ];
        
        // Delete base database
        for (const name of baseNames) {
          try {
            await window.indexedDB.deleteDatabase(name);
          } catch (e) {
            // Ignore errors during cleanup
          }
        }
        
        // Delete collections for the past day
        // (using timestamp-based naming from our uniqueId approach)
        for (let i = 0; i < 100; i++) {
          try {
            const randomId = Math.floor(Math.random() * 1000000).toString(36);
            await window.indexedDB.deleteDatabase(`rxdb-dexie-openagents_${randomId}`);
            await window.indexedDB.deleteDatabase(`openagents_${randomId}`);
          } catch (e) {
            // Ignore errors during cleanup
          }
        }
        
        // Delete known collection patterns
        const patterns = ['threads', 'messages', 'settings', '_rxdb_internal'];
        for (const base of baseNames) {
          for (const pattern of patterns) {
            try {
              await window.indexedDB.deleteDatabase(`${base}--0--${pattern}`);
            } catch (e) {
              // Ignore errors during cleanup
            }
          }
        }
      }
    } catch (err) {
      console.warn('Error during force cleanup:', err);
    }
  }
  
  // Reset counter
  instanceCounter = 0;
}
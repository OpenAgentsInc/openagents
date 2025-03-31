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

// Database name for production
const PROD_DB_NAME = 'openagents';

// Store created database names to clean them up later
const createdDatabases: string[] = [];

// Flag to indicate if we've set up warning removals
let warningsDisabled = false;

// To prevent infinite retries
let retryAttempts = 0;
const MAX_RETRY_ATTEMPTS = 3;

// Global beforeunload handler
const handleBeforeUnload = () => {
  cleanupAllDatabases().catch(console.error);
};

/**
 * Clear existing database connections to prevent the collection limit issue
 */
async function clearExistingDatabases() {
  // Skip in non-browser environments
  if (typeof window === 'undefined' || !window.indexedDB) {
    return;
  }

  try {
    // Get all database names
    const dbNames: string[] = [];
    
    // Use the databases() API if available (modern browsers)
    if (typeof window.indexedDB.databases === 'function') {
      const dbs = await window.indexedDB.databases() || [];
      
      for (const db of dbs) {
        if (db.name) {
          // Match any RxDB or openagents database
          if (db.name.includes('rxdb') || db.name.includes('openagents')) {
            dbNames.push(db.name);
          }
        }
      }
    }
    
    // If we got no databases or the API isn't supported, try to infer the database names
    if (dbNames.length === 0) {
      // Add common RxDB database prefixes
      dbNames.push('rxdb-dexie-openagents');
      dbNames.push('rxdb-dexie-openagents_dev_static');
      
      // Add possible collection suffixes
      const suffixes = ['', '--0--_rxdb_internal', '--0--threads', '--0--messages', '--0--settings'];
      
      // Generate all combinations
      const additionalNames: string[] = [];
      for (const name of dbNames) {
        for (const suffix of suffixes) {
          additionalNames.push(`${name}${suffix}`);
        }
      }
      
      // Merge all possible names
      dbNames.push(...additionalNames);
    }

    // Delete databases to prevent collection limit issues
    console.log(`Attempting to delete ${dbNames.length} IndexedDB databases`);
    for (const dbName of dbNames) {
      try {
        await window.indexedDB.deleteDatabase(dbName);
      } catch (err) {
        // Suppress errors during cleanup
      }
    }
  } catch (err) {
    console.warn('Could not clear existing databases:', err);
  }
}

/**
 * Attempts to disable RxDB warnings
 */
async function disableRxDBWarnings() {
  if (warningsDisabled) return;
  
  try {
    // Try to load the dev-mode plugin and disable warnings
    const devMode = await import('rxdb/plugins/dev-mode');
    if (devMode.RxDBDevModePlugin) {
      addRxPlugin(devMode.RxDBDevModePlugin);
    }
    
    // Disable warnings if the function exists
    if (typeof devMode.disableWarnings === 'function') {
      devMode.disableWarnings();
      warningsDisabled = true;
    }
  } catch (e) {
    console.warn('Could not disable RxDB warnings:', e);
  }
}

/**
 * Creates and initializes the RxDB database with all collections
 */
export async function createDatabase(): Promise<Database> {
  // If database already exists, return it
  if (dbInstance) {
    return dbInstance;
  }

  try {
    // Try to disable warnings
    await disableRxDBWarnings();
    
    // In development mode, be more aggressive with cleanup
    if (process.env.NODE_ENV !== 'production') {
      // Clear existing databases to prevent collection limit issues
      await cleanupAllDatabases();
      await clearExistingDatabases();
    }
    
    // In dev mode, use a single static database name to prevent proliferation
    // In production, use a fixed name for persistence
    const dbName = process.env.NODE_ENV === 'production' 
      ? PROD_DB_NAME 
      : 'openagents_dev_static';
    
    // Store database name for cleanup
    createdDatabases.push(dbName);
    
    const dbCreator: RxDatabaseCreator = {
      name: dbName,
      storage,
      // Disable multi-instance to reduce complexity
      multiInstance: false,
      // Allow duplicate DB creation (helps with hot reloading)
      ignoreDuplicate: true,
      // Add cleanup policy - more aggressive in dev mode
      cleanupPolicy: {
        minimumCollectionAge: process.env.NODE_ENV === 'production' ? 1000 * 60 * 60 : 1000 * 30, // 1 hour in prod, 30 seconds in dev
        minimumDeletedTime: process.env.NODE_ENV === 'production' ? 1000 * 60 * 60 : 1000 * 30, // 1 hour in prod, 30 seconds in dev
        runEach: process.env.NODE_ENV === 'production' ? 1000 * 60 * 60 : 1000 * 30, // 1 hour in prod, 30 seconds in dev
      },
      // RxDB specific options for performance tuning
      options: {
        // Specify conflict threshold - shorter in dev mode
        conflictThreshold: process.env.NODE_ENV === 'production' ? 1000 * 60 * 5 : 1000 * 30 // 5 minutes in prod, 30 seconds in dev
      }
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

    // Set up cleanup on window unload/reload
    if (typeof window !== 'undefined') {
      // Remove any existing listeners to prevent duplicates
      window.removeEventListener('beforeunload', handleBeforeUnload);
      window.addEventListener('beforeunload', handleBeforeUnload);
    }

    // Database successfully created
    dbInstance = db;
    return db;

  } catch (error: unknown) {
    console.error('Failed to create RxDB database:', error);
    
    // If we hit the collection limit, try to clean up and try again (with limits)
    if (
      error && 
      typeof error === 'object' && 
      'code' in error && 
      error.code === 'COL23' && 
      retryAttempts < MAX_RETRY_ATTEMPTS
    ) {
      retryAttempts++;
      console.log(`Cleanup attempt ${retryAttempts}/${MAX_RETRY_ATTEMPTS}`);
      
      // More aggressive cleanup
      await cleanupAllDatabases();
      
      // Force deletion of all RxDB databases
      if (typeof window !== 'undefined' && window.indexedDB) {
        try {
          const dbs = await window.indexedDB.databases?.() || [];
          for (const db of dbs) {
            if (db.name && db.name.includes('rxdb')) {
              await window.indexedDB.deleteDatabase(db.name);
            }
          }
        } catch (err) {
          console.warn('Error during force cleanup:', err);
        }
      }
      
      // Wait a bit before retrying to allow cleanup to complete
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      // Try to create again with a new name after cleanup
      return createDatabase();
    }
    
    // Reset retry count and throw the error
    retryAttempts = 0;
    
    // If we've exceeded retry attempts, return a fallback in-memory database
    if (error && typeof error === 'object' && 'code' in error && error.code === 'COL23') {
      console.warn('Could not create persistent database after multiple attempts. Using in-memory fallback.');
      // Return an in-memory mock database for fallback
      return createFallbackDatabase();
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
      // RxDatabase has a destroy method, but TypeScript might not recognize it
      // Use optional chaining to safely try to call it
      if (typeof (dbInstance as any).destroy === 'function') {
        await (dbInstance as any).destroy();
      } else if (typeof (dbInstance as any).remove === 'function') {
        await (dbInstance as any).remove();
      }
    } catch (err) {
      console.warn('Error during database cleanup:', err);
    }
    dbInstance = null;
  }
}

/**
 * Clean up all created databases to prevent collection limit issues
 */
async function cleanupAllDatabases() {
  // First clean up current instance
  await cleanupDatabase();
  
  // Then try to clean up any other databases
  if (typeof window !== 'undefined' && window.indexedDB) {
    // Clean up databases from our tracking list
    for (const dbName of createdDatabases) {
      try {
        // Try different possible prefixes
        await window.indexedDB.deleteDatabase(`rxdb-dexie-${dbName}`);
        await window.indexedDB.deleteDatabase(`rxdb-dexie-${dbName}--0--_rxdb_internal`);
        await window.indexedDB.deleteDatabase(`rxdb-dexie-${dbName}--0--threads`);
        await window.indexedDB.deleteDatabase(`rxdb-dexie-${dbName}--0--messages`);
        await window.indexedDB.deleteDatabase(`rxdb-dexie-${dbName}--0--settings`);
      } catch (err) {
        console.warn(`Failed to delete database ${dbName}:`, err);
      }
    }
    
    // Also try to get all databases and delete them
    try {
      const dbs = await window.indexedDB.databases?.() || [];
      for (const db of dbs) {
        if (db.name && db.name.includes('openagents')) {
          await window.indexedDB.deleteDatabase(db.name);
        }
      }
    } catch (err) {
      console.warn('Could not clear remaining databases:', err);
    }
  }
  
  // Clear the list
  createdDatabases.length = 0;
}

/**
 * Creates a fallback in-memory database when persistent storage fails
 */
function createFallbackDatabase(): Database {
  // Create a simple in-memory mock of the database API
  const inMemoryDb: Database = {
    threads: createInMemoryCollection(),
    messages: createInMemoryCollection(),
    settings: createInMemoryCollection(),
  } as unknown as Database;
  
  dbInstance = inMemoryDb;
  return inMemoryDb;
}

/**
 * Creates an in-memory collection with basic functionality
 */
function createInMemoryCollection() {
  const items: Record<string, any> = {};
  
  return {
    find: () => ({
      exec: async () => Object.values(items),
      limit: () => ({ exec: async () => Object.values(items).slice(0, 10) })
    }),
    findOne: (id: string) => ({
      exec: async () => items[id] || null
    }),
    insert: async (doc: any) => {
      items[doc.id] = doc;
      return doc;
    },
    bulkInsert: async (docs: any[]) => {
      for (const doc of docs) {
        items[doc.id] = doc;
      }
      return docs;
    },
    update: async (query: any) => {
      const id = query.id || query._id;
      if (id && items[id]) {
        Object.assign(items[id], query);
      }
    },
    remove: async () => {
      // No-op for safety
    },
    // Add minimal query builder support
    where: () => ({
      equals: () => ({
        exec: async () => []
      })
    })
  };
}
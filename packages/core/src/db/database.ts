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

// Add global RxDB interface for type safety
declare global {
  interface Window {
    RxDB?: {
      plugin?: {
        disableWarnings?: () => void;
      };
    };
  }
}

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
    // Force close any active connections
    if (typeof indexedDB.databases === 'function') {
      const connections = await indexedDB.databases();
      if (connections && connections.length > 0) {
        console.log(`Found ${connections.length} existing IndexedDB connections to clean up`);
      }
    }

    // CRITICAL: In development, nuke ALL IndexedDB databases to ensure we start fresh
    // This is more aggressive but prevents collection limit issues
    if (process.env.NODE_ENV !== 'production') {
      return new Promise((resolve) => {
        // Get all database names (if available)
        let dbNames: string[] = [];
        
        if (typeof indexedDB.databases === 'function') {
          indexedDB.databases().then(dbs => {
            dbNames = dbs.map(db => db.name || '').filter(Boolean);
            deleteAllDatabases();
          }).catch(() => {
            // Fallback if databases() is unavailable
            deleteAllDatabases();
          });
        } else {
          // Fallback hardcoded pattern deletion
          dbNames = [];
          for (let i = 0; i < 20; i++) {
            dbNames.push(`rxdb-dexie-openagents_dev_${i}`);
            dbNames.push(`rxdb-dexie-openagents--${i}`);
            dbNames.push(`rxdb-dexie-openagents_dev_static--${i}`);
          }
          deleteAllDatabases();
        }

        function deleteAllDatabases() {
          // Handle empty database list with fallback patterns
          if (dbNames.length === 0) {
            const baseNames = ['rxdb-dexie-openagents', 'openagents_dev'];
            
            // Generate pattern-based database names (for RxDB)
            for (const baseName of baseNames) {
              dbNames.push(baseName);
              dbNames.push(`rxdb-dexie-${baseName}`);
              dbNames.push(`rxdb-dexie-${baseName}--0--_rxdb_internal`);
              dbNames.push(`rxdb-dexie-${baseName}--0--threads`);
              dbNames.push(`rxdb-dexie-${baseName}--0--messages`);
              dbNames.push(`rxdb-dexie-${baseName}--0--settings`);
            }
          }

          console.log(`Attempting to delete up to ${dbNames.length} IndexedDB databases`);
          
          // Use Promise.all to delete databases in parallel for speed
          const deletePromises = dbNames.map(name => {
            if (!name) return Promise.resolve();
            
            return new Promise<void>(resolve => {
              try {
                const request = indexedDB.deleteDatabase(name);
                request.onsuccess = () => resolve();
                request.onerror = () => resolve(); // Continue even on error
                request.onblocked = () => {
                  console.log(`Database ${name} deletion blocked, waiting...`);
                  // Wait and resolve anyway
                  setTimeout(resolve, 100);
                };
              } catch (err) {
                resolve(); // Continue even on error
              }
            });
          });
          
          // Wait for all deletions to complete
          Promise.all(deletePromises).then(() => {
            console.log('Database cleanup completed');
            // Give the browser a moment to actually finish the deletions
            setTimeout(resolve, 100);
          });
        }
      });
    }
    
    // In production, we're more careful - only delete databases we know about
    const dbNames: string[] = [];
    
    if (typeof window.indexedDB.databases === 'function') {
      const dbs = await window.indexedDB.databases() || [];
      
      for (const db of dbs) {
        if (db.name) {
          // Match only our specific databases
          if (db.name.includes('openagents')) {
            dbNames.push(db.name);
          }
        }
      }
      
      // Delete databases
      for (const dbName of dbNames) {
        try {
          await window.indexedDB.deleteDatabase(dbName);
        } catch (err) {
          // Suppress errors during cleanup
        }
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
    // Dev-mode warnings in RxDB cannot be completely suppressed in development mode
    // But we can at least try to reduce them
    
    // First, add dev-mode plugin (this will be a no-op if already added)
    // The dev-mode plugin must be added before other plugins
    try {
      // Method 1: Try dynamic import
      const devMode = await import('rxdb/plugins/dev-mode');
      
      // Add plugin only once
      if (devMode.RxDBDevModePlugin) {
        addRxPlugin(devMode.RxDBDevModePlugin);
      }
      
      // Call disableWarnings if it exists
      if (typeof devMode.disableWarnings === 'function') {
        devMode.disableWarnings();
        console.log('RxDB warnings disabled successfully');
        warningsDisabled = true;
      }
    } catch (importError) {
      console.warn('Could not import RxDB dev-mode plugin:', importError);
      
      // Method 2: Try using global RxDB if available
      if (
        typeof window !== 'undefined' && 
        window.RxDB && 
        window.RxDB.plugin && 
        window.RxDB.plugin.disableWarnings
      ) {
        window.RxDB.plugin.disableWarnings();
        console.log('RxDB warnings disabled using global RxDB');
        warningsDisabled = true;
      }
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
    
    // Always use unique names in development to prevent collection conflicts
    // In production, use a fixed name for persistence
    const uniqueId = Date.now().toString(36) + Math.random().toString(36).substring(2);
    const dbName = process.env.NODE_ENV === 'production' 
      ? PROD_DB_NAME 
      : `openagents_dev_${uniqueId}`;
    
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
    
    // If we encounter collection limit error, immediately use in-memory database
    // This is more reliable than trying to clean up and retry
    if (
      error && 
      typeof error === 'object' && 
      'code' in error && 
      error.code === 'COL23' 
    ) {
      console.warn('RxDB collection limit reached - switching to in-memory database for better reliability');
      
      // Run cleanup in the background (don't wait for it)
      setTimeout(() => {
        cleanupAllDatabases().catch(e => 
          console.warn('Background cleanup error:', e)
        );
      }, 100);
      
      // Return in-memory database immediately
      return createFallbackDatabase();
    }
    
    // Reset retry count
    retryAttempts = 0;
    
    throw error;
  }
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
    console.error('Error getting database, falling back to in-memory:', error);
    // Always return a database even if there's an error
    return createFallbackDatabase();
  }
}

/**
 * Check if we're using the in-memory fallback database
 */
export function isUsingFallbackDatabase(): boolean {
  return dbInstance?.storage?.name === 'in-memory-fallback';
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
  console.info('Creating in-memory fallback database for better reliability');
  
  // Create a simple in-memory mock of the database API
  const inMemoryDb: Database = {
    threads: createInMemoryCollection('threads'),
    messages: createInMemoryCollection('messages'),
    settings: createInMemoryCollection('settings'),
    
    // Add database-level methods
    remove: async () => {
      // No-op for in-memory DB
      console.log('In-memory database "remove" called (no-op)');
    },
    destroy: async () => {
      // No-op for in-memory DB
      console.log('In-memory database "destroy" called (no-op)');
    },
    
    // Indicate this is an in-memory database
    storage: {
      name: 'in-memory-fallback'
    } as any
  } as unknown as Database;
  
  // Store the in-memory DB as our singleton instance
  dbInstance = inMemoryDb;
  console.info('In-memory fallback database created successfully');
  return inMemoryDb;
}

/**
 * Creates an in-memory collection with basic functionality
 */
function createInMemoryCollection(collectionName: string) {
  const items: Record<string, any> = {};
  let listeners: Array<(docs: any[]) => void> = [];
  
  // For logging
  const logPrefix = `[InMemoryDB:${collectionName}]`;
  
  return {
    name: collectionName,
    
    // Find documents
    find: (selector?: any) => {
      return {
        sort: () => ({
          limit: () => ({
            exec: async () => {
              const values = Object.values(items);
              if (selector) {
                // Very basic selector implementation
                return values.filter(item => {
                  for (const key in selector) {
                    if (item[key] !== selector[key]) return false;
                  }
                  return true;
                }).slice(0, 10);
              }
              return values.slice(0, 10);
            }
          }),
          exec: async () => Object.values(items)
        }),
        limit: (limit = 10) => ({ 
          exec: async () => Object.values(items).slice(0, limit) 
        }),
        exec: async () => {
          if (selector) {
            return Object.values(items).filter(item => {
              for (const key in selector) {
                if (item[key] !== selector[key]) return false;
              }
              return true;
            });
          }
          return Object.values(items);
        }
      };
    },
    
    // Find one document by ID
    findOne: (idOrSelector: string | any) => ({
      exec: async () => {
        if (typeof idOrSelector === 'string') {
          return items[idOrSelector] || null;
        } else if (idOrSelector && typeof idOrSelector === 'object') {
          const values = Object.values(items);
          return values.find(item => {
            for (const key in idOrSelector) {
              if (item[key] !== idOrSelector[key]) return false;
            }
            return true;
          }) || null;
        }
        return null;
      }
    }),
    
    // Insert a document
    insert: async (doc: any) => {
      console.log(`${logPrefix} Inserting document:`, doc.id);
      items[doc.id] = { ...doc };
      notifyListeners();
      return doc;
    },
    
    // Insert multiple documents
    bulkInsert: async (docs: any[]) => {
      console.log(`${logPrefix} Bulk inserting ${docs.length} documents`);
      for (const doc of docs) {
        items[doc.id] = { ...doc };
      }
      notifyListeners();
      return docs;
    },
    
    // Update a document
    update: async (query: any) => {
      const id = query.id || query._id;
      if (id && items[id]) {
        console.log(`${logPrefix} Updating document:`, id);
        items[id] = { ...items[id], ...query };
        notifyListeners();
      }
    },
    
    // Remove documents
    remove: async (query?: any) => {
      if (!query) {
        console.log(`${logPrefix} Removing all documents`);
        Object.keys(items).forEach(key => delete items[key]);
      } else if (query.id) {
        console.log(`${logPrefix} Removing document:`, query.id);
        delete items[query.id];
      }
      notifyListeners();
    },
    
    // Query builder
    where: (field: string) => ({
      equals: (value: any) => ({
        exec: async () => {
          return Object.values(items).filter(item => item[field] === value);
        }
      }),
      $eq: (value: any) => ({
        exec: async () => {
          return Object.values(items).filter(item => item[field] === value);
        }
      })
    }),
    
    // Subscribe to changes
    $: {
      subscribe: (fn: (docs: any[]) => void) => {
        listeners.push(fn);
        
        // Return unsubscribe function
        return {
          unsubscribe: () => {
            listeners = listeners.filter(l => l !== fn);
          }
        };
      }
    }
  };
  
  // Notify all listeners of changes
  function notifyListeners() {
    const allDocs = Object.values(items);
    listeners.forEach(fn => fn(allDocs));
  }
}
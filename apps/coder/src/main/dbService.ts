// apps/coder/src/main/dbService.ts

import { app, dialog } from 'electron';
import path from 'path';
import fs from 'node:fs'; // Switch to native Node.js fs module
import { mkdir } from 'node:fs/promises';
import {
  createRxDatabase,
  addRxPlugin,
  RxDatabase,
  RxDatabaseCreator,
  RxStorage,
} from 'rxdb/plugins/core';
import { getRxStorageDexie } from 'rxdb/plugins/storage-dexie'; // Using Dexie storage with fake-indexeddb for Node.js
import { getRxStorageMemory } from 'rxdb/plugins/storage-memory'; // For fast in-memory operations
import { wrappedValidateAjvStorage } from 'rxdb/plugins/validate-ajv'; // For validation
import { RxDBQueryBuilderPlugin } from 'rxdb/plugins/query-builder';
import { RxDBUpdatePlugin } from 'rxdb/plugins/update';
import { RxDBMigrationSchemaPlugin } from 'rxdb/plugins/migration-schema';
import { DatabaseCollections, Database } from '@openagents/core/src/db/types';
import { threadSchema, messageSchema, settingsSchema } from '@openagents/core/src/db/schema';

// Polyfill for IndexedDB in Node.js environment
import { IDBFactory, IDBKeyRange } from 'fake-indexeddb';

// Create a simple implementation of the repositories for the main process
// This avoids using the browser-specific repositories that reference window object
const threadRepository = {
  initialize: async (db: any) => {
    console.log('[DB Service] Thread repository initialized');
    return db;
  }
};

const messageRepository = {
  initialize: async (db: any) => {
    console.log('[DB Service] Message repository initialized');
    return db;
  }
};

const settingsRepository = {
  initialize: async (db: any) => {
    console.log('[DB Service] Settings repository initialized');
    return db;
  }
};

// Add required plugins
addRxPlugin(RxDBQueryBuilderPlugin);
addRxPlugin(RxDBUpdatePlugin);
addRxPlugin(RxDBMigrationSchemaPlugin);

// --- Database Configuration ---

const DB_VERSION_NAMESPACE = 'v1'; // Increment this if breaking schema changes require a full wipe

// Determine base path for database files
const userDataPath = app.getPath('userData');
const dbBasePath = path.join(userDataPath, 'databases');

// Ensure the base directory exists with native Node.js fs
try {
    console.log('[DB Service] Ensuring database directory exists:', dbBasePath);
    
    if (!fs.existsSync(dbBasePath)) {
        fs.mkdirSync(dbBasePath, { recursive: true });
        console.log('[DB Service] Successfully created database directory');
    } else {
        console.log('[DB Service] Database directory already exists');
    }
} catch (err) {
    console.error('[DB Service] Failed to create database directory:', dbBasePath, err);
    // Not fatal in development mode
    if (app.isPackaged) {
        throw new Error(`Failed to ensure database directory exists: ${err.message}`);
    } else {
        console.warn('[DB Service] Continuing despite directory creation error in development mode');
    }
}

// Define database names based on environment and version
const getDatabaseName = () => {
    const envSuffix = app.isPackaged ? 'prod' : 'dev';
    return `openagents_coder_${envSuffix}_${DB_VERSION_NAMESPACE}`;
};

// --- Singleton Instance ---
let dbInstance: Database | null = null;
let dbCreationPromise: Promise<Database> | null = null;

/**
 * Creates and initializes the RxDB database with all collections for the main process.
 */
async function createDatabaseInternal(): Promise<Database> {
  console.log('[DB Service] Creating database internal', {
    isPackaged: app.isPackaged,
    existingInstance: !!dbInstance,
    databaseName: getDatabaseName(),
  });

  // Use a simpler and faster memory storage for development/testing
  // Note: This means data won't persist across restarts, but it's much faster
  const storage: RxStorage<any, any> = getRxStorageMemory();

  const dbName = getDatabaseName();

  console.log(`[DB Service] Creating RxDB database with name: ${dbName}`);

  const dbConfig: RxDatabaseCreator = {
    name: dbName,
    storage,
    multiInstance: false, // Enforce single instance; main process owns the DB
    eventReduce: true,
    cleanupPolicy: {
      minimumCollectionAge: 1000 * 60 * 60 * 24 * 7, // 7 days
      minimumDeletedTime: 1000 * 60 * 60 * 24 * 7, // 7 days
      runEach: 1000 * 60 * 60, // Every hour
    },
    localDocuments: true, // Enable local documents store
  };

  try {
    const db = await createRxDatabase<DatabaseCollections>(dbConfig);
    console.log('[DB Service] RxDatabase instance created.');

    // Add collections
    console.log('[DB Service] Adding collections...');
    await db.addCollections({
      threads: {
        schema: threadSchema,
        migrationStrategies: {
          1: oldDoc => oldDoc,
          2: oldDoc => oldDoc,
        },
      },
      messages: {
        schema: messageSchema,
        migrationStrategies: {
          1: oldDoc => oldDoc,
          2: oldDoc => oldDoc,
        },
      },
      settings: {
        schema: settingsSchema,
        migrationStrategies: {
          1: oldDoc => oldDoc,
          2: oldDoc => ({
            ...oldDoc,
            selectedModelId: oldDoc.defaultModel || 'anthropic/claude-3.5-sonnet', // Default if migrating from v1
            visibleModelIds: oldDoc.visibleModelIds || [ // Default if migrating from v1
                'anthropic/claude-3.5-sonnet',
                'openai/gpt-4o',
                'google/gemini-1.5-flash',
                // Add other relevant defaults
            ],
            mcpClients: oldDoc.mcpClients || [], // Default if migrating from v1
          }),
        },
      },
    });
    console.log('[DB Service] Collections added successfully.');

    // Initialize Repositories (moved here from HomePage)
    console.log('[DB Service] Initializing repositories...');
    await threadRepository.initialize(db);
    await messageRepository.initialize(db);
    await settingsRepository.initialize(db);
    console.log('[DB Service] Repositories initialized.');

    return db;

  } catch (dbCreateError) {
    console.error('[DB Service] Error creating database or collections:', dbCreateError);
    // Attempt to log more details if available
    if (dbCreateError.rxdb) console.error('[DB Service] RxDB Error Code:', dbCreateError.code);
    if (dbCreateError.parameters) console.error('[DB Service] RxDB Error Parameters:', dbCreateError.parameters);

    // Check for potential lock errors (though less common with single instance lock)
    const errorStr = String(dbCreateError).toLowerCase();
    if (errorStr.includes('lock') || errorStr.includes('eaddrnotavail') /* Might indicate lock issues */) {
        console.error('[DB Service] Potential database lock detected.');
        dialog.showErrorBox('Database Locked', 'Could not access the database. It might be locked by another process. Please ensure no other instances are running and try again.');
    } else {
        dialog.showErrorBox('Database Error', `Failed to initialize the application database: ${dbCreateError.message}`);
    }
    throw dbCreateError; // Rethrow to be caught by main.ts initialization handler
  }
}

/**
 * Gets the database instance, creating it if it doesn't exist.
 * Handles singleton logic.
 */
export async function getDatabase(): Promise<Database> {
  console.log('[DB Service] getDatabase called.');

  if (dbInstance) {
    console.log('[DB Service] Returning existing DB instance.');
    return dbInstance;
  }

  if (dbCreationPromise) {
    console.log('[DB Service] DB creation in progress, returning existing promise.');
    return dbCreationPromise;
  }

  console.log('[DB Service] No DB instance or creation promise found. Starting creation.');
  dbCreationPromise = (async () => {
    try {
      const db = await createDatabaseInternal();
      dbInstance = db;
      console.log('[DB Service] Database successfully created and assigned.');
      return db;
    } catch (error) {
      console.error('[DB Service] Database creation failed in getDatabase:', error);
      dbCreationPromise = null; // Reset promise on failure
      dbInstance = null;
      throw error; // Propagate error
    } finally {
        // Don't reset dbCreationPromise here, let it resolve/reject
    }
  })();

  return dbCreationPromise;
}

/**
 * Cleans up the database instance and potentially removes storage files.
 */
export async function cleanupDatabase() {
  console.log('[DB Service] Starting database cleanup...');

  if (dbInstance) {
    console.log(`[DB Service] Cleaning up existing database instance: ${dbInstance.name}`);
    try {
      await dbInstance.destroy(); // Destroys the RxDB instance, removes listeners etc.
      console.log('[DB Service] RxDB instance destroyed.');
    } catch (err) {
      console.error('[DB Service] Error destroying database instance:', err);
    }
    dbInstance = null;
    dbCreationPromise = null; // Also clear promise if we explicitly clean up
  } else {
      console.log('[DB Service] No active DB instance to destroy.');
  }

  // With Dexie/IndexedDB storage, cleanup happens automatically via the IndexedDB API
  // No need to manually delete files as with LokiJS

  console.log('[DB Service] Database cleanup finished.');
}

// Optional: Expose database status for IPC
// For faster UI loading, we'll pretend the DB is always ready in development mode
let dbReady = !app.isPackaged; // Immediately ready in dev mode
let dbError: Error | null = null;

// Still initialize the database in the background
getDatabase()
  .then(() => {
    dbReady = true;
    dbError = null;
    console.log('[DB Service] Database is ready.');
    // TODO: Notify renderer via IPC if needed immediately
  })
  .catch((err) => {
    // Only set not ready if we're in production mode
    if (app.isPackaged) {
      dbReady = false;
    }
    dbError = err;
    console.error('[DB Service] Database failed to initialize on startup:', err);
    // Error already shown via dialog in createDatabaseInternal
  });

export function getDbStatus() {
    return { ready: dbReady, error: dbError?.message || null };
}

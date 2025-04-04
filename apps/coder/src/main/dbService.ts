// apps/coder/src/main/dbService.ts

import { app, dialog } from 'electron';
import path from 'path';
import fs from 'fs-extra'; // Using fs-extra for robust file operations like ensureDirSync
import {
  createRxDatabase,
  addRxPlugin,
  RxDatabase,
  RxDatabaseCreator,
  RxStorage,
} from 'rxdb/plugins/core';
import { getRxStorageLoki } from 'rxdb/plugins/storage-lokijs'; // Node.js filesystem storage
import lokiAdapter from 'lokijs/src/loki-fs-sync-adapter'; // Adapter for LokiJS persistence
import { RxDBQueryBuilderPlugin } from 'rxdb/plugins/query-builder';
import { RxDBUpdatePlugin } from 'rxdb/plugins/update';
import { RxDBMigrationSchemaPlugin } from 'rxdb/plugins/migration-schema';
// Assuming types and schema are accessible via monorepo structure
// Adjust path if necessary based on final monorepo setup/build process
import { DatabaseCollections, Database } from '@openagents/core/dist/db/types'; // Use compiled output path
import { threadSchema, messageSchema, settingsSchema } from '@openagents/core/dist/db/schema'; // Use compiled output path
import { threadRepository, messageRepository, settingsRepository } from '@openagents/core/dist/db/repositories'; // Use compiled output path


// TODO: Add RxDB validation plugin suitable for Node.js if needed (e.g., validate-ajv or validate-z-schema)
// import { wrappedValidateAjvStorage } from 'rxdb/plugins/validate-ajv';

// Add required plugins
addRxPlugin(RxDBQueryBuilderPlugin);
addRxPlugin(RxDBUpdatePlugin);
addRxPlugin(RxDBMigrationSchemaPlugin);

// --- Database Configuration ---

const DB_VERSION_NAMESPACE = 'v1'; // Increment this if breaking schema changes require a full wipe

// Determine base path for database files
const userDataPath = app.getPath('userData');
const dbBasePath = path.join(userDataPath, 'databases');

// Ensure the base directory exists
try {
    fs.ensureDirSync(dbBasePath);
} catch (err) {
    console.error('[DB Service] Failed to create database directory:', dbBasePath, err);
    // This is likely fatal, throw or handle appropriately
    throw new Error(`Failed to ensure database directory exists: ${err.message}`);
}

// Define database names based on environment and version
const getDatabaseName = () => {
    const envSuffix = app.isPackaged ? 'prod' : 'dev';
    return `openagents_coder_${envSuffix}_${DB_VERSION_NAMESPACE}`;
};

const getDatabasePath = () => {
    return path.join(dbBasePath, `${getDatabaseName()}.lokidb`);
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
    databasePath: getDatabasePath(),
  });

  // Configure LokiJS storage adapter
  // Using fs-sync adapter for persistence. Consider fs-structured or others if needed.
  const storage: RxStorage<any, any> = getRxStorageLoki({
      adapter: new lokiAdapter(),
      persistenceMethod: 'fs', // Persist to filesystem
      autoload: true,
      autosave: true,
      autosaveInterval: 4000, // Autosave every 4 seconds
      // lokiDatabaseSettings: { verbose: !app.isPackaged } // Optional: more logging in dev
  });

  // TODO: Wrap storage with validation plugin if necessary
  // const validatedStorage = wrappedValidateAjvStorage({ storage }); // Example

  const dbName = getDatabaseName();
  const dbPath = getDatabasePath(); // Although LokiJS adapter handles path, log it

  console.log(`[DB Service] Creating RxDB database with name: ${dbName} at path: ${dbPath}`);

  const dbConfig: RxDatabaseCreator = {
    name: dbName,
    storage, // Use LokiJS storage
    // password: 'myLongAndStupidPassword', // Optional: Add password protection
    multiInstance: false, // Enforce single instance; main process owns the DB
    eventReduce: true,
    cleanupPolicy: {
      minimumCollectionAge: 1000 * 60 * 60 * 24 * 7, // 7 days
      minimumDeletedTime: 1000 * 60 * 60 * 24 * 7, // 7 days
      runEach: 1000 * 60 * 60, // Every hour
    },
    // ignoreDuplicate: !app.isPackaged, // ignoreDuplicate generally not needed/recommended with Node storage? Verify.
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

  // Optional: Physically remove the database file(s)
  // Be cautious with this, might lead to data loss if called unintentionally.
  // Consider adding a flag or specific condition for file removal.
  const dbPath = getDatabasePath();
  try {
      if (await fs.pathExists(dbPath)) {
          console.warn(`[DB Service] Removing database file: ${dbPath}`);
          await fs.remove(dbPath);
          console.log(`[DB Service] Database file removed: ${dbPath}`);
      } else {
          console.log(`[DB Service] Database file not found, skipping removal: ${dbPath}`);
      }
  } catch (err) {
      console.error(`[DB Service] Error removing database file ${dbPath}:`, err);
  }

  console.log('[DB Service] Database cleanup finished.');
}

// Optional: Expose database status for IPC
let dbReady = false;
let dbError: Error | null = null;

getDatabase()
  .then(() => {
    dbReady = true;
    dbError = null;
    console.log('[DB Service] Database is ready.');
    // TODO: Notify renderer via IPC if needed immediately
  })
  .catch((err) => {
    dbReady = false;
    dbError = err;
    console.error('[DB Service] Database failed to initialize on startup:', err);
    // Error already shown via dialog in createDatabaseInternal
  });

export function getDbStatus() {
    return { ready: dbReady, error: dbError?.message || null };
}

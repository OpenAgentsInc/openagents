import { createRxDatabase, addRxPlugin } from 'rxdb';
import { getRxStorageDexie } from 'rxdb/plugins/storage-dexie';
import { RxDBQueryBuilderPlugin } from 'rxdb/plugins/query-builder';

import { threadSchema, messageSchema, settingsSchema } from './schema';
import { Database } from './types';

// Add required plugins
addRxPlugin(RxDBQueryBuilderPlugin);

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

  console.log('Creating RxDB database...');

  // Create the database
  const db = await createRxDatabase<Database>({
    name: 'openagents',
    storage: getRxStorageDexie()
  });

  // Add collections
  await db.addCollections({
    threads: {
      schema: threadSchema,
      migrationStrategies: {
        // Add migration strategies for future schema versions
        // 1: (oldDoc) => { ... }
      }
    },
    messages: {
      schema: messageSchema,
      migrationStrategies: {
        // Add migration strategies for future schema versions
        // 1: (oldDoc) => { ... }
      }
    },
    settings: {
      schema: settingsSchema,
      migrationStrategies: {
        // Add migration strategies for future schema versions
        // 1: (oldDoc) => { ... }
      }
    }
  });

  console.log('RxDB database created successfully');

  // Store instance
  dbInstance = db;

  return db;
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
 * Closes the database connection
 */
export async function closeDatabase(): Promise<void> {
  if (dbInstance) {
    await dbInstance.destroy();
    dbInstance = null;
    console.log('Database connection closed');
  }
}

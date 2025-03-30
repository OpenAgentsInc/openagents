import { createRxDatabase, addRxPlugin } from 'rxdb';
import { RxDBQueryBuilderPlugin } from 'rxdb';
import { RxDBMigrationPlugin } from 'rxdb';
import { wrappedValidateAjvStorage } from 'rxdb';
import { RxDBUpdatePlugin } from 'rxdb';
import Dexie from 'dexie';

import { threadSchema, messageSchema, settingsSchema } from './schema';
import { Database } from './types';

// Add required plugins
addRxPlugin(RxDBQueryBuilderPlugin);
addRxPlugin(RxDBMigrationPlugin);
addRxPlugin(RxDBUpdatePlugin);

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
  
  // Create a simple storage adapter using Dexie
  const dexieStorage = {
    name: 'dexie',
    async createDb(name: string) {
      const db = new Dexie(name);
      return db;
    }
  };
  
  // Create the database
  const db = await createRxDatabase<Database>({
    name: 'openagents',
    storage: wrappedValidateAjvStorage({
      storage: dexieStorage as any
    })
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
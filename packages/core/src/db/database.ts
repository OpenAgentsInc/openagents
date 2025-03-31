import {
  RxCollection,
  createRxDatabase,
  defaultHashSha256,
  addRxPlugin,
  randomToken,
  RxDocument,
  RxJsonSchema,
  deepEqual,
  RxConflictHandler,
  RXDB_VERSION,
  RxStorage,
  RxDatabase,
  RxDatabaseBase
} from 'rxdb/plugins/core';
import { replicateWebRTC, getConnectionHandlerSimplePeer, SimplePeer } from 'rxdb/plugins/replication-webrtc';
import { getRxStorageDexie } from 'rxdb/plugins/storage-dexie';

// Types for our Todo collection
export type TodoDocType = {
  id: string;
  name: string;
  state: 'open' | 'done';
  lastChange: number;
}
export type RxTodoDocument = RxDocument<TodoDocType>;

// Database type definition
export type DatabaseCollections = {
  todos: RxCollection<TodoDocType>;
};
export type Database = RxDatabase<DatabaseCollections, any, any>;

// Initialize storage
let storage: RxStorage<any, any> = getRxStorageDexie();

// Database instance (singleton)
let dbInstance: Database | null = null;

// Conflict handler for replication
const conflictHandler: RxConflictHandler<TodoDocType> = {
  isEqual(a, b) {
    return deepEqual(a, b);
  },
  resolve(input) {
    const ret = input.newDocumentState.lastChange > input.realMasterState.lastChange
      ? input.newDocumentState
      : input.realMasterState;
    return Promise.resolve(ret);
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

  console.log('Creating RxDB database...');

  try {
    // Import dev mode plugins in development
    if (process.env.NODE_ENV === 'development') {
      const devModeModule = await import('rxdb/plugins/dev-mode');
      addRxPlugin(devModeModule.RxDBDevModePlugin);

      const validateModule = await import('rxdb/plugins/validate-ajv');
      storage = validateModule.wrappedValidateAjvStorage({ storage });
    }

    // Generate room ID if not exists
    const roomId = window.location.hash;
    if (!roomId || roomId.length < 5) {
      window.location.hash = 'room-' + randomToken(10);
      window.location.reload();
    }

    const roomHash = await defaultHashSha256(roomId);

    // Create database
    const db = await createRxDatabase<{
      todos: RxCollection<TodoDocType, {}>
    }>({
      name: 'openagents-' + RXDB_VERSION.replace(/\./g, '-') + '-' + roomHash.substring(0, 10),
      storage,
      ignoreDuplicate: true,
      options: {
        validatorMaxLength: 10000
      }
    });

    // Add collections
    await db.addCollections({
      todos: {
        schema: {
          version: 0,
          primaryKey: 'id',
          type: 'object',
          properties: {
            id: {
              type: 'string',
              maxLength: 20
            },
            name: {
              type: 'string'
            },
            state: {
              type: 'string',
              enum: [
                'open',
                'done'
              ],
              maxLength: 10
            },
            lastChange: {
              type: 'integer',
              minimum: 0,
              maximum: 2701307494132,
              multipleOf: 1
            }
          },
          required: ['id', 'name', 'state', 'lastChange'],
          indexes: [
            'state',
            ['state', 'lastChange']
          ],
          additionalProperties: false
        } as RxJsonSchema<TodoDocType>,
        conflictHandler
      }
    });

    // Add pre-save hook to update lastChange
    db.todos.preSave(d => {
      d.lastChange = Date.now();
      return d;
    }, true);

    // Insert initial todos
    await db.todos.bulkInsert(
      [
        'First todo item',
        'Second todo item',
        'Third todo item'
      ].map((name, idx) => ({
        id: 'todo-' + idx,
        name,
        lastChange: 0,
        state: 'open'
      }))
    );

    // Setup WebRTC replication
    replicateWebRTC<TodoDocType, SimplePeer>({
      collection: db.todos,
      connectionHandlerCreator: getConnectionHandlerSimplePeer({}),
      topic: roomHash.substring(0, 10),
      pull: {},
      push: {},
    }).then(replicationState => {
      replicationState.error$.subscribe((err: any) => {
        console.log('replication error:');
        console.dir(err);
      });
      replicationState.peerStates$.subscribe(s => {
        console.log('new peer states:');
        console.dir(s);
      });
    });

    console.log('RxDB database created successfully');
    dbInstance = db;
    return db;

  } catch (error) {
    console.error('Failed to create RxDB database:', error);
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
    await (dbInstance as any).destroy();
    dbInstance = null;
  }
}

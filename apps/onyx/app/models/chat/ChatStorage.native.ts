import * as SQLite from "expo-sqlite"

let db: SQLite.SQLiteDatabase;

const initDB = async () => {
  if (!db) {
    db = await SQLite.openDatabaseAsync('onyx.db');
    await db.execAsync('PRAGMA journal_mode = WAL;'); // Enable WAL mode for better performance
  }
  return db;
};

export const initializeDatabase = async () => {
  const database = await initDB();
  await database.execAsync(
    'CREATE TABLE IF NOT EXISTS chats (id TEXT PRIMARY KEY NOT NULL, messages TEXT);'
  );
};

export const loadChat = async (chatId: string) => {
  const database = await initDB();
  const result = await database.getFirstAsync<{ messages: string }>(
    'SELECT messages FROM chats WHERE id = ?;',
    [chatId]
  );
  return result?.messages || '[]'; // Return empty array if no chat found
};

export const saveChat = async (chatId: string, messages: string) => {
  const database = await initDB();
  await database.runAsync(
    'INSERT OR REPLACE INTO chats (id, messages) values (?, ?);',
    [chatId, messages]
  );
};

export const getAllChats = async () => {
  const database = await initDB();
  const result = await database.getAllAsync<{ id: string, messages: string }>(
    'SELECT id, messages FROM chats ORDER BY id DESC;'
  );
  return result.map(chat => ({
    id: chat.id,
    messages: JSON.parse(chat.messages)
  }));
};
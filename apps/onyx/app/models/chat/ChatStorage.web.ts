const DB_NAME = 'onyx-web'
const STORE_NAME = 'chats'
const DB_VERSION = 1

let db: IDBDatabase | null = null

const initDB = (): Promise<IDBDatabase> => {
  return new Promise((resolve, reject) => {
    if (db) {
      resolve(db)
      return
    }

    const request = indexedDB.open(DB_NAME, DB_VERSION)

    request.onerror = () => {
      reject(request.error)
    }

    request.onsuccess = () => {
      db = request.result
      resolve(db)
    }

    request.onupgradeneeded = (event) => {
      const database = (event.target as IDBOpenDBRequest).result
      if (!database.objectStoreNames.contains(STORE_NAME)) {
        database.createObjectStore(STORE_NAME, { keyPath: 'id' })
      }
    }
  })
}

export const initializeDatabase = async () => {
  await initDB()
}

export const loadChat = async (chatId: string): Promise<string> => {
  const database = await initDB()
  return new Promise((resolve, reject) => {
    const transaction = database.transaction([STORE_NAME], 'readonly')
    const store = transaction.objectStore(STORE_NAME)
    const request = store.get(chatId)

    request.onerror = () => reject(request.error)
    request.onsuccess = () => {
      resolve(request.result?.messages || '[]')
    }
  })
}

export const saveChat = async (chatId: string, messages: string) => {
  const database = await initDB()
  return new Promise<void>((resolve, reject) => {
    const transaction = database.transaction([STORE_NAME], 'readwrite')
    const store = transaction.objectStore(STORE_NAME)
    const request = store.put({ id: chatId, messages })

    request.onerror = () => reject(request.error)
    request.onsuccess = () => resolve()
  })
}

export const getAllChats = async () => {
  const database = await initDB()
  return new Promise<Array<{ id: string; messages: any[] }>>((resolve, reject) => {
    const transaction = database.transaction([STORE_NAME], 'readonly')
    const store = transaction.objectStore(STORE_NAME)
    const request = store.getAll()

    request.onerror = () => reject(request.error)
    request.onsuccess = () => {
      const chats = request.result.map(chat => ({
        id: chat.id,
        messages: JSON.parse(chat.messages)
      }))
      resolve(chats)
    }
  })
}
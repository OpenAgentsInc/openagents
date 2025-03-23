import * as bip39 from "bip39"

const DB_NAME = 'onyx-secure-storage'
const STORE_NAME = 'keystore'
const DB_VERSION = 1
const MNEMONIC_KEY = "onyx_mnemonic_v1"

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
        database.createObjectStore(STORE_NAME)
      }
    }
  })
}

const getItem = async (key: string): Promise<string | null> => {
  const database = await initDB()
  return new Promise((resolve, reject) => {
    const transaction = database.transaction([STORE_NAME], 'readonly')
    const store = transaction.objectStore(STORE_NAME)
    const request = store.get(key)

    request.onerror = () => reject(request.error)
    request.onsuccess = () => resolve(request.result)
  })
}

const setItem = async (key: string, value: string): Promise<void> => {
  const database = await initDB()
  return new Promise((resolve, reject) => {
    const transaction = database.transaction([STORE_NAME], 'readwrite')
    const store = transaction.objectStore(STORE_NAME)
    const request = store.put(value, key)

    request.onerror = () => reject(request.error)
    request.onsuccess = () => resolve()
  })
}

const deleteItem = async (key: string): Promise<void> => {
  const database = await initDB()
  return new Promise((resolve, reject) => {
    const transaction = database.transaction([STORE_NAME], 'readwrite')
    const store = transaction.objectStore(STORE_NAME)
    const request = store.delete(key)

    request.onerror = () => reject(request.error)
    request.onsuccess = () => resolve()
  })
}

export class SecureStorageService {
  static async getMnemonic(): Promise<string | null> {
    try {
      return await getItem(MNEMONIC_KEY)
    } catch (error) {
      console.error("Error getting mnemonic from secure storage:", error)
      return null
    }
  }

  static async setMnemonic(mnemonic: string): Promise<boolean> {
    try {
      if (!bip39.validateMnemonic(mnemonic)) {
        throw new Error("Invalid mnemonic")
      }
      await setItem(MNEMONIC_KEY, mnemonic)
      return true
    } catch (error) {
      console.error("Error saving mnemonic to secure storage:", error)
      return false
    }
  }

  static async generateMnemonic(): Promise<string> {
    const mnemonic = bip39.generateMnemonic()
    await setItem(MNEMONIC_KEY, mnemonic)
    return mnemonic
  }

  static async deleteMnemonic(): Promise<void> {
    try {
      await deleteItem(MNEMONIC_KEY)
    } catch (error) {
      console.error("Error deleting mnemonic from secure storage:", error)
    }
  }
}
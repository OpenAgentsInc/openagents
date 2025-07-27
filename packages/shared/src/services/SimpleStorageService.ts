import { Effect, Data } from "effect";

// Tagged error types for storage operations
export class StorageError extends Data.TaggedError("StorageError")<{
  operation: string;
  key: string;
  message: string;
  cause?: unknown;
}> {}

export class StorageNotFoundError extends Data.TaggedError("StorageNotFoundError")<{
  key: string;
}> {}

// Simple storage functions without complex service patterns

// Browser localStorage functions
export const getFromLocalStorage = (key: string) =>
  Effect.try({
    try: () => {
      const value = localStorage.getItem(key);
      if (value === null) {
        throw new StorageNotFoundError({ key });
      }
      return value;
    },
    catch: (error) => {
      if (error instanceof StorageNotFoundError) {
        throw error;
      }
      throw new StorageError({
        operation: "get",
        key,
        message: String(error),
        cause: error
      });
    }
  });

export const setInLocalStorage = (key: string, value: string) =>
  Effect.try({
    try: () => localStorage.setItem(key, value),
    catch: (error) => new StorageError({
      operation: "set",
      key,
      message: String(error),
      cause: error
    })
  });

export const removeFromLocalStorage = (key: string) =>
  Effect.try({
    try: () => localStorage.removeItem(key),
    catch: (error) => new StorageError({
      operation: "remove",
      key,
      message: String(error),
      cause: error
    })
  });

// React Native SecureStore functions
export const getFromSecureStore = (key: string) =>
  Effect.tryPromise({
    try: async () => {
      const { getItemAsync } = await import('expo-secure-store');
      const value = await getItemAsync(key);
      if (value === null) {
        throw new StorageNotFoundError({ key });
      }
      return value;
    },
    catch: (error) => {
      if (error instanceof StorageNotFoundError) {
        throw error;
      }
      throw new StorageError({
        operation: "get",
        key,
        message: String(error),
        cause: error
      });
    }
  });

export const setInSecureStore = (key: string, value: string) =>
  Effect.tryPromise({
    try: async () => {
      const { setItemAsync } = await import('expo-secure-store');
      await setItemAsync(key, value);
    },
    catch: (error) => new StorageError({
      operation: "set",
      key,
      message: String(error),
      cause: error
    })
  });

export const removeFromSecureStore = (key: string) =>
  Effect.tryPromise({
    try: async () => {
      const { deleteItemAsync } = await import('expo-secure-store');
      await deleteItemAsync(key);
    },
    catch: (error) => new StorageError({
      operation: "remove",
      key,
      message: String(error),
      cause: error
    })
  });

// Platform detection and appropriate function selection
export const isReactNative = () => 
  typeof window !== 'undefined' && window.navigator?.product === 'ReactNative';

export const getStorageValue = (key: string) =>
  isReactNative() ? getFromSecureStore(key) : getFromLocalStorage(key);

export const setStorageValue = (key: string, value: string) =>
  isReactNative() ? setInSecureStore(key, value) : setInLocalStorage(key, value);

export const removeStorageValue = (key: string) =>
  isReactNative() ? removeFromSecureStore(key) : removeFromLocalStorage(key);

// Helper functions for JSON storage
export const getStoredJson = <T>(key: string, defaultValue: T) =>
  getStorageValue(key).pipe(
    Effect.map(value => JSON.parse(value) as T),
    Effect.catchAll(_ => Effect.succeed(defaultValue))
  );

export const setStoredJson = <T>(key: string, value: T) =>
  setStorageValue(key, JSON.stringify(value));

export const removeIfExists = (key: string) =>
  Effect.gen(function* () {
    try {
      yield* removeStorageValue(key);
    } catch {
      // Ignore errors if key doesn't exist
    }
  });
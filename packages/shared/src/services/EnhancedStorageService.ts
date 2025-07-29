import { Effect, Data, Schema, Layer } from "effect";
import { PlatformService, PlatformError, UnsupportedPlatformError } from "./PlatformService";
import { ConfigService } from "./ConfigService";

/**
 * EnhancedStorageService - Cross-platform storage with Effect patterns
 * 
 * Following EffectPatterns best practices:
 * - Service composition with PlatformService and ConfigService
 * - Schema validation for stored data
 * - Tagged errors for precise error handling
 * - Cross-platform abstraction (localStorage/SecureStore)
 * - Dependency injection ready
 * - Comprehensive testing support
 */

// Tagged error types for storage operations
export class StorageError extends Data.TaggedError("StorageError")<{
  operation: string;
  key: string;
  message: string;
  cause?: unknown;
}> {}

export class StorageNotFoundError extends Data.TaggedError("StorageNotFoundError")<{
  key: string;
  storageType: "localStorage" | "secureStore";
}> {}

export class StorageValidationError extends Data.TaggedError("StorageValidationError")<{
  key: string;
  schema: string;
  value: unknown;
  errors: unknown[];
}> {}

export class StorageQuotaExceededError extends Data.TaggedError("StorageQuotaExceededError")<{
  key: string;
  size: number;
  quota: number;
}> {}

// Storage configuration schema
export const StorageConfigSchema = Schema.Struct({
  maxKeyLength: Schema.Number.pipe(Schema.positive()),
  maxValueSize: Schema.Number.pipe(Schema.positive()),
  enableEncryption: Schema.Boolean,
  compressionThreshold: Schema.Number.pipe(Schema.positive()),
  ttlEnabled: Schema.Boolean,
  defaultTtl: Schema.Number.pipe(Schema.positive())
});

export type StorageConfig = Schema.Schema.Type<typeof StorageConfigSchema>;

// Storage metadata schema
export const StorageMetadataSchema = Schema.Struct({
  createdAt: Schema.Number,
  updatedAt: Schema.Number,
  ttl: Schema.optional(Schema.Number),
  compressed: Schema.Boolean,
  encrypted: Schema.Boolean,
  size: Schema.Number
});

export type StorageMetadata = Schema.Schema.Type<typeof StorageMetadataSchema>;

// Storage entry schema (for complex stored values)
export const StorageEntrySchema = <T>(valueSchema: Schema.Schema<T>) =>
  Schema.Struct({
    value: valueSchema,
    metadata: StorageMetadataSchema
  });

export type StorageEntry<T> = {
  value: T;
  metadata: StorageMetadata;
};

// Default storage configuration
const DEFAULT_STORAGE_CONFIG: StorageConfig = {
  maxKeyLength: 256,
  maxValueSize: 10 * 1024 * 1024, // 10MB
  enableEncryption: false,
  compressionThreshold: 1024, // 1KB
  ttlEnabled: true,
  defaultTtl: 7 * 24 * 60 * 60 * 1000 // 7 days
};

/**
 * EnhancedStorageService using Effect.Service pattern
 * 
 * This follows the "model-dependencies-as-services" pattern from EffectPatterns,
 * composing with PlatformService for cross-platform functionality.
 */
export class EnhancedStorageService extends Effect.Service<EnhancedStorageService>()(
  "EnhancedStorageService",
  {
    dependencies: [PlatformService.Default, ConfigService.Default],
    effect: Effect.gen(function* () {
      const platform = yield* PlatformService;
      const config = yield* ConfigService;
      const platformInfo = yield* platform.getPlatformInfo();
      
      // Get storage configuration
      const storageConfig = yield* config.getPath("storage", DEFAULT_STORAGE_CONFIG);
      
      // Platform-specific storage implementations
      const webStorage = {
        get: (key: string) => Effect.try({
          try: () => {
            const value = localStorage.getItem(key);
            if (value === null) {
              throw new StorageNotFoundError({ key, storageType: "localStorage" });
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
        }),

        set: (key: string, value: string) => Effect.try({
          try: () => {
            // Check quota before setting
            const estimatedSize = new Blob([value]).size;
            if (estimatedSize > storageConfig.maxValueSize) {
              throw new StorageQuotaExceededError({
                key,
                size: estimatedSize,
                quota: storageConfig.maxValueSize
              });
            }
            
            localStorage.setItem(key, value);
          },
          catch: (error) => {
            if (error instanceof StorageQuotaExceededError) {
              throw error;
            }
            
            // Handle quota exceeded error from localStorage
            if (error instanceof DOMException && error.name === "QuotaExceededError") {
              throw new StorageQuotaExceededError({
                key,
                size: new Blob([value]).size,
                quota: 0 // Unknown quota
              });
            }
            
            throw new StorageError({
              operation: "set",
              key,
              message: String(error),
              cause: error
            });
          }
        }),

        remove: (key: string) => Effect.try({
          try: () => localStorage.removeItem(key),
          catch: (error) => new StorageError({
            operation: "remove",
            key,
            message: String(error),
            cause: error
          })
        }),

        keys: () => Effect.try({
          try: () => Object.keys(localStorage),
          catch: (error) => new StorageError({
            operation: "keys",
            key: "",
            message: String(error),
            cause: error
          })
        }),

        clear: () => Effect.try({
          try: () => localStorage.clear(),
          catch: (error) => new StorageError({
            operation: "clear",
            key: "",
            message: String(error),
            cause: error
          })
        })
      };

      const secureStorage = {
        get: (key: string) => Effect.tryPromise({
          try: async () => {
            // Only attempt expo-secure-store import on mobile platforms
            if (platformInfo.type !== "mobile") {
              throw new StorageError({
                operation: "get",
                key,
                message: "Secure storage not available on this platform",
                cause: new Error(`Platform ${platformInfo.type} does not support secure storage`)
              });
            }
            
            try {
              const { getItemAsync } = await import('expo-secure-store');
              const value = await getItemAsync(key);
              if (value === null) {
                throw new StorageNotFoundError({ key, storageType: "secureStore" });
              }
              return value;
            } catch (importError) {
              // Handle import failures gracefully
              throw new StorageError({
                operation: "get",
                key,
                message: "Failed to import expo-secure-store. Ensure you're running on a mobile platform with Expo.",
                cause: importError
              });
            }
          },
          catch: (error) => {
            if (error instanceof StorageNotFoundError || error instanceof StorageError) {
              throw error;
            }
            throw new StorageError({
              operation: "get",
              key,
              message: String(error),
              cause: error
            });
          }
        }),

        set: (key: string, value: string) => Effect.tryPromise({
          try: async () => {
            // Only attempt expo-secure-store import on mobile platforms
            if (platformInfo.type !== "mobile") {
              throw new StorageError({
                operation: "set",
                key,
                message: "Secure storage not available on this platform",
                cause: new Error(`Platform ${platformInfo.type} does not support secure storage`)
              });
            }
            
            // Check value size
            const estimatedSize = new Blob([value]).size;
            if (estimatedSize > storageConfig.maxValueSize) {
              throw new StorageQuotaExceededError({
                key,
                size: estimatedSize,
                quota: storageConfig.maxValueSize
              });
            }
            
            try {
              const { setItemAsync } = await import('expo-secure-store');
              await setItemAsync(key, value);
            } catch (importError) {
              // Handle import failures gracefully
              throw new StorageError({
                operation: "set",
                key,
                message: "Failed to import expo-secure-store. Ensure you're running on a mobile platform with Expo.",
                cause: importError
              });
            }
          },
          catch: (error) => {
            if (error instanceof StorageQuotaExceededError || error instanceof StorageError) {
              throw error;
            }
            throw new StorageError({
              operation: "set",
              key,
              message: String(error),
              cause: error
            });
          }
        }),

        remove: (key: string) => Effect.tryPromise({
          try: async () => {
            // Only attempt expo-secure-store import on mobile platforms
            if (platformInfo.type !== "mobile") {
              throw new StorageError({
                operation: "remove",
                key,
                message: "Secure storage not available on this platform",
                cause: new Error(`Platform ${platformInfo.type} does not support secure storage`)
              });
            }
            
            try {
              const { deleteItemAsync } = await import('expo-secure-store');
              await deleteItemAsync(key);
            } catch (importError) {
              // Handle import failures gracefully
              throw new StorageError({
                operation: "remove",
                key,
                message: "Failed to import expo-secure-store. Ensure you're running on a mobile platform with Expo.",
                cause: importError
              });
            }
          },
          catch: (error) => {
            if (error instanceof StorageError) {
              throw error;
            }
            throw new StorageError({
              operation: "remove",
              key,
              message: String(error),
              cause: error
            });
          }
        }),

        keys: () => Effect.tryPromise({
          try: async () => {
            // SecureStore doesn't have a keys() method, so we maintain our own index
            const { getItemAsync } = await import('expo-secure-store');
            const keysIndex = await getItemAsync('__storage_keys_index__');
            return keysIndex ? JSON.parse(keysIndex) as string[] : [] as string[];
          },
          catch: (error) => new StorageError({
            operation: "keys",
            key: "",
            message: String(error),
            cause: error
          })
        }),

        clear: () => Effect.gen(function* () {
          const keys = yield* secureStorage.keys();
          yield* Effect.forEach(keys, (key) => secureStorage.remove(key), { concurrency: 5 });
        })
      };

      // Choose storage implementation based on platform
      const storage = yield* platform.executeForPlatform({
        web: () => Effect.succeed(webStorage),
        desktop: () => Effect.succeed(webStorage), // Tauri uses localStorage
        mobile: () => Effect.succeed(secureStorage),
        fallback: () => Effect.fail(new PlatformError({
          operation: "platform_detection",
          message: "Unsupported platform for storage operations"
        }))
      });

      // Helper functions
      const validateKey = (key: string) => Effect.gen(function* () {
        if (!key || key.length === 0) {
          yield* Effect.fail(new StorageError({
            operation: "validate",
            key,
            message: "Storage key cannot be empty"
          }));
        }
        
        if (key.length > storageConfig.maxKeyLength) {
          yield* Effect.fail(new StorageError({
            operation: "validate",
            key,
            message: `Storage key exceeds maximum length of ${storageConfig.maxKeyLength}`
          }));
        }
      });

      const createMetadata = (size: number, ttl?: number): StorageMetadata => ({
        createdAt: Date.now(),
        updatedAt: Date.now(),
        ttl,
        compressed: size > storageConfig.compressionThreshold,
        encrypted: storageConfig.enableEncryption,
        size
      });

      const isExpired = (metadata: StorageMetadata): boolean => {
        if (!storageConfig.ttlEnabled || !metadata.ttl) return false;
        return Date.now() > metadata.createdAt + metadata.ttl;
      };

      return {
        /**
         * Get a string value from storage
         */
        getString: (key: string) => Effect.gen(function* () {
          yield* validateKey(key);
          return yield* storage.get(key);
        }),

        /**
         * Set a string value in storage
         */
        setString: (key: string, value: string, ttl?: number) => Effect.gen(function* () {
          yield* validateKey(key);
          
          // Create storage entry with metadata
          const metadata = createMetadata(new Blob([value]).size, ttl);
          const entry = { value, metadata };
          const serialized = JSON.stringify(entry);
          
          return yield* storage.set(key, serialized);
        }),

        /**
         * Get a typed value from storage with schema validation
         */
        get: <T>(key: string, schema: Schema.Schema<T>) => Effect.gen(function* () {
          yield* validateKey(key);
          
          const raw = yield* storage.get(key);
          
          let entry: StorageEntry<unknown>;
          try {
            entry = JSON.parse(raw);
          } catch (error) {
            // Handle legacy storage without metadata
            return yield* Schema.decode(schema)(JSON.parse(raw)).pipe(
              Effect.catchTag("ParseError", (error) =>
                Effect.fail(new StorageValidationError({
                  key,
                  schema: schema.toString(),
                  value: raw,
                  errors: [String(error)]
                }))
              )
            );
          }

          // Check if entry is expired
          if (isExpired(entry.metadata)) {
            yield* storage.remove(key);
            yield* Effect.fail(new StorageNotFoundError({ 
              key, 
              storageType: yield* platform.isReactNative().pipe(
                Effect.map(isRN => isRN ? "secureStore" : "localStorage")
              )
            }));
          }

          // Validate and return the value
          return yield* Schema.decode(schema)(entry.value as T).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new StorageValidationError({
                key,
                schema: schema.toString(),
                value: entry.value,
                errors: [String(error)]
              }))
            )
          );
        }),

        /**
         * Set a typed value in storage with schema validation
         */
        set: <T>(key: string, value: T, schema: Schema.Schema<T>, ttl?: number) => Effect.gen(function* () {
          yield* validateKey(key);
          
          // Validate the value against the schema
          const validatedValue = yield* Schema.decode(schema)(value).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new StorageValidationError({
                key,
                schema: schema.toString(),
                value,
                errors: [String(error)]
              }))
            )
          );

          const serialized = JSON.stringify(validatedValue);
          const metadata = createMetadata(new Blob([serialized]).size, ttl);
          const entry = { value: validatedValue, metadata };
          
          return yield* storage.set(key, JSON.stringify(entry));
        }),

        /**
         * Remove a value from storage
         */
        remove: (key: string) => Effect.gen(function* () {
          yield* validateKey(key);
          return yield* storage.remove(key);
        }),

        /**
         * Check if a key exists in storage
         */
        has: (key: string) => Effect.gen(function* () {
          yield* validateKey(key);
          
          return yield* storage.get(key).pipe(
            Effect.map(() => true),
            Effect.catchAll((_error) => Effect.succeed(false))
          );
        }),

        /**
         * Get all storage keys
         */
        keys: () => storage.keys(),

        /**
         * Clear all storage
         */
        clear: () => storage.clear(),

        /**
         * Get storage usage information
         */
        getUsage: () => Effect.gen(function* () {
          const keys = yield* storage.keys();
          let totalSize = 0;
          let itemCount = keys.length;
          
          for (const key of keys) {
            try {
              const value = yield* storage.get(key);
              totalSize += new Blob([value]).size;
            } catch {
              // Skip keys that can't be read
              itemCount--;
            }
          }
          
          return {
            itemCount,
            totalSize,
            averageSize: itemCount > 0 ? totalSize / itemCount : 0,
            quota: storageConfig.maxValueSize
          };
        }),

        /**
         * Cleanup expired entries
         */
        cleanup: () => Effect.gen(function* () {
          if (!storageConfig.ttlEnabled) return { removedCount: 0 };
          
          const keys = yield* storage.keys();
          let removedCount = 0;
          
          for (const key of keys) {
            try {
              const raw = yield* storage.get(key);
              const entry = JSON.parse(raw) as StorageEntry<unknown>;
              
              if (isExpired(entry.metadata)) {
                yield* storage.remove(key);
                removedCount++;
              }
            } catch {
              // Skip malformed entries
            }
          }
          
          return { removedCount };
        }),

        /**
         * Get current storage configuration
         */
        getConfig: () => Effect.succeed(storageConfig),

        /**
         * Backup storage to a serializable format
         */
        backup: () => Effect.gen(function* () {
          const keys = yield* storage.keys();
          const backup: Record<string, string> = {};
          
          for (const key of keys) {
            try {
              backup[key] = yield* storage.get(key);
            } catch {
              // Skip keys that can't be read
            }
          }
          
          return {
            timestamp: Date.now(),
            platform: yield* platform.getPlatformInfo().pipe(Effect.map(info => info.type)),
            data: backup
          };
        }),

        /**
         * Restore storage from backup
         */
        restore: (backup: { data: Record<string, string> }) => Effect.gen(function* () {
          let restoredCount = 0;
          
          for (const [key, value] of Object.entries(backup.data)) {
            try {
              yield* storage.set(key, value);
              restoredCount++;
            } catch {
              // Skip keys that can't be restored
            }
          }
          
          return { restoredCount };
        })
      };
    })
  }
) {
  // Note: EnhancedStorageService.Default is automatically created by Effect.Service pattern
  // Dependencies (PlatformService.Default and ConfigService.Default) are handled by the dependencies property

  /**
   * Test implementation for mocking in tests
   */
  static Test = (mockData: Map<string, string> = new Map()) => {
    return EnhancedStorageService.of({
      _tag: "EnhancedStorageService" as const,
      getString: (key) => {
        const value = mockData.get(key);
        return value !== undefined 
          ? Effect.succeed(value)
          : Effect.fail(new StorageError({ 
              operation: "get", 
              key, 
              message: "Mock key not found in test storage" 
            }));
      },
      
      setString: (key, value) => Effect.sync(() => mockData.set(key, value)),
      
      get: (key, schema) => {
        const value = mockData.get(key);
        if (value === undefined) {
          return Effect.fail(new StorageError({ 
            operation: "get", 
            key, 
            message: "Mock key not found in test storage" 
          }));
        }
        
        try {
          const parsed = JSON.parse(value);
          return Schema.decode(schema)(parsed.value || parsed).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new StorageValidationError({
                key,
                schema: schema.toString(),
                value: parsed.value || parsed,
                errors: [String(error)]
              }))
            )
          );
        } catch {
          return Effect.fail(new StorageError({
            operation: "get",
            key,
            message: "Failed to parse stored value"
          }));
        }
      },
      
      set: (key, value, schema) => Effect.gen(function* () {
        const validated = yield* Schema.decode(schema)(value).pipe(
          Effect.catchTag("ParseError", (error) =>
            Effect.fail(new StorageValidationError({
              key,
              schema: schema.toString(),
              value,
              errors: [String(error)]
            }))
          )
        );
        const entry = {
          value: validated,
          metadata: {
            createdAt: Date.now(),
            updatedAt: Date.now(),
            ttl: undefined,
            compressed: false,
            encrypted: false,
            size: JSON.stringify(validated).length
          }
        };
        mockData.set(key, JSON.stringify(entry));
      }),
      
      remove: (key) => Effect.sync(() => mockData.delete(key)),
      has: (key) => Effect.succeed(mockData.has(key)),
      keys: () => Effect.succeed(Array.from(mockData.keys())),
      clear: () => Effect.sync(() => mockData.clear()),
      getUsage: () => Effect.succeed({
        itemCount: mockData.size,
        totalSize: Array.from(mockData.values()).reduce((sum, val) => sum + val.length, 0),
        averageSize: 0,
        quota: DEFAULT_STORAGE_CONFIG.maxValueSize
      }),
      cleanup: () => Effect.succeed({ removedCount: 0 }),
      getConfig: () => Effect.succeed(DEFAULT_STORAGE_CONFIG),
      backup: () => Effect.succeed({
        timestamp: Date.now(),
        platform: "web" as const,
        data: Object.fromEntries(mockData)
      }),
      restore: () => Effect.succeed({ restoredCount: 0 })
    });
  };

  /**
   * Test layer that provides the test EnhancedStorageService
   */
  static TestLive = (mockData?: Map<string, string>) =>
    Layer.succeed(EnhancedStorageService, EnhancedStorageService.Test(mockData));
}
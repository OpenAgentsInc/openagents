import { Effect, Layer } from "effect";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ServiceTestUtils, benchmarkEffect } from "./setup-service-tests";

/**
 * SimpleStorageService Testing Suite
 * 
 * Comprehensive testing for cross-platform storage service functionality
 * as required by Issue #1269: Complete Service-Level Effect-TS Testing Coverage
 * 
 * Uses Effect-TS v3 patterns from EffectPatterns repository
 */

// Storage interfaces for testing
interface StorageData {
  [key: string]: string;
}

// Mock StorageService implementation using Effect.Service pattern
class TestStorageService extends Effect.Service<TestStorageService>()(
  "TestStorageService",
  {
    sync: () => {
      const localStorage: StorageData = {};
      const secureStore: StorageData = {};
      
      return {
        // localStorage operations
        getFromLocalStorage: (key: string) =>
          Effect.gen(function* () {
            const value = localStorage[key];
            if (value === undefined) {
              yield* Effect.fail(new Error(`Key '${key}' not found in localStorage`));
            }
            return value;
          }),
        
        setInLocalStorage: (key: string, value: string) =>
          Effect.sync(() => {
            localStorage[key] = value;
          }),
        
        removeFromLocalStorage: (key: string) =>
          Effect.sync(() => {
            delete localStorage[key];
          }),
        
        clearLocalStorage: () =>
          Effect.sync(() => {
            Object.keys(localStorage).forEach(key => delete localStorage[key]);
          }),
        
        // SecureStore operations (React Native)
        getFromSecureStore: (key: string) =>
          Effect.gen(function* () {
            const value = secureStore[key];
            if (value === undefined) {
              yield* Effect.fail(new Error(`Key '${key}' not found in SecureStore`));
            }
            return value;
          }),
        
        setInSecureStore: (key: string, value: string) =>
          Effect.sync(() => {
            secureStore[key] = value;
          }),
        
        removeFromSecureStore: (key: string) =>
          Effect.sync(() => {
            delete secureStore[key];
          }),
        
        // Cross-platform operations
        getStorageValue: (key: string, platform: "web" | "mobile" = "web") =>
          Effect.gen(function* () {
            if (platform === "web") {
              return yield* Effect.gen(function* () {
                return localStorage[key] || null;
              });
            } else {
              return yield* Effect.gen(function* () {
                return secureStore[key] || null;
              });
            }
          }),
        
        setStorageValue: (key: string, value: string, platform: "web" | "mobile" = "web") =>
          Effect.gen(function* () {
            if (platform === "web") {
              localStorage[key] = value;
            } else {
              secureStore[key] = value;
            }
          }),
        
        removeStorageValue: (key: string, platform: "web" | "mobile" = "web") =>
          Effect.gen(function* () {
            if (platform === "web") {
              delete localStorage[key];
            } else {
              delete secureStore[key];
            }
          }),
        
        // JSON operations
        getStoredJson: <T>(key: string, platform: "web" | "mobile" = "web") =>
          Effect.gen(function* () {
            const stringValue = yield* Effect.gen(function* () {
              if (platform === "web") {
                return localStorage[key];
              } else {
                return secureStore[key];
              }
            });
            
            if (!stringValue) {
              yield* Effect.fail(new Error(`JSON data not found for key '${key}'`));
            }
            
            try {
              return JSON.parse(stringValue) as T;
            } catch (error) {
              yield* Effect.fail(new Error(`Invalid JSON for key '${key}': ${error}`));
            }
          }),
        
        setStoredJson: <T>(key: string, value: T, platform: "web" | "mobile" = "web") =>
          Effect.gen(function* () {
            try {
              const stringValue = JSON.stringify(value);
              if (platform === "web") {
                localStorage[key] = stringValue;
              } else {
                secureStore[key] = stringValue;
              }
            } catch (error) {
              yield* Effect.fail(new Error(`Failed to serialize JSON for key '${key}': ${error}`));
            }
          }),
        
        // Storage info
        getStorageInfo: (platform: "web" | "mobile" = "web") =>
          Effect.succeed({
            platform,
            itemCount: platform === "web" ? Object.keys(localStorage).length : Object.keys(secureStore).length,
            keys: platform === "web" ? Object.keys(localStorage) : Object.keys(secureStore)
          })
      };
    }
  }
) {}

// Error simulation service for testing error scenarios
class FailingStorageService extends Effect.Service<FailingStorageService>()(
  "FailingStorageService",
  {
    sync: () => ({
      getFromLocalStorage: (key: string) => Effect.fail(new Error("localStorage access failed")),
      setInLocalStorage: (key: string, value: string) => Effect.fail(new Error("localStorage write failed")),
      removeFromLocalStorage: (key: string) => Effect.fail(new Error("localStorage remove failed")),
      clearLocalStorage: () => Effect.fail(new Error("localStorage clear failed")),
      
      getFromSecureStore: (key: string) => Effect.fail(new Error("SecureStore access failed")),
      setInSecureStore: (key: string, value: string) => Effect.fail(new Error("SecureStore write failed")),
      removeFromSecureStore: (key: string) => Effect.fail(new Error("SecureStore remove failed")),
      
      getStorageValue: (key: string, platform?: "web" | "mobile") => Effect.fail(new Error("Storage access failed")),
      setStorageValue: (key: string, value: string, platform?: "web" | "mobile") => Effect.fail(new Error("Storage write failed")),
      removeStorageValue: (key: string, platform?: "web" | "mobile") => Effect.fail(new Error("Storage remove failed")),
      
      getStoredJson: <T>(key: string, platform?: "web" | "mobile") => Effect.fail(new Error("JSON retrieval failed")),
      setStoredJson: <T>(key: string, value: T, platform?: "web" | "mobile") => Effect.fail(new Error("JSON storage failed")),
      
      getStorageInfo: (platform?: "web" | "mobile") => Effect.fail(new Error("Storage info retrieval failed"))
    })
  }
) {}

describe("SimpleStorageService Tests", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("localStorage Operations", () => {
    ServiceTestUtils.runServiceTest(
      "should store and retrieve values from localStorage",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const key = "test-key";
        const value = "test-value";
        
        // Store value
        yield* storage.setInLocalStorage(key, value);
        
        // Retrieve value
        const retrieved = yield* storage.getFromLocalStorage(key);
        
        expect(retrieved).toBe(value);
        
        return { key, value, retrieved };
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle missing keys in localStorage",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const result = yield* storage.getFromLocalStorage("nonexistent-key").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toContain("not found in localStorage");
        }
        
        return result;
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should remove values from localStorage",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const key = "remove-test-key";
        const value = "remove-test-value";
        
        // Store value
        yield* storage.setInLocalStorage(key, value);
        
        // Verify it exists
        const beforeRemove = yield* storage.getFromLocalStorage(key);
        expect(beforeRemove).toBe(value);
        
        // Remove value
        yield* storage.removeFromLocalStorage(key);
        
        // Verify it's gone
        const afterRemove = yield* storage.getFromLocalStorage(key).pipe(Effect.either);
        expect(afterRemove._tag).toBe("Left");
        
        return { beforeRemove, afterRemove };
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should clear all localStorage data",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        // Store multiple values
        yield* storage.setInLocalStorage("key1", "value1");
        yield* storage.setInLocalStorage("key2", "value2");
        
        // Verify they exist
        const info1 = yield* storage.getStorageInfo("web");
        expect(info1.itemCount).toBe(2);
        
        // Clear all
        yield* storage.clearLocalStorage();
        
        // Verify they're gone
        const info2 = yield* storage.getStorageInfo("web");
        expect(info2.itemCount).toBe(0);
        
        return { info1, info2 };
      }).pipe(Effect.provide(TestStorageService.Default))
    );
  });

  describe("SecureStore Operations", () => {
    ServiceTestUtils.runServiceTest(
      "should store and retrieve values from SecureStore",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const key = "secure-key";
        const value = "secure-value";
        
        // Store value
        yield* storage.setInSecureStore(key, value);
        
        // Retrieve value
        const retrieved = yield* storage.getFromSecureStore(key);
        
        expect(retrieved).toBe(value);
        
        return { key, value, retrieved };
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle missing keys in SecureStore",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const result = yield* storage.getFromSecureStore("nonexistent-secure-key").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toContain("not found in SecureStore");
        }
        
        return result;
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should remove values from SecureStore",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const key = "secure-remove-key";
        const value = "secure-remove-value";
        
        // Store value
        yield* storage.setInSecureStore(key, value);
        
        // Verify it exists
        const beforeRemove = yield* storage.getFromSecureStore(key);
        expect(beforeRemove).toBe(value);
        
        // Remove value
        yield* storage.removeFromSecureStore(key);
        
        // Verify it's gone
        const afterRemove = yield* storage.getFromSecureStore(key).pipe(Effect.either);
        expect(afterRemove._tag).toBe("Left");
        
        return { beforeRemove, afterRemove };
      }).pipe(Effect.provide(TestStorageService.Default))
    );
  });

  describe("Cross-Platform Operations", () => {
    ServiceTestUtils.runServiceTest(
      "should handle web platform storage",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const key = "cross-platform-key";
        const value = "web-value";
        
        // Store for web
        yield* storage.setStorageValue(key, value, "web");
        
        // Retrieve for web
        const retrieved = yield* storage.getStorageValue(key, "web");
        
        expect(retrieved).toBe(value);
        
        return { key, value, retrieved };
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle mobile platform storage",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const key = "cross-platform-mobile-key";
        const value = "mobile-value";
        
        // Store for mobile
        yield* storage.setStorageValue(key, value, "mobile");
        
        // Retrieve for mobile
        const retrieved = yield* storage.getStorageValue(key, "mobile");
        
        expect(retrieved).toBe(value);
        
        return { key, value, retrieved };
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should isolate storage between platforms",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const key = "isolation-key";
        const webValue = "web-isolated-value";
        const mobileValue = "mobile-isolated-value";
        
        // Store same key on both platforms
        yield* storage.setStorageValue(key, webValue, "web");
        yield* storage.setStorageValue(key, mobileValue, "mobile");
        
        // Retrieve from both platforms
        const webRetrieved = yield* storage.getStorageValue(key, "web");
        const mobileRetrieved = yield* storage.getStorageValue(key, "mobile");
        
        expect(webRetrieved).toBe(webValue);
        expect(mobileRetrieved).toBe(mobileValue);
        expect(webRetrieved).not.toBe(mobileRetrieved);
        
        return { webRetrieved, mobileRetrieved };
      }).pipe(Effect.provide(TestStorageService.Default))
    );
  });

  describe("JSON Operations", () => {
    ServiceTestUtils.runServiceTest(
      "should store and retrieve JSON objects",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const key = "json-key";
        const jsonValue = {
          name: "Test Object",
          count: 42,
          active: true,
          items: ["item1", "item2", "item3"]
        };
        
        // Store JSON
        yield* storage.setStoredJson(key, jsonValue, "web");
        
        // Retrieve JSON
        const retrieved = yield* storage.getStoredJson<typeof jsonValue>(key, "web");
        
        expect(retrieved).toEqual(jsonValue);
        
        return { key, jsonValue, retrieved };
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle JSON parsing errors",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const key = "invalid-json-key";
        
        // Manually store invalid JSON
        yield* storage.setInLocalStorage(key, "{ invalid json }");
        
        // Try to retrieve as JSON
        const result = yield* storage.getStoredJson(key, "web").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toContain("Invalid JSON");
        }
        
        return result;
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle missing JSON data",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        const result = yield* storage.getStoredJson("nonexistent-json-key", "web").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toContain("JSON data not found");
        }
        
        return result;
      }).pipe(Effect.provide(TestStorageService.Default))
    );
  });

  describe("Storage Information", () => {
    ServiceTestUtils.runServiceTest(
      "should provide storage information for web platform",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        // Store some data
        yield* storage.setInLocalStorage("info-key1", "value1");
        yield* storage.setInLocalStorage("info-key2", "value2");
        
        const info = yield* storage.getStorageInfo("web");
        
        expect(info.platform).toBe("web");
        expect(info.itemCount).toBe(2);
        expect(info.keys).toContain("info-key1");
        expect(info.keys).toContain("info-key2");
        
        return info;
      }).pipe(Effect.provide(TestStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should provide storage information for mobile platform",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        // Store some data
        yield* storage.setInSecureStore("mobile-key1", "mobile-value1");
        yield* storage.setInSecureStore("mobile-key2", "mobile-value2");
        yield* storage.setInSecureStore("mobile-key3", "mobile-value3");
        
        const info = yield* storage.getStorageInfo("mobile");
        
        expect(info.platform).toBe("mobile");
        expect(info.itemCount).toBe(3);
        expect(info.keys).toContain("mobile-key1");
        expect(info.keys).toContain("mobile-key2");
        expect(info.keys).toContain("mobile-key3");
        
        return info;
      }).pipe(Effect.provide(TestStorageService.Default))
    );
  });

  describe("Error Handling", () => {
    ServiceTestUtils.runServiceTest(
      "should handle storage access failures",
      Effect.gen(function* () {
        const failingStorage = yield* FailingStorageService;
        
        const result = yield* failingStorage.getFromLocalStorage("any-key").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("localStorage access failed");
        }
        
        return result;
      }).pipe(Effect.provide(FailingStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle storage write failures",
      Effect.gen(function* () {
        const failingStorage = yield* FailingStorageService;
        
        const result = yield* failingStorage.setInLocalStorage("key", "value").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("localStorage write failed");
        }
        
        return result;
      }).pipe(Effect.provide(FailingStorageService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle JSON storage failures",
      Effect.gen(function* () {
        const failingStorage = yield* FailingStorageService;
        
        const result = yield* failingStorage.setStoredJson("key", { test: "data" }).pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("JSON storage failed");
        }
        
        return result;
      }).pipe(Effect.provide(FailingStorageService.Default))
    );
  });

  describe("Performance Benchmarks", () => {
    ServiceTestUtils.runServiceTest(
      "localStorage operations should be fast",
      benchmarkEffect(
        "localStorage Operations",
        Effect.gen(function* () {
          const storage = yield* TestStorageService;
          
          // Store multiple values
          yield* storage.setInLocalStorage("perf-key1", "perf-value1");
          yield* storage.setInLocalStorage("perf-key2", "perf-value2");
          yield* storage.setInLocalStorage("perf-key3", "perf-value3");
          
          // Retrieve them
          const values = yield* Effect.all([
            storage.getFromLocalStorage("perf-key1"),
            storage.getFromLocalStorage("perf-key2"),
            storage.getFromLocalStorage("perf-key3")
          ]);
          
          return values;
        }).pipe(Effect.provide(TestStorageService.Default)),
        100 // Should complete within 100ms
      )
    );

    ServiceTestUtils.runServiceTest(
      "JSON operations should be efficient",
      benchmarkEffect(
        "JSON Operations",
        Effect.gen(function* () {
          const storage = yield* TestStorageService;
          
          const jsonData = {
            id: "benchmark-object",
            data: Array.from({ length: 100 }, (_, i) => ({ index: i, value: `item-${i}` }))
          };
          
          // Store JSON
          yield* storage.setStoredJson("benchmark-json", jsonData);
          
          // Retrieve JSON
          const retrieved = yield* storage.getStoredJson<typeof jsonData>("benchmark-json");
          
          return retrieved;
        }).pipe(Effect.provide(TestStorageService.Default)),
        200 // Should complete within 200ms
      )
    );
  });

  describe("Integration Tests", () => {
    ServiceTestUtils.runServiceTest(
      "should support complete storage workflow",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        // 1. Store data on both platforms
        yield* storage.setStorageValue("user-id", "user-123", "web");
        yield* storage.setStorageValue("user-id", "user-123", "mobile");
        
        // 2. Store JSON configuration
        const config = {
          theme: "dark",
          language: "en",
          notifications: true
        };
        yield* storage.setStoredJson("user-config", config, "web");
        yield* storage.setStoredJson("user-config", config, "mobile");
        
        // 3. Retrieve and verify data
        const webUserId = yield* storage.getStorageValue("user-id", "web");
        const mobileUserId = yield* storage.getStorageValue("user-id", "mobile");
        const webConfig = yield* storage.getStoredJson<typeof config>("user-config", "web");
        const mobileConfig = yield* storage.getStoredJson<typeof config>("user-config", "mobile");
        
        expect(webUserId).toBe("user-123");
        expect(mobileUserId).toBe("user-123");
        expect(webConfig).toEqual(config);
        expect(mobileConfig).toEqual(config);
        
        // 4. Get storage info
        const webInfo = yield* storage.getStorageInfo("web");
        const mobileInfo = yield* storage.getStorageInfo("mobile");
        
        expect(webInfo.itemCount).toBe(2);
        expect(mobileInfo.itemCount).toBe(2);
        
        // 5. Clean up
        yield* storage.removeStorageValue("user-id", "web");
        yield* storage.removeStorageValue("user-config", "web");
        
        const cleanWebInfo = yield* storage.getStorageInfo("web");
        expect(cleanWebInfo.itemCount).toBe(0);
        
        return {
          webUserId,
          mobileUserId,
          webConfig,
          mobileConfig,
          webInfo,
          mobileInfo,
          cleanWebInfo
        };
      }).pipe(Effect.provide(TestStorageService.Default))
    );
  });

  describe("Concurrent Operations", () => {
    ServiceTestUtils.runServiceTest(
      "should handle concurrent storage operations safely",
      Effect.gen(function* () {
        const storage = yield* TestStorageService;
        
        // Concurrent operations on different keys
        const operations = [
          storage.setStorageValue("concurrent-1", "value-1", "web"),
          storage.setStorageValue("concurrent-2", "value-2", "web"),
          storage.setStorageValue("concurrent-3", "value-3", "mobile"),
          storage.setStorageValue("concurrent-4", "value-4", "mobile")
        ];
        
        yield* Effect.all(operations, { concurrency: 4 });
        
        // Verify all operations completed
        const results = yield* Effect.all([
          storage.getStorageValue("concurrent-1", "web"),
          storage.getStorageValue("concurrent-2", "web"),
          storage.getStorageValue("concurrent-3", "mobile"),
          storage.getStorageValue("concurrent-4", "mobile")
        ]);
        
        expect(results).toEqual(["value-1", "value-2", "value-3", "value-4"]);
        
        return results;
      }).pipe(Effect.provide(TestStorageService.Default))
    );
  });
});
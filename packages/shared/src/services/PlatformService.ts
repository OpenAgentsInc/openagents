import { Effect, Data } from "effect";

/**
 * PlatformService - Enhanced platform detection and capabilities service
 * 
 * Following EffectPatterns best practices for service architecture:
 * - Dependency injection ready
 * - Testable with mock implementations
 * - Tagged errors for precise error handling
 * - Pure functions wrapped in Effect context
 */

// Tagged error types for platform operations
export class PlatformError extends Data.TaggedError("PlatformError")<{
  operation: string;
  message: string;
  cause?: unknown;
}> {}

export class UnsupportedPlatformError extends Data.TaggedError("UnsupportedPlatformError")<{
  platform: string;
  feature: string;
}> {}

// Platform capabilities interface
export interface PlatformCapabilities {
  hasSecureStorage: boolean;
  hasNotifications: boolean;
  hasFileSystem: boolean;
  hasCamera: boolean;
  hasLocation: boolean;
  supportsWebGL: boolean;
  maxConcurrency: number;
}

// Platform information interface
export interface PlatformInfo {
  type: "web" | "mobile" | "desktop" | "unknown";
  os: "ios" | "android" | "windows" | "macos" | "linux" | "unknown";
  userAgent?: string;
  version?: string;
  capabilities: PlatformCapabilities;
}

/**
 * PlatformService using Effect.Service pattern for dependency injection
 * 
 * This follows the "model-dependencies-as-services" pattern from EffectPatterns,
 * enabling easy testing and environment-specific implementations.
 */
export class PlatformService extends Effect.Service<PlatformService>()(
  "PlatformService",
  {
    sync: () => {
      // Platform detection logic
      const detectPlatform = (): PlatformInfo["type"] => {
        // Check for Tauri-specific environment variables and globals
        if (typeof window === "undefined") {
          // More specific Tauri detection
          if (typeof process !== "undefined" && 
              (process.env.TAURI_PLATFORM || 
               process.env.TAURI_FAMILY || 
               process.env.TAURI_ARCH)) {
            return "desktop";
          }
          // Check for other Tauri indicators (when available in Node.js context)
          if (typeof global !== "undefined" && 
              (global as any).__TAURI_METADATA__) {
            return "desktop";
          }
          // Default to unknown for other Node.js contexts (SSR, servers, etc.)
          return "unknown" as any; // Will be handled as fallback
        }
        
        // Browser contexts
        if (typeof window !== "undefined") {
          // Check for Tauri APIs in browser context
          if ((window as any).__TAURI__ || (window as any).__TAURI_IPC__) {
            return "desktop";
          }
          // React Native detection
          if (typeof navigator !== "undefined" && /react-native/i.test(navigator.userAgent)) {
            return "mobile";
          }
          // Regular web browser
          return "web";
        }
        
        return "unknown" as any;
      };

      const detectOS = (): PlatformInfo["os"] => {
        if (typeof navigator === "undefined") return "unknown";
        
        const userAgent = navigator.userAgent.toLowerCase();
        if (userAgent.includes("ios") || userAgent.includes("iphone") || userAgent.includes("ipad")) return "ios";
        if (userAgent.includes("android")) return "android";
        if (userAgent.includes("windows")) return "windows";
        if (userAgent.includes("mac")) return "macos";
        if (userAgent.includes("linux")) return "linux";
        return "unknown";
      };

      const detectCapabilities = (platformType: PlatformInfo["type"], os: PlatformInfo["os"]): PlatformCapabilities => {
        switch (platformType) {
          case "mobile":
            return {
              hasSecureStorage: true,
              hasNotifications: true,
              hasFileSystem: true,
              hasCamera: true,
              hasLocation: true,
              supportsWebGL: true,
              maxConcurrency: 4
            };
          case "desktop":
            return {
              hasSecureStorage: true,
              hasNotifications: true,
              hasFileSystem: true,
              hasCamera: true,
              hasLocation: false,
              supportsWebGL: true,
              maxConcurrency: 8
            };
          case "web":
            return {
              hasSecureStorage: false,
              hasNotifications: "Notification" in window,
              hasFileSystem: "showOpenFilePicker" in window,
              hasCamera: "mediaDevices" in navigator,
              hasLocation: "geolocation" in navigator,
              supportsWebGL: (() => {
                try {
                  const canvas = document.createElement("canvas");
                  return !!(canvas.getContext("webgl") || canvas.getContext("experimental-webgl"));
                } catch {
                  return false;
                }
              })(),
              maxConcurrency: 6
            };
          case "unknown":
          default:
            // Conservative defaults for unknown/undetected platforms (SSR, servers, etc.)
            return {
              hasSecureStorage: false,
              hasNotifications: false,
              hasFileSystem: false,
              hasCamera: false,
              hasLocation: false,
              supportsWebGL: false,
              maxConcurrency: 2
            };
        }
      };

      return {
        /**
         * Get comprehensive platform information
         */
        getPlatformInfo: () =>
          Effect.sync(() => {
            const platformType = detectPlatform();
            const os = detectOS();
            const capabilities = detectCapabilities(platformType, os);

            return {
              type: platformType,
              os,
              userAgent: typeof navigator !== "undefined" ? navigator.userAgent : undefined,
              version: typeof process !== "undefined" ? process.version : undefined,
              capabilities
            } as PlatformInfo;
          }),

        /**
         * Check if current platform is React Native
         */
        isReactNative: () =>
          Effect.sync(() => detectPlatform() === "mobile"),

        /**
         * Check if current platform is web browser
         */
        isWeb: () =>
          Effect.sync(() => detectPlatform() === "web"),

        /**
         * Check if current platform is Tauri desktop
         */
        isDesktop: () =>
          Effect.sync(() => detectPlatform() === "desktop"),

        /**
         * Check if platform supports a specific capability
         */
        hasCapability: (capability: keyof PlatformCapabilities) =>
          Effect.gen(function* () {
            const info = yield* Effect.sync(() => {
              const platformType = detectPlatform();
              const os = detectOS();
              return detectCapabilities(platformType, os);
            });
            
            return info[capability];
          }),

        /**
         * Get optimal concurrency level for current platform
         */
        getOptimalConcurrency: () =>
          Effect.gen(function* () {
            const info = yield* Effect.sync(() => {
              const platformType = detectPlatform();
              const os = detectOS();
              return detectCapabilities(platformType, os);
            });
            
            return info.maxConcurrency;
          }),

        /**
         * Assert platform capability with helpful error
         */
        requireCapability: (capability: keyof PlatformCapabilities) =>
          Effect.gen(function* () {
            const hasCapability = yield* Effect.gen(function* () {
              const info = yield* Effect.sync(() => {
                const platformType = detectPlatform();
                const os = detectOS();
                return detectCapabilities(platformType, os);
              });
              
              return info[capability];
            });

            if (!hasCapability) {
              const platformInfo = yield* Effect.sync(() => {
                const platformType = detectPlatform();
                return platformType;
              });

              yield* Effect.fail(new UnsupportedPlatformError({
                platform: platformInfo,
                feature: capability
              }));
            }
          }),

        /**
         * Execute platform-specific code with fallback
         */
        executeForPlatform: <T>(implementations: {
          web?: () => Effect.Effect<T, PlatformError, never>;
          mobile?: () => Effect.Effect<T, PlatformError, never>;
          desktop?: () => Effect.Effect<T, PlatformError, never>;
          fallback?: () => Effect.Effect<T, PlatformError, never>;
        }) =>
          Effect.gen(function* () {
            const platformType = yield* Effect.sync(() => detectPlatform());
            
            // Handle "unknown" platform type by falling back to the fallback implementation
            const implementation = (platformType === "unknown") 
              ? implementations.fallback
              : (implementations[platformType as keyof typeof implementations] || implementations.fallback);
            
            if (!implementation) {
              yield* Effect.fail(new PlatformError({
                operation: "executeForPlatform",
                message: `No implementation available for platform: ${platformType}`
              }));
            }
            
            return yield* implementation!();
          }),

        /**
         * Get environment-specific configuration
         */
        getEnvironmentConfig: () =>
          Effect.gen(function* () {
            const platformType = yield* Effect.sync(() => detectPlatform());
            const os = yield* Effect.sync(() => detectOS());
            
            return {
              platform: platformType,
              os,
              isDevelopment: typeof process !== "undefined" && process.env.NODE_ENV === "development",
              isProduction: typeof process !== "undefined" && process.env.NODE_ENV === "production",
              isTest: typeof process !== "undefined" && process.env.NODE_ENV === "test",
              // Platform-specific settings
              maxRetries: platformType === "mobile" ? 5 : 3,
              timeoutMs: platformType === "mobile" ? 10000 : 5000,
              batchSize: platformType === "desktop" ? 100 : 50
            };
          })
      };
    }
  }
) {
  /**
   * Test implementation for mocking in tests
   */
  static Test = (overrides: Partial<PlatformInfo> = {}) => {
    const testService = {
      _tag: "PlatformService" as const,
      getPlatformInfo: () => Effect.succeed({
        type: "web",
        os: "unknown",
        userAgent: "test-agent",
        version: "test-version",
        capabilities: {
          hasSecureStorage: false,
          hasNotifications: false,
          hasFileSystem: false,
          hasCamera: false,
          hasLocation: false,
          supportsWebGL: false,
          maxConcurrency: 2
        },
        ...overrides
      } as PlatformInfo),
      
      isReactNative: () => Effect.succeed(overrides.type === "mobile"),
      isWeb: () => Effect.succeed(overrides.type === "web"),
      isDesktop: () => Effect.succeed(overrides.type === "desktop"),
      hasCapability: (capability: keyof PlatformCapabilities) => Effect.succeed(overrides.capabilities?.[capability] ?? false),
      getOptimalConcurrency: () => Effect.succeed(overrides.capabilities?.maxConcurrency ?? 2),
      requireCapability: (capability: keyof PlatformCapabilities) => 
        overrides.capabilities?.[capability] 
          ? Effect.void 
          : Effect.fail(new UnsupportedPlatformError({
              platform: overrides.type ?? "test",
              feature: capability
            })),
      executeForPlatform: (implementations: {
        web?: () => Effect.Effect<any, PlatformError, never>;
        mobile?: () => Effect.Effect<any, PlatformError, never>;
        desktop?: () => Effect.Effect<any, PlatformError, never>;
        fallback?: () => Effect.Effect<any, PlatformError, never>;
      }) => {
        const platformType = overrides.type ?? "web";
        // Handle "unknown" platform type by falling back to the fallback implementation
        const implementation = (platformType === "unknown") 
          ? implementations.fallback
          : (implementations[platformType as keyof typeof implementations] || implementations.fallback);
        return implementation ? implementation() : Effect.fail(new PlatformError({
          operation: "executeForPlatform",
          message: `No test implementation for platform: ${platformType}`
        }));
      },
      getEnvironmentConfig: () => Effect.succeed({
        platform: overrides.type ?? "web",
        os: overrides.os ?? "unknown",
        isDevelopment: true,
        isProduction: false,
        isTest: true,
        maxRetries: 3,
        timeoutMs: 5000,
        batchSize: 50
      })
    };
    
    return PlatformService.of(testService);
  };
}

// Note: PlatformService.Default is automatically created by Effect.Service pattern
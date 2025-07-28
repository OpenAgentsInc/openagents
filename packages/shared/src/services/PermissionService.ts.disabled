import { Effect, Schedule, Duration, Data, Option } from "effect";
import { Platform } from "react-native";

// Permission error types
export class PermissionError extends Data.TaggedError("PermissionError")<{
  permissionType: string;
  reason: string;
  platform: string;
  recoverable: boolean;
}> {}

export class PermissionDeniedError extends Data.TaggedError("PermissionDeniedError")<{
  permissionType: string;
  platform: string;
  canRetry: boolean;
  fallbackAvailable: boolean;
}> {}

export class PermissionNotSupportedError extends Data.TaggedError("PermissionNotSupportedError")<{
  permissionType: string;
  platform: string;
}> {}

// Permission types
export type PermissionType = 
  | "notifications"
  | "storage" 
  | "network"
  | "camera"
  | "microphone"
  | "location";

export type PermissionStatus = 
  | "granted" 
  | "denied" 
  | "not_requested" 
  | "restricted";

export interface PermissionResult {
  type: PermissionType;
  status: PermissionStatus;
  canRetry: boolean;
  fallbackAvailable: boolean;
  reason?: string;
}

export interface PermissionCheckResult {
  permissions: Record<PermissionType, PermissionResult>;
  allGranted: boolean;
  requiresUserAction: boolean;
}

// Platform-specific permission checking
const checkNotificationPermission = (): Effect.Effect<PermissionResult, PermissionError> =>
  Effect.gen(function* () {
    const platform = Platform.OS;
    
    try {
      if (platform === "ios" || platform === "android") {
        // Use Expo Notifications for mobile
        const Notifications = yield* Effect.tryPromise({
          try: async () => {
            const module = await import("expo-notifications");
            return module.default || module;
          },
          catch: (error) => new PermissionError({
            permissionType: "notifications",
            reason: `Failed to import expo-notifications: ${error}`,
            platform,
            recoverable: false
          })
        });

        const { status } = yield* Effect.tryPromise({
          try: () => Notifications.getPermissionsAsync(),
          catch: (error) => new PermissionError({
            permissionType: "notifications",
            reason: `Failed to check notification permissions: ${error}`,
            platform,
            recoverable: true
          })
        });

        return {
          type: "notifications" as const,
          status: status === "granted" ? "granted" : "denied",
          canRetry: status !== "granted",
          fallbackAvailable: true, // Can show in-app alerts
          reason: status !== "granted" ? "User has not granted notification permission" : undefined
        };
      } else {
        // Desktop/web
        const permission = yield* Effect.tryPromise({
          try: async () => {
            if ("Notification" in window) {
              return Notification.permission;
            }
            return "default";
          },
          catch: (error) => new PermissionError({
            permissionType: "notifications",
            reason: `Failed to check web notification permission: ${error}`,
            platform,
            recoverable: true
          })
        });

        return {
          type: "notifications" as const,
          status: permission === "granted" ? "granted" : permission === "denied" ? "denied" : "not_requested",
          canRetry: permission !== "denied",
          fallbackAvailable: true,
        };
      }
    } catch (error) {
      return yield* Effect.fail(new PermissionError({
        permissionType: "notifications",
        reason: `Unexpected error checking notifications: ${error}`,
        platform,
        recoverable: false
      }));
    }
  });

const checkStoragePermission = (): Effect.Effect<PermissionResult, PermissionError> =>
  Effect.gen(function* () {
    const platform = Platform.OS;
    
    // Storage permission is usually granted by default on mobile apps
    // For web, we check localStorage availability
    if (platform === "web") {
      const hasStorage = yield* Effect.sync(() => {
        try {
          localStorage.setItem("test", "test");
          localStorage.removeItem("test");
          return true;
        } catch {
          return false;
        }
      });

      return {
        type: "storage" as const,
        status: hasStorage ? "granted" : "denied",
        canRetry: false,
        fallbackAvailable: true, // Can use in-memory storage
      };
    }
    
    // Mobile apps typically have storage access
    return {
      type: "storage" as const,
      status: "granted",
      canRetry: false,
      fallbackAvailable: false,
    };
  });

const checkNetworkPermission = (): Effect.Effect<PermissionResult, PermissionError> =>
  Effect.gen(function* () {
    // Network permission is typically available for apps
    // We can check if we can make network requests
    const canMakeRequest = yield* Effect.tryPromise({
      try: async () => {
        // Try a simple network request
        const response = await fetch("https://httpbin.org/get", {
          method: "HEAD",
          mode: "no-cors",
        });
        return true;
      },
      catch: () => false
    }).pipe(
      Effect.timeout(Duration.seconds(5)),
      Effect.catchAll(() => Effect.succeed(false))
    );

    return {
      type: "network" as const,
      status: canMakeRequest ? "granted" : "denied",
      canRetry: true,
      fallbackAvailable: false,
      reason: !canMakeRequest ? "Network connectivity issues detected" : undefined
    };
  });

const checkCameraPermission = (): Effect.Effect<PermissionResult, PermissionError> =>
  Effect.gen(function* () {
    const platform = Platform.OS;
    
    if (platform === "ios" || platform === "android") {
      try {
        const { Camera } = yield* Effect.tryPromise({
          try: () => import("expo-camera"),
          catch: () => null // Camera not available
        });

        if (!Camera) {
          return {
            type: "camera" as const,
            status: "not_requested",
            canRetry: false,
            fallbackAvailable: true,
            reason: "Camera not available on this device"
          };
        }

        const { status } = yield* Effect.tryPromise({
          try: () => Camera.getCameraPermissionsAsync(),
          catch: (error) => new PermissionError({
            permissionType: "camera",
            reason: `Failed to check camera permissions: ${error}`,
            platform,
            recoverable: true
          })
        });

        return {
          type: "camera" as const,
          status: status === "granted" ? "granted" : "denied",
          canRetry: status !== "granted",
          fallbackAvailable: true,
        };
      } catch {
        return {
          type: "camera" as const,
          status: "not_requested",
          canRetry: false,
          fallbackAvailable: true,
        };
      }
    }

    // Web camera check
    const hasCamera = yield* Effect.tryPromise({
      try: async () => {
        const devices = await navigator.mediaDevices.enumerateDevices();
        return devices.some(device => device.kind === "videoinput");
      },
      catch: () => false
    });

    return {
      type: "camera" as const,
      status: hasCamera ? "not_requested" : "denied",
      canRetry: hasCamera,
      fallbackAvailable: true,
      reason: !hasCamera ? "No camera devices found" : undefined
    };
  });

// Request permission from the platform
const requestPlatformPermission = (
  permissionType: PermissionType,
  reason?: string
): Effect.Effect<PermissionResult, PermissionError> =>
  Effect.gen(function* () {
    const platform = Platform.OS;

    switch (permissionType) {
      case "notifications":
        if (platform === "ios" || platform === "android") {
          const { Notifications } = yield* Effect.tryPromise({
            try: () => import("expo-notifications"),
            catch: (error) => new PermissionError({
              permissionType,
              reason: `Failed to import expo-notifications: ${error}`,
              platform,
              recoverable: false
            })
          });

          const { status } = yield* Effect.tryPromise({
            try: () => Notifications.requestPermissionsAsync(),
            catch: (error) => new PermissionError({
              permissionType,
              reason: `Failed to request notification permissions: ${error}`,
              platform,
              recoverable: true
            })
          });

          return {
            type: permissionType,
            status: status === "granted" ? "granted" : "denied",
            canRetry: status !== "granted",
            fallbackAvailable: true,
          };
        } else {
          // Web notification request
          const permission = yield* Effect.tryPromise({
            try: () => Notification.requestPermission(),
            catch: (error) => new PermissionError({
              permissionType,
              reason: `Failed to request web notification permission: ${error}`,
              platform,
              recoverable: true
            })
          });

          return {
            type: permissionType,
            status: permission === "granted" ? "granted" : "denied",
            canRetry: permission !== "denied",
            fallbackAvailable: true,
          };
        }

      case "camera":
        if (platform === "ios" || platform === "android") {
          const { Camera } = yield* Effect.tryPromise({
            try: () => import("expo-camera"),
            catch: (error) => new PermissionError({
              permissionType,
              reason: `Camera not available: ${error}`,
              platform,
              recoverable: false
            })
          });

          const { status } = yield* Effect.tryPromise({
            try: () => Camera.requestCameraPermissionsAsync(),
            catch: (error) => new PermissionError({
              permissionType,
              reason: `Failed to request camera permissions: ${error}`,
              platform,
              recoverable: true
            })
          });

          return {
            type: permissionType,
            status: status === "granted" ? "granted" : "denied",
            canRetry: status !== "granted",
            fallbackAvailable: true,
          };
        } else {
          // Web camera permission is requested when accessing getUserMedia
          const hasAccess = yield* Effect.tryPromise({
            try: async () => {
              const stream = await navigator.mediaDevices.getUserMedia({ video: true });
              stream.getTracks().forEach(track => track.stop());
              return true;
            },
            catch: () => false
          });

          return {
            type: permissionType,
            status: hasAccess ? "granted" : "denied",
            canRetry: !hasAccess,
            fallbackAvailable: true,
          };
        }

      default:
        return yield* Effect.fail(new PermissionNotSupportedError({
          permissionType,
          platform
        }));
    }
  });

// Main permission service
export class PermissionService extends Effect.Service<PermissionService>()("PermissionService", {
  sync: () => ({
    // Check all required permissions
    checkRequiredPermissions: (
      requiredPermissions: PermissionType[] = ["notifications", "storage", "network"]
    ): Effect.Effect<PermissionCheckResult, PermissionError> =>
      Effect.gen(function* () {
        const results: PermissionResult[] = [];

        // Check each permission
        for (const permissionType of requiredPermissions) {
          const result = yield* Effect.match(
            Effect.gen(function* () {
              switch (permissionType) {
                case "notifications":
                  return yield* checkNotificationPermission();
                case "storage":
                  return yield* checkStoragePermission();
                case "network":
                  return yield* checkNetworkPermission();
                case "camera":
                  return yield* checkCameraPermission();
                default:
                  return yield* Effect.fail(new PermissionNotSupportedError({
                    permissionType,
                    platform: Platform.OS
                  }));
              }
            }),
            {
              onFailure: (error) => Effect.succeed({
                type: permissionType,
                status: "denied" as const,
                canRetry: false,
                fallbackAvailable: true,
                reason: error.reason
              }),
              onSuccess: (result) => Effect.succeed(result)
            }
          );

          results.push(result);
        }

        const permissions = results.reduce((acc, result) => {
          acc[result.type] = result;
          return acc;
        }, {} as Record<PermissionType, PermissionResult>);

        const allGranted = results.every(result => result.status === "granted");
        const requiresUserAction = results.some(result => 
          result.status === "not_requested" && result.canRetry
        );

        return {
          permissions,
          allGranted,
          requiresUserAction
        };
      }),

    // Request specific permission
    requestPermission: (
      permissionType: PermissionType,
      reason?: string
    ): Effect.Effect<PermissionResult, PermissionError> =>
      requestPlatformPermission(permissionType, reason).pipe(
        Effect.retry(
          Schedule.exponential(Duration.seconds(1)).pipe(
            Schedule.intersect(Schedule.recurs(2))
          )
        )
      ),

    // Request multiple permissions
    requestPermissions: (
      permissions: Array<{ type: PermissionType; reason?: string }>
    ): Effect.Effect<PermissionResult[], PermissionError> =>
      Effect.gen(function* () {
        const results: PermissionResult[] = [];

        for (const { type, reason } of permissions) {
          const result = yield* requestPlatformPermission(type, reason).pipe(
            Effect.catchAll(error => 
              Effect.succeed({
                type,
                status: "denied" as const,
                canRetry: error.recoverable,
                fallbackAvailable: true,
                reason: error.reason
              })
            )
          );

          results.push(result);

          // Add delay between permission requests
          yield* Effect.sleep(Duration.millis(500));
        }

        return results;
      }),

    // Handle permission denied scenario
    handlePermissionDenied: (
      permissionType: PermissionType,
      fallbackAction?: Effect.Effect<void, never>
    ): Effect.Effect<void, never> =>
      Effect.gen(function* () {
        yield* Effect.logWarning(`Permission denied for ${permissionType}, using fallback`);
        
        if (fallbackAction) {
          yield* fallbackAction;
        }
      }),

    // Get platform-specific permission explanation
    getPermissionExplanation: (permissionType: PermissionType): string => {
      const platform = Platform.OS;
      
      switch (permissionType) {
        case "notifications":
          return platform === "ios" || platform === "android"
            ? "We need notification permission to send you important updates about your Claude sessions and messages."
            : "Allow notifications to stay informed about Claude activity and important updates.";
            
        case "storage":
          return "Storage access is needed to save your preferences, session data, and provide offline functionality.";
          
        case "network":
          return "Network access is required to sync your data with the cloud and communicate with Claude.";
          
        case "camera":
          return "Camera access allows you to share images with Claude for visual analysis and assistance.";
          
        case "microphone":
          return "Microphone access enables voice input for hands-free interaction with Claude.";
          
        case "location":
          return "Location access helps provide location-aware assistance and contextual information.";
          
        default:
          return `Permission for ${permissionType} is needed for the app to function properly.`;
      }
    }
  })
}) {}
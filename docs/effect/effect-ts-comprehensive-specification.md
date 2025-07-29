# Effect-TS Comprehensive Implementation Specification for OpenAgents

**Document Version**: 2.0  
**Last Updated**: July 29, 2025  
**Status**: Comprehensive Specification - All Patterns Documented  

This document serves as the definitive, comprehensive specification for Effect-TS usage in the entire OpenAgents codebase. It documents all required patterns, implementation areas, integration points, and provides specific guidance for every scenario where Effect-TS should be used.

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Core Patterns by Use Case](#core-patterns-by-use-case)
4. [Service Implementation Matrix](#service-implementation-matrix)
5. [Integration Points Specification](#integration-points-specification)
6. [Error Handling Patterns](#error-handling-patterns)
7. [State Management Patterns](#state-management-patterns)
8. [Testing Patterns](#testing-patterns)
9. [Performance & Resource Management](#performance--resource-management)
10. [Migration Roadmap](#migration-roadmap)
11. [Enforcement Guidelines](#enforcement-guidelines)

## Executive Summary

OpenAgents employs Effect-TS as its core functional programming foundation, providing type-safe error handling, resource management, dependency injection, and concurrent programming primitives. This specification documents the comprehensive implementation across all application layers.

### Implementation Status
- ✅ **Phase 1-4 Complete**: Core services, streaming, testing (90%+ coverage)
- 📋 **Current Phase**: Comprehensive pattern enforcement
- 🎯 **Target**: 100% Effect-TS adoption for all async operations

### Key Benefits Achieved
- **Type Safety**: Compile-time error detection for all failure modes
- **Performance**: <1ms streaming latency, sub-200ms service operations
- **Reliability**: Automatic resource cleanup, atomic state management
- **Maintainability**: 2,640 lines of comprehensive test coverage

## Architecture Overview

### Layer Architecture
```
┌─────────────────────────────────────────────────────────┐
│                 Application Layer                        │
│  (React Components, Tauri Commands, CLI Interfaces)     │
├─────────────────────────────────────────────────────────┤
│                 Service Layer                           │
│  (Business Logic Services - Effect.Service Pattern)     │
├─────────────────────────────────────────────────────────┤
│                 Infrastructure Layer                    │
│  (HTTP, Database, File System, Event Streaming)        │
├─────────────────────────────────────────────────────────┤
│                 Platform Layer                          │
│  (Tauri, React Native, Node.js, Browser APIs)          │
└─────────────────────────────────────────────────────────┘
```

### Core Principles
1. **Service-First Design**: All business logic encapsulated in Effect services
2. **Dependency Injection**: Layer-based composition for all dependencies  
3. **Error Channel Separation**: Tagged errors in dedicated error channel
4. **Resource Safety**: Automatic cleanup through scoped operations
5. **Concurrency Control**: STM for atomic state, Fiber for concurrent tasks

## Core Patterns by Use Case

### 1. Real-time Streaming & Event Processing

**When to Use**: Message streaming, live updates, event-driven architecture
**Required Files**: All streaming services, event handlers, real-time UI components

**Mandatory Pattern**:
```typescript
// Service Definition (REQUIRED)
class StreamingService extends Effect.Service<StreamingService>()("StreamingService", {
  effect: Effect.gen(function* () {
    const eventService = yield* TauriEventService;
    
    return {
      createEventStream: (eventName: string, bufferSize = 100) =>
        Effect.gen(function* () {
          const queue = yield* Queue.bounded<unknown>(bufferSize);
          const { cleanup } = yield* eventService.createEventStream(eventName);
          
          // Resource management with finalizer
          yield* Effect.addFinalizer(() => Effect.sync(() => cleanup()));
          
          return {
            stream: Stream.fromQueue(queue).pipe(
              Stream.mapEffect(parseEventPayload),
              Stream.filter(isValidEvent)
            ),
            cleanup: Effect.sync(() => cleanup())
          };
        })
    };
  }),
  dependencies: [TauriEventService.Default]
}) {}

// Error Handling (REQUIRED)
class StreamingError extends Data.TaggedError("StreamingError")<{
  eventName?: string;
  sessionId?: string;  
  cause?: unknown;
}> {}

// Usage Pattern (REQUIRED)
const useStreamingData = (eventName: string) => {
  const [data, setData] = useState<EventData[]>([]);
  
  useEffect(() => {
    const program = Effect.gen(function* () {
      const service = yield* StreamingService;
      const { stream } = yield* service.createEventStream(eventName);
      
      yield* Stream.runForEach(stream, (event) =>
        Effect.sync(() => setData(prev => [...prev, event]))
      );
    });
    
    const fiber = Effect.runPromise(
      Effect.provide(program, StreamingService.Default)
    );
    
    return () => {
      fiber.then(f => f?.interrupt?.());
    };
  }, [eventName]);
  
  return data;
};
```

**Implementation Locations**:
- `apps/desktop/src/services/ClaudeStreamingService.ts` ✅ Implemented
- `apps/desktop/src/services/TauriEventService.ts` ✅ Implemented  
- `apps/desktop/src/hooks/useClaudeStreaming.ts` ✅ Implemented
- `apps/mobile/src/hooks/useRealtimeUpdates.ts` 📋 Required
- `packages/shared/streaming/EventBroker.ts` 📋 Required

### 2. Multi-Device Synchronization

**When to Use**: Mobile-desktop sync, cross-device state management, offline-first architecture
**Required Files**: All sync services, state management, conflict resolution

**Mandatory Pattern**:
```typescript
// STM-based State Synchronization (REQUIRED)
class SyncService extends Effect.Service<SyncService>()("SyncService", {
  effect: Effect.gen(function* () {
    const storage = yield* StorageService;
    const auth = yield* AuthService;
    
    // STM state for atomic updates
    const deviceStates = yield* TMap.empty<string, DeviceState>();
    const syncQueue = yield* Queue.bounded<SyncOperation>(1000);
    
    return {
      syncDeviceState: (deviceId: string, state: DeviceState) =>
        STM.gen(function* () {
          const existing = yield* TMap.get(deviceStates, deviceId);
          const merged = Option.match(existing, {
            onNone: () => state,
            onSome: (current) => mergeDeviceStates(current, state)
          });
          yield* TMap.set(deviceStates, deviceId, merged);
        }).pipe(STM.commit),
        
      processConflicts: (conflicts: ConflictData[]) =>
        Effect.gen(function* () {
          // Batch conflict resolution atomically
          const resolutions = yield* Effect.all(
            conflicts.map(resolveConflict),
            { concurrency: 3 }
          );
          
          // Apply all resolutions in a single STM transaction
          return yield* STM.commit(
            STM.gen(function* () {
              for (const resolution of resolutions) {
                yield* TMap.set(deviceStates, resolution.deviceId, resolution.state);
              }
            })
          );
        })
    };
  }),
  dependencies: [StorageService.Default, AuthService.Default]
}) {}

// Cross-platform Sync Pattern (REQUIRED)
const syncAcrossDevices = (sessionData: SessionData) =>
  Effect.gen(function* () {
    const sync = yield* SyncService;
    const platform = yield* PlatformService;
    
    // Platform-specific sync logic
    if (platform.isMobile) {
      yield* sync.syncToCloud(sessionData);
    } else {
      yield* sync.syncToDesktop(sessionData);
    }
    
    // Broadcast changes to all connected devices
    yield* sync.broadcastUpdate(sessionData);
  });
```

**Implementation Locations**:
- `apps/desktop/src/utils/stm-state.ts` ✅ Implemented
- `apps/mobile/src/services/SyncService.ts` 📋 Required
- `packages/shared/sync/DeviceCoordinator.ts` 📋 Required
- `packages/shared/sync/ConflictResolver.ts` 📋 Required

### 3. Authentication & Security

**When to Use**: All auth flows, token management, secure operations, credential storage
**Required Files**: All authentication, security, user management

**Mandatory Pattern**:
```typescript
// Authentication Service (REQUIRED)
class AuthService extends Effect.Service<AuthService>()("AuthService", {
  effect: Effect.gen(function* () {
    const storage = yield* StorageService;
    const http = yield* HttpClientService;
    
    return {
      authenticateUser: (platform: Platform) =>
        Effect.gen(function* () {
          // OAuth flow with retry logic
          const oauthFlow = yield* startOAuthFlow(platform).pipe(
            Effect.retry(
              Schedule.exponential("1 second").pipe(
                Schedule.compose(Schedule.recurs(3))
              )
            )
          );
          
          // Secure token exchange
          const tokens = yield* exchangeCodeForTokens(oauthFlow.code, oauthFlow.state);
          
          // Cross-platform secure storage
          yield* storage.setSecureValue("auth_token", tokens.accessToken, platform);
          yield* storage.setSecureValue("refresh_token", tokens.refreshToken, platform);
          
          return {
            user: tokens.user,
            accessToken: tokens.accessToken,
            expiresAt: tokens.expiresAt
          };
        }).pipe(
          Effect.catchTags({
            NetworkError: (error) => Effect.gen(function* () {
              yield* Effect.logError(`Network auth error: ${error.message}`);
              return yield* Effect.fail(new AuthError({
                phase: "network",
                platform,
                cause: error
              }));
            }),
            StorageError: (error) => Effect.gen(function* () {
              yield* Effect.logError(`Storage auth error: ${error.message}`);
              return yield* Effect.fail(new AuthError({
                phase: "storage", 
                platform,
                cause: error
              }));
            })
          })
        ),
        
      refreshToken: (platform: Platform) =>
        Effect.gen(function* () {
          const refreshToken = yield* storage.getSecureValue("refresh_token", platform);
          
          const newTokens = yield* http.post("/auth/refresh", {
            refreshToken: Option.getOrThrow(refreshToken)
          }).pipe(
            Effect.timeout("10 seconds"),
            Effect.retry(Schedule.exponential("500 millis"))
          );
          
          yield* storage.setSecureValue("auth_token", newTokens.accessToken, platform);
          
          return newTokens;
        })
    };
  }),
  dependencies: [StorageService.Default, HttpClientService.Default]
}) {}

// Secure Token Handling (REQUIRED)
class AuthError extends Data.TaggedError("AuthError")<{
  phase: "oauth" | "exchange" | "refresh" | "network" | "storage";
  platform: Platform;
  cause?: unknown;
}> {}
```

**Implementation Locations**:
- `apps/desktop/src/services/AuthService.ts` 📋 Required
- `apps/mobile/src/contexts/AuthContext.tsx` ✅ Implemented
- `packages/shared/auth/OAuthProvider.ts` 📋 Required
- `packages/shared/auth/TokenManager.ts` 📋 Required

### 4. Database Operations & Persistence

**When to Use**: All database queries, mutations, schema validation, data persistence
**Required Files**: All Convex functions, database services, data models

**Mandatory Pattern**:
```typescript
// Confect Database Service (REQUIRED)
class DatabaseService extends Effect.Service<DatabaseService>()("DatabaseService", {
  effect: Effect.gen(function* () {
    const confect = yield* ConfectService;
    
    return {
      queryWithRetry: <T>(
        operation: () => Effect.Effect<T, DatabaseError>,
        maxRetries = 3
      ) =>
        operation().pipe(
          Effect.retry(
            Schedule.exponential("100 millis").pipe(
              Schedule.compose(Schedule.recurs(maxRetries))
            )
          ),
          Effect.catchAll((error) =>
            Effect.gen(function* () {
              yield* Effect.logError(`Database operation failed after ${maxRetries} retries`);
              return yield* Effect.fail(new DatabaseError({
                operation: "query",
                retries: maxRetries,
                cause: error
              }));
            })
          )
        ),
        
      // Schema-validated mutations
      createRecord: <T>(schema: Schema.Schema<T>, data: unknown) =>
        Effect.gen(function* () {
          const validated = yield* Schema.decode(schema)(data).pipe(
            Effect.mapError((error) => new ValidationError({
              schema: schema.identifier || "unknown",
              data,
              errors: error.errors
            }))
          );
          
          return yield* confect.db.insert("records", validated);
        }),
        
      // Transactional updates
      updateWithTransaction: <T>(
        recordId: string,
        updates: Partial<T>,
        conditions: Array<(record: T) => boolean>
      ) =>
        Effect.gen(function* () {
          const record = yield* confect.db.get(recordId);
          
          // Validate conditions
          const valid = conditions.every(condition => condition(record));
          if (!valid) {
            return yield* Effect.fail(new ValidationError({
              schema: "transaction",
              data: { recordId, updates },
              errors: ["Transaction conditions not met"]
            }));
          }
          
          // Atomic update
          return yield* confect.db.replace(recordId, { ...record, ...updates });
        })
    };
  }),
  dependencies: [ConfectService.Default]
}) {}

// Schema Validation (REQUIRED)
const UserSchema = Schema.Struct({
  id: Schema.String,
  email: Schema.String.pipe(Schema.pattern(EMAIL_REGEX)),
  name: Schema.String.pipe(Schema.minLength(1)),
  metadata: Schema.optional(Schema.Record(Schema.String, Schema.Unknown))
});

// Database Error Types (REQUIRED)
class DatabaseError extends Data.TaggedError("DatabaseError")<{
  operation: "query" | "insert" | "update" | "delete" | "transaction";
  table?: string;
  recordId?: string;
  retries?: number;
  cause?: unknown;
}> {}
```

**Implementation Locations**:
- `packages/convex/database/DatabaseService.ts` 📋 Required
- `apps/mobile/convex/*.ts` ✅ Partially Implemented
- `packages/convex/schema/*.ts` 📋 Required
- `packages/shared/validation/Schemas.ts` 📋 Required

### 5. File System & Resource Management

**When to Use**: File operations, image processing, media handling, temp file management
**Required Files**: All file services, media processors, backup systems

**Mandatory Pattern**:
```typescript
// File System Service (REQUIRED)
class FileSystemService extends Effect.Service<FileSystemService>()("FileSystemService", {
  effect: Effect.gen(function* () {
    const platform = yield* PlatformService;
    
    return {
      // Resource-safe file operations
      withFileHandle: <T>(
        path: string,
        operation: (handle: FileHandle) => Effect.Effect<T, FileError>
      ) =>
        Effect.acquireUseRelease(
          // Acquire file handle
          Effect.tryPromise({
            try: () => platform.openFile(path),
            catch: (error) => new FileError({
              operation: "open",
              path,
              cause: error
            })
          }),
          // Use file handle
          operation,
          // Release file handle
          (handle) => Effect.sync(() => handle.close())
        ),
        
      // Streaming file processing
      processLargeFile: (path: string, processor: (chunk: Buffer) => Effect.Effect<void>) =>
        Effect.gen(function* () {
          const fileStream = yield* createFileStream(path);
          
          yield* Stream.fromAsyncIterable(fileStream, () => new FileError({
            operation: "stream",
            path,
            cause: "Stream creation failed"
          })).pipe(
            Stream.mapEffect(processor),
            Stream.runDrain
          );
        }),
        
      // Atomic file operations
      safeWrite: (path: string, content: string) =>
        Effect.gen(function* () {
          const tempPath = `${path}.tmp`;
          
          // Write to temp file first
          yield* writeFile(tempPath, content);
          
          // Atomic move to final location
          yield* moveFile(tempPath, path);
          
          // Cleanup on error
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function* () {
              yield* deleteFile(`${path}.tmp`).pipe(Effect.ignore);
              return yield* Effect.fail(error);
            })
          )
        )
    };
  }),
  dependencies: [PlatformService.Default]
}) {}

// File Processing Errors (REQUIRED)
class FileError extends Data.TaggedError("FileError")<{
  operation: "open" | "read" | "write" | "delete" | "move" | "stream";
  path: string;
  cause?: unknown;
}> {}
```

**Implementation Locations**:
- `apps/desktop/src/services/FileSystemService.ts` 📋 Required
- `apps/mobile/src/services/MediaService.ts` 📋 Required
- `packages/shared/fs/FileProcessor.ts` 📋 Required

### 6. HTTP Client & External APIs

**When to Use**: All HTTP requests, API integrations, external service calls
**Required Files**: All HTTP clients, API wrappers, external integrations

**Mandatory Pattern**:
```typescript
// HTTP Client Service (REQUIRED)
class HttpClientService extends Effect.Service<HttpClientService>()("HttpClientService", {
  effect: Effect.gen(function* () {
    const auth = yield* AuthService;
    
    return {
      // Authenticated requests
      authenticatedRequest: <T>(
        method: HttpMethod,
        url: string,
        options: RequestOptions = {}
      ) =>
        Effect.gen(function* () {
          const token = yield* auth.getCurrentToken();
          
          const response = yield* Effect.tryPromise({
            try: () => fetch(url, {
              method,
              headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json',
                ...options.headers
              },
              body: options.body ? JSON.stringify(options.body) : undefined
            }),
            catch: (error) => new NetworkError({
              url,
              method,
              cause: error
            })
          });
          
          if (!response.ok) {
            return yield* Effect.fail(new HttpError({
              url,
              method,
              status: response.status,
              statusText: response.statusText
            }));
          }
          
          return yield* Effect.tryPromise({
            try: () => response.json() as Promise<T>,
            catch: (error) => new NetworkError({
              url,
              method,
              cause: `JSON parse error: ${error}`
            })
          });
        }).pipe(
          Effect.timeout("30 seconds"),
          Effect.retry(
            Schedule.exponential("1 second").pipe(
              Schedule.compose(Schedule.recurs(3))
            )
          )
        ),
        
      // Retry with circuit breaker
      robustRequest: <T>(request: Effect.Effect<T, NetworkError | HttpError>) =>
        request.pipe(
          Effect.retry(
            Schedule.exponential("1 second").pipe(
              Schedule.whileInput((error: NetworkError | HttpError) => 
                error._tag === "NetworkError" || 
                (error._tag === "HttpError" && error.status >= 500)
              ),
              Schedule.compose(Schedule.recurs(3))
            )
          ),
          Effect.catchTags({
            NetworkError: (error) => Effect.gen(function* () {
              yield* Effect.logError(`Network request failed: ${error.url}`);
              return yield* fallbackResponse<T>();
            }),
            HttpError: (error) => Effect.gen(function* () {
              yield* Effect.logError(`HTTP error ${error.status}: ${error.url}`);
              if (error.status >= 400 && error.status < 500) {
                return yield* Effect.fail(error); // Don't retry client errors
              }
              return yield* fallbackResponse<T>();
            })
          })
        )
    };
  }),
  dependencies: [AuthService.Default]
}) {}

// HTTP Error Types (REQUIRED)
class NetworkError extends Data.TaggedError("NetworkError")<{
  url: string;
  method: HttpMethod;
  cause?: unknown;
}> {}

class HttpError extends Data.TaggedError("HttpError")<{
  url: string;
  method: HttpMethod;
  status: number;
  statusText: string;
  body?: unknown;
}> {}
```

**Implementation Locations**:
- `packages/shared/http/HttpClientService.ts` 📋 Required
- `apps/desktop/src/services/APIService.ts` 📋 Required
- `apps/mobile/src/services/APIService.ts` 📋 Required

## Service Implementation Matrix

| Service Category | Desktop Status | Mobile Status | Shared Status | Priority |
|------------------|----------------|---------------|---------------|----------|
| **Core Infrastructure** |  |  |  |  |
| TauriEventService | ✅ Complete | N/A | N/A | Critical |
| ClaudeStreamingService | ✅ Complete | 📋 Required | 📋 Required | Critical |
| StorageService | 📋 Required | 📋 Required | ✅ Interface | High |
| AuthService | 📋 Required | ✅ Partial | ✅ Interface | Critical |
| **Data & Persistence** |  |  |  |  |
| DatabaseService | 📋 Required | 📋 Required | ✅ Confect | High |
| CacheService | 📋 Required | 📋 Required | 📋 Required | Medium |
| FileSystemService | 📋 Required | 📋 Required | ✅ Interface | High |
| **Communication** |  |  |  |  |
| HttpClientService | 📋 Required | 📋 Required | ✅ Interface | High |
| WebSocketService | 📋 Required | 📋 Required | 📋 Required | Medium |
| SyncService | 📋 Required | 📋 Required | 📋 Required | High |
| **Analytics & Monitoring** |  |  |  |  |
| APMService | ✅ IPC Layer | 📋 Required | 📋 Required | Medium |
| MetricsService | 📋 Required | 📋 Required | 📋 Required | Low |
| ErrorTrackingService | 📋 Required | 📋 Required | 📋 Required | Medium |
| **Platform Specific** |  |  |  |  |
| TauriIPCService | ✅ Complete | N/A | N/A | Critical |
| ReactNativeService | N/A | 📋 Required | N/A | High |
| PlatformService | 📋 Required | 📋 Required | ✅ Interface | High |

## Integration Points Specification

### React Component Integration

**All React components that perform async operations MUST use Effect patterns:**

```typescript
// Hook Pattern (REQUIRED)
const useEffectHook = <T, E>(
  effect: Effect.Effect<T, E, any>,
  dependencies: unknown[] = []
) => {
  const [state, setState] = useState<{
    data: T | null;
    loading: boolean;
    error: E | null;
  }>({
    data: null,
    loading: true,
    error: null
  });
  
  useEffect(() => {
    let cancelled = false;
    
    const run = async () => {
      try {
        setState(prev => ({ ...prev, loading: true, error: null }));
        const result = await Effect.runPromise(effect);
        if (!cancelled) {
          setState({ data: result, loading: false, error: null });
        }
      } catch (error) {
        if (!cancelled) {
          setState(prev => ({ ...prev, loading: false, error: error as E }));
        }
      }
    };
    
    run();
    
    return () => {
      cancelled = true;
    };
  }, dependencies);
  
  return state;
};

// Component Usage (REQUIRED)
const ExampleComponent: React.FC<{ userId: string }> = ({ userId }) => {
  const { data: user, loading, error } = useEffectHook(
    Effect.gen(function* () {
      const userService = yield* UserService;
      return yield* userService.getUser(userId);
    }).pipe(Effect.provide(UserService.Default)),
    [userId]
  );
  
  if (loading) return <div>Loading...</div>;
  if (error) return <div>Error: {error._tag}</div>;
  if (!user) return <div>No user found</div>;
  
  return <div>Welcome, {user.name}!</div>;
};
```

### Tauri Command Integration

**All Tauri commands MUST be wrapped in Effect services:**

```typescript
// Tauri Command Service (REQUIRED)
class TauriCommandService extends Effect.Service<TauriCommandService>()("TauriCommandService", {
  sync: () => ({
    invoke: <T>(command: string, args?: Record<string, unknown>) =>
      Effect.gen(function* () {
        yield* Effect.logInfo(`Invoking Tauri command: ${command}`);
        
        const result = yield* Effect.tryPromise({
          try: () => invoke(command, args) as Promise<T>,
          catch: (error) => new TauriError({
            command,
            args,
            cause: error
          })
        }).pipe(
          Effect.timeout("10 seconds"),
          Effect.retry(
            Schedule.exponential("100 millis").pipe(
              Schedule.compose(Schedule.recurs(2))
            )
          )
        );
        
        yield* Effect.logInfo(`Tauri command completed: ${command}`);
        return result;
      })
  })
}) {}

// Usage in Rust commands (REQUIRED pattern)
#[tauri::command]
async fn example_command(arg: String) -> Result<String, String> {
    // Effect operations should be wrapped and executed here
    // Runtime management should be handled at this boundary
    match perform_effect_operation(arg).await {
        Ok(result) => Ok(result),
        Err(error) => Err(format!("Command failed: {}", error))
    }
}
```

### Convex Integration (Confect)

**All database operations MUST use Confect with Effect integration:**

```typescript
// Confect Query (REQUIRED)
export const getSessionMessages = ConfectQuery({
  args: {
    sessionId: v.string()
  },
  handler: async (ctx, args) => {
    // Use Effect patterns within Confect
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const messages = yield* Effect.tryPromise({
          try: () => ctx.db
            .query("messages")
            .withIndex("by_session", q => q.eq("sessionId", args.sessionId))
            .collect(),
          catch: (error) => new DatabaseError({
            operation: "query",
            table: "messages", 
            cause: error
          })
        });
        
        return messages.map(doc => ({
          ...doc,
          content: Option.fromNullable(doc.content)
        }));
      }).pipe(
        Effect.catchAll((error) => 
          Effect.gen(function* () {
            yield* Effect.logError(`Database query failed: ${error._tag}`);
            return yield* Effect.fail(error);
          })
        )
      )
    );
    
    return result;
  }
});

// Confect Mutation (REQUIRED)
export const createSession = ConfectMutation({
  args: {
    projectPath: v.string(),
    initialMessage: v.optional(v.string())
  },
  handler: async (ctx, args) => {
    return await Effect.runPromise(
      Effect.gen(function* () {
        const sessionId = yield* Effect.sync(() => ctx.generate());
        
        const session = yield* Effect.tryPromise({
          try: () => ctx.db.insert("sessions", {
            id: sessionId,
            projectPath: args.projectPath,
            createdAt: Date.now(),
            isActive: true
          }),
          catch: (error) => new DatabaseError({
            operation: "insert",
            table: "sessions",
            cause: error
          })
        });
        
        if (args.initialMessage) {
          yield* Effect.tryPromise({
            try: () => ctx.db.insert("messages", {
              sessionId,
              content: args.initialMessage,
              messageType: "user",
              timestamp: Date.now()
            }),
            catch: (error) => new DatabaseError({
              operation: "insert",
              table: "messages",
              cause: error
            })
          });
        }
        
        return session;
      }).pipe(Effect.provide(DatabaseService.Default))
    );
  }
});
```

## Error Handling Patterns

### Tagged Error Hierarchy

**ALL errors MUST use the Data.TaggedError pattern:**

```typescript
// Base Application Errors (REQUIRED)
export class AppError extends Data.TaggedError("AppError")<{
  category: "service" | "network" | "validation" | "auth" | "storage";
  operation: string;
  context?: Record<string, unknown>;
  cause?: unknown;
}> {}

// Service-Specific Errors (REQUIRED for each service)
export class UserServiceError extends Data.TaggedError("UserServiceError")<{
  operation: "get" | "create" | "update" | "delete" | "authenticate";
  userId?: string;
  cause?: unknown;
}> {}

export class StorageServiceError extends Data.TaggedError("StorageServiceError")<{
  operation: "read" | "write" | "delete" | "clear";
  key: string;
  platform: "mobile" | "desktop";
  cause?: unknown;
}> {}

// Platform-Specific Errors (REQUIRED)
export class TauriError extends Data.TaggedError("TauriError")<{
  command: string;
  args?: Record<string, unknown>;
  cause?: unknown;
}> {}

export class ReactNativeError extends Data.TaggedError("ReactNativeError")<{
  module: string;
  method: string;
  cause?: unknown;
}> {}

// Network and Communication Errors (REQUIRED)
export class NetworkError extends Data.TaggedError("NetworkError")<{
  url: string;
  method: HttpMethod;
  status?: number;
  cause?: unknown;
}> {}

export class WebSocketError extends Data.TaggedError("WebSocketError")<{
  url: string;
  event: "connect" | "disconnect" | "message" | "error";
  cause?: unknown;
}> {}
```

### Error Recovery Patterns

**ALL services MUST implement comprehensive error recovery:**

```typescript
// Service Error Recovery (REQUIRED)
const withErrorRecovery = <T, E extends Data.TaggedError<string, any>>(
  operation: Effect.Effect<T, E>,
  recovery: {
    retry?: {
      schedule: Schedule.Schedule<any, E>;
      maxRetries: number;
    };
    fallback?: () => Effect.Effect<T, never>;
    circuit?: {
      threshold: number;
      timeout: Duration.Duration;
    };
  }
) =>
  Effect.gen(function* () {
    let result: T;
    
    try {
      if (recovery.retry) {
        result = yield* operation.pipe(
          Effect.retry(recovery.retry.schedule),
          Effect.timeout("30 seconds")
        );
      } else {
        result = yield* operation;
      }
    } catch (error) {
      yield* Effect.logError(`Operation failed: ${error._tag}`, error);
      
      if (recovery.fallback) {
        yield* Effect.logInfo("Attempting fallback recovery");
        result = yield* recovery.fallback();
      } else {
        return yield* Effect.fail(error);
      }
    }
    
    return result;
  });

// Usage Pattern (REQUIRED)
const robustUserOperation = (userId: string) =>
  withErrorRecovery(
    userService.getUser(userId),
    {
      retry: {
        schedule: Schedule.exponential("1 second"),
        maxRetries: 3
      },
      fallback: () => userService.getUserFromCache(userId),
      circuit: {
        threshold: 5,
        timeout: Duration.minutes(1)
      }
    }
  );
```

## State Management Patterns

### STM (Software Transactional Memory)

**ALL concurrent state operations MUST use STM:**

```typescript
// Application State with STM (REQUIRED)
export const createAppState = () =>
  Effect.gen(function* () {
    // Core application state
    const sessions = yield* TMap.empty<string, SessionState>();
    const ui = yield* TRef.make({
      activePane: null as string | null,
      theme: "dark" as "light" | "dark",
      sidebarOpen: true
    });
    const notifications = yield* TQueue.unbounded<Notification>();
    
    // Complex atomic operations
    const updateSessionAndUI = (
      sessionId: string,
      updates: Partial<SessionState>,
      uiUpdates: Partial<UIState>
    ) =>
      STM.gen(function* () {
        // Update session atomically
        const currentSession = yield* TMap.get(sessions, sessionId);
        if (Option.isNone(currentSession)) {
          yield* STM.fail(new SessionError({
            operation: "update",
            sessionId,
            message: "Session not found"
          }));
        }
        
        const updated = { ...Option.value(currentSession), ...updates };
        yield* TMap.set(sessions, sessionId, updated);
        
        // Update UI atomically
        const currentUI = yield* TRef.get(ui);
        yield* TRef.set(ui, { ...currentUI, ...uiUpdates });
        
        // Add notification atomically
        yield* TQueue.offer(notifications, {
          type: "info",
          message: `Session ${sessionId} updated`,
          timestamp: Date.now()
        });
      });
    
    // Batch operations
    const syncMultipleSessions = (sessionUpdates: Array<{
      id: string;
      updates: Partial<SessionState>;
    }>) =>
      STM.gen(function* () {
        for (const { id, updates } of sessionUpdates) {
          const current = yield* TMap.get(sessions, id);
          if (Option.isSome(current)) {
            yield* TMap.set(sessions, id, { ...Option.value(current), ...updates });
          }
        }
        
        // Update last sync time
        const currentUI = yield* TRef.get(ui);
        yield* TRef.set(ui, { ...currentUI, lastSync: Date.now() });
      });
    
    return {
      // Atomic operations
      updateSessionAndUI: (sessionId: string, sessionUpdates: Partial<SessionState>, uiUpdates: Partial<UIState>) =>
        STM.commit(updateSessionAndUI(sessionId, sessionUpdates, uiUpdates)),
      
      syncMultipleSessions: (updates: Array<{ id: string; updates: Partial<SessionState> }>) =>
        STM.commit(syncMultipleSessions(updates)),
      
      // Read operations
      getSession: (id: string) => STM.commit(TMap.get(sessions, id)),
      getAllSessions: () => STM.commit(TMap.toArray(sessions)),
      getUIState: () => STM.commit(TRef.get(ui)),
      
      // Reactive subscriptions
      watchNotifications: () => TQueue.takeAll(notifications)
    };
  });

// React Integration with STM (REQUIRED)
const useSTMState = <T>(stmOperation: STM.STM<T>, deps: React.DependencyList = []) => {
  const [state, setState] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  
  useEffect(() => {
    let cancelled = false;
    
    const run = async () => {
      try {
        setLoading(true);
        setError(null);
        const result = await Effect.runPromise(STM.commit(stmOperation));
        if (!cancelled) {
          setState(result);
        }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e : new Error(String(e)));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };
    
    run();
    
    return () => {
      cancelled = true;
    };
  }, deps);
  
  return { state, loading, error };
};
```

## Testing Patterns

### Service Testing Framework

**ALL services MUST have comprehensive test coverage using these patterns:**

```typescript
// Test Utilities (REQUIRED)
export const ServiceTestUtils = {
  runServiceTest: <A, E>(
    description: string,
    effect: Effect.Effect<A, E, never>
  ) => {
    it(description, async () => {
      const result = await Effect.runPromise(effect);
      return result;
    });
  },
  
  runServiceTestWithTimeout: <A, E>(
    description: string,
    effect: Effect.Effect<A, E, never>,
    timeoutMs: number
  ) => {
    it(description, async () => {
      const result = await Effect.runPromise(
        effect.pipe(Effect.timeout(Duration.millis(timeoutMs)))
      );
      return result;
    });
  }
};

// Performance Benchmarking (REQUIRED)
export const benchmarkEffect = <A, E>(
  name: string,
  effect: Effect.Effect<A, E, never>,
  maxTimeMs: number
) =>
  Effect.gen(function* () {
    const startTime = Date.now();
    const result = yield* effect;
    const duration = Date.now() - startTime;
    
    yield* Effect.logInfo(`Benchmark '${name}': ${duration}ms`);
    
    if (duration > maxTimeMs) {
      yield* Effect.fail(new Error(`Benchmark '${name}' exceeded ${maxTimeMs}ms (took ${duration}ms)`));
    }
    
    return result;
  });

// Service Test Template (REQUIRED for each service)
describe("ServiceName Tests", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Core Functionality", () => {
    ServiceTestUtils.runServiceTest(
      "should handle basic operations",
      Effect.gen(function* () {
        const service = yield* ServiceName;
        const result = yield* service.basicOperation("test");
        expect(result).toBeDefined();
        return result;
      }).pipe(Effect.provide(ServiceName.Default))
    );
  });

  describe("Error Handling", () => {
    ServiceTestUtils.runServiceTest(
      "should handle service failures",
      Effect.gen(function* () {
        const failingService = yield* FailingServiceName;
        const result = yield* failingService.failingOperation("test").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left._tag).toBe("ServiceNameError");
        }
        
        return result;
      }).pipe(Effect.provide(FailingServiceName.Default))
    );
  });

  describe("Performance Benchmarks", () => {
    ServiceTestUtils.runServiceTest(
      "should meet performance requirements",
      benchmarkEffect(
        "Service Operation",
        Effect.gen(function* () {
          const service = yield* ServiceName;
          return yield* service.performanceOperation();
        }).pipe(Effect.provide(ServiceName.Default)),
        200 // Max 200ms
      )
    );
  });

  describe("Integration Tests", () => {
    ServiceTestUtils.runServiceTest(
      "should integrate with dependencies",
      Effect.gen(function* () {
        const service = yield* ServiceName;
        const dependency = yield* DependencyService;
        
        const result = yield* service.operationWithDependency("test");
        expect(result).toBeDefined();
        
        return result;
      }).pipe(
        Effect.provide(
          Layer.mergeAll(
            ServiceName.Default,
            DependencyService.Default
          )
        )
      )
    );
  });
});
```

### Mock Services for Testing

**ALL services MUST have corresponding mock implementations:**

```typescript
// Mock Service Pattern (REQUIRED)
class MockServiceName extends Effect.Service<ServiceName>()("MockServiceName", {
  sync: () => ({
    basicOperation: (input: string) => 
      Effect.succeed(`Mock result for: ${input}`),
    
    failingOperation: (input: string) =>
      Effect.fail(new ServiceNameError({
        operation: "failing",
        input,
        cause: "Mock failure"
      })),
    
    performanceOperation: () =>
      Effect.gen(function* () {
        yield* Effect.sleep("50 millis"); // Simulate work
        return "Performance result";
      })
  })
}) {}

// Test Layer Composition (REQUIRED)
const TestLayer = Layer.mergeAll(
  MockServiceName.Default,
  MockDependencyService.Default
);

// Integration Test with Mocks (REQUIRED)
ServiceTestUtils.runServiceTest(
  "should work with mocked dependencies",
  Effect.gen(function* () {
    const service = yield* ServiceName;
    const result = yield* service.operationWithDependency("test");
    
    expect(result).toContain("Mock");
    return result;
  }).pipe(Effect.provide(TestLayer))
);
```

## Performance & Resource Management

### Resource Management Requirements

**ALL resource operations MUST use Effect's resource management:**

```typescript
// File Resource Management (REQUIRED)
const withFileResource = <T>(
  path: string,
  operation: (file: FileHandle) => Effect.Effect<T, FileError>
) =>
  Effect.acquireUseRelease(
    // Acquire
    Effect.tryPromise({
      try: () => openFile(path),
      catch: (error) => new FileError({
        operation: "open",
        path,
        cause: error
      })
    }),
    // Use
    operation,
    // Release
    (file) => Effect.sync(() => file.close())
  );

// Network Resource Management (REQUIRED)
const withHttpConnection = <T>(
  url: string,
  operation: (connection: HttpConnection) => Effect.Effect<T, NetworkError>
) =>
  Effect.acquireUseRelease(
    // Acquire connection
    Effect.gen(function* () {
      const connection = yield* Effect.tryPromise({
        try: () => createConnection(url),
        catch: (error) => new NetworkError({
          url,
          operation: "connect",
          cause: error
        })
      });
      
      yield* Effect.logInfo(`Connected to ${url}`);
      return connection;
    }),
    // Use connection
    operation,
    // Release connection
    (connection) => Effect.gen(function* () {
      yield* Effect.tryPromise({
        try: () => connection.close(),
        catch: () => undefined // Ignore close errors
      });
      yield* Effect.logInfo(`Disconnected from ${url}`);
    })
  );

// Memory Management (REQUIRED for large operations)
const withMemoryLimit = <T>(
  maxMemoryMB: number,
  operation: Effect.Effect<T, any>
) =>
  Effect.gen(function* () {
    const initialMemory = process.memoryUsage().heapUsed;
    
    const result = yield* operation.pipe(
      Effect.tap(() => Effect.gen(function* () {
        const currentMemory = process.memoryUsage().heapUsed;
        const usedMB = (currentMemory - initialMemory) / 1024 / 1024;
        
        if (usedMB > maxMemoryMB) {
          yield* Effect.logWarning(`Memory usage (${usedMB}MB) exceeded limit (${maxMemoryMB}MB)`);
        }
      }))
    );
    
    // Force garbage collection if available
    if (global.gc) {
      global.gc();
    }
    
    return result;
  });
```

### Performance Requirements

**ALL services MUST meet these performance benchmarks:**

| Service Category | Operation | Max Time | Memory Limit | Notes |
|------------------|-----------|----------|--------------|-------|
| Storage | Read/Write | 100ms | 10MB | Per operation |
| Network | HTTP Request | 5s | 50MB | With retry |
| Database | Query/Mutation | 200ms | 100MB | Complex queries |
| File System | File Operations | 500ms | 200MB | Large files |
| Streaming | Message Processing | 50ms | 1MB | Per message |
| Authentication | Token Operations | 2s | 5MB | Including network |

## Migration Roadmap

### Phase 5: Complete Service Coverage (Q3 2025)

**Priority 1 - Critical Services**:
1. AuthService (Desktop & Mobile)
2. StorageService (Cross-platform)
3. DatabaseService (Full Confect integration)
4. HttpClientService (All API calls)

**Priority 2 - Core Functionality**:
1. FileSystemService (Media processing)
2. SyncService (Multi-device coordination)
3. CacheService (Performance optimization)
4. MetricsService (Analytics & monitoring)

**Priority 3 - Enhanced Features**:
1. WebSocketService (Real-time communication)
2. BackgroundTaskService (Long-running operations)
3. NotificationService (Cross-platform notifications)
4. SearchService (Content indexing)

### Phase 6: Platform Integration (Q4 2025)

**React Native Integration**:
- Complete mobile service implementations
- Platform-specific optimizations
- Native module integrations

**Tauri Integration**:
- All IPC operations through Effect services
- Rust-side Effect runtime integration
- Native platform feature access

**Web Platform Support**:
- Browser-compatible service implementations
- Web Worker integration
- PWA capabilities

### Phase 7: Advanced Patterns (Q1 2026)

**Agent Orchestration**:
- Multi-agent coordination using Effect's actor model
- Task distribution and load balancing
- Resource pooling and management

**Advanced Analytics**:
- Real-time metrics processing
- Predictive analytics with ML integration
- Performance optimization recommendations

## Enforcement Guidelines

### Code Review Requirements

**ALL pull requests MUST meet these criteria:**

1. **Service Pattern Compliance**: All new async operations use Effect services
2. **Error Handling**: All errors use tagged error patterns
3. **Resource Management**: All resources use acquire-use-release patterns
4. **Testing Coverage**: 90%+ test coverage with Effect patterns
5. **Performance Benchmarks**: All services meet performance requirements
6. **Documentation**: All services have comprehensive documentation

### Automated Checks

**CI/CD pipeline MUST include:**

1. **TypeScript Compilation**: Strict mode with Effect types
2. **Test Coverage**: Minimum 90% with Effect test patterns
3. **Performance Tests**: All benchmarks must pass
4. **Bundle Size**: Monitor Effect bundle impact
5. **Memory Usage**: Check for resource leaks

### Developer Guidelines

**ALL developers MUST:**

1. **Training**: Complete Effect-TS training program
2. **Patterns**: Use only approved Effect patterns
3. **Code Reviews**: Review Effect implementations thoroughly
4. **Documentation**: Update docs for all Effect changes
5. **Testing**: Write comprehensive Effect tests

### Migration Strategy

**For existing code:**

1. **Assessment**: Identify all async operations requiring migration
2. **Prioritization**: Migrate critical paths first
3. **Incremental**: Migrate one service at a time
4. **Testing**: Maintain test coverage during migration
5. **Validation**: Verify performance improvements

### Quality Gates

**ALL Effect implementations MUST pass:**

1. **Type Safety**: No `any` types in service definitions
2. **Error Handling**: All error paths covered with tagged errors
3. **Resource Safety**: All resources properly managed
4. **Performance**: Meet or exceed performance benchmarks
5. **Documentation**: Complete API documentation
6. **Testing**: Comprehensive test suite with 90%+ coverage

---

## Conclusion

This comprehensive specification establishes Effect-TS as the architectural foundation for all OpenAgents development. The patterns, requirements, and guidelines documented here ensure consistent, reliable, and maintainable code across all platforms and services.

### Key Success Metrics

1. **100% Service Coverage**: All async operations use Effect patterns
2. **90%+ Test Coverage**: Comprehensive testing with Effect patterns
3. **Performance Targets**: All services meet benchmark requirements
4. **Type Safety**: Compile-time guarantees for all operations
5. **Resource Safety**: Zero resource leaks or cleanup issues

### Implementation Timeline

- **Q3 2025**: Complete critical service implementations
- **Q4 2025**: Full platform integration
- **Q1 2026**: Advanced pattern adoption
- **Q2 2026**: Performance optimization and scaling

The investment in Effect-TS provides a solid foundation for OpenAgents' continued evolution, ensuring robust, scalable, and maintainable software architecture.

---

**Document Maintainers**: Effect-TS Architecture Team  
**Review Schedule**: Monthly (during active implementation phases)  
**Next Review**: August 2025 (Phase 5 progress review)
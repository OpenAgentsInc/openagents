# Claude Code Sessions in Effect-TS: Complete System Architecture

## Overview

This document provides a comprehensive analysis of how Claude Code sessions are represented and managed using Effect-TS patterns in the OpenAgents project. The system represents a sophisticated multi-layer architecture that combines Effect-TS functional programming patterns with Convex real-time database capabilities through the Confect framework.

Claude Code sessions in this system are **not implemented as a dedicated Effect-TS service layer**, but rather as **database entities with Effect-based operations** that provide type-safe, composable, and error-safe session management across desktop and mobile platforms.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Core Data Model](#core-data-model)
3. [Effect-TS Integration Patterns](#effect-ts-integration-patterns)
4. [Session Lifecycle Management](#session-lifecycle-management)
5. [Cross-Platform Considerations](#cross-platform-considerations)
6. [Error Handling & Type Safety](#error-handling--type-safety)
7. [Performance & Scalability](#performance--scalability)
8. [Integration with React](#integration-with-react)
9. [Mobile UI Interaction Flow](#mobile-ui-interaction-flow)
10. [Testing Patterns](#testing-patterns)
11. [Future Enhancements](#future-enhancements)

## Architecture Overview

### System Architecture Diagram

```mermaid
graph TB
    subgraph "Frontend Layer"
        DR[Desktop React Hooks]
        MR[Mobile React Native Hooks]
    end
    
    subgraph "Effect Runtime Layer"
        RT[Effect Runtime]
        SRV[Service Layer]
    end
    
    subgraph "Confect Layer (Effect + Convex)"
        CM[Confect Mutations]
        CQ[Confect Queries]
        CS[Confect Schema]
    end
    
    subgraph "Database Layer"
        CD[claudeSessions Table]
        CM_DB[claudeMessages Table]
        SS[syncStatus Table]
        UDS[userDeviceSessions Table]
    end
    
    subgraph "External Systems"
        CC[Claude Code CLI]
        TR[Tauri Runtime]
    end
    
    DR --> RT
    MR --> RT
    RT --> CM
    RT --> CQ
    CM --> CD
    CM --> CM_DB
    CQ --> CD
    CQ --> CM_DB
    CS --> CD
    CS --> CM_DB
    DR --> TR
    TR --> CC
    
    style RT fill:#e1f5fe
    style CS fill:#f3e5f5
    style CD fill:#e8f5e8
    style CC fill:#fff3e0
```

### Architectural Principles

1. **Effect-First Design**: All database operations use `Effect.gen` patterns for composable, type-safe operations
2. **Schema-Driven Development**: Effect Schema definitions provide compile-time type safety and runtime validation
3. **Functional Error Handling**: Tagged errors (`MobileSyncError`, `SessionValidationError`) replace traditional exception handling
4. **Option Types**: `Option` types eliminate null/undefined errors throughout the session lifecycle
5. **Immutable State**: All session updates are immutable transformations through Effect operations

## Core Data Model

### Primary Session Schema

The `claudeSessions` table represents the core session entity:

```typescript
// Schema Definition (packages/convex/confect/schema.ts)
claudeSessions: defineTable(
  Schema.Struct({
    sessionId: Schema.String.pipe(Schema.nonEmptyString()),
    projectPath: Schema.String.pipe(Schema.nonEmptyString()),
    title: Schema.optional(Schema.String),
    status: Schema.Literal("active", "inactive", "error", "processed"),
    createdBy: Schema.Literal("desktop", "mobile"),
    lastActivity: Schema.Number,
    userId: Schema.optional(Id.Id("users")),
    metadata: Schema.optional(
      Schema.Struct({
        workingDirectory: Schema.optional(Schema.String),
        model: Schema.optional(Schema.String),
        systemPrompt: Schema.optional(Schema.String),
        originalMobileSessionId: Schema.optional(Schema.String),
      })
    ),
  })
).index("by_session_id", ["sessionId"])
 .index("by_status", ["status"])
 .index("by_last_activity", ["lastActivity"])
 .index("by_user_id", ["userId"])
```

### Session Message Schema

Messages within sessions are stored in the `claudeMessages` table:

```typescript
claudeMessages: defineTable(
  Schema.Struct({
    sessionId: Schema.String.pipe(Schema.nonEmptyString()),
    messageId: Schema.String.pipe(Schema.nonEmptyString()),
    messageType: Schema.Literal("user", "assistant", "tool_use", "tool_result", "thinking"),
    content: Schema.String,
    timestamp: Schema.String, // ISO timestamp
    userId: Schema.optional(Id.Id("users")),
    toolInfo: Schema.optional(
      Schema.Struct({
        toolName: Schema.String,
        toolUseId: Schema.String,
        input: Schema.Any,
        output: Schema.optional(Schema.String),
      })
    ),
    metadata: Schema.optional(Schema.Any),
  })
).index("by_session_id", ["sessionId"])
 .index("by_timestamp", ["timestamp"])
 .index("by_user_id", ["userId"])
```

### Session Status Tracking

The `syncStatus` table tracks synchronization state across platforms:

```typescript
syncStatus: defineTable(
  Schema.Struct({
    sessionId: Schema.String.pipe(Schema.nonEmptyString()),
    lastSyncedMessageId: Schema.optional(Schema.String),
    desktopLastSeen: Schema.optional(Schema.Number),
    mobileLastSeen: Schema.optional(Schema.Number),
    syncErrors: Schema.optional(Schema.Array(Schema.String)),
  })
).index("by_session_id", ["sessionId"])
```

## Effect-TS Integration Patterns

### Effect Generators for Database Operations

All session operations use `Effect.gen` patterns for composable, type-safe database access:

```typescript
// Session Creation (packages/convex/confect/mobile_sync.ts)
export const createClaudeSession = mutation({
  args: CreateClaudeSessionArgs,
  returns: CreateClaudeSessionResult,
  handler: ({ sessionId, projectPath, createdBy, title, metadata }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      // Get authenticated user with Option handling
      const identity = yield* auth.getUserIdentity();
      let userId: any = undefined;
      
      if (Option.isSome(identity)) {
        const authSubject = identity.value.subject;
        const user = yield* db
          .query("users")
          .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
          .first();
          
        if (Option.isSome(user)) {
          userId = user.value._id;
        }
      }

      // Check if session exists using Option pattern
      const existingSession = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      return yield* Option.match(existingSession, {
        onSome: (session) =>
          // Update existing session
          db.patch(session._id, {
            title,
            status: "active" as const,
            lastActivity: Date.now(),
            metadata,
            ...(userId ? { userId } : {}),
          }).pipe(Effect.as(session._id)),
        
        onNone: () =>
          Effect.gen(function* () {
            // Create new session
            const sessionDoc = yield* db.insert("claudeSessions", {
              sessionId,
              projectPath,
              title: title || `${createdBy} Session - ${new Date().toLocaleString()}`,
              status: "active" as const,
              createdBy,
              lastActivity: Date.now(),
              ...(userId ? { userId } : {}),
              metadata,
            });

            // Initialize sync status
            yield* db.insert("syncStatus", {
              sessionId,
            });

            return sessionDoc;
          })
      });
    }),
});
```

### Option Types for Null Safety

The system extensively uses `Option` types to eliminate null/undefined errors:

```typescript
// Message Addition with Option Handling
export const addClaudeMessage = mutation({
  args: AddClaudeMessageArgs,
  returns: AddClaudeMessageResult,
  handler: ({ sessionId, messageId, messageType, content, timestamp, toolInfo, metadata }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      // Check for existing message using Option pattern
      const existingMessage = yield* db
        .query("claudeMessages")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .filter((q) => q.eq(q.field("messageId"), messageId))
        .first();

      return yield* Option.match(existingMessage, {
        onSome: (msg) => 
          Effect.gen(function* () {
            yield* Effect.logInfo(`⚠️ [CONFECT] Message ${messageId} already exists, skipping`);
            return msg._id;
          }),
        onNone: () =>
          Effect.gen(function* () {
            const messageDoc = yield* db.insert("claudeMessages", {
              sessionId,
              messageId,
              messageType,
              content,
              timestamp,
              toolInfo,
              metadata,
            });

            yield* Effect.logInfo(`✅ [CONFECT] Successfully added message ${messageId} to session ${sessionId}`);
            return messageDoc;
          })
      });
    }),
});
```

### Authentication Context Integration

The system integrates authentication through Effect context patterns:

```typescript
// Authentication Helper with Effect Patterns
const getAuthenticatedUserEffectMutation = Effect.gen(function* () {
  const { db, auth } = yield* ConfectMutationCtx;
  
  const identity = yield* auth.getUserIdentity();
  if (Option.isNone(identity)) {
    return yield* Effect.fail(new Error("Not authenticated"));
  }

  // Look up user by OpenAuth subject first
  const authSubject = identity.value.subject;
  const user = yield* db
    .query("users")
    .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
    .first();

  return yield* Option.match(user, {
    onSome: (u) => Effect.succeed(u),
    onNone: () =>
      // Fallback: try looking up by GitHub ID (for backwards compatibility)
      Effect.gen(function* () {
        const fallbackUser = yield* db
          .query("users")
          .withIndex("by_github_id", (q) => q.eq("githubId", authSubject))
          .first();
        
        return yield* Option.match(fallbackUser, {
          onSome: (u) => Effect.succeed(u),
          onNone: () => Effect.fail(new Error("User not found"))
        });
      })
  });
});
```

## Session Lifecycle Management

### Session Creation Flow

1. **Schema Validation**: Input parameters validated through Effect Schema
2. **Authentication Check**: User identity resolved through Effect context
3. **Duplicate Detection**: Existing sessions checked using Option patterns
4. **Database Transaction**: Session created with sync status initialization
5. **Activity Tracking**: Last activity timestamp and metadata recorded

```typescript
// Session Status Update with Effect Logging
export const updateSessionStatus = mutation({
  args: UpdateSessionStatusArgs,
  returns: UpdateSessionStatusResult,
  handler: ({ sessionId, status }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      yield* Option.match(session, {
        onSome: (s) =>
          db.patch(s._id, {
            status,
            lastActivity: Date.now(),
          }),
        onNone: () => Effect.void,
      });

      return null;
    }),
});
```

### Message Processing Pipeline

Messages flow through a multi-stage pipeline with Effect transformations:

1. **Deduplication**: Check for existing messages using Option patterns
2. **Validation**: Message type and content validation through Schema
3. **Persistence**: Atomic database insertion with metadata
4. **Sync Update**: Session activity and sync status updates
5. **Side Effects**: Logging and notification through Effect operations

### Session Query Patterns

```typescript
// Pending Mobile Sessions Query
export const getPendingMobileSessions = query({
  args: GetPendingMobileSessionsArgs,
  returns: GetPendingMobileSessionsResult,
  handler: () =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      const results = yield* db
        .query("claudeSessions")
        .withIndex("by_status", (q) => q.eq("status", "active"))
        .filter((q) => q.eq(q.field("createdBy"), "mobile"))
        .order("desc")
        .take(10);

      yield* Effect.logInfo(`🔍 [CONFECT] getPendingMobileSessions query returned: ${results.length} sessions`);
      
      if (results.length > 0) {
        yield* Effect.logInfo("📋 [CONFECT] Mobile sessions:", results.map(s => ({
          sessionId: s.sessionId,
          status: s.status,
          createdBy: s.createdBy,
          metadata: s.metadata
        })));
      }

      return results;
    }),
});
```

## Cross-Platform Considerations

### Mobile-Desktop Session Synchronization

The system handles two distinct session ID formats for cross-platform compatibility:

1. **Mobile Session IDs**: `mobile-${timestamp}-${random}` format for Convex persistence
2. **Claude Code UUIDs**: Standard UUID format required by Claude Code CLI
3. **Session Mapping**: Desktop maintains mapping between UUIDs and mobile session IDs

```typescript
// Mobile Session Request (created from mobile)
export const requestDesktopSession = mutation({
  args: RequestDesktopSessionArgs,
  returns: RequestDesktopSessionResult,
  handler: ({ projectPath, initialMessage, title }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      const sessionId = `mobile-${Date.now()}-${Math.random().toString(36).substring(2)}`;

      yield* Effect.logInfo(`📱 [CONFECT] requestDesktopSession creating session:`, {
        sessionId,
        projectPath,
        title,
      });

      // Create session
      yield* db.insert("claudeSessions", {
        sessionId,
        projectPath,
        title: title ?? `Mobile Session - ${new Date().toLocaleString()}`,
        status: "active" as const,
        createdBy: "mobile" as const,
        lastActivity: Date.now(),
        userId: user._id,
      });

      // Initialize sync status
      yield* db.insert("syncStatus", {
        sessionId,
      });

      // Add initial message if provided
      if (initialMessage) {
        yield* Effect.logInfo(`📱 [CONFECT] Adding initial message to session ${sessionId}`);
        const messageId = `user-${Date.now()}`;
        
        yield* db.insert("claudeMessages", {
          sessionId,
          messageId,
          messageType: "user" as const,
          content: initialMessage,
          timestamp: new Date().toISOString(),
        });

        yield* Effect.logInfo(`✅ [CONFECT] Initial message created with ID: ${messageId}`);
      }

      return sessionId;
    }),
});
```

### Device Session Tracking

The system tracks sessions across multiple device types using the `userDeviceSessions` table with Effect Schema:

```typescript
userDeviceSessions: defineTable(
  Schema.Struct({
    userId: Id.Id("users"),
    deviceId: Schema.String.pipe(Schema.nonEmptyString()),
    deviceType: Schema.Literal("desktop", "mobile", "github"),
    sessionPeriods: Schema.Array(
      Schema.Struct({
        start: Schema.Number,
        end: Schema.optional(Schema.Number),
      })
    ),
    actionsCount: Schema.Struct({
      messages: Schema.Number,
      toolUses: Schema.Number,
      githubEvents: Schema.optional(Schema.Number),
    }),
    lastActivity: Schema.Number,
    metadata: Schema.optional(
      Schema.Struct({
        platform: Schema.optional(Schema.String),
        version: Schema.optional(Schema.String),
        location: Schema.optional(Schema.String),
      })
    ),
  })
)
```

## Error Handling & Type Safety

### Tagged Error System

The session system uses a comprehensive tagged error system built on Effect-TS:

```typescript
// Custom Error Types (packages/shared/src/services/MobileSyncService.ts)
export class MobileSyncError extends Schema.TaggedError<MobileSyncError>()(
  "MobileSyncError",
  { message: Schema.String }
) {}

export class SessionValidationError extends Schema.TaggedError<SessionValidationError>()(
  "SessionValidationError", 
  { reason: Schema.String, sessionId: Schema.String }
) {}

export class ProcessingTimeoutError extends Schema.TaggedError<ProcessingTimeoutError>()(
  "ProcessingTimeoutError",
  { timeoutMs: Schema.Number, operation: Schema.String }
) {}
```

### Effect Error Handling Patterns

```typescript
// Hook with Comprehensive Error Handling
export function useConfectMobileSync(
  confectApi: any,
  options: UseConfectMobileSyncOptions = {}
): UseConfectMobileSyncReturn {
  const processMobileSession = useCallback(async (session: MobileSession) => {
    const program = Effect.gen(function* () {
      const syncService = yield* MobileSyncService;
      
      console.log(`🔄 [CONFECT-SYNC] Processing mobile session: ${session.sessionId}`);
      
      // Process the session using Effect patterns
      const result = yield* syncService.processMobileSession(session);
      
      // Update status to processed
      yield* syncService.updateSessionStatus(session.sessionId, "processed");
      
      return result;
    });

    try {
      const exit = await Runtime.runPromiseExit(runtimeRef.current)(program);
      
      if (Exit.isFailure(exit)) {
        const cause = exit.cause;
        let errorMessage = "Unknown error";
        
        // Handle specific error types
        if (cause._tag === "Fail") {
          const error = cause.error;
          if (error instanceof MobileSyncError) {
            errorMessage = `Mobile sync failed: ${error.message}`;
          } else if (error instanceof SessionValidationError) {
            errorMessage = `Session validation failed: ${error.reason}`;
          } else if (error instanceof ProcessingTimeoutError) {
            errorMessage = `Processing timeout after ${error.timeoutMs}ms`;
          }
        }
        
        console.error(`❌ [CONFECT-SYNC] Failed to process session ${session.sessionId}:`, errorMessage);
        setError(errorMessage);
      }
    } catch (error) {
      console.error(`❌ [CONFECT-SYNC] Unexpected error processing session:`, error);
      setError(String(error));
    }
  }, [enabled]);
}
```

### Schema Validation

All session operations include compile-time and runtime type safety through Effect Schema:

```typescript
// Schema Definitions (packages/convex/confect/mobile_sync_schemas.ts)
export const CreateClaudeSessionArgs = Schema.Struct({
  sessionId: Schema.String,
  projectPath: Schema.String,
  createdBy: Schema.Literal("desktop", "mobile"),
  title: Schema.optional(Schema.String),
  metadata: Schema.optional(
    Schema.Struct({
      workingDirectory: Schema.optional(Schema.String),
      model: Schema.optional(Schema.String),
      systemPrompt: Schema.optional(Schema.String),
      originalMobileSessionId: Schema.optional(Schema.String),
    })
  ),
});

export const AddClaudeMessageArgs = Schema.Struct({
  sessionId: Schema.String,
  messageId: Schema.String,
  messageType: Schema.Literal("user", "assistant", "tool_use", "tool_result", "thinking"),
  content: Schema.String,
  timestamp: Schema.String,
  toolInfo: Schema.optional(
    Schema.Struct({
      toolName: Schema.String,
      toolUseId: Schema.String,
      input: Schema.Any,
      output: Schema.optional(Schema.String),
    })
  ),
  metadata: Schema.optional(Schema.Any),
});
```

## Performance & Scalability

### Database Indexing Strategy

Strategic indexes optimize session queries across different access patterns:

```typescript
claudeSessions: defineTable(/* schema */)
  .index("by_session_id", ["sessionId"])      // Primary lookup
  .index("by_status", ["status"])             // Status filtering
  .index("by_last_activity", ["lastActivity"]) // Temporal queries
  .index("by_user_id", ["userId"])            // User-specific queries

claudeMessages: defineTable(/* schema */)
  .index("by_session_id", ["sessionId"])      // Session message queries
  .index("by_timestamp", ["timestamp"])       // Temporal ordering
  .index("by_user_id", ["userId"])            // User-specific messages
```

### Query Optimization Patterns

```typescript
// Optimized Pending Sessions Query
export const getPendingMobileSessions = query({
  args: GetPendingMobileSessionsArgs,
  returns: GetPendingMobileSessionsResult,
  handler: () =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      // Use status index for efficient filtering
      const results = yield* db
        .query("claudeSessions")
        .withIndex("by_status", (q) => q.eq("status", "active"))
        .filter((q) => q.eq(q.field("createdBy"), "mobile"))
        .order("desc")
        .take(10); // Limit results for performance

      return results;
    }),
});
```

### Batch Operations

```typescript
// Batch Message Addition
export const batchAddMessages = mutation({
  args: BatchAddMessagesArgs,
  returns: BatchAddMessagesResult,
  handler: ({ sessionId, messages }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      const insertedIds: any[] = [];

      // Process messages in sequence with deduplication
      for (const message of messages) {
        const existingMessage = yield* db
          .query("claudeMessages")
          .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
          .filter((q) => q.eq(q.field("messageId"), message.messageId))
          .first();

        if (Option.isNone(existingMessage)) {
          const messageDoc = yield* db.insert("claudeMessages", {
            sessionId,
            messageId: message.messageId,
            messageType: message.messageType,
            content: message.content,
            timestamp: message.timestamp,
            toolInfo: message.toolInfo,
            metadata: message.metadata,
          });

          insertedIds.push(messageDoc);
        }
      }

      // Single activity update for batch
      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      yield* Option.match(session, {
        onSome: (s) => db.patch(s._id, { lastActivity: Date.now() }),
        onNone: () => Effect.void,
      });

      return insertedIds;
    }),
});
```

## Integration with React

### Hook Architecture

React components integrate with Effect-TS sessions through specialized hooks:

```typescript
// Desktop Session Manager Hook (apps/desktop/src/hooks/useSessionManager.ts)
export const useSessionManager = () => {
  const [sessions, setSessions] = useState<Session[]>([]);
  const { openChatPane, updateSessionMessages } = usePaneStore();
  
  // Convex mutations using Effect-based operations
  const createClaudeSession = useMutation(api.confect.mobile_sync.createClaudeSession);
  const updateSessionStatus = useMutation(api.confect.mobile_sync.updateSessionStatus);

  const createSession = useCallback(async () => {
    try {
      // Tauri command to create Claude Code session
      const result = await invoke<CommandResult<string>>("create_session", {
        projectPath: newProjectPath,
      });

      if (result.success && result.data) {
        const sessionId = result.data;
        
        // Create the Confect session document so mobile can see it
        await createClaudeSession({
          sessionId,
          projectPath: newProjectPath,
          createdBy: "desktop" as const,
          title: `Desktop Session - ${newProjectPath}`,
        });

        // Session creation continues...
      }
    } catch (error) {
      console.error("Failed to create session:", error);
    }
  }, [newProjectPath, createClaudeSession]);
};
```

### Runtime Integration

Effect runtime is integrated at the application level:

```typescript
// Effect Runtime Context Integration
const useEffectService = () => {
  const runtime = useContext(EffectRuntimeContext);
  
  const runEffect = useCallback(async <A, E>(effect: Effect.Effect<A, E>) => {
    const exit = await Runtime.runPromiseExit(runtime)(effect);
    
    if (Exit.isFailure(exit)) {
      // Handle specific errors
      throw new Error(`Effect execution failed: ${Exit.causeSquash(exit.cause)}`);
    }
    
    return exit.value;
  }, [runtime]);
  
  return { runEffect };
};
```

## Mobile UI Interaction Flow

This section provides a comprehensive analysis of how the mobile UI interacts with the Effect-TS session system, including the complete flow from user button clicks to real-time session updates.

### UI Component Architecture

The mobile app uses a layered component architecture that integrates seamlessly with Effect-TS session operations:

```mermaid
graph TB
    subgraph "Mobile UI Layer"
        CC[ClaudeCodeMobile.tsx]
        SWS[ScreenWithSidebar.tsx]  
        SC[SidebarContent.tsx]
        CL[ChatList.tsx]
    end
    
    subgraph "State Management"
        CH[Convex Hooks]
        APM[APM Tracking]
        AUTH[Auth Context]
    end
    
    subgraph "Effect Operations"
        CCS[createClaudeSession]
        ACM[addClaudeMessage]
        USS[updateSessionStatus]
        GPS[getPendingMobileSessions]
    end
    
    subgraph "Real-time Updates"
        UQ[useQuery - Live Sessions]
        UM[useMutation - Session Ops]
        RT[Real-time Subscriptions]
    end
    
    CC --> CH
    CC --> APM
    CC --> AUTH
    SWS --> CC
    SC --> SWS
    CH --> CCS
    CH --> ACM
    CH --> USS
    CH --> GPS
    CH --> UQ
    CH --> UM
    UQ --> RT
    UM --> RT
    
    style CC fill:#e1f5fe
    style CH fill:#f3e5f5
    style CCS fill:#e8f5e8
    style RT fill:#fff3e0
```

### Session Creation Flow: Complete User Journey

When a user clicks the "Create Session" button, the following comprehensive flow is triggered:

#### 1. UI Button Interactions

**Two Entry Points for Session Creation:**

1. **Header Plus Button** (`ScreenWithSidebar.tsx` line 126):
   ```typescript
   <Pressable
     onPress={() => onNewChat?.()}
     hitSlop={{ top: 10, bottom: 10, left: 10, right: 10 }}
   >
     <IconPlus />
   </Pressable>
   ```

2. **Sidebar New Chat Button** (`ScreenWithSidebar.tsx` line 97):
   ```typescript
   <SidebarContent
     onNewChat={handleNewChatFromSidebar}
     // ... other props
   />
   ```

Both buttons trigger the same `handleNewChat` function in `ClaudeCodeMobile.tsx`:

```typescript
const handleNewChat = () => {
  setShowCreateModal(true);  // Opens the session creation modal
};
```

#### 2. Modal Interaction & Form State

The create session modal presents three input fields with Effect-TS-friendly validation:

```typescript
// Session creation state with defaults from environment
const [newProjectPath, setNewProjectPath] = useState(DEFAULT_PROJECT_PATH);
const [newSessionTitle, setNewSessionTitle] = useState(`Testing ${generateRandomString()}`);
const [initialMessage, setInitialMessage] = useState(DEFAULT_INITIAL_MESSAGE);

// Form validation before Effect-TS operations
const handleCreateSession = async () => {
  if (!newProjectPath.trim()) {
    Alert.alert("Error", "Please enter a project path");
    return;  // Prevent invalid Effect operations
  }
  // ... Effect-TS session creation continues
};
```

#### 3. Effect-TS Session Creation Pipeline

When the user clicks "Create Session" in the modal, a comprehensive Effect-TS pipeline is triggered:

```typescript
// Step 1: Call Confect mutation with Effect Schema validation
const sessionId = await requestDesktopSession({
  sessionId: `mobile-${Date.now()}`,              // Mobile-specific ID format
  projectPath: newProjectPath.trim(),
  createdBy: "mobile" as const,                   // Platform identifier
  title: newSessionTitle.trim() || undefined,    // Optional title with fallback
  metadata: {
    workingDirectory: newProjectPath.trim(),
    originalMobileSessionId: `mobile-${Date.now()}`,
  },
});
```

This triggers the Effect-TS mutation in `packages/convex/confect/mobile_sync.ts`:

```typescript
export const requestDesktopSession = mutation({
  args: RequestDesktopSessionArgs,  // Effect Schema validation
  returns: RequestDesktopSessionResult,
  handler: ({ projectPath, initialMessage, title }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;  // Auth check

      const sessionId = `mobile-${Date.now()}-${Math.random().toString(36).substring(2)}`;

      yield* Effect.logInfo(`📱 [CONFECT] requestDesktopSession creating session:`, {
        sessionId, projectPath, title,
      });

      // Database insertion with Effect error handling
      yield* db.insert("claudeSessions", {
        sessionId,
        projectPath,
        title: title ?? `Mobile Session - ${new Date().toLocaleString()}`,
        status: "active" as const,
        createdBy: "mobile" as const,
        lastActivity: Date.now(),
        userId: user._id,
      });

      // Initialize sync status for cross-platform coordination
      yield* db.insert("syncStatus", {
        sessionId,
      });

      return sessionId;  // Return to UI layer
    }),
});
```

#### 4. APM Tracking Integration

Immediately after successful session creation, APM tracking is triggered:

```typescript
// Track session creation for APM (useAPMTracking hook)
trackSessionCreated();

// APM tracking calls Effect-TS operations:
const trackDeviceSession = useMutation(api.confect.apm.trackDeviceSession);

// This updates the userDeviceSessions table with Effect patterns
```

#### 5. UI State Updates & User Feedback

After successful Effect-TS operations, the UI provides immediate feedback:

```typescript
Alert.alert(
  "Session Created",
  `New Claude Code session created! Session ID: ${sessionId}\n\nThe desktop app will automatically start this session.`,
  [
    {
      text: "View Session",
      onPress: () => {
        console.log('📱 [MOBILE] User selected to view session:', sessionId);
        setSelectedSessionId(sessionId);  // Triggers real-time queries
        setShowCreateModal(false);
      },
    },
    { 
      text: "OK",
      onPress: () => setShowCreateModal(false),
    },
  ]
);
```

#### 6. Real-time Session List Updates

The session list updates automatically through Convex real-time subscriptions:

```typescript
// Real-time query automatically updates when new sessions are created
const sessions = useQuery(
  api.confect.mobile_sync.getPendingMobileSessions, 
  authReady ? {} : "skip"
) || [];
```

This query triggers the Effect-TS operation:

```typescript
export const getPendingMobileSessions = query({
  handler: () =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      const results = yield* db
        .query("claudeSessions")
        .withIndex("by_status", (q) => q.eq("status", "active"))
        .filter((q) => q.eq(q.field("createdBy"), "mobile"))
        .order("desc")
        .take(10);

      yield* Effect.logInfo(`🔍 [CONFECT] getPendingMobileSessions query returned: ${results.length} sessions`);
      
      return results;  // Automatically triggers UI re-render
    }),
});
```

### Message Sending Flow

When a user types a message and clicks "Send", another Effect-TS pipeline is triggered:

#### 1. Message Input Validation

```typescript
const handleSendMessage = async (sessionId: string, content: string) => {
  if (!content.trim()) return;  // Client-side validation

  console.log('💬 [MOBILE] Sending message to session:', {
    sessionId,
    contentLength: content.trim().length,
  });
  // ... Effect-TS operations continue
};
```

#### 2. Effect-TS Message Creation

```typescript
const messageId = `mobile-${Date.now()}-${Math.random().toString(36).substring(2)}`;

// Confect mutation with Effect error handling
await addMessage({
  sessionId,
  messageId,
  messageType: "user",
  content: content.trim(),
  timestamp: new Date().toISOString(),
  metadata: { source: "mobile" },
});
```

This triggers the Effect-TS operation in `addClaudeMessage`:

```typescript
export const addClaudeMessage = mutation({
  handler: ({ sessionId, messageId, messageType, content, timestamp, metadata }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      // Duplicate check using Option patterns
      const existingMessage = yield* db
        .query("claudeMessages")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .filter((q) => q.eq(q.field("messageId"), messageId))
        .first();

      return yield* Option.match(existingMessage, {
        onSome: (msg) => {
          yield* Effect.logWarning('⚠️ [CONFECT] Message already exists, skipping duplicate:', messageId);
          return Effect.succeed(msg._id);
        },
        onNone: () =>
          Effect.gen(function* () {
            // Insert message with full metadata
            const messageDoc = yield* db.insert("claudeMessages", {
              sessionId, messageId, messageType, content, timestamp, metadata,
            });
            
            // Update session activity timestamp
            const session = yield* db
              .query("claudeSessions")
              .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
              .first();

            yield* Option.match(session, {
              onSome: (s) => db.patch(s._id, { lastActivity: Date.now() }),
              onNone: () => Effect.void,
            });

            return messageDoc;
          })
      });
    }),
});
```

#### 3. APM Tracking & Status Updates

```typescript
// Track message sent for APM
trackMessageSent();

// Update session status to ensure desktop processing
await updateSyncStatus({
  sessionId,
  status: "active",
});
```

### Real-time Message Updates

The mobile app receives real-time message updates through Convex subscriptions integrated with Effect-TS:

#### 1. Live Message Query

```typescript
const selectedSessionMessages = useQuery(
  api.confect.mobile_sync.getSessionMessages, 
  authReady && selectedSessionId ? { sessionId: selectedSessionId } : "skip"
) || [];
```

#### 2. Effect-TS Message Retrieval

```typescript
export const getSessionMessages = query({
  handler: ({ sessionId, limit }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectQueryCtx;
      
      // Authentication check
      const identity = yield* auth.getUserIdentity();
      if (Option.isNone(identity)) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      // Session ownership verification
      const session = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      if (Option.isNone(session)) {
        return yield* Effect.fail(new Error("Session not found"));
      }

      // Retrieve messages with ordering
      const results = limit 
        ? yield* queryBuilder.take(limit)
        : yield* queryBuilder.take(1000);

      yield* Effect.logInfo(`📋 [CONFECT] getSessionMessages returned ${results.length} messages for session ${sessionId}`);

      return results;  // Triggers automatic UI updates
    }),
});
```

### Component State Management

The mobile UI maintains several layers of state that integrate with Effect-TS operations:

#### 1. Authentication State

```typescript
const { isAuthenticated, user } = useConfectAuth();
const { isSynced } = useUserSync();

// Authentication is ready when user is authenticated AND synced to Convex
// This prevents race conditions where queries execute before user sync completes
const authReady = isAuthenticated && isSynced;
```

#### 2. Session Selection State

```typescript
const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);

// Session selection triggers new Effect-TS queries
const handleSessionSelect = (sessionId: string) => {
  setSelectedSessionId(sessionId);  // Triggers useQuery re-execution
};
```

#### 3. Modal State Management

```typescript
const [showCreateModal, setShowCreateModal] = useState(false);

// Modal state controls when Effect-TS operations can be triggered
const handleNewChat = () => {
  setShowCreateModal(true);
};
```

### Error Handling in UI Layer

The mobile UI includes comprehensive error handling that integrates with Effect-TS error patterns:

#### 1. Session Creation Errors

```typescript
try {
  const sessionId = await requestDesktopSession({...});
  // Success path
} catch (error) {
  console.error("❌ [MOBILE] Failed to create session:", error);
  Alert.alert("Error", "Failed to create session. Please try again.");
}
```

#### 2. Message Sending Errors

```typescript
try {
  await addMessage({...});
  trackMessageSent();
} catch (error) {
  console.error("❌ [MOBILE] Failed to send message:", error);
  Alert.alert("Error", "Failed to send message. Please try again.");
}
```

#### 3. Component-Level Error Boundaries

```typescript
<ErrorBoundary
  onError={(error, errorInfo) => {
    const timestamp = new Date().toISOString();
    console.error(`❌ [CLAUDE_CODE_MOBILE] ${timestamp} Component error:`, {
      error: error.message,
      stack: error.stack,
      componentStack: errorInfo.componentStack,
      user: user?.githubUsername,
      isAuthenticated,
      selectedSessionId
    });
    // TODO: Report to crash analytics service
  }}
>
  {/* UI components */}
</ErrorBoundary>
```

### Session Object Updates & Real-time Sync

The session object is updated through several mechanisms that integrate with Effect-TS:

#### 1. Automatic Query Re-execution

When Effect-TS mutations modify session data, Convex automatically triggers query re-execution:

```typescript
// Any mutation that modifies claudeSessions table triggers this query to re-run
const sessions = useQuery(api.confect.mobile_sync.getPendingMobileSessions, {});
```

#### 2. Last Activity Tracking

Every user interaction updates the session's `lastActivity` timestamp through Effect-TS operations:

```typescript
// In addClaudeMessage mutation
yield* db.patch(session.value._id, {
  lastActivity: Date.now(),  // Triggers real-time updates
});
```

#### 3. Status Synchronization

Session status is synchronized across platforms through Effect-TS status updates:

```typescript
await updateSyncStatus({
  sessionId,
  status: "active",  // Desktop will pick up this change
});
```

### Performance Optimizations

The mobile UI includes several performance optimizations that work with Effect-TS:

#### 1. Conditional Query Execution

```typescript
// Queries only execute when authentication is complete
const sessions = useQuery(
  api.confect.mobile_sync.getPendingMobileSessions, 
  authReady ? {} : "skip"  // Prevents unnecessary Effect operations
) || [];
```

#### 2. Message Input Debouncing

```typescript
// Clear message input when switching sessions to prevent cross-session confusion
useEffect(() => {
  setNewMessage("");
}, [selectedSessionId]);
```

#### 3. Efficient List Rendering

```typescript
<FlatList
  data={selectedSessionMessages}
  keyExtractor={(item) => item._id}  // Stable keys for React optimization
  renderItem={renderMessageItem}
  inverted  // New messages appear at bottom
  showsVerticalScrollIndicator={false}
/>
```

### Development & Debugging Features

The mobile UI includes extensive logging that integrates with Effect-TS logging patterns:

#### 1. Session Creation Logging

```typescript
console.log('📱 [MOBILE] Creating new Claude Code session with:', {
  projectPath: newProjectPath.trim(),
  title: newSessionTitle.trim() || undefined,
  hasInitialMessage: !!initialMessage.trim()
});
```

#### 2. Message Flow Logging

```typescript
console.log('💬 [MOBILE] Sending message to session:', {
  sessionId,
  contentLength: content.trim().length,
  content: content.trim().substring(0, 100) + (content.trim().length > 100 ? '...' : '')
});
```

#### 3. Effect-TS Operation Logging

```typescript
// In Effect-TS operations
yield* Effect.logInfo(`📱 [CONFECT] requestDesktopSession creating session:`, {
  sessionId, projectPath, title,
});
```

This comprehensive mobile UI interaction flow demonstrates how the Effect-TS session system integrates seamlessly with React Native components, providing type-safe, error-handled, and real-time session management across the entire user experience.

## Testing Patterns

### Service Testing Infrastructure

Effect-TS sessions are tested using comprehensive service testing patterns:

```typescript
// Session Service Test Suite
describe("MobileSyncService", () => {
  const TestRuntime = Runtime.defaultRuntime.pipe(
    Runtime.provide(MobileSyncServiceLive.Default)
  );

  it("should process mobile session successfully", async () => {
    const mockSession: MobileSession = {
      sessionId: "mobile-123-abc",
      projectPath: "/test/project",
      status: "active",
      createdBy: "mobile",
      lastActivity: Date.now(),
    };

    const program = Effect.gen(function* () {
      const syncService = yield* MobileSyncService;
      return yield* syncService.processMobileSession(mockSession);
    });

    const result = await Runtime.runPromise(TestRuntime)(program);
    
    expect(result.success).toBe(true);
    expect(result.sessionId).toBe("mobile-123-abc");
  });

  it("should handle session validation errors", async () => {
    const invalidSession: MobileSession = {
      sessionId: "", // Invalid empty session ID
      projectPath: "/test/project",
      status: "active",
      createdBy: "mobile",
      lastActivity: Date.now(),
    };

    const program = Effect.gen(function* () {
      const syncService = yield* MobileSyncService;
      return yield* syncService.processMobileSession(invalidSession);
    });

    const exit = await Runtime.runPromiseExit(TestRuntime)(program);
    
    expect(Exit.isFailure(exit)).toBe(true);
    if (Exit.isFailure(exit)) {
      const cause = exit.cause;
      expect(cause._tag).toBe("Fail");
      if (cause._tag === "Fail") {
        expect(cause.error).toBeInstanceOf(SessionValidationError);
      }
    }
  });
});
```

### Integration Testing

```typescript
// Full Session Lifecycle Test
describe("Session Lifecycle Integration", () => {
  it("should handle complete mobile-to-desktop session flow", async () => {
    // 1. Create mobile session
    const mobileSessionId = await runEffect(
      createClaudeSession({
        sessionId: "mobile-test-123",
        projectPath: "/test/project",
        createdBy: "mobile",
        title: "Test Mobile Session",
      })
    );

    // 2. Add initial message
    await runEffect(
      addClaudeMessage({
        sessionId: "mobile-test-123",
        messageId: "user-msg-1",
        messageType: "user",
        content: "Hello Claude",
        timestamp: new Date().toISOString(),
      })
    );

    // 3. Query pending sessions
    const pendingSessions = await runEffect(
      getPendingMobileSessions({})
    );

    expect(pendingSessions).toHaveLength(1);
    expect(pendingSessions[0].sessionId).toBe("mobile-test-123");

    // 4. Process session
    const syncService = await runEffect(MobileSyncService);
    const result = await runEffect(
      syncService.processMobileSession(pendingSessions[0])
    );

    expect(result.success).toBe(true);

    // 5. Verify session status update
    const updatedSession = await runEffect(
      getSession({ sessionId: "mobile-test-123" })
    );

    expect(updatedSession?.status).toBe("processed");
  });
});
```

## Future Enhancements

### Agent Orchestration Integration

The session system is designed to integrate with Effect-TS actor model patterns for agent orchestration:

```typescript
// Future: Agent Orchestration Layer
export class AgentOrchestratorService extends Effect.Service<AgentOrchestratorService>()(
  'AgentOrchestratorService',
  {
    sync: () => ({
      orchestrateSession: (sessionId: string) =>
        Effect.gen(function* () {
          // Load session context
          const session = yield* getSession({ sessionId });
          
          // Create agent actor
          const agent = yield* AgentActor.make({
            sessionId,
            context: session?.metadata,
          });
          
          // Begin orchestration
          return yield* agent.start();
        }),
    }),
    dependencies: [MobileSyncService]
  }
) {}
```

### Streaming Integration

Future enhancements will integrate Effect Streams for real-time session updates:

```typescript
// Future: Session Event Streaming
export const sessionEventStream = (sessionId: string) =>
  Stream.fromQueue(sessionEventQueue).pipe(
    Stream.filter(event => event.sessionId === sessionId),
    Stream.map(event => ({
      type: event.type,
      sessionId: event.sessionId,
      timestamp: Date.now(),
      data: event.data,
    })),
    Stream.tap(event => Effect.logInfo(`Session event: ${event.type}`))
  );
```

### Enhanced Error Recovery

```typescript
// Future: Automatic Session Recovery
export const recoverFailedSession = (sessionId: string) =>
  Effect.gen(function* () {
    const session = yield* getSession({ sessionId });
    
    if (Option.isNone(session)) {
      return yield* Effect.fail(new SessionValidationError("Session not found"));
    }
    
    // Implement recovery logic with retry policies
    return yield* Effect.retry(
      syncSessionFromHook({ 
        hookData: { 
          sessionId, 
          event: "recovery",
          projectPath: session.value.projectPath 
        } 
      }),
      Schedule.exponential("1 second").pipe(
        Schedule.compose(Schedule.recurs(3))
      )
    );
  });
```

## Conclusion

The Claude Code session representation in Effect-TS demonstrates a sophisticated functional programming approach to session management. Rather than implementing sessions as a dedicated service layer, the system leverages:

1. **Database-centric design** with Effect-enhanced operations
2. **Comprehensive type safety** through Schema validation
3. **Functional error handling** with tagged errors and Option types
4. **Cross-platform synchronization** with mobile-desktop session mapping
5. **Performance optimization** through strategic indexing and batch operations

This architecture provides a robust, scalable, and maintainable foundation for Claude Code session management across desktop and mobile platforms, with clear patterns for future enhancements including agent orchestration and real-time streaming capabilities.

The system successfully eliminates common source of bugs (null/undefined errors, unhandled exceptions) while providing excellent developer experience through comprehensive type safety and composable operations. The integration with React through specialized hooks maintains separation of concerns while enabling real-time updates and error handling at the UI layer.
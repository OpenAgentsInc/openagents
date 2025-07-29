# Confect: Effect-TS + Convex Integration

## Overview

Confect is a powerful integration layer that combines [Effect-TS](https://effect.website/) functional programming patterns with [Convex](https://convex.dev/) real-time backend capabilities. Created by [@rjdellecese](https://github.com/rjdellecese), it allows developers to write Convex functions using Effect-TS's robust error handling, type safety, and composability patterns.

## What is Confect?

Confect provides:
- **Effect-TS Integration**: Write Convex functions using Effect.gen, Option, Either, and other Effect patterns
- **Type Safety**: End-to-end type safety from frontend to backend with Effect Schema validation
- **Functional Error Handling**: Robust error handling using Effect's tagged error system
- **Composable Operations**: Build complex database operations by composing smaller Effects
- **Schema Validation**: Use Effect Schema for runtime type validation and transformation

## Installation & Setup

```bash
npm install @rjdellecese/confect
```

## Core Concepts

### 1. Confect Context

Confect provides its own context types that wrap Convex's context:

```typescript
import {
  ConfectMutationCtx,
  ConfectQueryCtx,
  ConfectActionCtx,
  mutation,
  query,
  action,
} from "./confect";

// In our app's confect.ts
export const mutation = confectMutation.bind(null, internal);
export const query = confectQuery.bind(null, internal);
export const action = confectAction.bind(null, internal);
```

### 2. Database Operations with Effect

```typescript
export const getUserById = query({
  args: GetUserByIdArgs,
  returns: GetUserByIdResult,
  handler: ({ userId }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      const user = yield* db
        .query("users")
        .withIndex("by_id", (q) => q.eq("_id", userId))
        .first();

      return user; // Returns Option<User>
    }),
});
```

### 3. Schema Definition with Effect Schema

```typescript
import { Schema } from "effect";
import { Id } from "@rjdellecese/confect/server";

export const CreateUserArgs = Schema.Struct({
  email: Schema.String,
  githubId: Schema.String,
  githubUsername: Schema.String,
  name: Schema.optional(Schema.String),
  avatar: Schema.optional(Schema.String),
});

export const CreateUserResult = Id.Id("users");
```

### 4. Error Handling with Tagged Errors

```typescript
// Define custom error types
export class GitHubAPIError extends Schema.TaggedError<GitHubAPIError>()(
  "GitHubAPIError",
  {
    message: Schema.String,
    status: Schema.optional(Schema.Number),
    response: Schema.optional(Schema.String),
  }
) {}

// Use in Effect chains
const fetchRepositories = Effect.gen(function* () {
  const response = yield* Effect.tryPromise({
    try: () => fetch(url, { headers }),
    catch: (error) => new GitHubAPIError({
      message: `Network error: ${error.message}`,
    }),
  });

  if (!response.ok) {
    return yield* Effect.fail(new GitHubAPIError({
      message: `GitHub API request failed: ${response.statusText}`,
      status: response.status,
    }));
  }

  const data = yield* Effect.tryPromise({
    try: () => response.json(),
    catch: () => new GitHubAPIError({
      message: "Failed to parse GitHub API response",
    }),
  });

  return data;
});
```

## Our App's Confect Architecture

### Directory Structure

```
apps/mobile/convex/confect/
├── confect.ts              # Core Confect setup and configuration
├── users.ts                # User management operations
├── mobile-sync.ts          # Mobile-desktop session synchronization
├── github.ts               # GitHub API integration
├── github.schemas.ts       # GitHub-related schemas and error types
├── messages.ts             # Claude message handling
├── messages.schemas.ts     # Message schemas
├── mobile-sync.schemas.ts  # Sync schemas
├── validation.ts           # Input validation utilities
├── http-api.ts             # HTTP API endpoints
└── *.test.ts              # Effect-TS test files
```

### Key Features in Our Implementation

#### 1. User Management

```typescript
// users.ts - Effect-based user operations
export const getOrCreateUser = mutation({
  args: GetOrCreateUserArgs,
  returns: GetOrCreateUserResult,
  handler: (args) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      // Get authenticated user
      const identity = yield* auth.getUserIdentity();
      
      if (Option.isNone(identity)) {
        return yield* Effect.fail(new AuthError("User not authenticated"));
      }

      // Check if user exists
      const existingUser = yield* db
        .query("users")
        .withIndex("by_openauth_subject", (q) => 
          q.eq("openAuthSubject", identity.value.subject)
        )
        .first();

      return yield* Option.match(existingUser, {
        onSome: (user) => Effect.succeed(user._id),
        onNone: () => createNewUser(args),
      });
    }),
});
```

#### 2. GitHub Integration

```typescript
// github.ts - Effect-based GitHub API calls
export const fetchUserRepositories = action({
  args: FetchUserRepositoriesArgs,
  returns: FetchUserRepositoriesResult,
  handler: () =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectActionCtx;

      // Get user with GitHub metadata
      const user = yield* getUserWithGitHubData();
      
      // Check cache freshness
      const isCacheStale = yield* checkCacheExpiry(user);
      
      if (!isCacheStale) {
        return yield* Effect.succeed({
          repositories: user.githubMetadata.cachedRepos,
          isCached: true,
        });
      }

      // Fetch fresh data from GitHub API
      const repositories = yield* fetchFromGitHubAPI(user);
      
      // Update cache
      yield* updateRepositoryCache(user._id, repositories);
      
      return yield* Effect.succeed({
        repositories,
        isCached: false,
      });
    }),
});
```

#### 3. Session Synchronization

```typescript
// mobile-sync.ts - Cross-platform session sync
export const createClaudeSession = mutation({
  args: CreateClaudeSessionArgs,
  returns: CreateClaudeSessionResult,
  handler: ({ sessionId, projectPath, createdBy, title, metadata }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      // Get authenticated user (optional for backwards compatibility)
      const identity = yield* auth.getUserIdentity();
      let userId = Option.none<string>();
      
      if (Option.isSome(identity)) {
        const user = yield* db
          .query("users")
          .withIndex("by_openauth_subject", (q) => 
            q.eq("openAuthSubject", identity.value.subject)
          )
          .first();
          
        if (Option.isSome(user)) {
          userId = Option.some(user.value._id);
        }
      }

      // Check for existing session
      const existingSession = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();

      return yield* Option.match(existingSession, {
        onSome: (session) => updateExistingSession(session, title, metadata, userId),
        onNone: () => createNewSession(sessionId, projectPath, createdBy, title, metadata, userId),
      });
    }),
});
```

## Benefits of Using Confect

### 1. **Type Safety**
- End-to-end type safety from frontend to backend
- Runtime schema validation with Effect Schema
- Compile-time error detection for database operations

### 2. **Robust Error Handling**
- Tagged errors with structured error information
- Composable error handling with Effect.catchTag
- No more try/catch blocks or unhandled promise rejections

### 3. **Functional Composition**
- Build complex operations by composing smaller Effects
- Reusable business logic components
- Clean separation of concerns

### 4. **Option Types**
- Explicit handling of nullable database results
- No more null/undefined runtime errors
- Pattern matching for clean conditional logic

### 5. **Performance Benefits**
- Lazy evaluation of Effects
- Efficient error short-circuiting
- Optimized database query patterns

## Common Patterns

### 1. Database Queries with Option Handling

```typescript
const getUser = (userId: Id<"users">) =>
  Effect.gen(function* () {
    const { db } = yield* ConfectQueryCtx;
    
    const user = yield* db.get(userId);
    
    return yield* Option.match(user, {
      onSome: (u) => Effect.succeed(u),
      onNone: () => Effect.fail(new UserNotFoundError({ userId })),
    });
  });
```

### 2. Error Recovery

```typescript
const fetchWithFallback = Effect.gen(function* () {
  const primary = yield* primaryDataSource.pipe(
    Effect.catchTag("NetworkError", () => 
      fallbackDataSource.pipe(
        Effect.map(data => ({ ...data, fromCache: true }))
      )
    )
  );
  
  return primary;
});
```

### 3. Conditional Operations

```typescript
const conditionalUpdate = (condition: boolean, updateData: any) =>
  Effect.gen(function* () {
    if (condition) {
      yield* db.patch(documentId, updateData);
      yield* Effect.logInfo("Document updated");
    } else {
      yield* Effect.logInfo("Update skipped due to condition");
    }
  });
```

## Testing with Confect

```typescript
import { describe, it, expect, vi } from "vitest";
import { Effect, Option, Runtime } from "effect";

describe("Confect Operations", () => {
  it("should handle user creation", async () => {
    const userEffect = Effect.gen(function* () {
      // Mock database operations
      return yield* Effect.succeed(Option.some({
        _id: "user123",
        email: "test@example.com",
      }));
    });

    const result = await Runtime.runPromise(Runtime.defaultRuntime)(userEffect);
    
    expect(Option.isSome(result)).toBe(true);
    if (Option.isSome(result)) {
      expect(result.value.email).toBe("test@example.com");
    }
  });
});
```

## Migration Considerations

### From Regular Convex to Confect

1. **Function Definitions**: Replace `defineQuery`/`defineMutation` with Confect's `query`/`mutation`
2. **Context Access**: Use `yield* ConfectMutationCtx` instead of direct context parameter
3. **Error Handling**: Replace try/catch with Effect error handling patterns
4. **Schema Validation**: Use Effect Schema instead of Convex validators
5. **Option Types**: Handle nullable results with Option instead of null checks

### Performance Considerations

- **Effect Overhead**: Minimal runtime overhead, mostly at build time
- **Bundle Size**: Effect-TS adds ~100KB to bundle size
- **Learning Curve**: Functional programming patterns require team training
- **TypeScript Compilation**: More complex types may slow compilation slightly

## Resources

- [Confect GitHub Repository](https://github.com/rjdellecese/confect)
- [Effect-TS Documentation](https://effect.website/)
- [Convex Documentation](https://docs.convex.dev/)
- [Effect-TS Schema Documentation](https://effect.website/docs/schema/introduction)

## Common Issues & Solutions

### 1. Type Assertion for Effect Option Values

```typescript
// Problem: TypeScript doesn't know the type of Option value
if (Option.isSome(result)) {
  const email = result.value.email; // TS Error: Property 'email' does not exist
}

// Solution: Type assertion in tests or explicit typing
if (Option.isSome(result)) {
  const email = (result.value as any).email; // For tests only
}
```

### 2. Schema.nonEmpty() Removal

```typescript
// Old (doesn't exist in current Effect version)
sessionId: Schema.String.pipe(Schema.nonEmpty())

// New 
sessionId: Schema.String
```

### 3. Id Type Casting

```typescript
// When working with Convex document IDs
...(Option.isSome(userId) ? { userId: userId.value as Id<"users"> } : {})
```

This documentation covers our current understanding and usage of Confect. As we continue development, we should update this document with new patterns and learnings.
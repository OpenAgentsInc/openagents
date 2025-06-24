# @openagentsinc/convex

Convex database integration for OpenAgents with Effect service architecture.

## Overview

This package provides a fully Effect-based integration with Convex, replacing PlanetScale MySQL as the unified database for the entire OpenAgents.com platform. It includes:

- **Schema Definitions**: Convex validators mapped from existing MySQL schemas
- **Effect Services**: Type-safe Convex operations with Effect error handling
- **High-Level Client**: Convenient abstractions for common database operations
- **Migration Utilities**: Tools for converting existing data to Convex format

## Features

- 🚀 **Real-time Updates**: Built-in subscriptions for live data synchronization
- 🔒 **Type Safety**: Full TypeScript integration with Effect schemas
- 🛡️ **Error Handling**: Tagged errors with retry policies
- 📊 **Schema Mapping**: Automatic conversion between Effect schemas and Convex validators
- 🔄 **Migration Support**: Tools for transitioning from MySQL to Convex

## Installation

```bash
pnpm add @openagentsinc/convex
```

## Quick Start

### 1. Setup Environment

Copy the environment variables from `.env.local` (created during deployment):

```bash
CONVEX_URL=https://proficient-panther-764.convex.cloud
CONVEX_AUTH_TOKEN=your_auth_token_here  # Optional
```

### 2. Initialize Convex

```typescript
import { ConvexService, ConvexServiceLive } from "@openagentsinc/convex"
import { Effect, Layer } from "effect"

const program = Effect.gen(function* () {
  const convex = yield* ConvexService
  
  // Your database operations here
  const result = yield* convex.query(api.events.list, { limit: 10 })
  
  return result
})

// Run with ConvexService layer
const result = await Effect.runPromise(
  program.pipe(Effect.provide(ConvexServiceLive))
)
```

### 3. Using the High-Level Client

```typescript
import { ConvexClient } from "@openagentsinc/convex"

// Create a Nostr event
const program = ConvexClient.events.create({
  id: "event_id",
  pubkey: "pubkey",
  created_at: Date.now(),
  kind: 1,
  tags: [],
  content: "Hello Convex!",
  sig: "signature"
})

const eventId = await Effect.runPromise(
  program.pipe(Effect.provide(ConvexServiceLive))
)
```

## Architecture

### Database Tables

The package maps the following MySQL tables to Convex:

**Nostr Relay Tables:**
- `events` - Core Nostr events (NIP-01)
- `event_tags` - Denormalized tags for efficient filtering
- `agent_profiles` - Agent metadata and status
- `service_offerings` - NIP-90 marketplace services
- `channels` - NIP-28 public channels
- `job_requests` - Service request tracking
- `relay_stats` - Monitoring and analytics

**Chat/Overlord Tables:**
- `sessions` - Chat sessions with metadata
- `messages` - Conversation history
- `images` - Message attachments

### Service Architecture

```typescript
// Effect-based service interface
interface ConvexService {
  query<T>(ref: FunctionReference, args?: any): Effect<T, ConvexQueryError>
  mutation<T>(ref: FunctionReference, args?: any): Effect<T, ConvexMutationError>
  action<T>(ref: FunctionReference, args?: any): Effect<T, ConvexActionError>
  subscribe<T>(ref: FunctionReference, args: any, callback: (T) => void): Effect<() => void>
}
```

## Examples

### Real-time Agent Updates

```typescript
import { ConvexClient } from "@openagentsinc/convex"

const program = Effect.gen(function* () {
  // Subscribe to agent updates
  const unsubscribe = yield* ConvexClient.agents.subscribeToActive((agents) => {
    console.log("Active agents updated:", agents.length)
  })
  
  // Cleanup when done
  return unsubscribe
})
```

### Chat Session Management

```typescript
// Create a new chat session
const session = yield* ConvexClient.sessions.create({
  id: "session_123",
  user_id: "user_456",
  project_path: "/path/to/project",
  status: "active",
  started_at: Date.now(),
  last_activity: Date.now(),
  message_count: 0,
  total_cost: 0
})

// Add messages to the session
const message = yield* ConvexClient.messages.create({
  session_id: "session_123",
  entry_uuid: "msg_789",
  entry_type: "user",
  role: "user",
  content: "Hello!",
  timestamp: Date.now()
})
```

### Error Handling with Retries

```typescript
import { ConvexHelpers, RetryPolicies } from "@openagentsinc/convex"

const robustQuery = pipe(
  ConvexHelpers.queryWithRetry(api.events.list, { limit: 100 }),
  Effect.retry(RetryPolicies.aggressiveRetry),
  Effect.catchTags({
    ConvexConnectionError: (error) => 
      Effect.logError(`Connection failed: ${error.message}`)
  })
)
```

## Migration from MySQL

The package includes utilities for migrating existing PlanetScale data:

```typescript
import { Migration } from "@openagentsinc/convex"

// Convert MySQL timestamps
const convexTimestamp = Migration.timestampToNumber(mysqlDate)

// Convert MySQL JSON
const convexObject = Migration.mysqlJsonToConvex(mysqlJsonString)

// Convert MySQL booleans
const convexBoolean = Migration.mysqlBooleanToBoolean(mysqlTinyint)
```

## Development

### Building

```bash
pnpm build
```

### Testing

```bash
pnpm test
```

### Convex Setup

✅ **Already Deployed!** The Convex backend is live at:
- **Project**: `openagentsdotcom` in team `christopher-david`
- **Dashboard**: https://dashboard.convex.dev/t/christopher-david/openagentsdotcom
- **Deployment URL**: https://proficient-panther-764.convex.cloud

```bash
# Development mode with hot reloading
npx convex dev

# Deploy updates to schema and functions
npx convex dev --once
```

## Environment Variables

| Variable | Description | Required | Value |
|----------|-------------|----------|-------|
| `CONVEX_URL` | Convex deployment URL | Yes | `https://proficient-panther-764.convex.cloud` |
| `CONVEX_AUTH_TOKEN` | Auth token for server operations | No | (Generate from dashboard if needed) |

## License

MIT - See [LICENSE](../../LICENSE) for details
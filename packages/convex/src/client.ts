/**
 * Convex client utilities for OpenAgents integration
 * Provides high-level abstractions for common database operations
 * @since 1.0.0
 */

import * as Effect from "effect/Effect"
import * as Schema from "effect/Schema"

/**
 * Event data types based on existing relay schemas
 */
export const NostrEvent = Schema.Struct({
  id: Schema.String,
  pubkey: Schema.String,
  created_at: Schema.Number,
  kind: Schema.Number,
  tags: Schema.Array(Schema.Array(Schema.String)),
  content: Schema.String,
  sig: Schema.String,
  received_at: Schema.optional(Schema.Number),
  relay_url: Schema.optional(Schema.String)
})

export type NostrEvent = Schema.Schema.Type<typeof NostrEvent>

/**
 * Agent profile data types
 */
export const AgentProfile = Schema.Struct({
  pubkey: Schema.String,
  agent_id: Schema.String,
  name: Schema.optional(Schema.String),
  status: Schema.String,
  balance: Schema.optional(Schema.Number),
  metabolic_rate: Schema.optional(Schema.Number),
  capabilities: Schema.Array(Schema.String),
  last_activity: Schema.Number,
  profile_event_id: Schema.optional(Schema.String),
  created_at: Schema.Number,
  updated_at: Schema.Number
})

export type AgentProfile = Schema.Schema.Type<typeof AgentProfile>

/**
 * Chat session data types
 */
export const ChatSession = Schema.Struct({
  id: Schema.String,
  user_id: Schema.String,
  project_path: Schema.String,
  project_name: Schema.optional(Schema.String),
  status: Schema.String,
  started_at: Schema.Number,
  last_activity: Schema.Number,
  message_count: Schema.Number,
  total_cost: Schema.Number
})

export type ChatSession = Schema.Schema.Type<typeof ChatSession>

/**
 * Chat message data types
 */
export const ChatMessage = Schema.Struct({
  session_id: Schema.String,
  entry_uuid: Schema.String,
  entry_type: Schema.String,
  role: Schema.optional(Schema.String),
  content: Schema.optional(Schema.String),
  thinking: Schema.optional(Schema.String),
  summary: Schema.optional(Schema.String),
  model: Schema.optional(Schema.String),
  token_usage: Schema.optional(Schema.Struct({
    input_tokens: Schema.Number,
    output_tokens: Schema.Number,
    total_tokens: Schema.Number
  })),
  cost: Schema.optional(Schema.Number),
  timestamp: Schema.Number,
  turn_count: Schema.optional(Schema.Number),
  tool_name: Schema.optional(Schema.String),
  tool_input: Schema.optional(Schema.Any),
  tool_use_id: Schema.optional(Schema.String),
  tool_output: Schema.optional(Schema.String),
  tool_is_error: Schema.optional(Schema.Boolean)
})

export type ChatMessage = Schema.Schema.Type<typeof ChatMessage>

/**
 * High-level client for Convex operations
 * 
 * Ready for Convex backend deployment:
 * - Convex backend deployed at: https://proficient-panther-764.convex.cloud
 * - Schema and functions are live and ready
 * - To connect, import and use the generated API: `import { api } from "../convex/_generated/api.js"`
 * - Replace these placeholders with actual Convex service calls
 */
export class ConvexClient {
  /**
   * Nostr event operations
   */
  static readonly events = {
    /**
     * Store a new Nostr event
     */
    create: (_event: NostrEvent) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.mutationWithRetry(api.events.create, event)
        yield* Effect.logInfo("ConvexClient.events.create - Convex backend ready")
        return yield* Effect.succeed("convex-event-id")
      }),

    /**
     * Query events by filters
     */
    list: (_filters: {
      pubkey?: string
      kind?: number
      since?: number
      until?: number
      limit?: number
    }) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.queryWithRetry(api.events.list, filters)
        yield* Effect.logInfo("ConvexClient.events.list - Convex backend ready")
        return yield* Effect.succeed([])
      }),

    /**
     * Get event by ID
     */
    getById: (id: string) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.queryWithRetry(api.events.getById, { id })
        yield* Effect.logInfo(`ConvexClient.events.getById(${id}) - Convex backend ready`)
        return yield* Effect.succeed(null)
      })
  }

  /**
   * Agent profile operations
   */
  static readonly agents = {
    /**
     * Create or update an agent profile
     */
    upsert: (_profile: AgentProfile) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.mutationWithRetry(api.agents.upsert, profile)
        yield* Effect.logInfo("ConvexClient.agents.upsert - Convex backend ready")
        return yield* Effect.succeed("convex-agent-id")
      }),

    /**
     * List active agents
     */
    listActive: () =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.queryWithRetry(api.agents.listActive, {})
        yield* Effect.logInfo("ConvexClient.agents.listActive - Convex backend ready")
        return yield* Effect.succeed([])
      }),

    /**
     * Get agent by pubkey
     */
    getByPubkey: (pubkey: string) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.queryWithRetry(api.agents.getByPubkey, { pubkey })
        yield* Effect.logInfo(`ConvexClient.agents.getByPubkey(${pubkey}) - Convex backend ready`)
        return yield* Effect.succeed(null)
      })
  }

  /**
   * Chat session operations
   */
  static readonly sessions = {
    /**
     * Create a new chat session
     */
    create: (_session: ChatSession) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.mutationWithRetry(api.sessions.create, session)
        yield* Effect.logInfo("ConvexClient.sessions.create - Convex backend ready")
        return yield* Effect.succeed("convex-session-id")
      }),

    /**
     * Get sessions for a user
     */
    listByUser: (userId: string) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.queryWithRetry(api.sessions.listByUser, { userId })
        yield* Effect.logInfo(`ConvexClient.sessions.listByUser(${userId}) - Convex backend ready`)
        return yield* Effect.succeed([])
      }),

    /**
     * Update session activity
     */
    updateActivity: (sessionId: string) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.mutationWithRetry(api.sessions.updateActivity, { sessionId, timestamp: Date.now() })
        yield* Effect.logInfo(`ConvexClient.sessions.updateActivity(${sessionId}) - Convex backend ready`)
        return yield* Effect.succeed(undefined)
      })
  }

  /**
   * Chat message operations
   */
  static readonly messages = {
    /**
     * Add a message to a session
     */
    create: (_message: ChatMessage) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.mutationWithRetry(api.messages.create, message)
        yield* Effect.logInfo("ConvexClient.messages.create - Convex backend ready")
        return yield* Effect.succeed("convex-message-id")
      }),

    /**
     * Get messages for a session
     */
    listBySession: (sessionId: string, limit = 50) =>
      Effect.gen(function*() {
        // TODO: Replace with: ConvexHelpers.queryWithRetry(api.messages.listBySession, { sessionId, limit })
        yield* Effect.logInfo(`ConvexClient.messages.listBySession(${sessionId}, ${limit}) - Convex backend ready`)
        return yield* Effect.succeed([])
      }),

    /**
     * Subscribe to new messages in a session
     */
    subscribeToSession: (sessionId: string, _callback: (messages: Array<ChatMessage>) => void) =>
      Effect.gen(function*() {
        // TODO: Replace with: convex.subscribe(api.messages.listBySession, { sessionId }, callback)
        yield* Effect.logInfo(`ConvexClient.messages.subscribeToSession(${sessionId}) - Convex backend ready`)
        return yield* Effect.succeed(() => {})
      })
  }
}

/**
 * Migration utilities for existing data
 */
export namespace Migration {
  /**
   * Convert PlanetScale/MySQL timestamp to Convex number
   */
  export const timestampToNumber = (timestamp: Date | string | number): number => {
    if (typeof timestamp === "number") return timestamp
    if (typeof timestamp === "string") return new Date(timestamp).getTime()
    return timestamp.getTime()
  }

  /**
   * Convert MySQL boolean to Convex boolean
   */
  export const mysqlBooleanToBoolean = (value: number | boolean): boolean => {
    if (typeof value === "boolean") return value
    return value === 1
  }

  /**
   * Convert MySQL JSON to Convex object/array
   */
  export const mysqlJsonToConvex = <T>(value: string | T): T => {
    if (typeof value === "string") {
      try {
        return JSON.parse(value)
      } catch {
        return value as T
      }
    }
    return value
  }
}

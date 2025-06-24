/**
 * Convex client utilities for OpenAgents integration
 * Provides high-level abstractions for common database operations
 * @since 1.0.0
 */

import { ConvexHttpClient } from "convex/browser"
import * as Effect from "effect/Effect"
import * as Schema from "effect/Schema"
import { api } from "../convex/_generated/api.js"

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
 * Get Convex client from environment
 */
const getConvexClient = () => {
  // Support both Node and browser environments
  const url = typeof process !== "undefined" && process.env?.CONVEX_URL
    ? process.env.CONVEX_URL
    : "https://proficient-panther-764.convex.cloud"
  return new ConvexHttpClient(url)
}

/**
 * Helper to strip undefined values from optional properties
 * Convex expects truly optional properties, not T | undefined
 */
const stripUndefined = <T extends Record<string, any>>(obj: T): any => {
  const result: any = {}
  for (const [key, value] of Object.entries(obj)) {
    if (value !== undefined) {
      result[key] = value
    }
  }
  return result
}

/**
 * High-level client for Convex operations
 */
export class ConvexClient {
  private static client = getConvexClient()

  /**
   * Nostr event operations
   */
  static readonly events = {
    /**
     * Store a new Nostr event
     */
    create: (event: NostrEvent) =>
      Effect.tryPromise({
        try: async () => {
          const result = await ConvexClient.client.mutation(api.events.create, stripUndefined(event))
          return result as string
        },
        catch: (error) => new Error(`Failed to create event: ${error}`)
      }),

    /**
     * Query events by filters
     */
    list: (filters: {
      pubkey?: string
      kind?: number
      since?: number
      until?: number
      limit?: number
    }) =>
      Effect.tryPromise({
        try: async () => {
          const results = await ConvexClient.client.query(api.events.list, stripUndefined(filters))
          return results as Array<NostrEvent>
        },
        catch: (error) => new Error(`Failed to list events: ${error}`)
      }),

    /**
     * Get event by ID
     */
    getById: (id: string) =>
      Effect.tryPromise({
        try: async () => {
          const result = await ConvexClient.client.query(api.events.getById, { id })
          return result as NostrEvent | null
        },
        catch: (error) => new Error(`Failed to get event: ${error}`)
      })
  }

  /**
   * Agent profile operations
   */
  static readonly agents = {
    /**
     * Create or update an agent profile
     */
    upsert: (profile: AgentProfile) =>
      Effect.tryPromise({
        try: async () => {
          const result = await ConvexClient.client.mutation(api.agents.upsert, stripUndefined(profile))
          return result as string
        },
        catch: (error) => new Error(`Failed to upsert agent: ${error}`)
      }),

    /**
     * List active agents
     */
    listActive: () =>
      Effect.tryPromise({
        try: async () => {
          const results = await ConvexClient.client.query(api.agents.listActive, {})
          return results as Array<AgentProfile>
        },
        catch: (error) => new Error(`Failed to list agents: ${error}`)
      }),

    /**
     * Get agent by pubkey
     */
    getByPubkey: (pubkey: string) =>
      Effect.tryPromise({
        try: async () => {
          const result = await ConvexClient.client.query(api.agents.getByPubkey, { pubkey })
          return result as AgentProfile | null
        },
        catch: (error) => new Error(`Failed to get agent: ${error}`)
      })
  }

  /**
   * Chat session operations
   */
  static readonly sessions = {
    /**
     * Create a new chat session
     */
    create: (session: ChatSession) =>
      Effect.tryPromise({
        try: async () => {
          const result = await ConvexClient.client.mutation(api.sessions.create, stripUndefined(session))
          return result as string
        },
        catch: (error) => new Error(`Failed to create session: ${error}`)
      }),

    /**
     * Get sessions for a user
     */
    listByUser: (userId: string) =>
      Effect.tryPromise({
        try: async () => {
          const results = await ConvexClient.client.query(api.sessions.listByUser, { userId })
          return results as Array<ChatSession>
        },
        catch: (error) => new Error(`Failed to list sessions: ${error}`)
      }),

    /**
     * Get session by ID
     */
    getById: (sessionId: string) =>
      Effect.tryPromise({
        try: async () => {
          const result = await ConvexClient.client.query(api.sessions.getById, { sessionId })
          return result as ChatSession | null
        },
        catch: (error) => new Error(`Failed to get session: ${error}`)
      }),

    /**
     * Update session activity
     */
    updateActivity: (sessionId: string) =>
      Effect.tryPromise({
        try: async () => {
          await ConvexClient.client.mutation(api.sessions.updateActivity, {
            sessionId,
            timestamp: Date.now()
          })
          return undefined
        },
        catch: (error) => new Error(`Failed to update activity: ${error}`)
      })
  }

  /**
   * Chat message operations
   */
  static readonly messages = {
    /**
     * Add a message to a session
     */
    create: (message: ChatMessage) =>
      Effect.tryPromise({
        try: async () => {
          const result = await ConvexClient.client.mutation(api.messages.create, stripUndefined(message))
          return result as string
        },
        catch: (error) => new Error(`Failed to create message: ${error}`)
      }),

    /**
     * Get messages for a session
     */
    listBySession: (sessionId: string, limit = 50) =>
      Effect.tryPromise({
        try: async () => {
          const results = await ConvexClient.client.query(api.messages.listBySession, {
            sessionId,
            limit
          })
          return results as Array<ChatMessage>
        },
        catch: (error) => new Error(`Failed to list messages: ${error}`)
      }),

    /**
     * Get message by UUID
     */
    getByUuid: (sessionId: string, entryUuid: string) =>
      Effect.tryPromise({
        try: async () => {
          const result = await ConvexClient.client.query(api.messages.getByUuid, { entryUuid })
          return result as ChatMessage | null
        },
        catch: (error) => new Error(`Failed to get message: ${error}`)
      }),

    /**
     * Add image to a message
     * Note: messageId should be the Convex document ID returned from create()
     */
    addImage: (messageId: string, _image: { image_data: string; mime_type: string; position: number }) =>
      Effect.tryPromise({
        try: async () => {
          // Images table is separate but we don't have a specific mutation for it yet
          // For now, we'll need to handle this differently
          console.warn("Image storage not yet implemented in Convex functions")
          return messageId
        },
        catch: (error) => new Error(`Failed to add image: ${error}`)
      }),

    /**
     * Subscribe to new messages in a session
     */
    subscribeToSession: (_sessionId: string, _callback: (messages: Array<ChatMessage>) => void) =>
      Effect.sync(() => {
        // Note: For real-time subscriptions, you need ConvexClient (not HttpClient)
        // This requires importing from "convex/browser" and using WebSocket connection
        const unsubscribe = () => {
          // Cleanup logic would go here
        }

        // For now, return a cleanup function
        return unsubscribe
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

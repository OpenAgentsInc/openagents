/**
 * Database service combining Effect SQL with Drizzle ORM
 * Provides type-safe database operations for Nostr relay
 */
import type { Schema as NostrSchema } from "@openagentsinc/nostr"
import { Client } from "@planetscale/database"
import { and, desc, eq, gte, inArray, lte, or, sql } from "drizzle-orm"
import { drizzle } from "drizzle-orm/planetscale-serverless"
import { Config, Context, Effect, Layer, Schema } from "effect"
import * as schema from "./schema.js"

// Type aliases for cleaner code
type NostrEvent = NostrSchema.NostrEvent
type Filter = NostrSchema.Filter

// Type conversion helpers
const convertToNostrEvent = (dbRow: any): NostrEvent => {
  // For now, cast directly - in production we'd want proper validation
  return dbRow as NostrEvent
}

// Error types
export class DatabaseError extends Schema.TaggedError<DatabaseError>()(
  "DatabaseError",
  {
    message: Schema.String,
    cause: Schema.Unknown,
    operation: Schema.optional(Schema.String)
  }
) {}

export class ValidationError extends Schema.TaggedError<ValidationError>()(
  "ValidationError",
  {
    message: Schema.String,
    event: Schema.Unknown
  }
) {}

// Database configuration
const DatabaseConfig = Config.all({
  host: Config.string("DATABASE_HOST"),
  username: Config.string("DATABASE_USERNAME"),
  password: Config.secret("DATABASE_PASSWORD"),
  name: Config.string("DATABASE_NAME").pipe(Config.withDefault("openagents_relay"))
})

// Database service interface
export class RelayDatabase extends Context.Tag("RelayDatabase")<
  RelayDatabase,
  {
    // Event operations
    readonly storeEvent: (event: NostrEvent) => Effect.Effect<boolean, DatabaseError | ValidationError>
    readonly queryEvents: (filters: Array<Filter>) => Effect.Effect<Array<NostrEvent>, DatabaseError>
    readonly getEvent: (id: string) => Effect.Effect<NostrEvent | null, DatabaseError>
    readonly deleteEvent: (id: string) => Effect.Effect<boolean, DatabaseError>

    // Agent operations (NIP-OA)
    readonly getAgentProfile: (pubkey: string) => Effect.Effect<schema.AgentProfile | null, DatabaseError>
    readonly updateAgentProfile: (profile: schema.NewAgentProfile) => Effect.Effect<schema.AgentProfile, DatabaseError>
    readonly getActiveAgents: () => Effect.Effect<Array<schema.AgentProfile>, DatabaseError>

    // Service marketplace (NIP-90)
    readonly getServiceOfferings: (
      filters?: { agentPubkey?: string; capabilities?: Array<string> }
    ) => Effect.Effect<Array<schema.ServiceOffering>, DatabaseError>
    readonly updateServiceOffering: (
      offering: schema.NewServiceOffering
    ) => Effect.Effect<schema.ServiceOffering, DatabaseError>

    // Channel operations (NIP-28)
    readonly getChannels: () => Effect.Effect<Array<schema.Channel>, DatabaseError>
    readonly updateChannelStats: (channelId: string, messageCount: number) => Effect.Effect<void, DatabaseError>

    // Statistics
    readonly recordMetric: (
      name: string,
      value: number,
      metadata?: Record<string, unknown>
    ) => Effect.Effect<void, DatabaseError>
    readonly getMetrics: (name: string, since?: Date) => Effect.Effect<Array<schema.RelayStat>, DatabaseError>
  }
>() {}

// Event validation
const validateEvent = (event: NostrEvent): Effect.Effect<void, ValidationError> =>
  Effect.gen(function*() {
    // Basic NIP-01 validation
    if (!event.id || event.id.length !== 64) {
      return yield* Effect.fail(
        new ValidationError({
          message: "Invalid event ID",
          event
        })
      )
    }

    if (!event.pubkey || event.pubkey.length !== 64) {
      return yield* Effect.fail(
        new ValidationError({
          message: "Invalid pubkey",
          event
        })
      )
    }

    if (!event.sig || event.sig.length !== 128) {
      return yield* Effect.fail(
        new ValidationError({
          message: "Invalid signature",
          event
        })
      )
    }

    if (typeof event.kind !== "number" || event.kind < 0 || event.kind > 65535) {
      return yield* Effect.fail(
        new ValidationError({
          message: "Invalid event kind",
          event
        })
      )
    }

    if (!Array.isArray(event.tags)) {
      return yield* Effect.fail(
        new ValidationError({
          message: "Invalid tags format",
          event
        })
      )
    }
  })

// Filter to SQL conversion
const buildEventQuery = async (db: ReturnType<typeof drizzle>, filters: Array<Filter>) => {
  if (filters.length === 0) {
    return await db.select()
      .from(schema.events)
      .orderBy(desc(schema.events.created_at))
      .limit(100)
  }

  // Build WHERE conditions for each filter (OR logic between filters)
  const filterConditions = filters.map((filter) => {
    const conditions = []

    if (filter.ids?.length) {
      conditions.push(inArray(schema.events.id, filter.ids))
    }

    if (filter.authors?.length) {
      conditions.push(inArray(schema.events.pubkey, filter.authors))
    }

    if (filter.kinds?.length) {
      conditions.push(inArray(schema.events.kind, filter.kinds))
    }

    if (filter.since) {
      conditions.push(gte(schema.events.created_at, filter.since))
    }

    if (filter.until) {
      conditions.push(lte(schema.events.created_at, filter.until))
    }

    // Tag filters (#e, #p, etc.)
    Object.entries(filter).forEach(([key, values]) => {
      if (key.startsWith("#") && Array.isArray(values) && values.length > 0) {
        const tagName = key.slice(1)
        // Subquery for tag filtering
        const tagSubquery = db
          .select({ event_id: schema.event_tags.event_id })
          .from(schema.event_tags)
          .where(
            and(
              eq(schema.event_tags.tag_name, tagName),
              inArray(schema.event_tags.tag_value, values)
            )
          )

        conditions.push(inArray(schema.events.id, tagSubquery))
      }
    })

    return conditions.length > 0 ? and(...conditions) : undefined
  })

  const validConditions = filterConditions.filter(Boolean)
  const whereClause = validConditions.length > 0
    ? (validConditions.length === 1 ? validConditions[0]! : or(...validConditions))
    : undefined

  if (whereClause) {
    return await db.select()
      .from(schema.events)
      .where(whereClause)
      .orderBy(desc(schema.events.created_at))
      .limit(filters[0]?.limit || 100)
  } else {
    return await db.select()
      .from(schema.events)
      .orderBy(desc(schema.events.created_at))
      .limit(filters[0]?.limit || 100)
  }
}

// Extract tags for indexing
const extractEventTags = (event: NostrEvent): Array<schema.NewEventTag> => {
  const tags: Array<schema.NewEventTag> = []

  event.tags.forEach((tag, index) => {
    if (tag.length >= 2) {
      tags.push({
        event_id: event.id,
        tag_name: tag[0],
        tag_value: tag[1],
        tag_index: index,
        created_at: event.created_at
      })
    }
  })

  return tags
}

// Live implementation
export const RelayDatabaseLive = Layer.effect(
  RelayDatabase,
  Effect.gen(function*() {
    const config = yield* DatabaseConfig

    // Create PlanetScale client
    const client = new Client({
      host: config.host,
      username: config.username,
      password: config.password as unknown as string // Config.secret provides string value
    })

    const db = drizzle(client, { schema })

    const storeEvent = (event: NostrEvent) =>
      Effect.gen(function*() {
        // Validate event
        yield* validateEvent(event)

        // Check for duplicates
        const [existing] = yield* Effect.tryPromise({
          try: () => db.select().from(schema.events).where(eq(schema.events.id, event.id)).limit(1),
          catch: (error) =>
            new DatabaseError({
              message: "Failed to check for duplicate event",
              cause: error,
              operation: "duplicate_check"
            })
        })

        if (existing) {
          return false // Event already exists
        }

        // Insert event and tags in transaction
        yield* Effect.tryPromise({
          try: async () => {
            await db.transaction(async (tx) => {
              // Insert event
              await tx.insert(schema.events).values({
                id: event.id,
                pubkey: event.pubkey,
                created_at: event.created_at,
                kind: event.kind,
                tags: event.tags as Array<Array<string>>,
                content: event.content,
                sig: event.sig
              })

              // Insert tags for indexing
              const eventTags = extractEventTags(event)
              if (eventTags.length > 0) {
                await tx.insert(schema.event_tags).values(eventTags)
              }

              // Update denormalized caches based on event kind
              if (event.kind === 31337) { // Agent profile
                // Extract agent profile data and update cache
                try {
                  const content = JSON.parse(event.content)
                  const agentId = event.tags.find((t) => t[0] === "d")?.[1]
                  const name = event.tags.find((t) => t[0] === "name")?.[1]
                  const status = event.tags.find((t) => t[0] === "status")?.[1] || "active"
                  const balance = event.tags.find((t) => t[0] === "balance")?.[1]

                  if (agentId) {
                    await tx.insert(schema.agent_profiles).values({
                      pubkey: event.pubkey,
                      agent_id: agentId,
                      name,
                      status,
                      balance: balance ? Number(balance) : 0,
                      capabilities: content.capabilities?.map((c: any) => c.id) || [],
                      profile_event_id: event.id
                    }).onDuplicateKeyUpdate({
                      set: {
                        name,
                        status,
                        balance: balance ? Number(balance) : 0,
                        capabilities: content.capabilities?.map((c: any) => c.id) || [],
                        profile_event_id: event.id,
                        updated_at: sql`NOW()`
                      }
                    })
                  }
                } catch (e) {
                  // Ignore JSON parsing errors for profile events
                  console.warn("Failed to parse agent profile event:", e)
                }
              }

              if (event.kind === 31990) { // Service offering
                try {
                  const serviceId = event.tags.find((t) => t[0] === "d")?.[1]
                  const serviceName = event.tags.find((t) => t[0] === "name")?.[1]
                  const agentTag = event.tags.find((t) => t[0] === "agent")
                  const amountTag = event.tags.find((t) => t[0] === "amount")

                  if (serviceId && serviceName && agentTag) {
                    await tx.insert(schema.service_offerings).values({
                      id: serviceId,
                      agent_pubkey: event.pubkey,
                      service_name: serviceName,
                      nip90_kinds: event.tags.filter((t) => t[0] === "k").map((t) => parseInt(t[1])),
                      pricing: {
                        base: amountTag ? parseInt(amountTag[1]) : 0,
                        currency: amountTag?.[2] || "sats"
                      },
                      capabilities: event.tags.filter((t) => t[0] === "t").map((t) => t[1]),
                      offering_event_id: event.id
                    }).onDuplicateKeyUpdate({
                      set: {
                        service_name: serviceName,
                        pricing: {
                          base: amountTag ? parseInt(amountTag[1]) : 0,
                          currency: amountTag?.[2] || "sats"
                        },
                        capabilities: event.tags.filter((t) => t[0] === "t").map((t) => t[1]),
                        offering_event_id: event.id,
                        updated_at: sql`NOW()`
                      }
                    })
                  }
                } catch (e) {
                  console.warn("Failed to parse service offering event:", e)
                }
              }
            })
          },
          catch: (error) =>
            new DatabaseError({
              message: "Failed to store event",
              cause: error,
              operation: "store_event"
            })
        })

        return true
      })

    const queryEvents = (filters: Array<Filter>) =>
      Effect.tryPromise({
        try: async () => {
          const results = await buildEventQuery(db, filters)

          return results.map((row) =>
            convertToNostrEvent({
              id: row.id,
              pubkey: row.pubkey,
              created_at: Number(row.created_at),
              kind: row.kind,
              tags: row.tags as Array<Array<string>>,
              content: row.content,
              sig: row.sig
            })
          )
        },
        catch: (error) =>
          new DatabaseError({
            message: "Failed to query events",
            cause: error,
            operation: "query_events"
          })
      })

    const getEvent = (id: string) =>
      Effect.tryPromise({
        try: async () => {
          const [result] = await db.select().from(schema.events).where(eq(schema.events.id, id)).limit(1)

          if (!result) return null

          return convertToNostrEvent({
            id: result.id,
            pubkey: result.pubkey,
            created_at: Number(result.created_at),
            kind: result.kind,
            tags: result.tags as Array<Array<string>>,
            content: result.content,
            sig: result.sig
          })
        },
        catch: (error) =>
          new DatabaseError({
            message: "Failed to get event",
            cause: error,
            operation: "get_event"
          })
      })

    const deleteEvent = (id: string) =>
      Effect.tryPromise({
        try: async () => {
          const result = await db.delete(schema.events).where(eq(schema.events.id, id))
          return result.rowsAffected > 0
        },
        catch: (error) =>
          new DatabaseError({
            message: "Failed to delete event",
            cause: error,
            operation: "delete_event"
          })
      })

    const getAgentProfile = (pubkey: string) =>
      Effect.tryPromise({
        try: async () => {
          const [profile] = await db.select().from(schema.agent_profiles).where(
            eq(schema.agent_profiles.pubkey, pubkey)
          ).limit(1)
          return profile || null
        },
        catch: (error) =>
          new DatabaseError({
            message: "Failed to get agent profile",
            cause: error,
            operation: "get_agent_profile"
          })
      })

    const updateAgentProfile = (profile: schema.NewAgentProfile) =>
      Effect.tryPromise({
        try: async () => {
          await db.insert(schema.agent_profiles).values(profile).onDuplicateKeyUpdate({
            set: {
              ...profile,
              updated_at: sql`NOW()`
            }
          })

          // Since MySQL doesn't support returning, query for the updated record
          const [updated] = await db.select().from(schema.agent_profiles).where(
            eq(schema.agent_profiles.pubkey, profile.pubkey)
          ).limit(1)
          return updated!
        },
        catch: (error) =>
          new DatabaseError({
            message: "Failed to update agent profile",
            cause: error,
            operation: "update_agent_profile"
          })
      })

    const getActiveAgents = () =>
      Effect.tryPromise({
        try: () =>
          db.select().from(schema.agent_profiles).where(eq(schema.agent_profiles.status, "active")).orderBy(
            desc(schema.agent_profiles.last_activity)
          ),
        catch: (error) =>
          new DatabaseError({
            message: "Failed to get active agents",
            cause: error,
            operation: "get_active_agents"
          })
      })

    const getServiceOfferings = (filters?: { agentPubkey?: string; capabilities?: Array<string> }) =>
      Effect.tryPromise({
        try: async () => {
          const conditions = [eq(schema.service_offerings.availability, "available")]

          if (filters?.agentPubkey) {
            conditions.push(eq(schema.service_offerings.agent_pubkey, filters.agentPubkey))
          }

          return await db.select()
            .from(schema.service_offerings)
            .where(and(...conditions))
            .orderBy(desc(schema.service_offerings.updated_at))
        },
        catch: (error) =>
          new DatabaseError({
            message: "Failed to get service offerings",
            cause: error,
            operation: "get_service_offerings"
          })
      })

    const updateServiceOffering = (offering: schema.NewServiceOffering) =>
      Effect.tryPromise({
        try: async () => {
          await db.insert(schema.service_offerings).values(offering).onDuplicateKeyUpdate({
            set: {
              ...offering,
              updated_at: sql`NOW()`
            }
          })

          // Since MySQL doesn't support returning, query for the updated record
          const [updated] = await db.select().from(schema.service_offerings).where(
            eq(schema.service_offerings.id, offering.id)
          ).limit(1)
          return updated!
        },
        catch: (error) =>
          new DatabaseError({
            message: "Failed to update service offering",
            cause: error,
            operation: "update_service_offering"
          })
      })

    const getChannels = () =>
      Effect.tryPromise({
        try: () => db.select().from(schema.channels).orderBy(desc(schema.channels.last_message_at)),
        catch: (error) =>
          new DatabaseError({
            message: "Failed to get channels",
            cause: error,
            operation: "get_channels"
          })
      })

    const updateChannelStats = (channelId: string, messageCount: number) =>
      Effect.tryPromise({
        try: () =>
          db.update(schema.channels)
            .set({
              message_count: messageCount,
              last_message_at: sql`NOW()`,
              updated_at: sql`NOW()`
            })
            .where(eq(schema.channels.id, channelId)),
        catch: (error) =>
          new DatabaseError({
            message: "Failed to update channel stats",
            cause: error,
            operation: "update_channel_stats"
          })
      })

    const recordMetric = (name: string, value: number, metadata?: Record<string, unknown>) =>
      Effect.tryPromise({
        try: () =>
          db.insert(schema.relay_stats).values({
            metric_name: name,
            metric_value: value,
            metadata: metadata || {}
          }),
        catch: (error) =>
          new DatabaseError({
            message: "Failed to record metric",
            cause: error,
            operation: "record_metric"
          })
      }).pipe(Effect.asVoid)

    const getMetrics = (name: string, since?: Date) =>
      Effect.tryPromise({
        try: async () => {
          const conditions = [eq(schema.relay_stats.metric_name, name)]

          if (since) {
            conditions.push(gte(schema.relay_stats.timestamp, since))
          }

          return await db.select()
            .from(schema.relay_stats)
            .where(and(...conditions))
            .orderBy(desc(schema.relay_stats.timestamp))
        },
        catch: (error) =>
          new DatabaseError({
            message: "Failed to get metrics",
            cause: error,
            operation: "get_metrics"
          })
      })

    return {
      storeEvent,
      queryEvents,
      getEvent,
      deleteEvent,
      getAgentProfile,
      updateAgentProfile,
      getActiveAgents,
      getServiceOfferings,
      updateServiceOffering,
      getChannels,
      updateChannelStats,
      recordMetric,
      getMetrics
    } as const
  })
)

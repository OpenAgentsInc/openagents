/**
 * Browser Agent Service with Effect.js
 * Real-time agent status and profile updates
 */

import type { Schema as NostrSchema } from "@openagentsinc/nostr"
import { Context, Data, Effect, Layer, Option, Ref, Schema, Stream } from "effect"
import { WebSocketService } from "./WebSocketService.js"

// Use NostrEvent type from the nostr package
type NostrEvent = NostrSchema.NostrEvent

// Agent schemas
export const AgentProfile = Schema.Struct({
  pubkey: Schema.String,
  agent_id: Schema.String,
  name: Schema.String,
  status: Schema.Union(
    Schema.Literal("active"),
    Schema.Literal("hibernating"),
    Schema.Literal("offline")
  ),
  balance: Schema.Number,
  metabolic_rate: Schema.Number,
  capabilities: Schema.Array(Schema.String),
  description: Schema.String,
  avatar: Schema.optional(Schema.String),
  last_activity: Schema.Number,
  created_at: Schema.Number,
  updated_at: Schema.Number
})
export type AgentProfile = Schema.Schema.Type<typeof AgentProfile>

export const AgentStatus = Schema.Struct({
  pubkey: Schema.String,
  status: Schema.Union(
    Schema.Literal("active"),
    Schema.Literal("hibernating"),
    Schema.Literal("offline")
  ),
  last_seen: Schema.Number,
  current_job: Schema.optional(Schema.String),
  metrics: Schema.optional(Schema.Struct({
    requests_completed: Schema.Number,
    average_response_time: Schema.Number,
    success_rate: Schema.Number
  }))
})
export type AgentStatus = Schema.Schema.Type<typeof AgentStatus>

// Errors
export class AgentError extends Data.TaggedError("AgentError")<{
  reason: "connection_failed" | "invalid_profile" | "subscription_failed" | "not_found"
  message: string
  cause?: unknown
}> {}

// Nostr message types
type NostrMessage =
  | ["EVENT", string, NostrEvent]
  | ["EOSE", string]
  | ["OK", string, boolean, string]
  | ["CLOSED", string, string]
  | ["NOTICE", string]

// Agent Service
export class AgentService extends Context.Tag("sdk/AgentService")<
  AgentService,
  {
    readonly agents: Stream.Stream<AgentProfile, AgentError>
    readonly agentStatus: (pubkey: string) => Stream.Stream<AgentStatus, AgentError>
    readonly getAgent: (pubkey: string) => Effect.Effect<AgentProfile | null, AgentError>
  }
>() {}

// Generate subscription ID
const generateSubId = () => `sub-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`

// Live implementation
export const AgentServiceLive = Layer.effect(
  AgentService,
  Effect.gen(function*() {
    const wsService = yield* WebSocketService

    // Connect to relay
    const connection = yield* wsService.connect("ws://localhost:3003/relay").pipe(
      Effect.scoped,
      Effect.catchAll((error) =>
        Effect.fail(
          new AgentError({
            reason: "connection_failed",
            message: error.message,
            cause: error
          })
        )
      )
    )

    // Agent cache
    const agentCache = yield* Ref.make(new Map<string, AgentProfile>())

    // Parse Nostr messages
    const parseMessage = (data: string): Option.Option<NostrMessage> => {
      try {
        const msg = JSON.parse(data)
        if (Array.isArray(msg) && msg.length >= 2) {
          return Option.some(msg as NostrMessage)
        }
        return Option.none()
      } catch {
        return Option.none()
      }
    }

    // Subscribe to agent profiles
    const subscribeToAgents = Effect.gen(function*() {
      const subId = generateSubId()

      // Send subscription request for NIP-OA agent profiles
      const req = JSON.stringify([
        "REQ",
        subId,
        {
          kinds: [31337], // NIP-OA agent profile
          limit: 100
        }
      ])

      yield* connection.send(req).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new AgentError({
              reason: "subscription_failed",
              message: "Failed to subscribe to agents",
              cause: error
            })
          )
        )
      )

      // Process agent events
      return connection.messages.pipe(
        Stream.mapEffect((data) =>
          Effect.gen(function*() {
            const msg = parseMessage(data)
            if (Option.isNone(msg)) return Option.none()

            const message = msg.value
            if (message[0] === "EVENT" && message[1] === subId) {
              const event = message[2]
              if (event.kind === 31337) {
                try {
                  const content = JSON.parse(event.content)

                  // Extract agent ID from d tag
                  const dTag = event.tags.find((tag: ReadonlyArray<string>) => tag[0] === "d")
                  if (!dTag) return Option.none()

                  // Extract other metadata from tags
                  const nameTag = event.tags.find((tag: ReadonlyArray<string>) => tag[0] === "name")
                  const statusTag = event.tags.find((tag: ReadonlyArray<string>) => tag[0] === "status")
                  const balanceTag = event.tags.find((tag: ReadonlyArray<string>) => tag[0] === "balance")
                  const metabolicRateTag = event.tags.find((tag: ReadonlyArray<string>) => tag[0] === "metabolic_rate")

                  const agent: AgentProfile = {
                    pubkey: event.pubkey,
                    agent_id: dTag[1],
                    name: nameTag?.[1] || "Unknown Agent",
                    status: (statusTag?.[1] || "offline") as "active" | "hibernating" | "offline",
                    balance: parseInt(balanceTag?.[1] || "0"),
                    metabolic_rate: parseInt(metabolicRateTag?.[1] || "100"),
                    capabilities: content.capabilities?.map((c: any) => c.name) || [],
                    description: content.description || "",
                    avatar: content.avatar,
                    last_activity: event.created_at,
                    created_at: event.created_at,
                    updated_at: event.created_at
                  }

                  // Update cache
                  yield* Ref.update(agentCache, (cache) => {
                    const newCache = new Map(cache)
                    newCache.set(agent.pubkey, agent)
                    return newCache
                  })

                  return Option.some(agent)
                } catch {
                  return Option.none()
                }
              }
            }
            return Option.none()
          })
        ),
        Stream.filter(Option.isSome),
        Stream.map((opt) => opt.value)
      )
    })

    // Subscribe to agent status updates
    const subscribeToAgentStatus = (pubkey: string) =>
      Effect.gen(function*() {
        const subId = generateSubId()

        // Send subscription request for status updates
        const req = JSON.stringify([
          "REQ",
          subId,
          {
            kinds: [30078], // NIP-78 application-specific data (for status)
            authors: [pubkey],
            "#d": ["status"],
            limit: 1
          }
        ])

        yield* connection.send(req).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new AgentError({
                reason: "subscription_failed",
                message: "Failed to subscribe to agent status",
                cause: error
              })
            )
          )
        )

        // Process status events
        return connection.messages.pipe(
          Stream.mapEffect((data) =>
            Effect.sync(() => {
              const msg = parseMessage(data)
              if (Option.isNone(msg)) return Option.none()

              const message = msg.value
              if (message[0] === "EVENT" && message[1] === subId) {
                const event = message[2]
                if (event.kind === 30078 && event.pubkey === pubkey) {
                  try {
                    const content = JSON.parse(event.content)
                    const status: AgentStatus = {
                      pubkey: event.pubkey,
                      status: content.status || "offline",
                      last_seen: event.created_at,
                      current_job: content.current_job,
                      metrics: content.metrics
                    }
                    return Option.some(status)
                  } catch {
                    return Option.none()
                  }
                }
              }
              return Option.none()
            })
          ),
          Stream.filter(Option.isSome),
          Stream.map((opt) => opt.value)
        )
      })

    return {
      agents: Stream.unwrap(subscribeToAgents).pipe(
        Stream.catchAll((error) =>
          Stream.fail(
            new AgentError({
              reason: "subscription_failed",
              message: error instanceof Error ? error.message : "Unknown error",
              cause: error
            })
          )
        )
      ),

      agentStatus: (pubkey: string) =>
        Stream.unwrap(subscribeToAgentStatus(pubkey)).pipe(
          Stream.catchAll((error) =>
            Stream.fail(
              new AgentError({
                reason: "subscription_failed",
                message: error instanceof Error ? error.message : "Unknown error",
                cause: error
              })
            )
          )
        ),

      getAgent: (pubkey: string) =>
        Effect.gen(function*() {
          const cache = yield* Ref.get(agentCache)
          return cache.get(pubkey) || null
        })
    }
  })
)

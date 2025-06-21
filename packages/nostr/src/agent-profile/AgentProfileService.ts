/**
 * Agent Profile Service - NIP-OA Agent Identity Management
 * Handles creation, updates, and queries for agent profiles
 */

import { Context, Effect, Layer, Schema } from "effect"
import type { PrivateKey } from "../core/Schema.js"
import { EventService } from "../services/EventService.js"
import { RelayService } from "../services/RelayService.js"

// NIP-OA Agent Profile Event Kind
const AGENT_PROFILE_KIND = 31337

// Agent profile content schema
export const AgentProfileContent = Schema.Struct({
  description: Schema.String,
  avatar: Schema.optional(Schema.String),
  capabilities: Schema.Array(Schema.Struct({
    id: Schema.String,
    name: Schema.String,
    description: Schema.String,
    pricing: Schema.Struct({
      base: Schema.Number,
      per_unit: Schema.optional(Schema.String),
      unit_limit: Schema.optional(Schema.Number)
    }),
    nip90_kinds: Schema.optional(Schema.Array(Schema.Number))
  })),
  pricing_models: Schema.optional(Schema.Struct({
    subscription_monthly: Schema.optional(Schema.Number),
    per_request: Schema.optional(Schema.Number)
  })),
  constraints: Schema.optional(Schema.Struct({
    max_monthly_requests: Schema.optional(Schema.Number),
    max_concurrent_jobs: Schema.optional(Schema.Number),
    supported_languages: Schema.optional(Schema.Array(Schema.String))
  })),
  metrics: Schema.optional(Schema.Struct({
    total_earned: Schema.optional(Schema.Number),
    total_spent: Schema.optional(Schema.Number),
    requests_completed: Schema.optional(Schema.Number),
    average_rating: Schema.optional(Schema.Number),
    uptime_percentage: Schema.optional(Schema.Number)
  }))
})

export type AgentProfileContent = Schema.Schema.Type<typeof AgentProfileContent>

// Agent profile with metadata
export const AgentProfile = Schema.Struct({
  pubkey: Schema.String,
  agent_id: Schema.String,
  name: Schema.String,
  status: Schema.Literal("active", "hibernating", "offline"),
  balance: Schema.Number,
  metabolic_rate: Schema.Number,
  capabilities: Schema.Array(Schema.String),
  content: AgentProfileContent,
  last_activity: Schema.String,
  profile_event_id: Schema.optional(Schema.String),
  created_at: Schema.String,
  updated_at: Schema.String
})

export type AgentProfile = Schema.Schema.Type<typeof AgentProfile>

// Agent profile creation parameters
export const CreateAgentProfileParams = Schema.Struct({
  agent_id: Schema.String,
  name: Schema.String,
  content: AgentProfileContent,
  status: Schema.optional(Schema.Literal("active", "hibernating", "offline")),
  balance: Schema.optional(Schema.Number),
  metabolic_rate: Schema.optional(Schema.Number)
})

export type CreateAgentProfileParams = Schema.Schema.Type<typeof CreateAgentProfileParams>

// Agent profile filters
export const AgentProfileFilters = Schema.Struct({
  pubkeys: Schema.optional(Schema.Array(Schema.String)),
  agent_ids: Schema.optional(Schema.Array(Schema.String)),
  status: Schema.optional(Schema.Array(Schema.Literal("active", "hibernating", "offline"))),
  capabilities: Schema.optional(Schema.Array(Schema.String)),
  min_balance: Schema.optional(Schema.Number),
  limit: Schema.optional(Schema.Number),
  since: Schema.optional(Schema.Number)
})

export type AgentProfileFilters = Schema.Schema.Type<typeof AgentProfileFilters>

// Errors
export class AgentProfileError extends Schema.TaggedError<AgentProfileError>()("AgentProfileError", {
  reason: Schema.Literal("validation_failed", "event_creation_failed", "relay_error", "not_found"),
  message: Schema.String,
  details: Schema.optional(Schema.Unknown)
}) {}

// Stub service for now - will be improved later
export class AgentProfileService extends Context.Tag("nostr/AgentProfileService")<
  AgentProfileService,
  {
    readonly createProfile: (
      pubkey: string,
      privateKey: string,
      params: CreateAgentProfileParams
    ) => Effect.Effect<any, AgentProfileError>

    readonly updateProfile: (
      pubkey: string,
      privateKey: string,
      updates: Partial<CreateAgentProfileParams>
    ) => Effect.Effect<any, AgentProfileError>

    readonly getProfile: (
      pubkey: string
    ) => Effect.Effect<AgentProfile | null, AgentProfileError>

    readonly listProfiles: (
      filters?: AgentProfileFilters
    ) => Effect.Effect<Array<AgentProfile>, AgentProfileError>

    readonly publishProfile: (
      event: any,
      relays?: Array<string>
    ) => Effect.Effect<void, AgentProfileError>

    readonly subscribeToProfiles: (
      filters: AgentProfileFilters,
      onProfile: (profile: AgentProfile) => void
    ) => Effect.Effect<() => void, AgentProfileError>
  }
>() {}

export const AgentProfileServiceLive = Layer.effect(
  AgentProfileService,
  Effect.gen(function*() {
    const eventService = yield* EventService
    const relayService = yield* RelayService

    const createProfile = (
      pubkey: string,
      privateKey: string,
      params: CreateAgentProfileParams
    ) =>
      Effect.gen(function*() {
        // Build tags for the profile event
        const tags: Array<Array<string>> = [
          ["d", params.agent_id],
          ["name", params.name]
        ]

        if (params.status) {
          tags.push(["status", params.status])
        }

        if (params.balance !== undefined) {
          tags.push(["balance", params.balance.toString()])
        }

        if (params.metabolic_rate !== undefined) {
          tags.push(["metabolic_rate", params.metabolic_rate.toString()])
        }

        // Create the event
        const event = yield* eventService.create(
          {
            kind: AGENT_PROFILE_KIND,
            tags,
            content: JSON.stringify(params.content)
          },
          privateKey as PrivateKey
        )

        return event
      }).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new AgentProfileError({
              reason: "event_creation_failed",
              message: String(error),
              details: error
            })
          )
        )
      )

    const updateProfile = (
      pubkey: string,
      privateKey: string,
      updates: Partial<CreateAgentProfileParams>
    ) =>
      Effect.gen(function*() {
        // For updates, we need to merge with existing profile
        // In a real implementation, we'd fetch the current profile first
        // For now, create a new event with the updates
        const tags: Array<Array<string>> = []

        if (updates.agent_id) {
          tags.push(["d", updates.agent_id])
        }

        if (updates.name) {
          tags.push(["name", updates.name])
        }

        if (updates.status) {
          tags.push(["status", updates.status])
        }

        if (updates.balance !== undefined) {
          tags.push(["balance", updates.balance.toString()])
        }

        if (updates.metabolic_rate !== undefined) {
          tags.push(["metabolic_rate", updates.metabolic_rate.toString()])
        }

        const event = yield* eventService.create(
          {
            kind: AGENT_PROFILE_KIND,
            tags,
            content: JSON.stringify(updates.content || {})
          },
          privateKey as PrivateKey
        )

        return event
      }).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new AgentProfileError({
              reason: "event_creation_failed",
              message: String(error),
              details: error
            })
          )
        )
      )

    const getProfile = (_pubkey: string) =>
      Effect.gen(function*() {
        // In a real implementation, query relays for the profile
        // For now, return null as we don't have relay querying yet
        yield* Effect.succeed(undefined) // Dummy yield
        return null
      })

    const listProfiles = (_filters?: AgentProfileFilters) =>
      Effect.gen(function*() {
        // In a real implementation, query relays for profiles
        // For now, return empty array
        yield* Effect.succeed(undefined) // Dummy yield
        return []
      })

    const publishProfile = (event: any, relays?: Array<string>) =>
      Effect.gen(function*() {
        if (!relays || relays.length === 0) {
          return // No relays to publish to
        }

        // Connect to relays and publish
        for (const relay of relays) {
          yield* Effect.scoped(
            Effect.gen(function*() {
              const connection = yield* relayService.connect(relay)
              yield* connection.publish(event)
            })
          ).pipe(
            Effect.catchAll((error) => {
              // Log error but continue with other relays
              console.error(`Failed to publish to relay ${relay}:`, error)
              return Effect.succeed(undefined)
            })
          )
        }
      }).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new AgentProfileError({
              reason: "relay_error",
              message: String(error),
              details: error
            })
          )
        )
      )

    const subscribeToProfiles = (
      _filters: AgentProfileFilters,
      _onProfile: (profile: AgentProfile) => void
    ) =>
      Effect.gen(function*() {
        // In a real implementation, subscribe to relay events
        // For now, return a no-op unsubscribe function
        yield* Effect.succeed(undefined) // Dummy yield
        return () => {}
      })

    return {
      createProfile,
      updateProfile,
      getProfile,
      listProfiles,
      publishProfile,
      subscribeToProfiles
    }
  })
)

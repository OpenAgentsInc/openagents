/**
 * Service Marketplace API - Real NIP-90 service offerings and job management
 */

import { RelayDatabase } from "@openagentsinc/relay"
import { Effect, Layer } from "effect"

export async function GET(request: Request): Promise<Response> {
  try {
    const url = new URL(request.url)
    const agentPubkey = url.searchParams.get("agentPubkey")
    const capabilities = url.searchParams.get("capabilities")?.split(",")

    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase

      // Get service offerings from database
      const offerings = yield* database.getServiceOfferings({
        agentPubkey: agentPubkey ?? undefined,
        capabilities: capabilities ?? undefined
      })

      // Get all agent profiles for provider names
      const agents = yield* database.getActiveAgents()
      const agentMap = new Map(agents.map((a) => [a.pubkey, a]))

      // Transform database offerings to UI format
      const services = offerings.map((offering) => ({
        id: offering.id,
        name: offering.service_name,
        provider: agentMap.get(offering.agent_pubkey)?.name || `Agent-${offering.agent_pubkey.slice(0, 8)}`,
        description: `NIP-90 service: ${offering.service_name}`,
        basePrice: offering.pricing.base,
        capabilities: offering.capabilities,
        nip90_kinds: offering.nip90_kinds,
        availability: offering.availability,
        agent_pubkey: offering.agent_pubkey
      }))

      return { services }
    })

    // Use stub database for now - in production this would use real database layer
    const result = await Effect.runPromise(
      program.pipe(
        Effect.provide(
          Layer.succeed(RelayDatabase, {
            storeEvent: () => Effect.succeed(true),
            getAgentProfile: () => Effect.succeed(null),
            updateAgentProfile: () => Effect.succeed({} as any),
            getActiveAgents: () =>
              Effect.succeed([
                {
                  pubkey: "agent_beta_pubkey",
                  agent_id: "agent-beta",
                  name: "Agent Beta",
                  status: "active",
                  balance: 25000,
                  metabolic_rate: 120,
                  capabilities: ["TypeScript", "Security", "Code Review"],
                  last_activity: new Date(),
                  profile_event_id: "profile_event_beta",
                  created_at: new Date(),
                  updated_at: new Date()
                },
                {
                  pubkey: "agent_delta_pubkey",
                  agent_id: "agent-delta",
                  name: "Agent Delta",
                  status: "active",
                  balance: 30000,
                  metabolic_rate: 100,
                  capabilities: ["Documentation", "API Design"],
                  last_activity: new Date(),
                  profile_event_id: "profile_event_delta",
                  created_at: new Date(),
                  updated_at: new Date()
                }
              ]),
            queryEvents: () => Effect.succeed([]),
            getEvent: () => Effect.succeed(null),
            deleteEvent: () => Effect.succeed(true),
            getServiceOfferings: () =>
              Effect.succeed([
                {
                  id: "service-beta-security",
                  agent_pubkey: "agent_beta_pubkey",
                  service_name: "Security Code Review",
                  nip90_kinds: [5901, 5902], // NIP-90 code analysis
                  pricing: { base: 500, currency: "sats" },
                  capabilities: ["TypeScript", "React", "Security", "Authentication"],
                  availability: "available" as const,
                  offering_event_id: "event123",
                  created_at: new Date(),
                  updated_at: new Date()
                },
                {
                  id: "service-delta-docs",
                  agent_pubkey: "agent_delta_pubkey",
                  service_name: "API Documentation Generator",
                  nip90_kinds: [5000], // NIP-90 text generation
                  pricing: { base: 250, currency: "sats" },
                  capabilities: ["Documentation", "OpenAPI", "REST", "GraphQL"],
                  availability: "available" as const,
                  offering_event_id: "event456",
                  created_at: new Date(),
                  updated_at: new Date()
                }
              ]),
            updateServiceOffering: () => Effect.succeed({} as any),
            getJobRequests: () => Effect.succeed([]),
            updateJobRequest: () => Effect.succeed({} as any),
            getChannels: () => Effect.succeed([]),
            updateChannelStats: () => Effect.succeed(undefined),
            recordMetric: () => Effect.succeed(undefined),
            getMetrics: () => Effect.succeed([])
          })
        )
      )
    )

    return Response.json(result)
  } catch (error) {
    console.error("Service query error:", error)
    return Response.json(
      {
        success: false,
        error: "Failed to query services",
        details: error instanceof Error ? error.message : String(error)
      },
      { status: 500 }
    )
  }
}

export async function POST(request: Request): Promise<Response> {
  try {
    const body = await request.json()
    const { budget, description, serviceType, targetAgentPubkey } = body

    // For now, create a mock job request - in production this would:
    // 1. Create a NIP-90 job request event
    // 2. Publish it to relays
    // 3. Store in database
    // 4. Notify target agent

    const job = {
      id: `job-${Date.now()}`,
      type: serviceType,
      status: "pending" as const,
      requester: "Current Agent", // Would be real agent pubkey
      provider: targetAgentPubkey ? `Agent-${targetAgentPubkey.slice(0, 8)}` : "Looking for provider...",
      amount: parseInt(budget),
      description,
      timestamp: Date.now(),
      nip90_event_id: null // Would be real event ID
    }

    return Response.json({
      success: true,
      job
    })
  } catch (error) {
    console.error("Service request error:", error)
    return Response.json(
      {
        success: false,
        error: "Failed to create service request",
        details: error instanceof Error ? error.message : String(error)
      },
      { status: 500 }
    )
  }
}

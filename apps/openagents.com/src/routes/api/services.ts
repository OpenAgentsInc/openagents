/**
 * Service Marketplace API - Real NIP-90 service offerings and job management
 */

import { RelayDatabase, RelayDatabaseLive } from "@openagentsinc/relay"
import { Effect } from "effect"

export async function GET(request: Request): Promise<Response> {
  try {
    const url = new URL(request.url)
    const agentPubkey = url.searchParams.get("agentPubkey")
    const capabilities = url.searchParams.get("capabilities")?.split(",")

    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase

      // Get service offerings from database
      const offerings = yield* database.getServiceOfferings(
        agentPubkey || capabilities
          ? {
            ...(agentPubkey && { agentPubkey }),
            ...(capabilities && { capabilities })
          }
          : undefined
      )

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

    // Use real database layer
    const result = await Effect.runPromise(
      program.pipe(
        Effect.provide(RelayDatabaseLive)
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
    const { budget, description, requesterPubkey, serviceType, targetAgentPubkey } = body

    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase

      // Create a real job request in the database
      const jobId = `job-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`

      const jobRequest = yield* database.updateJobRequest({
        id: jobId,
        request_event_id: null, // Will be set when NIP-90 event is created
        requester_pubkey: requesterPubkey || "pending-agent-creation",
        provider_pubkey: targetAgentPubkey || null,
        service_type: serviceType,
        status: "pending",
        payment_amount: parseInt(budget),
        description,
        result_data: {},
        created_at: new Date(),
        updated_at: new Date()
      })

      return {
        success: true,
        job: {
          id: jobRequest.id,
          type: jobRequest.service_type,
          status: jobRequest.status,
          requester: jobRequest.requester_pubkey,
          provider: jobRequest.provider_pubkey || "Looking for provider...",
          amount: jobRequest.payment_amount,
          description: jobRequest.description,
          timestamp: jobRequest.created_at.getTime()
        }
      }
    })

    const result = await Effect.runPromise(
      program.pipe(
        Effect.provide(RelayDatabaseLive)
      )
    )

    return Response.json(result)
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

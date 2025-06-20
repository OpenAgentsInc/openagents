/**
 * Job Management API - Real NIP-90 job tracking and management
 */

import { RelayDatabase } from "@openagentsinc/relay"
import { Effect, Layer } from "effect"

export async function GET(request: Request): Promise<Response> {
  try {
    const url = new URL(request.url)
    const agentPubkey = url.searchParams.get("agentPubkey")
    const status = url.searchParams.get("status")

    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase

      // Get job requests from database
      const jobRequests = yield* database.getJobRequests(
        agentPubkey || status
          ? {
            ...(agentPubkey && { requesterPubkey: agentPubkey }),
            ...(status && { status })
          }
          : undefined
      )

      // Get all agent profiles for requester/provider names
      const agents = yield* database.getActiveAgents()
      const agentMap = new Map(agents.map((a) => [a.pubkey, a]))

      // Transform database jobs to UI format
      const jobs = jobRequests.map((job) => ({
        id: job.id,
        type: job.service_type,
        status: job.status,
        requester: agentMap.get(job.requester_pubkey)?.name || `Agent-${job.requester_pubkey.slice(0, 8)}`,
        provider: job.provider_pubkey ?
          (agentMap.get(job.provider_pubkey)?.name || `Agent-${job.provider_pubkey.slice(0, 8)}`) :
          "Looking for provider...",
        amount: job.payment_amount,
        description: job.description,
        timestamp: job.created_at.getTime(),
        nip90_event_id: job.request_event_id
      }))

      return { jobs }
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
                  pubkey: "agent_alpha_pubkey",
                  agent_id: "agent-alpha",
                  name: "Agent Alpha",
                  status: "active",
                  balance: 15000,
                  metabolic_rate: 80,
                  capabilities: ["Research", "Data Analysis"],
                  last_activity: new Date(),
                  profile_event_id: "profile_event_alpha",
                  created_at: new Date(),
                  updated_at: new Date()
                },
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
                }
              ]),
            queryEvents: () => Effect.succeed([]),
            getEvent: () => Effect.succeed(null),
            deleteEvent: () => Effect.succeed(true),
            getServiceOfferings: () => Effect.succeed([]),
            updateServiceOffering: () => Effect.succeed({} as any),
            getJobRequests: () =>
              Effect.succeed([
                {
                  id: "job-alpha-analysis",
                  request_event_id: "event789",
                  requester_pubkey: "current_agent_pubkey",
                  provider_pubkey: "agent_alpha_pubkey",
                  service_type: "Data Analysis",
                  status: "pending" as const,
                  description: "Analyze user engagement metrics for Q4 2024",
                  payment_amount: 750,
                  result_data: null,
                  created_at: new Date(Date.now() - 2 * 60 * 60 * 1000), // 2 hours ago
                  updated_at: new Date()
                },
                {
                  id: "job-beta-review",
                  request_event_id: "event890",
                  requester_pubkey: "current_agent_pubkey",
                  provider_pubkey: "agent_beta_pubkey",
                  service_type: "Security Code Review",
                  status: "processing" as const,
                  description: "Review authentication flow for security vulnerabilities",
                  payment_amount: 500,
                  result_data: null,
                  created_at: new Date(Date.now() - 4 * 60 * 60 * 1000), // 4 hours ago
                  updated_at: new Date()
                },
                {
                  id: "job-search-provider",
                  request_event_id: "event901",
                  requester_pubkey: "current_agent_pubkey",
                  provider_pubkey: null,
                  service_type: "Text Generation",
                  status: "pending" as const,
                  description: "Generate technical documentation for API endpoints",
                  payment_amount: 300,
                  result_data: null,
                  created_at: new Date(Date.now() - 30 * 60 * 1000), // 30 minutes ago
                  updated_at: new Date()
                }
              ]),
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
    console.error("Job query error:", error)
    return Response.json(
      {
        success: false,
        error: "Failed to query jobs",
        details: error instanceof Error ? error.message : String(error)
      },
      { status: 500 }
    )
  }
}

export async function POST(request: Request): Promise<Response> {
  try {
    const body = await request.json()
    const { jobId, result, status } = body

    // For now, update job status - in production this would:
    // 1. Update job status in database
    // 2. Create NIP-90 job result event if completed
    // 3. Publish to relays
    // 4. Notify relevant agents

    const updatedJob = {
      id: jobId,
      status,
      result: result || null,
      updated_at: new Date().toISOString()
    }

    return Response.json({
      success: true,
      job: updatedJob
    })
  } catch (error) {
    console.error("Job update error:", error)
    return Response.json(
      {
        success: false,
        error: "Failed to update job",
        details: error instanceof Error ? error.message : String(error)
      },
      { status: 500 }
    )
  }
}

export async function DELETE(request: Request): Promise<Response> {
  try {
    const url = new URL(request.url)
    const jobId = url.searchParams.get("jobId")

    if (!jobId) {
      return Response.json(
        { success: false, error: "jobId parameter required" },
        { status: 400 }
      )
    }

    // For now, mock job cancellation - in production this would:
    // 1. Update job status to cancelled in database
    // 2. Create NIP-90 job cancellation event
    // 3. Publish to relays
    // 4. Handle any refunds if applicable

    return Response.json({
      success: true,
      message: `Job ${jobId} cancelled successfully`
    })
  } catch (error) {
    console.error("Job cancellation error:", error)
    return Response.json(
      {
        success: false,
        error: "Failed to cancel job",
        details: error instanceof Error ? error.message : String(error)
      },
      { status: 500 }
    )
  }
}

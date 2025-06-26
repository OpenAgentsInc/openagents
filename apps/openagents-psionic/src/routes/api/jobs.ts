/**
 * Job Management API - Real NIP-90 job tracking and management
 */

import { RelayDatabase, RelayDatabaseLive } from "@openagentsinc/relay"
import { Effect } from "effect"

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

    // Use real database layer
    const result = await Effect.runPromise(
      program.pipe(
        Effect.provide(RelayDatabaseLive)
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
    const { jobId, result_data, status } = body

    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase

      // Get the existing job request
      const jobs = yield* database.getJobRequests({ status: "pending" })
      const existingJob = jobs.find((j) => j.id === jobId)

      if (!existingJob) {
        throw new Error(`Job ${jobId} not found`)
      }

      // Update job status in database
      const updatedJob = yield* database.updateJobRequest({
        ...existingJob,
        status,
        result_data: result_data || existingJob.result_data,
        updated_at: new Date()
      })

      return {
        success: true,
        job: {
          id: updatedJob.id,
          status: updatedJob.status,
          result_data: updatedJob.result_data,
          updated_at: updatedJob.updated_at.toISOString()
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

    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase

      // Get the existing job request
      const jobs = yield* database.getJobRequests()
      const existingJob = jobs.find((j) => j.id === jobId)

      if (!existingJob) {
        throw new Error(`Job ${jobId} not found`)
      }

      // Update job status to cancelled in database
      yield* database.updateJobRequest({
        ...existingJob,
        status: "cancelled",
        updated_at: new Date()
      })

      return {
        success: true,
        message: `Job ${jobId} cancelled successfully`
      }
    })

    const result = await Effect.runPromise(
      program.pipe(
        Effect.provide(RelayDatabaseLive)
      )
    )

    return Response.json(result)
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

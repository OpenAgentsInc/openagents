/**
 * Agent Management API - Real agent creation and profile management
 */

import * as NostrLib from "@openagentsinc/nostr"
import { RelayDatabase, RelayDatabaseLive } from "@openagentsinc/relay"
import * as SDK from "@openagentsinc/sdk"
import { Effect, Layer } from "effect"

export async function POST(request: Request): Promise<Response> {
  try {
    const body = await request.json()
    const { capital = 50000, metabolicRate = 100, name } = body

    // Create real agent with proper NIP-06 key derivation
    const program = Effect.gen(function*() {
      // Generate real BIP39 mnemonic
      const mnemonic = yield* Effect.promise(() => SDK.Agent.generateMnemonic())

      // Create agent from mnemonic (gets real keys)
      const agent = yield* Effect.promise(() =>
        SDK.Agent.createFromMnemonic(mnemonic, {
          name,
          initial_capital: capital,
          stop_price: metabolicRate
        })
      )

      // Create agent profile content for NIP-OA
      const profileContent: NostrLib.AgentProfileService.AgentProfileContent = {
        description: `Autonomous agent specialized in ${name.toLowerCase()}`,
        capabilities: [
          {
            id: "basic-tasks",
            name: "Basic Task Execution",
            description: "Can perform basic computational tasks",
            pricing: {
              base: 100,
              per_unit: "per task"
            }
          }
        ],
        pricing_models: {
          per_request: 100,
          subscription_monthly: 10000
        },
        constraints: {
          max_concurrent_jobs: 5,
          supported_languages: ["javascript", "typescript"]
        },
        metrics: {
          requests_completed: 0,
          total_earned: 0,
          total_spent: 0,
          average_rating: 5.0,
          uptime_percentage: 100.0
        }
      }

      // Create NIP-OA agent profile event using proper service
      const agentProfileService = yield* NostrLib.AgentProfileService.AgentProfileService
      const profileEvent = yield* agentProfileService.createProfile(
        agent.nostrKeys.public.replace("npub", ""), // Convert npub to hex pubkey
        agent.nostrKeys.private,
        {
          agent_id: agent.id,
          name: agent.name,
          content: profileContent,
          status: "active",
          balance: capital,
          metabolic_rate: metabolicRate
        }
      )

      // Store the event in the relay database
      const database = yield* RelayDatabase
      const stored = yield* database.storeEvent(profileEvent)

      if (!stored) {
        return yield* Effect.fail(new Error("Failed to store agent profile"))
      }

      // Get the stored agent profile from database
      const storedProfile = yield* database.getAgentProfile(agent.nostrKeys.public.replace("npub", ""))

      return {
        success: true,
        agent: {
          id: agent.id,
          name: agent.name,
          publicKey: agent.nostrKeys.public,
          npub: agent.nostrKeys.public,
          balance: capital,
          metabolicRate,
          lifecycleState: agent.lifecycleState,
          mnemonic // Include for recovery (in production, encrypt this)
        },
        profile: storedProfile
      }
    })

    // Build Effect layers for all required services
    const baseLayer = Layer.merge(
      NostrLib.WebSocketService.WebSocketServiceLive,
      NostrLib.CryptoService.CryptoServiceLive
    )

    const serviceLayer = Layer.merge(
      NostrLib.EventService.EventServiceLive,
      NostrLib.RelayService.RelayServiceLive
    )

    const nostrLayer = Layer.provideMerge(
      Layer.provideMerge(NostrLib.AgentProfileService.AgentProfileServiceLive, serviceLayer),
      baseLayer
    )

    // Database layer - use real database
    const dbLayer = RelayDatabaseLive

    const AppLayer = Layer.merge(nostrLayer, dbLayer)

    const result = await Effect.runPromise(
      program.pipe(
        Effect.provide(AppLayer)
      )
    )

    return Response.json(result)
  } catch (error) {
    console.error("Agent creation error:", error)
    return Response.json(
      {
        success: false,
        error: "Failed to create agent",
        details: error instanceof Error ? error.message : String(error)
      },
      { status: 500 }
    )
  }
}

export async function GET(request: Request): Promise<Response> {
  try {
    const url = new URL(request.url)
    const pubkey = url.searchParams.get("pubkey")

    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase

      if (pubkey) {
        // Get specific agent profile
        const profile = yield* database.getAgentProfile(pubkey)
        return { agent: profile }
      } else {
        // Get all active agents
        const agents = yield* database.getActiveAgents()
        return { agents }
      }
    })

    // Use real database layer
    const result = await Effect.runPromise(
      program.pipe(
        Effect.provide(RelayDatabaseLive)
      )
    )

    return Response.json(result)
  } catch (error) {
    console.error("Agent query error:", error)
    return Response.json(
      {
        success: false,
        error: "Failed to query agents",
        details: error instanceof Error ? error.message : String(error)
      },
      { status: 500 }
    )
  }
}

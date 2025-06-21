/**
 * Autonomous Marketplace Agent - AI-powered agents that participate in NIP-90 service economy
 * Extends chat capabilities with job discovery, bidding, and service delivery
 */

import { Context, Data, Effect, Fiber, Layer, Option, Schema, Stream } from "effect"
import { AgentPersonality } from "./AutonomousChatAgent.js"

import * as AI from "@openagentsinc/ai"
import * as NostrLib from "@openagentsinc/nostr"

// Marketplace personality extension
export const MarketplacePersonality = Schema.extend(
  AgentPersonality,
  Schema.Struct({
    // Economic behavior
    riskTolerance: Schema.Union(
      Schema.Literal("low"),
      Schema.Literal("medium"),
      Schema.Literal("high")
    ),
    pricingStrategy: Schema.Union(
      Schema.Literal("competitive"),
      Schema.Literal("premium"),
      Schema.Literal("budget")
    ),
    serviceSpecializations: Schema.Array(Schema.String), // ["code-review", "text-generation"]
    minimumProfit: Schema.Number, // Minimum sats to accept a job
    workloadCapacity: Schema.Number.pipe(Schema.between(1, 10)) // Max concurrent jobs
  })
)
export type MarketplacePersonality = Schema.Schema.Type<typeof MarketplacePersonality>

// Job evaluation result
export const JobEvaluation = Schema.Struct({
  shouldBid: Schema.Boolean,
  bidAmount: Schema.Number,
  reasoning: Schema.String,
  confidence: Schema.Number.pipe(Schema.between(0, 1)),
  estimatedCompletionTime: Schema.Number // milliseconds
})
export type JobEvaluation = Schema.Schema.Type<typeof JobEvaluation>

// Service delivery result
export const ServiceDelivery = Schema.Struct({
  jobId: Schema.String,
  result: Schema.String,
  status: NostrLib.Nip90Service.JobStatus,
  computeTime: Schema.Number,
  tokensUsed: Schema.optional(Schema.Number),
  confidence: Schema.optional(Schema.Number)
})
export type ServiceDelivery = Schema.Schema.Type<typeof ServiceDelivery>

// Agent economic state
export interface AgentEconomicState {
  balance: number // sats
  activeJobs: Map<string, NostrLib.Nip90Service.JobRequest>
  completedJobs: number
  totalEarnings: number
  averageRating: number
}

// Errors
export class MarketplaceError extends Data.TaggedError("MarketplaceError")<{
  reason: "job_evaluation_failed" | "service_delivery_failed" | "payment_failed" | "capacity_exceeded"
  message: string
  cause?: unknown
}> {}

// Autonomous Marketplace Agent Service
export class AutonomousMarketplaceAgent extends Context.Tag("sdk/AutonomousMarketplaceAgent")<
  AutonomousMarketplaceAgent,
  {
    readonly startMarketplaceLoop: (
      personality: MarketplacePersonality,
      agentKeys: { privateKey: string; publicKey: string }
    ) => Effect.Effect<void, MarketplaceError, NostrLib.Nip90Service.Nip90Service>

    readonly stopMarketplaceLoop: (agentId: string) => Effect.Effect<void, never>

    readonly evaluateJob: (
      job: NostrLib.Nip90Service.JobRequest,
      personality: MarketplacePersonality,
      economicState: AgentEconomicState
    ) => Effect.Effect<JobEvaluation, MarketplaceError>

    readonly deliverService: (
      job: NostrLib.Nip90Service.JobRequest,
      personality: MarketplacePersonality
    ) => Effect.Effect<ServiceDelivery, MarketplaceError>

    readonly publishServiceOffering: (
      personality: MarketplacePersonality,
      privateKey: string
    ) => Effect.Effect<NostrLib.Schema.NostrEvent, MarketplaceError>
  }
>() {}

// AI-powered job evaluation
const evaluateJobWithAI = (
  job: NostrLib.Nip90Service.JobRequest,
  personality: MarketplacePersonality,
  economicState: AgentEconomicState,
  languageModel: AI.AiLanguageModel.AiLanguageModel.Service<never>
): Effect.Effect<JobEvaluation, MarketplaceError> =>
  Effect.gen(function*() {
    const prompt = `You are an autonomous agent with the following marketplace profile:
- Name: ${personality.name}
- Role: ${personality.role}
- Service Specializations: ${personality.serviceSpecializations.join(", ")}
- Risk Tolerance: ${personality.riskTolerance}
- Pricing Strategy: ${personality.pricingStrategy}
- Minimum Profit Requirement: ${personality.minimumProfit} sats
- Current Workload: ${economicState.activeJobs.size}/${personality.workloadCapacity} jobs

Current Economic State:
- Balance: ${economicState.balance} sats
- Active Jobs: ${economicState.activeJobs.size}
- Completed Jobs: ${economicState.completedJobs}
- Average Rating: ${economicState.averageRating}

Job Request:
- Type: ${getJobTypeName(job.requestKind)}
- Input: ${job.input.slice(0, 200)}...
- Current Bid: ${job.bidAmount} sats
- Requester: ${job.requester.slice(0, 8)}...

Should you bid on this job? If yes, what amount?

Respond with JSON:
{
  "shouldBid": true/false,
  "bidAmount": number,
  "reasoning": "explanation",
  "confidence": 0.0-1.0,
  "estimatedCompletionTime": milliseconds
}`

    const response = yield* languageModel.generateText({
      prompt: AI.AiPrompt.make(prompt)
    }).pipe(
      Effect.mapError((error) =>
        new MarketplaceError({
          reason: "job_evaluation_failed",
          message: `AI evaluation failed: ${error}`
        })
      )
    )

    const jsonMatch = response.text.match(/\{[\s\S]*\}/)
    if (!jsonMatch) {
      return yield* Effect.fail(
        new MarketplaceError({
          reason: "job_evaluation_failed",
          message: "No valid JSON in AI response"
        })
      )
    }

    try {
      const evaluation = JSON.parse(jsonMatch[0])
      return {
        shouldBid: Boolean(evaluation.shouldBid),
        bidAmount: Math.max(personality.minimumProfit, Number(evaluation.bidAmount) || job.bidAmount),
        reasoning: String(evaluation.reasoning || "AI evaluation"),
        confidence: Math.max(0, Math.min(1, Number(evaluation.confidence) || 0.5)),
        estimatedCompletionTime: Number(evaluation.estimatedCompletionTime) || 30000
      }
    } catch (error) {
      return yield* Effect.fail(
        new MarketplaceError({
          reason: "job_evaluation_failed",
          message: `Failed to parse evaluation: ${error}`
        })
      )
    }
  })

// AI-powered service delivery
const deliverServiceWithAI = (
  job: NostrLib.Nip90Service.JobRequest,
  personality: MarketplacePersonality,
  languageModel: AI.AiLanguageModel.AiLanguageModel.Service<never>
): Effect.Effect<ServiceDelivery, MarketplaceError> =>
  Effect.gen(function*() {
    const startTime = Date.now()

    // Build service-specific prompt based on job type
    const servicePrompt = buildServicePrompt(job, personality)

    const response = yield* languageModel.generateText({
      prompt: AI.AiPrompt.make(servicePrompt)
    }).pipe(
      Effect.mapError((error) =>
        new MarketplaceError({
          reason: "service_delivery_failed",
          message: `AI service failed: ${error}`
        })
      )
    )

    const computeTime = Date.now() - startTime

    return {
      jobId: job.jobId,
      result: response.text,
      status: "success" as NostrLib.Nip90Service.JobStatus,
      computeTime,
      tokensUsed: response.text.split(/\s+/).length, // Rough estimate
      confidence: 0.9
    }
  })

// Helper function to build service prompts
function buildServicePrompt(job: NostrLib.Nip90Service.JobRequest, personality: MarketplacePersonality): string {
  const baseContext = `You are an AI agent named "${personality.name}" with expertise in ${
    personality.serviceSpecializations.join(", ")
  }.`

  switch (job.requestKind) {
    case NostrLib.Nip90Service.NIP90_JOB_REQUEST_KINDS.CODE_REVIEW:
      return `${baseContext}\n\nPlease review this code:\n\n${job.input}\n\nProvide a thorough code review including:\n1. Code quality assessment\n2. Potential bugs or issues\n3. Performance considerations\n4. Best practice recommendations\n5. Security concerns if any`

    case NostrLib.Nip90Service.NIP90_JOB_REQUEST_KINDS.TEXT_GENERATION:
      return `${baseContext}\n\nGenerate text based on this request:\n\n${job.input}\n\nBe ${personality.responseStyle} in your writing style.`

    case NostrLib.Nip90Service.NIP90_JOB_REQUEST_KINDS.CODE_GENERATION:
      return `${baseContext}\n\nGenerate code based on this specification:\n\n${job.input}\n\nProvide clean, well-commented code that follows best practices.`

    case NostrLib.Nip90Service.NIP90_JOB_REQUEST_KINDS.SUMMARIZATION:
      return `${baseContext}\n\nSummarize the following content:\n\n${job.input}\n\nProvide a ${personality.responseStyle} summary that captures the key points.`

    default:
      return `${baseContext}\n\nComplete this task:\n\n${job.input}\n\nProvide a high-quality response.`
  }
}

// Helper function to get job type name
function getJobTypeName(kind: number): string {
  const kinds = Object.entries(NostrLib.Nip90Service.NIP90_JOB_REQUEST_KINDS)
  const found = kinds.find(([, value]) => value === kind)
  return found ? found[0].replace(/_/g, " ").toLowerCase() : "unknown service"
}

// Implementation
export const AutonomousMarketplaceAgentLive = Layer.effect(
  AutonomousMarketplaceAgent,
  Effect.gen(function*() {
    const languageModel = yield* AI.AiLanguageModel.AiLanguageModel
    const nip90Service = yield* NostrLib.Nip90Service.Nip90Service

    // Active marketplace loops
    const activeMarketplaceLoops = new Map<string, { stop: () => void }>()

    // Agent economic states
    const agentStates = new Map<string, AgentEconomicState>()

    const publishServiceOffering = (
      personality: MarketplacePersonality,
      privateKey: string
    ): Effect.Effect<NostrLib.Schema.NostrEvent, MarketplaceError> =>
      Effect.gen(function*() {
        const capabilities: Array<NostrLib.Nip90Service.ServiceCapability> = personality.serviceSpecializations.map(
          (spec) => ({
            id: spec,
            name: spec.replace(/-/g, " ").replace(/\b\w/g, (l) => l.toUpperCase()),
            description: `AI-powered ${spec} by ${personality.name}`,
            inputTypes: ["text"] as Array<NostrLib.Nip90Service.JobInputType>,
            outputType: "text",
            pricing: {
              basePrice: personality.pricingStrategy === "premium" ?
                1000 :
                personality.pricingStrategy === "budget"
                ? 100
                : 500,
              perUnit: "request"
            }
          })
        )

        const offeringEvent = yield* nip90Service.publishServiceOffering({
          serviceId: `agent-${personality.name}-${Date.now()}`,
          name: `${personality.name}'s AI Services`,
          description: `Autonomous AI agent offering ${personality.serviceSpecializations.join(", ")}. ${
            personality.traits.join(", ")
          }.`,
          capabilities,
          privateKey: privateKey as NostrLib.Schema.PrivateKey
        }).pipe(
          Effect.mapError((error) =>
            new MarketplaceError({
              reason: "service_delivery_failed",
              message: `Failed to publish service offering: ${error}`
            })
          )
        )

        return offeringEvent
      })

    const evaluateJob = (
      job: NostrLib.Nip90Service.JobRequest,
      personality: MarketplacePersonality,
      economicState: AgentEconomicState
    ): Effect.Effect<JobEvaluation, MarketplaceError> =>
      Effect.gen(function*() {
        // Check capacity
        if (economicState.activeJobs.size >= personality.workloadCapacity) {
          return {
            shouldBid: false,
            bidAmount: 0,
            reasoning: "At full capacity",
            confidence: 1.0,
            estimatedCompletionTime: 0
          }
        }

        // Check if job matches specializations
        const jobType = getJobTypeName(job.requestKind)
        const isSpecialized = personality.serviceSpecializations.some((spec) =>
          jobType.includes(spec.replace(/-/g, " "))
        )

        if (!isSpecialized && personality.riskTolerance === "low") {
          return {
            shouldBid: false,
            bidAmount: 0,
            reasoning: "Job outside specialization",
            confidence: 1.0,
            estimatedCompletionTime: 0
          }
        }

        // Use AI for detailed evaluation
        return yield* evaluateJobWithAI(job, personality, economicState, languageModel)
      })

    const deliverService = (
      job: NostrLib.Nip90Service.JobRequest,
      personality: MarketplacePersonality
    ): Effect.Effect<ServiceDelivery, MarketplaceError> =>
      Effect.gen(function*() {
        // Deliver service using AI
        return yield* deliverServiceWithAI(job, personality, languageModel)
      })

    const startMarketplaceLoop = (
      personality: MarketplacePersonality,
      agentKeys: { privateKey: string; publicKey: string }
    ): Effect.Effect<void, MarketplaceError, NostrLib.Nip90Service.Nip90Service> =>
      Effect.gen(function*() {
        console.log(`Starting marketplace loop for ${personality.name}`)

        // Initialize economic state
        const economicState: AgentEconomicState = {
          balance: 1000, // Starting balance
          activeJobs: new Map(),
          completedJobs: 0,
          totalEarnings: 0,
          averageRating: 4.5
        }
        agentStates.set(agentKeys.publicKey, economicState)

        // Publish service offering
        yield* publishServiceOffering(personality, agentKeys.privateKey)

        // Start REAL job discovery using WebSocket subscription
        const jobDiscoveryLoop = Effect.gen(function*() {
          console.log(`${personality.name} starting real-time job discovery...`)

          // Subscribe to actual job requests from Nostr relays
          const jobStream = nip90Service.subscribeToJobRequests(agentKeys.publicKey)

          yield* jobStream.pipe(
            Stream.tap((job) => Effect.log(`${personality.name} discovered job: ${job.jobId}`)),
            Stream.filter((job) => job.requester !== agentKeys.publicKey), // Don't bid on own jobs
            Stream.mapEffect((job) =>
              Effect.gen(function*() {
                // Evaluate job
                const evaluation = yield* evaluateJob(job, personality, economicState)
                console.log(`${personality.name} evaluation:`, evaluation)

                if (evaluation.shouldBid) {
                  // Submit bid (job result with payment-required status)
                  yield* nip90Service.submitJobResult({
                    jobId: job.jobId,
                    requestEventId: job.jobId,
                    resultKind: job.requestKind + 1000, // Convert request kind to result kind
                    result: JSON.stringify({
                      status: "payment-required",
                      bidAmount: evaluation.bidAmount,
                      estimatedTime: evaluation.estimatedCompletionTime,
                      provider: personality.name
                    }),
                    status: "payment-required" as NostrLib.Nip90Service.JobStatus,
                    privateKey: agentKeys.privateKey as NostrLib.Schema.PrivateKey
                  })

                  console.log(`${personality.name} submitted bid for ${evaluation.bidAmount} sats`)

                  // Add to active jobs
                  economicState.activeJobs.set(job.jobId, job)

                  // Start job execution in background
                  yield* Effect.fork(
                    Effect.gen(function*() {
                      // Monitor for payment confirmation
                      const monitor = nip90Service.monitorJob(job.jobId)

                      const paid = yield* monitor.pipe(
                        Stream.takeUntil((m) => m.status !== "payment-required"),
                        Stream.runLast
                      )

                      if (paid && Option.isSome(paid) && paid.value.status === "processing") {
                        // Deliver service
                        const delivery = yield* deliverService(job, personality)

                        // Submit result
                        const params: NostrLib.Nip90Service.SubmitJobResultParams = {
                          jobId: job.jobId,
                          requestEventId: job.jobId,
                          resultKind: job.requestKind + 1000,
                          result: delivery.result,
                          status: delivery.status,
                          computeTime: delivery.computeTime,
                          privateKey: agentKeys.privateKey as NostrLib.Schema.PrivateKey
                        }

                        if (delivery.tokensUsed !== undefined) {
                          params.tokensUsed = delivery.tokensUsed
                        }
                        if (delivery.confidence !== undefined) {
                          params.confidence = delivery.confidence
                        }

                        yield* nip90Service.submitJobResult(params)

                        // Update economic state
                        economicState.activeJobs.delete(job.jobId)
                        economicState.completedJobs++
                        economicState.totalEarnings += evaluation.bidAmount
                        economicState.balance += evaluation.bidAmount

                        console.log(`${personality.name} completed job ${job.jobId} for ${evaluation.bidAmount} sats`)
                      }
                    }).pipe(
                      Effect.catchAll((error) => Effect.log(`Job execution error: ${error}`))
                    )
                  )
                }
              })
            ),
            Stream.catchAll((error) => Stream.fromEffect(Effect.log(`Job discovery stream error: ${error}`))),
            Stream.runDrain
          )
        })

        // Start the marketplace loop in background
        const fiber = yield* Effect.fork(jobDiscoveryLoop)

        // Store the loop
        activeMarketplaceLoops.set(agentKeys.publicKey, {
          stop: () => Effect.runSync(Fiber.interrupt(fiber))
        })
      })

    const stopMarketplaceLoop = (agentId: string): Effect.Effect<void, never> =>
      Effect.sync(() => {
        const loop = activeMarketplaceLoops.get(agentId)
        if (loop) {
          loop.stop()
          activeMarketplaceLoops.delete(agentId)
          agentStates.delete(agentId)
          console.log(`Stopped marketplace loop for agent ${agentId}`)
        }
      })

    return {
      startMarketplaceLoop,
      stopMarketplaceLoop,
      evaluateJob,
      deliverService,
      publishServiceOffering
    }
  })
)

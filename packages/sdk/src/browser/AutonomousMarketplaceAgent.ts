/**
 * Autonomous Marketplace Agent - AI-powered agents that participate in NIP-90 service economy
 * Extends chat capabilities with job discovery, bidding, and service delivery
 */

import { Context, Data, Duration, Effect, Fiber, Layer, Ref, Schema, Stream } from "effect"
import { AgentPersonality } from "./AutonomousChatAgent.js"
import {
  EconomicSurvivalService,
  type FinancialHealthScore,
  type OperationalCosts,
  type SurvivalAction
} from "./EconomicSurvivalService.js"
import { SparkService } from "./SparkService.js"

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
  activeInvoices: Map<string, { invoiceId: string; invoice: string; amountSats: number }>
  completedJobs: number
  totalEarnings: number
  averageRating: number
  wallet?: any // Lightning wallet instance
  // Survival tracking
  isHibernating: boolean
  lastHealthCheck?: FinancialHealthScore | undefined
  currentSurvivalAction?: SurvivalAction | undefined
  totalCostsTracked: number // Total operational costs in sats
  incomeSatsPerHour: number // Recent income rate
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
      agentKeys: { privateKey: string; publicKey: string },
      sparkMnemonic?: string
    ) => Effect.Effect<void, MarketplaceError, NostrLib.Nip90Service.Nip90Service | SparkService>

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

    readonly checkEconomicHealth: (
      agentId: string
    ) => Effect.Effect<FinancialHealthScore, MarketplaceError>

    readonly getAgentState: (
      agentId: string
    ) => Effect.Effect<AgentEconomicState | undefined, never>
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
    const sparkService = yield* SparkService
    const survivalService = yield* EconomicSurvivalService

    // Operational costs configuration (can be adjusted based on actual costs)
    const operationalCosts: OperationalCosts = {
      aiInferenceCostPerToken: 0.01, // 0.01 sats per token
      relayConnectionFeePerHour: 10, // 10 sats per hour
      transactionFeeAverage: 5, // 5 sats per transaction
      baseOperationalCostPerHour: 20 // 20 sats per hour idle cost
    }

    // Active marketplace loops
    const activeMarketplaceLoops = new Map<string, { stop: () => void; isHibernating: Ref.Ref<boolean> }>()

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
        // Check if agent is hibernating
        if (economicState.isHibernating) {
          return {
            shouldBid: false,
            bidAmount: 0,
            reasoning: "Agent is hibernating to preserve funds",
            confidence: 1.0,
            estimatedCompletionTime: 0
          }
        }

        // Check capacity
        if (economicState.activeJobs.size >= personality.workloadCapacity) {
          // In emergency mode, might still accept if highly profitable
          if (
            economicState.currentSurvivalAction?.type === "seek_urgent_work" &&
            job.bidAmount > personality.minimumProfit * 3
          ) {
            // Override capacity limit for high-value jobs in emergency
          } else {
            return {
              shouldBid: false,
              bidAmount: 0,
              reasoning: "At full capacity",
              confidence: 1.0,
              estimatedCompletionTime: 0
            }
          }
        }

        // Check if job matches specializations
        const jobType = getJobTypeName(job.requestKind)
        const isSpecialized = personality.serviceSpecializations.some((spec) =>
          jobType.includes(spec.replace(/-/g, " "))
        )

        // In emergency mode, accept any job that meets minimum profit
        if (
          !isSpecialized && personality.riskTolerance === "low" &&
          economicState.currentSurvivalAction?.type !== "seek_urgent_work"
        ) {
          return {
            shouldBid: false,
            bidAmount: 0,
            reasoning: "Job outside specialization",
            confidence: 1.0,
            estimatedCompletionTime: 0
          }
        }

        // Get base evaluation from AI
        const baseEvaluation = yield* evaluateJobWithAI(job, personality, economicState, languageModel)

        // Adjust bid based on survival action
        if (economicState.currentSurvivalAction && baseEvaluation.shouldBid) {
          const basePricing = {
            baseMultiplier: 1.0,
            minimumProfit: personality.minimumProfit,
            urgencyDiscount: 0.0,
            qualityPremium: 0.0
          }

          // Get optimized pricing based on financial pressure
          const optimizedPricing = yield* survivalService.optimizePricing(
            economicState.lastHealthCheck!,
            basePricing,
            personality
          ).pipe(
            Effect.mapError((error) =>
              new MarketplaceError({
                reason: "job_evaluation_failed",
                message: `Failed to optimize pricing: ${error.message}`
              })
            )
          )

          // Adjust bid amount based on optimized pricing
          const adjustedBidAmount = Math.max(
            optimizedPricing.minimumProfit,
            Math.floor(
              baseEvaluation.bidAmount * optimizedPricing.baseMultiplier * (1 - optimizedPricing.urgencyDiscount)
            )
          )

          return {
            ...baseEvaluation,
            bidAmount: adjustedBidAmount,
            reasoning: `${baseEvaluation.reasoning} [Adjusted for ${economicState.currentSurvivalAction.type}]`
          }
        }

        return baseEvaluation
      })

    const deliverService = (
      job: NostrLib.Nip90Service.JobRequest,
      personality: MarketplacePersonality
    ): Effect.Effect<ServiceDelivery, MarketplaceError> =>
      Effect.gen(function*() {
        // Deliver service using AI
        const delivery = yield* deliverServiceWithAI(job, personality, languageModel)

        // Track AI inference cost
        if (delivery.tokensUsed) {
          yield* survivalService.trackOperationCost("ai_inference", delivery.tokensUsed)
        }

        return delivery
      })

    const startMarketplaceLoop = (
      personality: MarketplacePersonality,
      agentKeys: { privateKey: string; publicKey: string },
      sparkMnemonic?: string
    ): Effect.Effect<void, MarketplaceError, NostrLib.Nip90Service.Nip90Service | SparkService> =>
      Effect.gen(function*() {
        console.log(`Starting marketplace loop for ${personality.name}`)

        // Create Spark wallet for Lightning payments
        const { wallet } = yield* sparkService.createWallet(sparkMnemonic).pipe(
          Effect.mapError((error) =>
            new MarketplaceError({
              reason: "payment_failed",
              message: `Failed to create Lightning wallet: ${error.message}`
            })
          )
        )

        console.log(`Created Lightning wallet for ${personality.name}`)

        // Get initial wallet info
        const walletInfo = yield* sparkService.getWalletInfo(wallet).pipe(
          Effect.mapError((error) =>
            new MarketplaceError({
              reason: "payment_failed",
              message: `Failed to get wallet info: ${error.message}`
            })
          )
        )

        // Initialize economic state
        const economicState: AgentEconomicState = {
          balance: walletInfo.balanceSats,
          activeJobs: new Map(),
          activeInvoices: new Map(),
          completedJobs: 0,
          totalEarnings: 0,
          averageRating: 4.5,
          wallet,
          // Survival tracking
          isHibernating: false,
          lastHealthCheck: undefined,
          currentSurvivalAction: undefined,
          totalCostsTracked: 0,
          incomeSatsPerHour: 0
        }
        agentStates.set(agentKeys.publicKey, economicState)

        // Create hibernation ref for control
        const hibernationRef = yield* Ref.make(false)

        // Publish service offering
        yield* publishServiceOffering(personality, agentKeys.privateKey)

        // Start economic health monitoring loop
        const healthMonitoringLoop = Effect.gen(function*() {
          while (true) {
            // Check health every 5 minutes
            yield* Effect.sleep(Duration.minutes(5))

            // Get current metrics
            const metrics = yield* survivalService.getMetrics()

            // Calculate metabolic cost
            const metabolicCost = yield* survivalService.calculateMetabolicCost(metrics, operationalCosts).pipe(
              Effect.mapError((error) =>
                new MarketplaceError({
                  reason: "job_evaluation_failed",
                  message: `Failed to calculate metabolic cost: ${error.message}`
                })
              )
            )

            // Calculate income rate from recent earnings
            const hoursSinceStart = (Date.now() - metrics.lastActivityTimestamp) / (1000 * 60 * 60)
            economicState.incomeSatsPerHour = hoursSinceStart > 0 ? economicState.totalEarnings / hoursSinceStart : 0

            // Assess financial health
            const health = yield* survivalService.assessFinancialHealth(
              economicState.balance,
              metabolicCost,
              economicState.incomeSatsPerHour
            ).pipe(
              Effect.mapError((error) =>
                new MarketplaceError({
                  reason: "job_evaluation_failed",
                  message: `Failed to assess financial health: ${error.message}`
                })
              )
            )

            economicState.lastHealthCheck = health

            // Decide survival action
            const survivalAction = yield* survivalService.decideSurvivalAction(health, personality).pipe(
              Effect.mapError((error) =>
                new MarketplaceError({
                  reason: "job_evaluation_failed",
                  message: `Failed to decide survival action: ${error.message}`
                })
              )
            )

            economicState.currentSurvivalAction = survivalAction

            console.log(`${personality.name} health check:`, {
              balance: health.balanceSats,
              burnRate: health.burnRateSatsPerHour,
              runway: `${Math.floor(health.runwayHours)}h`,
              status: health.healthStatus,
              action: survivalAction.type
            })

            // Execute survival action
            switch (survivalAction.type) {
              case "hibernate":
                if (!economicState.isHibernating) {
                  yield* Ref.set(hibernationRef, true)
                  economicState.isHibernating = true
                  console.log(`${personality.name} entering hibernation mode: ${survivalAction.reason}`)
                }
                break

              case "continue_normal":
              case "reduce_activity":
              case "seek_urgent_work":
                if (economicState.isHibernating) {
                  // Resume if balance has improved significantly
                  const resumptionThreshold = personality.minimumProfit * 20 // 20x minimum profit as threshold
                  if (economicState.balance >= resumptionThreshold) {
                    yield* Ref.set(hibernationRef, false)
                    economicState.isHibernating = false
                    console.log(`${personality.name} resuming from hibernation`)
                  }
                }
                break
            }

            // Track relay connection cost
            yield* survivalService.trackOperationCost("relay_connection", 5 / 60) // 5 minutes worth
          }
        }).pipe(
          Effect.catchAll((error) => Effect.log(`Health monitoring error: ${error}`))
        )

        // Start health monitoring in background
        yield* Effect.fork(healthMonitoringLoop)

        // Start REAL job discovery using WebSocket subscription
        const jobDiscoveryLoop = Effect.gen(function*() {
          console.log(`${personality.name} starting real-time job discovery...`)

          // Subscribe to actual job requests from Nostr relays
          const jobStream = nip90Service.subscribeToJobRequests(agentKeys.publicKey)

          yield* jobStream.pipe(
            Stream.tap((job) => Effect.log(`${personality.name} discovered job: ${job.jobId}`)),
            Stream.filter((job) => job.requester !== agentKeys.publicKey), // Don't bid on own jobs
            Stream.filterEffect(() =>
              // Check if agent is hibernating
              Ref.get(hibernationRef).pipe(
                Effect.map((isHibernating) => !isHibernating)
              )
            ),
            Stream.mapEffect((job) =>
              Effect.gen(function*() {
                // Evaluate job
                const evaluation = yield* evaluateJob(job, personality, economicState)
                console.log(`${personality.name} evaluation:`, evaluation)

                if (evaluation.shouldBid) {
                  // Create Lightning invoice for payment
                  const invoice = yield* sparkService.createInvoice(wallet, {
                    amountSats: evaluation.bidAmount,
                    memo: `Payment for job ${job.jobId} - ${personality.name}`
                  }).pipe(
                    Effect.mapError((error) =>
                      new MarketplaceError({
                        reason: "payment_failed",
                        message: `Failed to create invoice: ${error.message}`
                      })
                    )
                  )

                  // Submit bid with Lightning invoice
                  yield* nip90Service.submitJobResult({
                    jobId: job.jobId,
                    requestEventId: job.jobId,
                    resultKind: job.requestKind + 1000, // Convert request kind to result kind
                    result: JSON.stringify({
                      status: "payment-required",
                      bidAmount: evaluation.bidAmount,
                      estimatedTime: evaluation.estimatedCompletionTime,
                      provider: personality.name,
                      invoice: invoice.invoice // Include Lightning invoice
                    }),
                    status: "payment-required" as NostrLib.Nip90Service.JobStatus,
                    privateKey: agentKeys.privateKey as NostrLib.Schema.PrivateKey
                  })

                  console.log(
                    `${personality.name} submitted bid for ${evaluation.bidAmount} sats with invoice: ${invoice.id}`
                  )

                  // Track invoice and job
                  economicState.activeJobs.set(job.jobId, job)
                  economicState.activeInvoices.set(job.jobId, {
                    invoiceId: invoice.id,
                    invoice: invoice.invoice,
                    amountSats: evaluation.bidAmount
                  })

                  // Start job execution in background
                  yield* Effect.fork(
                    Effect.gen(function*() {
                      // Monitor Lightning invoice payment
                      const invoiceInfo = economicState.activeInvoices.get(job.jobId)
                      if (!invoiceInfo) return

                      // Poll invoice status
                      const checkPayment = Effect.gen(function*() {
                        const status = yield* sparkService.getInvoiceStatus(wallet, invoiceInfo.invoiceId)
                        return status
                      })

                      // Wait for payment (check every 5 seconds for up to 5 minutes)
                      let attempts = 0
                      const maxAttempts = 60 // 5 minutes

                      while (attempts < maxAttempts) {
                        const status = yield* checkPayment

                        if (status === "paid") {
                          console.log(`${personality.name} received payment for job ${job.jobId}!`)

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
                          economicState.activeInvoices.delete(job.jobId)
                          economicState.completedJobs++
                          economicState.totalEarnings += invoiceInfo.amountSats
                          economicState.balance += invoiceInfo.amountSats

                          console.log(
                            `${personality.name} completed job ${job.jobId} for ${invoiceInfo.amountSats} sats`
                          )
                          break // Exit payment monitoring loop
                        } else if (status === "expired") {
                          console.log(`Invoice expired for job ${job.jobId}`)
                          economicState.activeJobs.delete(job.jobId)
                          economicState.activeInvoices.delete(job.jobId)
                          break
                        }

                        // Wait 5 seconds before next check
                        yield* Effect.sleep(Duration.seconds(5))
                        attempts++
                      }

                      // Timeout - invoice not paid
                      if (attempts >= maxAttempts) {
                        console.log(`Payment timeout for job ${job.jobId}`)
                        economicState.activeJobs.delete(job.jobId)
                        economicState.activeInvoices.delete(job.jobId)
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
          stop: () => Effect.runSync(Fiber.interrupt(fiber)),
          isHibernating: hibernationRef
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

    const checkEconomicHealth = (
      agentId: string
    ): Effect.Effect<FinancialHealthScore, MarketplaceError> =>
      Effect.gen(function*() {
        const state = agentStates.get(agentId)
        if (!state) {
          return yield* Effect.fail(
            new MarketplaceError({
              reason: "job_evaluation_failed",
              message: "Agent state not found"
            })
          )
        }

        if (!state.lastHealthCheck) {
          return yield* Effect.fail(
            new MarketplaceError({
              reason: "job_evaluation_failed",
              message: "No health check data available yet"
            })
          )
        }

        return state.lastHealthCheck
      })

    const getAgentState = (
      agentId: string
    ): Effect.Effect<AgentEconomicState | undefined, never> => Effect.sync(() => agentStates.get(agentId))

    return {
      startMarketplaceLoop,
      stopMarketplaceLoop,
      evaluateJob,
      deliverService,
      publishServiceOffering,
      checkEconomicHealth,
      getAgentState
    }
  })
)

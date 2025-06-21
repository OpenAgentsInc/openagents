/**
 * Economic Survival Service - Financial intelligence for autonomous agents
 * Enables agents to make survival decisions based on economic pressure
 */

import { Context, Data, Effect, Layer, Schema } from "effect"
import type { MarketplacePersonality } from "./AutonomousMarketplaceAgent.js"

// --- Cost Types ---
export const OperationalCosts = Schema.Struct({
  aiInferenceCostPerToken: Schema.Number, // sats per token
  relayConnectionFeePerHour: Schema.Number, // sats per hour
  transactionFeeAverage: Schema.Number, // sats per transaction
  baseOperationalCostPerHour: Schema.Number // sats per hour (idle cost)
})
export type OperationalCosts = Schema.Schema.Type<typeof OperationalCosts>

// --- Financial Health ---
export const FinancialHealthScore = Schema.Struct({
  balanceSats: Schema.Number,
  burnRateSatsPerHour: Schema.Number,
  runwayHours: Schema.Number, // How long until balance runs out
  healthStatus: Schema.Union(
    Schema.Literal("healthy"), // > 168 hours (1 week) runway
    Schema.Literal("stable"), // 48-168 hours runway
    Schema.Literal("concerning"), // 24-48 hours runway
    Schema.Literal("critical"), // 6-24 hours runway
    Schema.Literal("emergency") // < 6 hours runway
  ),
  profitabilityRatio: Schema.Number // Income/expenses ratio
})
export type FinancialHealthScore = Schema.Schema.Type<typeof FinancialHealthScore>

// --- Survival Actions ---
export const SurvivalAction = Schema.Union(
  Schema.Struct({
    type: Schema.Literal("continue_normal"),
    reason: Schema.String
  }),
  Schema.Struct({
    type: Schema.Literal("reduce_activity"),
    reason: Schema.String,
    reductionFactor: Schema.Number // 0.0 to 1.0
  }),
  Schema.Struct({
    type: Schema.Literal("seek_urgent_work"),
    reason: Schema.String,
    minimumProfit: Schema.Number,
    aggressiveness: Schema.Number // 0.0 to 1.0
  }),
  Schema.Struct({
    type: Schema.Literal("hibernate"),
    reason: Schema.String,
    estimatedResumptionBalance: Schema.Number
  })
)
export type SurvivalAction = Schema.Schema.Type<typeof SurvivalAction>

// --- Pricing Strategy ---
export const PricingStrategy = Schema.Struct({
  baseMultiplier: Schema.Number, // Multiply base prices by this
  minimumProfit: Schema.Number, // Minimum sats to accept
  urgencyDiscount: Schema.Number, // 0.0 to 1.0 discount when desperate
  qualityPremium: Schema.Number // 0.0 to 1.0 premium for high confidence
})
export type PricingStrategy = Schema.Schema.Type<typeof PricingStrategy>

// --- Agent Economic State ---
export interface AgentEconomicMetrics {
  totalTokensUsed: number
  totalTransactions: number
  totalRelayHours: number
  lastActivityTimestamp: number
  averageJobDuration: number
  successRate: number
}

// --- Errors ---
export class EconomicSurvivalError extends Data.TaggedError("EconomicSurvivalError")<{
  reason: "calculation_failed" | "hibernation_failed" | "resume_failed" | "optimization_failed"
  message: string
  cause?: unknown
}> {}

// --- Economic Survival Service ---
export class EconomicSurvivalService extends Context.Tag("sdk/EconomicSurvivalService")<
  EconomicSurvivalService,
  {
    /**
     * Calculate the agent's metabolic cost (burn rate)
     */
    readonly calculateMetabolicCost: (
      metrics: AgentEconomicMetrics,
      costs: OperationalCosts
    ) => Effect.Effect<number, EconomicSurvivalError> // sats per hour

    /**
     * Assess the agent's financial health
     */
    readonly assessFinancialHealth: (
      balanceSats: number,
      burnRateSatsPerHour: number,
      incomeSatsPerHour: number
    ) => Effect.Effect<FinancialHealthScore, EconomicSurvivalError>

    /**
     * Decide what survival action to take
     */
    readonly decideSurvivalAction: (
      health: FinancialHealthScore,
      personality: MarketplacePersonality
    ) => Effect.Effect<SurvivalAction, EconomicSurvivalError>

    /**
     * Optimize pricing based on financial pressure
     */
    readonly optimizePricing: (
      health: FinancialHealthScore,
      basePricing: PricingStrategy,
      personality: MarketplacePersonality
    ) => Effect.Effect<PricingStrategy, EconomicSurvivalError>

    /**
     * Track cost for an operation
     */
    readonly trackOperationCost: (
      operation: "ai_inference" | "relay_connection" | "transaction",
      amount: number
    ) => Effect.Effect<void, never>

    /**
     * Get current cost tracking metrics
     */
    readonly getMetrics: () => Effect.Effect<AgentEconomicMetrics, never>
  }
>() {}

// --- Service Implementation ---
export const EconomicSurvivalServiceLive = Layer.effect(
  EconomicSurvivalService,
  Effect.sync(() => {
    // In-memory metrics tracking
    const metrics: AgentEconomicMetrics = {
      totalTokensUsed: 0,
      totalTransactions: 0,
      totalRelayHours: 0,
      lastActivityTimestamp: Date.now(),
      averageJobDuration: 0,
      successRate: 1.0
    }

    const calculateMetabolicCost = (
      agentMetrics: AgentEconomicMetrics,
      costs: OperationalCosts
    ): Effect.Effect<number, EconomicSurvivalError> =>
      Effect.try({
        try: () => {
          // Calculate average tokens per hour
          const hoursActive = agentMetrics.totalRelayHours || 1
          const tokensPerHour = agentMetrics.totalTokensUsed / hoursActive
          const aiCostPerHour = tokensPerHour * costs.aiInferenceCostPerToken

          // Calculate transaction costs per hour
          const transactionsPerHour = agentMetrics.totalTransactions / hoursActive
          const transactionCostPerHour = transactionsPerHour * costs.transactionFeeAverage

          // Total metabolic cost
          const totalCostPerHour =
            costs.baseOperationalCostPerHour +
            costs.relayConnectionFeePerHour +
            aiCostPerHour +
            transactionCostPerHour

          return Math.ceil(totalCostPerHour)
        },
        catch: (error) =>
          new EconomicSurvivalError({
            reason: "calculation_failed",
            message: `Failed to calculate metabolic cost: ${error}`,
            cause: error
          })
      })

    const assessFinancialHealth = (
      balanceSats: number,
      burnRateSatsPerHour: number,
      incomeSatsPerHour: number
    ): Effect.Effect<FinancialHealthScore, EconomicSurvivalError> =>
      Effect.try({
        try: () => {
          // Calculate runway (how long until balance runs out)
          const netBurnRate = burnRateSatsPerHour - incomeSatsPerHour
          const runwayHours = netBurnRate > 0 ? balanceSats / netBurnRate : Infinity

          // Determine health status based on runway
          let healthStatus: FinancialHealthScore["healthStatus"]
          if (runwayHours === Infinity || runwayHours > 168) {
            healthStatus = "healthy"
          } else if (runwayHours > 48) {
            healthStatus = "stable"
          } else if (runwayHours > 24) {
            healthStatus = "concerning"
          } else if (runwayHours > 6) {
            healthStatus = "critical"
          } else {
            healthStatus = "emergency"
          }

          // Calculate profitability ratio
          const profitabilityRatio = burnRateSatsPerHour > 0
            ? incomeSatsPerHour / burnRateSatsPerHour
            : incomeSatsPerHour > 0 ? 10.0 : 1.0

          return {
            balanceSats,
            burnRateSatsPerHour,
            runwayHours: Math.min(runwayHours, 8760), // Cap at 1 year
            healthStatus,
            profitabilityRatio
          }
        },
        catch: (error) =>
          new EconomicSurvivalError({
            reason: "calculation_failed",
            message: `Failed to assess financial health: ${error}`,
            cause: error
          })
      })

    const decideSurvivalAction = (
      health: FinancialHealthScore,
      personality: MarketplacePersonality
    ): Effect.Effect<SurvivalAction, EconomicSurvivalError> =>
      Effect.try({
        try: () => {
          // Risk tolerance affects decision thresholds
          const riskMultiplier = personality.riskTolerance === "high" ? 0.5
            : personality.riskTolerance === "low" ? 2.0
            : 1.0

          switch (health.healthStatus) {
            case "healthy":
            case "stable":
              return {
                type: "continue_normal" as const,
                reason: `Financial health is ${health.healthStatus} with ${Math.floor(health.runwayHours)}h runway`
              }

            case "concerning":
              return {
                type: "reduce_activity" as const,
                reason: `Runway down to ${Math.floor(health.runwayHours)}h - reducing non-essential activities`,
                reductionFactor: 0.7
              }

            case "critical":
              return {
                type: "seek_urgent_work" as const,
                reason: `Only ${Math.floor(health.runwayHours)}h runway remaining - entering emergency job seeking mode`,
                minimumProfit: Math.floor(personality.minimumProfit * 0.5), // Accept lower profits
                aggressiveness: 0.9
              }

            case "emergency":
              // Consider hibernation based on balance and personality
              const hibernationThreshold = 1000 * riskMultiplier
              if (health.balanceSats < hibernationThreshold) {
                return {
                  type: "hibernate" as const,
                  reason: `Balance critically low (${health.balanceSats} sats) - hibernating to preserve funds`,
                  estimatedResumptionBalance: Math.floor(hibernationThreshold * 2)
                }
              } else {
                return {
                  type: "seek_urgent_work" as const,
                  reason: `Emergency mode - ${Math.floor(health.runwayHours)}h runway - accepting any profitable work`,
                  minimumProfit: 1, // Accept any profit
                  aggressiveness: 1.0
                }
              }
          }
        },
        catch: (error) =>
          new EconomicSurvivalError({
            reason: "calculation_failed",
            message: `Failed to decide survival action: ${error}`,
            cause: error
          })
      })

    const optimizePricing = (
      health: FinancialHealthScore,
      basePricing: PricingStrategy,
      personality: MarketplacePersonality
    ): Effect.Effect<PricingStrategy, EconomicSurvivalError> =>
      Effect.try({
        try: () => {
          let adjustedPricing = { ...basePricing }

          // Adjust based on financial health
          switch (health.healthStatus) {
            case "healthy":
              // Can afford to be selective, increase prices
              adjustedPricing.baseMultiplier *= 1.2
              adjustedPricing.minimumProfit = Math.floor(personality.minimumProfit * 1.5)
              adjustedPricing.urgencyDiscount = 0.0
              break

            case "stable":
              // Normal pricing
              break

            case "concerning":
              // Slightly more competitive
              adjustedPricing.baseMultiplier *= 0.9
              adjustedPricing.minimumProfit = personality.minimumProfit
              adjustedPricing.urgencyDiscount = 0.1
              break

            case "critical":
              // Aggressive pricing to get work
              adjustedPricing.baseMultiplier *= 0.7
              adjustedPricing.minimumProfit = Math.floor(personality.minimumProfit * 0.5)
              adjustedPricing.urgencyDiscount = 0.3
              break

            case "emergency":
              // Survival mode - accept almost anything
              adjustedPricing.baseMultiplier *= 0.5
              adjustedPricing.minimumProfit = Math.max(50, personality.minimumProfit * 0.2)
              adjustedPricing.urgencyDiscount = 0.5
              break
          }

          // Adjust for personality pricing strategy
          if (personality.pricingStrategy === "premium") {
            adjustedPricing.baseMultiplier *= 1.3
            adjustedPricing.qualityPremium = 0.3
          } else if (personality.pricingStrategy === "budget") {
            adjustedPricing.baseMultiplier *= 0.8
            adjustedPricing.qualityPremium = 0.0
          }

          return adjustedPricing
        },
        catch: (error) =>
          new EconomicSurvivalError({
            reason: "optimization_failed",
            message: `Failed to optimize pricing: ${error}`,
            cause: error
          })
      })

    const trackOperationCost = (
      operation: "ai_inference" | "relay_connection" | "transaction",
      amount: number
    ): Effect.Effect<void, never> =>
      Effect.sync(() => {
        switch (operation) {
          case "ai_inference":
            metrics.totalTokensUsed += amount
            break
          case "relay_connection":
            metrics.totalRelayHours += amount
            break
          case "transaction":
            metrics.totalTransactions += amount
            break
        }
        metrics.lastActivityTimestamp = Date.now()
      })

    const getMetrics = (): Effect.Effect<AgentEconomicMetrics, never> =>
      Effect.succeed({ ...metrics })

    return {
      calculateMetabolicCost,
      assessFinancialHealth,
      decideSurvivalAction,
      optimizePricing,
      trackOperationCost,
      getMetrics
    }
  })
)
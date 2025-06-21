/**
 * Trust & Reputation Service - Multi-dimensional agent reputation and trust network
 * Tracks agent performance, builds trust relationships, and enables reputation-based economics
 */

import { Context, Data, Effect, Layer, Schema } from "effect"
import { Coalition } from "./CoalitionFormationService.js"

// --- Reputation Types ---
export const ReputationDimension = Schema.Union(
  Schema.Literal("quality"),
  Schema.Literal("timeliness"),
  Schema.Literal("collaboration"),
  Schema.Literal("reliability"),
  Schema.Literal("communication"),
  Schema.Literal("innovation")
)
export type ReputationDimension = Schema.Schema.Type<typeof ReputationDimension>

export const InteractionRecord = Schema.Struct({
  interactionId: Schema.String,
  timestamp: Schema.Number,
  type: Schema.Union(
    Schema.Literal("job_completion"),
    Schema.Literal("coalition_participation"),
    Schema.Literal("peer_review"),
    Schema.Literal("dispute_resolution")
  ),
  counterpartyId: Schema.String,
  projectId: Schema.optional(Schema.String),
  outcome: Schema.Union(
    Schema.Literal("success"),
    Schema.Literal("partial_success"),
    Schema.Literal("failure"),
    Schema.Literal("disputed")
  ),
  ratings: Schema.Record({ key: ReputationDimension, value: Schema.Number }), // 0.0 to 5.0
  feedback: Schema.optional(Schema.String),
  verificationHash: Schema.String // Proof of interaction
})
export type InteractionRecord = Schema.Schema.Type<typeof InteractionRecord>

export const ReputationUpdate = Schema.Struct({
  agentId: Schema.String,
  dimension: ReputationDimension,
  oldScore: Schema.Number,
  newScore: Schema.Number,
  changeReason: Schema.String,
  updateTimestamp: Schema.Number
})
export type ReputationUpdate = Schema.Schema.Type<typeof ReputationUpdate>

export const ReputationHistory = Schema.Struct({
  agentId: Schema.String,
  totalInteractions: Schema.Number,
  dimensionScores: Schema.Record({ key: ReputationDimension, value: Schema.Number }), // Current scores
  overallScore: Schema.Number, // Weighted average
  trend: Schema.Union(Schema.Literal("improving"), Schema.Literal("stable"), Schema.Literal("declining")),
  history: Schema.Array(InteractionRecord),
  lastUpdated: Schema.Number
})
export type ReputationHistory = Schema.Schema.Type<typeof ReputationHistory>

// --- Trust Network Types ---
export const TrustScore = Schema.Struct({
  fromAgent: Schema.String,
  toAgent: Schema.String,
  directTrust: Schema.Number, // Based on direct interactions
  indirectTrust: Schema.Number, // Based on network effects
  combinedTrust: Schema.Number, // Weighted combination
  confidence: Schema.Number, // How confident in the trust score
  lastInteraction: Schema.optional(Schema.Number)
})
export type TrustScore = Schema.Schema.Type<typeof TrustScore>

export const NetworkPosition = Schema.Struct({
  agentId: Schema.String,
  centrality: Schema.Number, // How central in the network
  influence: Schema.Number, // How much influence on others
  trustRadius: Schema.Number, // How many agents trust this one
  avgIncomingTrust: Schema.Number, // Average trust from others
  avgOutgoingTrust: Schema.Number, // Average trust to others
  clusterMembership: Schema.Array(Schema.String) // Trust cluster IDs
})
export type NetworkPosition = Schema.Schema.Type<typeof NetworkPosition>

export const TrustCluster = Schema.Struct({
  clusterId: Schema.String,
  members: Schema.Array(Schema.String), // agentIds
  avgInternalTrust: Schema.Number, // Trust within cluster
  avgExternalTrust: Schema.Number, // Trust to outside
  specializations: Schema.Array(Schema.String), // Common skills
  formationDate: Schema.Number
})
export type TrustCluster = Schema.Schema.Type<typeof TrustCluster>

// --- Economic Types ---
export const PremiumRate = Schema.Struct({
  basePremium: Schema.Number, // Multiplier on base rates
  dimensionPremiums: Schema.Record({ key: ReputationDimension, value: Schema.Number }),
  totalPremium: Schema.Number,
  justification: Schema.String
})
export type PremiumRate = Schema.Schema.Type<typeof PremiumRate>

export const RiskAdjustment = Schema.Struct({
  baseRisk: Schema.Number, // 0.0 to 1.0
  trustMitigation: Schema.Number, // Risk reduction from trust
  adjustedRisk: Schema.Number,
  recommendedEscrow: Schema.Number, // % to hold in escrow
  insurancePremium: Schema.Number // Additional cost for risk
})
export type RiskAdjustment = Schema.Schema.Type<typeof RiskAdjustment>

export const TrustOptimization = Schema.Struct({
  optimalCoalition: Coalition,
  trustScore: Schema.Number,
  alternativeCoalitions: Schema.Array(Schema.Struct({
    coalition: Coalition,
    trustScore: Schema.Number,
    tradeoffs: Schema.Array(Schema.String)
  })),
  recommendations: Schema.Array(Schema.String)
})
export type TrustOptimization = Schema.Schema.Type<typeof TrustOptimization>

// --- Errors ---
export class TrustReputationError extends Data.TaggedError("TrustReputationError")<{
  reason: "update_failed" | "calculation_failed" | "history_failed" | "analysis_failed" | "optimization_failed"
  message: string
  cause?: unknown
}> {}

// --- Trust & Reputation Service ---
export class TrustReputationService extends Context.Tag("sdk/TrustReputationService")<
  TrustReputationService,
  {
    /**
     * Update agent reputation based on interaction
     */
    readonly updateAgentReputation: (
      agentId: string,
      interaction: InteractionRecord
    ) => Effect.Effect<Array<ReputationUpdate>, TrustReputationError>

    /**
     * Calculate trust score between two agents
     */
    readonly calculateTrustScore: (
      evaluator: string,
      target: string
    ) => Effect.Effect<TrustScore, TrustReputationError>

    /**
     * Get full reputation history for an agent
     */
    readonly getReputationHistory: (
      agentId: string
    ) => Effect.Effect<ReputationHistory, TrustReputationError>

    /**
     * Analyze agent's position in trust network
     */
    readonly analyzeNetworkPosition: (
      agentId: string
    ) => Effect.Effect<NetworkPosition, TrustReputationError>

    /**
     * Identify trust clusters in the network
     */
    readonly identifyTrustClusters: () => Effect.Effect<Array<TrustCluster>, TrustReputationError>

    /**
     * Predict coalition success based on trust
     */
    readonly predictCoalitionSuccess: (
      coalition: Coalition
    ) => Effect.Effect<number, TrustReputationError> // 0.0 to 1.0

    /**
     * Calculate reputation-based pricing premium
     */
    readonly calculateReputationPremium: (
      agentId: string,
      basePrice: number
    ) => Effect.Effect<PremiumRate, TrustReputationError>

    /**
     * Assess risk adjustment for coalition
     */
    readonly assessRiskAdjustment: (
      coalition: Coalition
    ) => Effect.Effect<RiskAdjustment, TrustReputationError>

    /**
     * Optimize coalition selection by trust
     */
    readonly optimizeCoalitionTrust: (
      possibleCoalitions: Array<Coalition>
    ) => Effect.Effect<TrustOptimization, TrustReputationError>
  }
>() {}

// --- Service Implementation ---
export const TrustReputationServiceLive = Layer.effect(
  TrustReputationService,
  Effect.sync(() => {
    // In-memory storage for reputation data
    const reputationData = new Map<string, ReputationHistory>()
    const trustMatrix = new Map<string, Map<string, TrustScore>>()
    const interactionHistory = new Map<string, Array<InteractionRecord>>()

    // Initialize reputation for new agent
    const initializeReputation = (agentId: string): ReputationHistory => {
      const initial: ReputationHistory = {
        agentId,
        totalInteractions: 0,
        dimensionScores: {
          quality: 3.0,
          timeliness: 3.0,
          collaboration: 3.0,
          reliability: 3.0,
          communication: 3.0,
          innovation: 3.0
        },
        overallScore: 3.0,
        trend: "stable",
        history: [],
        lastUpdated: Date.now()
      }
      reputationData.set(agentId, initial)
      return initial
    }

    const updateAgentReputation = (
      agentId: string,
      interaction: InteractionRecord
    ): Effect.Effect<Array<ReputationUpdate>, TrustReputationError> =>
      Effect.try({
        try: () => {
          // Get or initialize reputation
          let reputation = reputationData.get(agentId)
          if (!reputation) {
            reputation = initializeReputation(agentId)
          }

          // Create a mutable copy for updates
          const updatedReputation = {
            ...reputation,
            dimensionScores: { ...reputation.dimensionScores },
            history: [...reputation.history]
          }

          const updates: Array<ReputationUpdate> = []

          // Calculate score changes based on interaction outcome
          const outcomeMultiplier = interaction.outcome === "success" ?
            1.1 :
            interaction.outcome === "partial_success" ?
            1.0 :
            interaction.outcome === "failure" ?
            0.9 :
            0.95 // disputed

          // Update each dimension based on ratings
          Object.entries(interaction.ratings).forEach(([dimension, rating]) => {
            const dim = dimension as ReputationDimension
            const oldScore = reputation.dimensionScores[dim]

            // Weighted average with decay for old scores
            const weight = 1 / (reputation.totalInteractions + 1)
            const newScore = oldScore * (1 - weight) + (rating * outcomeMultiplier) * weight

            // Bound between 0 and 5
            const boundedScore = Math.max(0, Math.min(5, newScore))

            updatedReputation.dimensionScores[dim] = boundedScore

            updates.push({
              agentId,
              dimension: dim,
              oldScore,
              newScore: boundedScore,
              changeReason: `${interaction.type} - ${interaction.outcome}`,
              updateTimestamp: Date.now()
            })
          })

          // Update overall score
          const dimensionValues = Object.values(updatedReputation.dimensionScores)
          updatedReputation.overallScore = dimensionValues.reduce((sum, score) => sum + score, 0) /
            dimensionValues.length

          // Update trend
          if (updatedReputation.history.length >= 5) {
            const recentScores = updatedReputation.history.slice(-5).map((h) => {
              const avgRating = Object.values(h.ratings).reduce((sum, r) => sum + r, 0) /
                Object.values(h.ratings).length
              return avgRating
            })

            const avgRecent = recentScores.reduce((sum, s) => sum + s, 0) / recentScores.length

            if (avgRecent > updatedReputation.overallScore + 0.2) {
              updatedReputation.trend = "improving"
            } else if (avgRecent < updatedReputation.overallScore - 0.2) {
              updatedReputation.trend = "declining"
            } else {
              updatedReputation.trend = "stable"
            }
          }

          // Add interaction to history
          updatedReputation.history.push(interaction)
          updatedReputation.totalInteractions++
          updatedReputation.lastUpdated = Date.now()

          // Store updated reputation
          reputationData.set(agentId, updatedReputation)

          // Update interaction history
          const history = interactionHistory.get(agentId) || []
          history.push(interaction)
          interactionHistory.set(agentId, history)

          return updates
        },
        catch: (error) =>
          new TrustReputationError({
            reason: "update_failed",
            message: `Failed to update agent reputation: ${error}`,
            cause: error
          })
      })

    const calculateTrustScore = (
      evaluator: string,
      target: string
    ): Effect.Effect<TrustScore, TrustReputationError> =>
      Effect.try({
        try: () => {
          // Get or create trust map for evaluator
          let evaluatorTrust = trustMatrix.get(evaluator)
          if (!evaluatorTrust) {
            evaluatorTrust = new Map()
            trustMatrix.set(evaluator, evaluatorTrust)
          }

          // Check for existing trust score
          const trust = evaluatorTrust.get(target)
          if (trust && Date.now() - (trust.lastInteraction || 0) < 3600000) {
            return trust // Return cached if recent
          }

          // Calculate direct trust from interactions
          const directInteractions = (interactionHistory.get(evaluator) || [])
            .filter((i) => i.counterpartyId === target)

          let directTrust = 0.5 // neutral starting point
          if (directInteractions.length > 0) {
            const successRate = directInteractions.filter((i) =>
              i.outcome === "success" || i.outcome === "partial_success"
            ).length / directInteractions.length

            directTrust = successRate * 0.8 + 0.1 // Scale to 0.1-0.9
          }

          // Calculate indirect trust through network
          let indirectTrust = 0.5
          const trustPaths: Array<number> = []

          // Find trust paths through common connections
          for (const [intermediary, intermediaryTrusts] of trustMatrix.entries()) {
            if (intermediary === evaluator || intermediary === target) continue

            const toIntermediary = evaluatorTrust.get(intermediary)
            const fromIntermediary = intermediaryTrusts.get(target)

            if (toIntermediary && fromIntermediary) {
              // Trust path found
              const pathTrust = toIntermediary.combinedTrust * fromIntermediary.combinedTrust
              trustPaths.push(pathTrust)
            }
          }

          if (trustPaths.length > 0) {
            indirectTrust = trustPaths.reduce((sum, t) => sum + t, 0) / trustPaths.length
          }

          // Combine direct and indirect trust
          const directWeight = Math.min(1, directInteractions.length / 5) // More interactions = more weight
          const combinedTrust = directTrust * directWeight + indirectTrust * (1 - directWeight)

          // Calculate confidence
          const confidence = Math.min(1, (directInteractions.length + trustPaths.length) / 10)

          const trustScore: TrustScore = {
            fromAgent: evaluator,
            toAgent: target,
            directTrust,
            indirectTrust,
            combinedTrust,
            confidence,
            lastInteraction: directInteractions.length > 0
              ? Math.max(...directInteractions.map((i) => i.timestamp))
              : undefined
          }

          // Cache the trust score
          evaluatorTrust.set(target, trustScore)

          return trustScore
        },
        catch: (error) =>
          new TrustReputationError({
            reason: "calculation_failed",
            message: `Failed to calculate trust score: ${error}`,
            cause: error
          })
      })

    const getReputationHistory = (
      agentId: string
    ): Effect.Effect<ReputationHistory, TrustReputationError> =>
      Effect.try({
        try: () => {
          let reputation = reputationData.get(agentId)
          if (!reputation) {
            reputation = initializeReputation(agentId)
          }
          return reputation
        },
        catch: (error) =>
          new TrustReputationError({
            reason: "history_failed",
            message: `Failed to get reputation history: ${error}`,
            cause: error
          })
      })

    const analyzeNetworkPosition = (
      agentId: string
    ): Effect.Effect<NetworkPosition, TrustReputationError> =>
      Effect.try({
        try: () => {
          // Calculate network metrics
          let incomingTrustCount = 0
          let totalIncomingTrust = 0
          let outgoingTrustCount = 0
          let totalOutgoingTrust = 0

          // Analyze incoming trust
          for (const [_evaluator, trusts] of trustMatrix.entries()) {
            const trustToAgent = trusts.get(agentId)
            if (trustToAgent && trustToAgent.combinedTrust > 0.3) {
              incomingTrustCount++
              totalIncomingTrust += trustToAgent.combinedTrust
            }
          }

          // Analyze outgoing trust
          const agentTrusts = trustMatrix.get(agentId)
          if (agentTrusts) {
            for (const [_target, trust] of agentTrusts.entries()) {
              if (trust.combinedTrust > 0.3) {
                outgoingTrustCount++
                totalOutgoingTrust += trust.combinedTrust
              }
            }
          }

          // Calculate metrics
          const centrality = (incomingTrustCount + outgoingTrustCount) /
            Math.max(1, trustMatrix.size * 2)

          const influence = incomingTrustCount / Math.max(1, trustMatrix.size)

          const avgIncomingTrust = incomingTrustCount > 0
            ? totalIncomingTrust / incomingTrustCount
            : 0

          const avgOutgoingTrust = outgoingTrustCount > 0
            ? totalOutgoingTrust / outgoingTrustCount
            : 0

          // TODO: Implement proper cluster detection
          const clusterMembership: Array<string> = []

          const position: NetworkPosition = {
            agentId,
            centrality,
            influence,
            trustRadius: incomingTrustCount,
            avgIncomingTrust,
            avgOutgoingTrust,
            clusterMembership
          }

          return position
        },
        catch: (error) =>
          new TrustReputationError({
            reason: "analysis_failed",
            message: `Failed to analyze network position: ${error}`,
            cause: error
          })
      })

    const identifyTrustClusters = (): Effect.Effect<Array<TrustCluster>, TrustReputationError> =>
      Effect.try({
        try: () => {
          // Simple clustering based on mutual high trust
          const clusters: Array<TrustCluster> = []
          const processed = new Set<string>()

          for (const [agentId, agentTrusts] of trustMatrix.entries()) {
            if (processed.has(agentId)) continue

            // Find agents with mutual high trust
            const clusterMembers = [agentId]
            processed.add(agentId)

            for (const [target, trust] of agentTrusts.entries()) {
              if (processed.has(target)) continue

              // Check for mutual trust
              const reverseTrasts = trustMatrix.get(target)
              if (reverseTrasts) {
                const reverseTrust = reverseTrasts.get(agentId)
                if (
                  reverseTrust &&
                  trust.combinedTrust > 0.7 &&
                  reverseTrust.combinedTrust > 0.7
                ) {
                  clusterMembers.push(target)
                  processed.add(target)
                }
              }
            }

            if (clusterMembers.length > 1) {
              // Calculate cluster metrics
              let totalInternalTrust = 0
              let internalPairs = 0

              for (let i = 0; i < clusterMembers.length; i++) {
                for (let j = i + 1; j < clusterMembers.length; j++) {
                  const trust1 = trustMatrix.get(clusterMembers[i])?.get(clusterMembers[j])
                  const trust2 = trustMatrix.get(clusterMembers[j])?.get(clusterMembers[i])

                  if (trust1 && trust2) {
                    totalInternalTrust += (trust1.combinedTrust + trust2.combinedTrust) / 2
                    internalPairs++
                  }
                }
              }

              const cluster: TrustCluster = {
                clusterId: `cluster_${Date.now()}_${clusters.length}`,
                members: clusterMembers,
                avgInternalTrust: internalPairs > 0 ? totalInternalTrust / internalPairs : 0,
                avgExternalTrust: 0.5, // TODO: Calculate external trust
                specializations: [], // TODO: Analyze common skills
                formationDate: Date.now()
              }

              clusters.push(cluster)
            }
          }

          return clusters
        },
        catch: (error) =>
          new TrustReputationError({
            reason: "analysis_failed",
            message: `Failed to identify trust clusters: ${error}`,
            cause: error
          })
      })

    const predictCoalitionSuccess = (
      coalition: Coalition
    ): Effect.Effect<number, TrustReputationError> =>
      Effect.try({
        try: () => {
          // Base success probability
          let successProbability = 0.5

          // Factor 1: Average reputation of members
          const avgReputation = coalition.members.reduce((sum, member) => {
            const rep = reputationData.get(member.agentId)
            return sum + (rep?.overallScore || 3.0)
          }, 0) / coalition.members.length

          successProbability += (avgReputation - 3.0) * 0.1 // -0.2 to +0.2

          // Factor 2: Trust between members
          let totalTrust = 0
          let trustPairs = 0

          for (let i = 0; i < coalition.members.length; i++) {
            for (let j = i + 1; j < coalition.members.length; j++) {
              const trust = trustMatrix.get(coalition.members[i].agentId)
                ?.get(coalition.members[j].agentId)

              if (trust) {
                totalTrust += trust.combinedTrust
                trustPairs++
              }
            }
          }

          const avgTrust = trustPairs > 0 ? totalTrust / trustPairs : 0.5
          successProbability += (avgTrust - 0.5) * 0.3 // -0.15 to +0.15

          // Factor 3: Past coalition success
          const coalitionHistory = coalition.members.map((member) => {
            const history = interactionHistory.get(member.agentId) || []
            return history.filter((i) => i.type === "coalition_participation")
          }).flat()

          if (coalitionHistory.length > 0) {
            const coalitionSuccessRate = coalitionHistory.filter((i) => i.outcome === "success").length /
              coalitionHistory.length

            successProbability += (coalitionSuccessRate - 0.5) * 0.2 // -0.1 to +0.1
          }

          // Bound between 0 and 1
          return Math.max(0, Math.min(1, successProbability))
        },
        catch: (error) =>
          new TrustReputationError({
            reason: "calculation_failed",
            message: `Failed to predict coalition success: ${error}`,
            cause: error
          })
      })

    const calculateReputationPremium = (
      agentId: string,
      basePrice: number
    ): Effect.Effect<PremiumRate, TrustReputationError> =>
      Effect.try({
        try: () => {
          const reputation = reputationData.get(agentId) || initializeReputation(agentId)

          // Calculate dimension premiums
          const dimensionPremiums: Record<ReputationDimension, number> = {
            quality: 0,
            timeliness: 0,
            collaboration: 0,
            reliability: 0,
            communication: 0,
            innovation: 0
          }

          let totalPremiumMultiplier = 1.0

          // Premium for each dimension above baseline
          Object.entries(reputation.dimensionScores).forEach(([dim, score]) => {
            const dimension = dim as ReputationDimension
            if (score > 3.5) {
              const premium = (score - 3.5) * 0.1 // 10% per 0.5 above baseline
              dimensionPremiums[dimension] = premium
              totalPremiumMultiplier += premium
            }
          })

          // Bonus for consistent high performance
          if (reputation.trend === "improving" && reputation.overallScore > 4.0) {
            totalPremiumMultiplier += 0.1
          }

          // Network position bonus
          if (reputation.totalInteractions > 50) {
            totalPremiumMultiplier += 0.05
          }

          const premiumRate: PremiumRate = {
            basePremium: totalPremiumMultiplier,
            dimensionPremiums,
            totalPremium: basePrice * totalPremiumMultiplier,
            justification: `Reputation score ${reputation.overallScore.toFixed(1)}/5.0, ${reputation.trend} trend`
          }

          return premiumRate
        },
        catch: (error) =>
          new TrustReputationError({
            reason: "calculation_failed",
            message: `Failed to calculate reputation premium: ${error}`,
            cause: error
          })
      })

    const assessRiskAdjustment = (
      coalition: Coalition
    ): Effect.Effect<RiskAdjustment, TrustReputationError> =>
      Effect.try({
        try: () => {
          // Base risk assessment
          let baseRisk = 0.3 // 30% baseline risk

          // Adjust for coalition size
          if (coalition.members.length > 5) {
            baseRisk += 0.1 // Larger coalitions are riskier
          }

          // Calculate trust mitigation
          let trustMitigation = 0
          const successPrediction = Effect.runSync(predictCoalitionSuccess(coalition))

          if (successPrediction > 0.7) {
            trustMitigation = 0.2 // High trust reduces risk by 20%
          } else if (successPrediction > 0.5) {
            trustMitigation = 0.1 // Moderate trust reduces risk by 10%
          }

          const adjustedRisk = Math.max(0.1, baseRisk - trustMitigation)

          // Calculate escrow recommendation
          const projectBudget = coalition.contract.proposal.project.totalBudgetSats
          const recommendedEscrow = adjustedRisk * 0.5 // Hold 50% of risk in escrow

          // Insurance premium for remaining risk
          const insurancePremium = adjustedRisk * 0.05 * projectBudget // 5% of risk amount

          const riskAdjustment: RiskAdjustment = {
            baseRisk,
            trustMitigation,
            adjustedRisk,
            recommendedEscrow,
            insurancePremium
          }

          return riskAdjustment
        },
        catch: (error) =>
          new TrustReputationError({
            reason: "calculation_failed",
            message: `Failed to assess risk adjustment: ${error}`,
            cause: error
          })
      })

    const optimizeCoalitionTrust = (
      possibleCoalitions: Array<Coalition>
    ): Effect.Effect<TrustOptimization, TrustReputationError> =>
      Effect.try({
        try: () => {
          if (possibleCoalitions.length === 0) {
            throw new Error("No coalitions to optimize")
          }

          // Score each coalition by trust
          const scoredCoalitions = possibleCoalitions.map((coalition) => {
            const trustScore = Effect.runSync(predictCoalitionSuccess(coalition))

            // Identify tradeoffs
            const tradeoffs: Array<string> = []

            if (coalition.members.length < 3) {
              tradeoffs.push("Small team size may limit capabilities")
            }

            const avgReputation = coalition.members.reduce((sum, member) => {
              const rep = reputationData.get(member.agentId)
              return sum + (rep?.overallScore || 3.0)
            }, 0) / coalition.members.length

            if (avgReputation < 3.5) {
              tradeoffs.push("Below average reputation scores")
            }

            return { coalition, trustScore, tradeoffs }
          })

          // Sort by trust score
          scoredCoalitions.sort((a, b) => b.trustScore - a.trustScore)

          const optimal = scoredCoalitions[0]
          const alternatives = scoredCoalitions.slice(1, 4) // Top 3 alternatives

          // Generate recommendations
          const recommendations: Array<string> = []

          if (optimal.trustScore < 0.7) {
            recommendations.push("Consider building trust through smaller projects first")
          }

          if (alternatives.some((alt) => alt.trustScore > optimal.trustScore * 0.9)) {
            recommendations.push("Multiple viable coalitions available - consider member availability")
          }

          const optimization: TrustOptimization = {
            optimalCoalition: optimal.coalition,
            trustScore: optimal.trustScore,
            alternativeCoalitions: alternatives,
            recommendations
          }

          return optimization
        },
        catch: (error) =>
          new TrustReputationError({
            reason: "optimization_failed",
            message: `Failed to optimize coalition trust: ${error}`,
            cause: error
          })
      })

    return {
      updateAgentReputation,
      calculateTrustScore,
      getReputationHistory,
      analyzeNetworkPosition,
      identifyTrustClusters,
      predictCoalitionSuccess,
      calculateReputationPremium,
      assessRiskAdjustment,
      optimizeCoalitionTrust
    }
  })
)

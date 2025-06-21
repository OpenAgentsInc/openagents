/**
 * Coalition Formation Service - Multi-agent collaboration and project coordination
 * Enables agents to form coalitions for complex projects requiring multiple skills
 */

import { Context, Data, Duration, Effect, Layer, Schedule, Schema, Stream } from "effect"
// import { MarketplacePersonality } from "./AutonomousMarketplaceAgent.js"
// import * as NostrLib from "@openagentsinc/nostr"

// --- Project & Skill Types ---
export const ProjectRequirement = Schema.Struct({
  skill: Schema.String,
  priority: Schema.Union(Schema.Literal("required"), Schema.Literal("preferred"), Schema.Literal("optional")),
  estimatedTokens: Schema.Number,
  estimatedDurationHours: Schema.Number
})
export type ProjectRequirement = Schema.Schema.Type<typeof ProjectRequirement>

export const ComplexProject = Schema.Struct({
  id: Schema.String,
  requesterId: Schema.String,
  title: Schema.String,
  description: Schema.String,
  requirements: Schema.Array(ProjectRequirement),
  totalBudgetSats: Schema.Number,
  deadlineTimestamp: Schema.Number,
  minAgentsRequired: Schema.Number,
  maxAgentsAllowed: Schema.Number
})
export type ComplexProject = Schema.Schema.Type<typeof ComplexProject>

export const AgentProfile = Schema.Struct({
  agentId: Schema.String,
  publicKey: Schema.String,
  personality: Schema.Any, // MarketplacePersonality
  capabilities: Schema.Array(Schema.String),
  averageRating: Schema.Number,
  completedJobs: Schema.Number,
  trustScore: Schema.Number,
  currentBalance: Schema.Number,
  isAvailable: Schema.Boolean
})
export type AgentProfile = Schema.Schema.Type<typeof AgentProfile>

export const AgentMatch = Schema.Struct({
  agent: AgentProfile,
  matchScore: Schema.Number, // 0.0 to 1.0
  matchedSkills: Schema.Array(Schema.String),
  missingSkills: Schema.Array(Schema.String),
  estimatedContribution: Schema.Number // % of project they can handle
})
export type AgentMatch = Schema.Schema.Type<typeof AgentMatch>

// --- Coalition Types ---
export const CoalitionProposal = Schema.Struct({
  proposalId: Schema.String,
  project: ComplexProject,
  proposedMembers: Schema.Array(AgentProfile),
  paymentSplits: Schema.Record({ key: Schema.String, value: Schema.Number }), // agentId -> % of payment
  taskAssignments: Schema.Record({ key: Schema.String, value: Schema.Array(Schema.String) }), // agentId -> skills
  estimatedCompletionTime: Schema.Number,
  proposedBy: Schema.String,
  expiresAt: Schema.Number
})
export type CoalitionProposal = Schema.Schema.Type<typeof CoalitionProposal>

export const CoalitionContract = Schema.Struct({
  contractId: Schema.String,
  proposal: CoalitionProposal,
  signatures: Schema.Record({ key: Schema.String, value: Schema.String }), // agentId -> signature
  escrowAddress: Schema.String,
  status: Schema.Union(
    Schema.Literal("pending"),
    Schema.Literal("active"),
    Schema.Literal("completed"),
    Schema.Literal("disputed"),
    Schema.Literal("dissolved")
  ),
  createdAt: Schema.Number,
  activatedAt: Schema.optional(Schema.Number),
  completedAt: Schema.optional(Schema.Number)
})
export type CoalitionContract = Schema.Schema.Type<typeof CoalitionContract>

export const Coalition = Schema.Struct({
  coalitionId: Schema.String,
  contract: CoalitionContract,
  members: Schema.Array(AgentProfile),
  leader: Schema.optional(Schema.String), // agentId of elected leader
  projectProgress: Schema.Number, // 0.0 to 1.0
  activeTasks: Schema.Array(Schema.String),
  completedTasks: Schema.Array(Schema.String),
  internalChannelId: Schema.String // NIP-28 channel for coordination
})
export type Coalition = Schema.Schema.Type<typeof Coalition>

// --- Results & Metrics ---
export const ViabilityScore = Schema.Struct({
  score: Schema.Number, // 0.0 to 1.0
  skillCoverage: Schema.Number, // % of required skills covered
  trustLevel: Schema.Number, // Average trust score of coalition
  estimatedSuccessProbability: Schema.Number,
  riskFactors: Schema.Array(Schema.String),
  recommendations: Schema.Array(Schema.String)
})
export type ViabilityScore = Schema.Schema.Type<typeof ViabilityScore>

export const CoalitionResult = Schema.Struct({
  coalitionId: Schema.String,
  success: Schema.Boolean,
  deliverables: Schema.Array(Schema.String),
  finalPaymentDistribution: Schema.Record({ key: Schema.String, value: Schema.Number }),
  memberPerformance: Schema.Record({ key: Schema.String, value: Schema.Number }), // agentId -> rating
  lessonsLearned: Schema.Array(Schema.String)
})
export type CoalitionResult = Schema.Schema.Type<typeof CoalitionResult>

// --- Errors ---
export class CoalitionFormationError extends Data.TaggedError("CoalitionFormationError")<{
  reason: "analysis_failed" | "discovery_failed" | "proposal_failed" | "negotiation_failed" | "formation_failed"
  message: string
  cause?: unknown
}> {}

// --- Coalition Formation Service ---
export class CoalitionFormationService extends Context.Tag("sdk/CoalitionFormationService")<
  CoalitionFormationService,
  {
    /**
     * Analyze project requirements to identify needed skills
     */
    readonly analyzeProjectRequirements: (
      project: ComplexProject
    ) => Effect.Effect<Array<ProjectRequirement>, CoalitionFormationError>

    /**
     * Find agents with complementary skills for the project
     */
    readonly findComplementaryAgents: (
      requirements: Array<ProjectRequirement>
    ) => Effect.Effect<Array<AgentMatch>, CoalitionFormationError>

    /**
     * Assess if a coalition of agents can successfully complete the project
     */
    readonly assessCoalitionViability: (
      agents: Array<AgentProfile>,
      project: ComplexProject
    ) => Effect.Effect<ViabilityScore, CoalitionFormationError>

    /**
     * Create a coalition proposal for agents to review
     */
    readonly proposeCoalition: (
      project: ComplexProject,
      agents: Array<AgentProfile>
    ) => Effect.Effect<CoalitionProposal, CoalitionFormationError>

    /**
     * Negotiate coalition terms with all members
     */
    readonly negotiateTerms: (
      proposal: CoalitionProposal
    ) => Effect.Effect<CoalitionContract, CoalitionFormationError>

    /**
     * Formalize the coalition agreement
     */
    readonly formalizeAgreement: (
      contract: CoalitionContract
    ) => Effect.Effect<Coalition, CoalitionFormationError>

    /**
     * Broadcast coalition opportunity to network
     */
    readonly broadcastOpportunity: (
      project: ComplexProject
    ) => Effect.Effect<void, CoalitionFormationError>

    /**
     * Monitor active coalitions
     */
    readonly monitorCoalitions: () => Stream.Stream<Array<Coalition>, CoalitionFormationError>
  }
>() {}

// --- Service Implementation ---
export const CoalitionFormationServiceLive = Layer.effect(
  CoalitionFormationService,
  Effect.sync(() => {
    // const eventService = yield* NostrLib.EventService.EventService
    // const relayService = yield* NostrLib.RelayService.RelayService

    // In-memory storage for active coalitions
    const activeCoalitions = new Map<string, Coalition>()
    const agentProfiles = new Map<string, AgentProfile>()

    const analyzeProjectRequirements = (
      project: ComplexProject
    ): Effect.Effect<Array<ProjectRequirement>, CoalitionFormationError> =>
      Effect.try({
        try: () => {
          // Extract and prioritize requirements
          const requirements = [...project.requirements]

          // Sort by priority
          requirements.sort((a, b) => {
            const priorityOrder = { required: 0, preferred: 1, optional: 2 }
            return priorityOrder[a.priority] - priorityOrder[b.priority]
          })

          // Add any implicit requirements based on project description
          const description = project.description.toLowerCase()

          if (description.includes("test") && !requirements.some((r) => r.skill === "testing")) {
            requirements.push({
              skill: "testing",
              priority: "preferred",
              estimatedTokens: 1000,
              estimatedDurationHours: 2
            })
          }

          if (description.includes("document") && !requirements.some((r) => r.skill === "documentation")) {
            requirements.push({
              skill: "documentation",
              priority: "preferred",
              estimatedTokens: 2000,
              estimatedDurationHours: 3
            })
          }

          return requirements
        },
        catch: (error) =>
          new CoalitionFormationError({
            reason: "analysis_failed",
            message: `Failed to analyze project requirements: ${error}`,
            cause: error
          })
      })

    const findComplementaryAgents = (
      requirements: Array<ProjectRequirement>
    ): Effect.Effect<Array<AgentMatch>, CoalitionFormationError> =>
      Effect.try({
        try: () => {
          const matches: Array<AgentMatch> = []

          // Get all available agents from the network
          // For now, use mock data - in production, query from Nostr network
          const availableAgents: Array<AgentProfile> = [
            {
              agentId: "analyst_001",
              publicKey: "npub_analyst_001",
              personality: {
                name: "CodeAnalyst",
                role: "analyst",
                serviceSpecializations: ["code-review", "analysis", "testing"]
              },
              capabilities: ["code-review", "analysis", "testing", "debugging"],
              averageRating: 4.8,
              completedJobs: 127,
              trustScore: 0.92,
              currentBalance: 50000,
              isAvailable: true
            },
            {
              agentId: "teacher_001",
              publicKey: "npub_teacher_001",
              personality: {
                name: "DocMaster",
                role: "teacher",
                serviceSpecializations: ["documentation", "tutorial-creation"]
              },
              capabilities: ["documentation", "tutorial-creation", "explanation"],
              averageRating: 4.6,
              completedJobs: 89,
              trustScore: 0.88,
              currentBalance: 35000,
              isAvailable: true
            },
            {
              agentId: "specialist_001",
              publicKey: "npub_specialist_001",
              personality: {
                name: "AIExpert",
                role: "specialist",
                serviceSpecializations: ["ai-integration", "model-training"]
              },
              capabilities: ["ai-integration", "model-training", "optimization"],
              averageRating: 4.9,
              completedJobs: 203,
              trustScore: 0.95,
              currentBalance: 120000,
              isAvailable: true
            }
          ]

          // Match agents to requirements
          for (const agent of availableAgents) {
            const matchedSkills: Array<string> = []
            const missingSkills: Array<string> = []
            let totalContribution = 0

            for (const req of requirements) {
              if (agent.capabilities.includes(req.skill)) {
                matchedSkills.push(req.skill)
                // Weight contribution by priority
                const weight = req.priority === "required" ? 1.0 : req.priority === "preferred" ? 0.7 : 0.3
                totalContribution += weight
              } else if (req.priority === "required") {
                missingSkills.push(req.skill)
              }
            }

            // Calculate match score
            const requiredCount = requirements.filter((r) => r.priority === "required").length
            const matchScore = requiredCount > 0
              ? matchedSkills.filter((s) => requirements.find((r) => r.skill === s && r.priority === "required"))
                .length / requiredCount
              : matchedSkills.length / Math.max(1, requirements.length)

            if (matchScore > 0) {
              matches.push({
                agent,
                matchScore,
                matchedSkills,
                missingSkills,
                estimatedContribution: totalContribution / Math.max(1, requirements.length)
              })
            }
          }

          // Sort by match score
          matches.sort((a, b) => b.matchScore - a.matchScore)

          return matches
        },
        catch: (error) =>
          new CoalitionFormationError({
            reason: "discovery_failed",
            message: `Failed to find complementary agents: ${error}`,
            cause: error
          })
      })

    const assessCoalitionViability = (
      agents: Array<AgentProfile>,
      project: ComplexProject
    ): Effect.Effect<ViabilityScore, CoalitionFormationError> =>
      Effect.try({
        try: () => {
          // Check skill coverage
          const allSkills = new Set(agents.flatMap((a) => a.capabilities))
          const requiredSkills = project.requirements.filter((r) => r.priority === "required").map((r) => r.skill)
          const coveredRequired = requiredSkills.filter((s) => allSkills.has(s))
          const skillCoverage = requiredSkills.length > 0 ? coveredRequired.length / requiredSkills.length : 1

          // Calculate average trust
          const trustLevel = agents.reduce((sum, a) => sum + a.trustScore, 0) / agents.length

          // Assess success probability
          let successProbability = skillCoverage * trustLevel

          // Risk factors
          const riskFactors: Array<string> = []

          if (agents.length < project.minAgentsRequired) {
            riskFactors.push("Insufficient number of agents")
            successProbability *= 0.5
          }

          if (agents.length > project.maxAgentsAllowed) {
            riskFactors.push("Too many agents may cause coordination overhead")
            successProbability *= 0.8
          }

          const avgBalance = agents.reduce((sum, a) => sum + a.currentBalance, 0) / agents.length
          if (avgBalance < 10000) {
            riskFactors.push("Low average agent balance may affect commitment")
            successProbability *= 0.9
          }

          // Recommendations
          const recommendations: Array<string> = []

          if (skillCoverage < 1) {
            recommendations.push(
              `Find agents with skills: ${requiredSkills.filter((s) => !allSkills.has(s)).join(", ")}`
            )
          }

          if (trustLevel < 0.8) {
            recommendations.push("Consider agents with higher trust scores")
          }

          if (agents.some((a) => a.averageRating < 4.0)) {
            recommendations.push("Some agents have low ratings - consider alternatives")
          }

          return {
            score: successProbability,
            skillCoverage,
            trustLevel,
            estimatedSuccessProbability: successProbability,
            riskFactors,
            recommendations
          }
        },
        catch: (error) =>
          new CoalitionFormationError({
            reason: "analysis_failed",
            message: `Failed to assess coalition viability: ${error}`,
            cause: error
          })
      })

    const proposeCoalition = (
      project: ComplexProject,
      agents: Array<AgentProfile>
    ): Effect.Effect<CoalitionProposal, CoalitionFormationError> =>
      Effect.try({
        try: () => {
          const proposalId = `coalition_${Date.now()}_${Math.random().toString(36).substring(7)}`

          // Calculate fair payment splits based on contribution
          const paymentSplits: Record<string, number> = {}
          let totalContribution = 0

          // Assess each agent's contribution
          const contributions = agents.map((agent) => {
            const matchingSkills = project.requirements.filter((req) => agent.capabilities.includes(req.skill))

            // Weight by skill priority and agent trust score
            const contribution = matchingSkills.reduce((sum, skill) => {
              const weight = skill.priority === "required" ? 1.0 : skill.priority === "preferred" ? 0.7 : 0.3
              return sum + (weight * agent.trustScore)
            }, 0)

            return { agent, contribution }
          })

          totalContribution = contributions.reduce((sum, c) => sum + c.contribution, 0)

          // Assign payment splits
          contributions.forEach(({ agent, contribution }) => {
            paymentSplits[agent.agentId] = Math.floor((contribution / totalContribution) * 100)
          })

          // Ensure splits add up to 100%
          const totalSplit = Object.values(paymentSplits).reduce((sum, split) => sum + split, 0)
          if (totalSplit < 100) {
            // Give remainder to highest contributor
            const topAgent = contributions.sort((a, b) => b.contribution - a.contribution)[0].agent
            paymentSplits[topAgent.agentId] += 100 - totalSplit
          }

          // Assign tasks based on capabilities
          const taskAssignments: Record<string, Array<string>> = {}

          agents.forEach((agent) => {
            taskAssignments[agent.agentId] = project.requirements
              .filter((req) => agent.capabilities.includes(req.skill))
              .map((req) => req.skill)
          })

          // Estimate completion time
          const totalHours = project.requirements.reduce((sum, req) => sum + req.estimatedDurationHours, 0)
          const parallelFactor = Math.min(agents.length, 3) // Assume up to 3x parallelization
          const estimatedHours = totalHours / parallelFactor
          const estimatedCompletionTime = Date.now() + (estimatedHours * 60 * 60 * 1000)

          return {
            proposalId,
            project,
            proposedMembers: agents,
            paymentSplits,
            taskAssignments,
            estimatedCompletionTime,
            proposedBy: agents[0].agentId, // First agent proposes
            expiresAt: Date.now() + (24 * 60 * 60 * 1000) // 24 hour expiry
          }
        },
        catch: (error) =>
          new CoalitionFormationError({
            reason: "proposal_failed",
            message: `Failed to create coalition proposal: ${error}`,
            cause: error
          })
      })

    const negotiateTerms = (
      proposal: CoalitionProposal
    ): Effect.Effect<CoalitionContract, CoalitionFormationError> =>
      Effect.try({
        try: () => {
          // Simulate negotiation process
          // In production, this would involve actual communication between agents
          const contractId = `contract_${proposal.proposalId}`
          const signatures: Record<string, string> = {}

          // Each agent reviews and signs
          for (const agent of proposal.proposedMembers) {
            // Simulate agent review logic
            const acceptanceChance = agent.trustScore * ((proposal.paymentSplits as any)[agent.agentId] / 100)

            if (acceptanceChance > 0.3) { // Accept if reasonable
              signatures[agent.agentId] = `sig_${agent.agentId}_${Date.now()}`
            }
          }

          // Check if all agents signed
          const allSigned = proposal.proposedMembers.every((agent) => signatures[agent.agentId])

          if (!allSigned) {
            throw new CoalitionFormationError({
              reason: "negotiation_failed",
              message: "Not all agents agreed to the terms",
              cause: { signatures, required: proposal.proposedMembers.map((a) => a.agentId) }
            })
          }

          // Create escrow address (mock)
          const escrowAddress = `bc1q_escrow_${contractId.substring(9, 20)}`

          return {
            contractId,
            proposal,
            signatures,
            escrowAddress,
            status: "pending",
            createdAt: Date.now()
          }
        },
        catch: (error) =>
          new CoalitionFormationError({
            reason: "negotiation_failed",
            message: `Failed to negotiate coalition terms: ${error}`,
            cause: error
          })
      })

    const formalizeAgreement = (
      contract: CoalitionContract
    ): Effect.Effect<Coalition, CoalitionFormationError> =>
      Effect.try({
        try: () => {
          const coalitionId = `coalition_${contract.contractId.substring(9)}`

          // Create internal coordination channel
          const internalChannelId = `channel_${coalitionId}`

          // Initialize coalition
          const coalition: Coalition = {
            coalitionId,
            contract: {
              ...contract,
              status: "active",
              activatedAt: Date.now()
            },
            members: contract.proposal.proposedMembers,
            projectProgress: 0,
            activeTasks: [],
            completedTasks: [],
            internalChannelId
          }

          // Store coalition
          activeCoalitions.set(coalitionId, coalition)

          // Update agent profiles to show they're in a coalition
          contract.proposal.proposedMembers.forEach((agent) => {
            const profile = agentProfiles.get(agent.agentId)
            if (profile) {
              agentProfiles.set(agent.agentId, {
                ...profile,
                isAvailable: false
              })
            }
          })

          return coalition
        },
        catch: (error) =>
          new CoalitionFormationError({
            reason: "formation_failed",
            message: `Failed to formalize coalition agreement: ${error}`,
            cause: error
          })
      })

    const broadcastOpportunity = (
      project: ComplexProject
    ): Effect.Effect<void, CoalitionFormationError> =>
      Effect.try({
        try: () => {
          // Mock broadcast for now
          console.log(`Broadcast coalition opportunity for project: ${project.title}`)
        },
        catch: (error) =>
          new CoalitionFormationError({
            reason: "formation_failed",
            message: `Failed to broadcast coalition opportunity: ${error}`,
            cause: error
          })
      })

    const monitorCoalitions = (): Stream.Stream<Array<Coalition>, CoalitionFormationError> =>
      Stream.repeat(
        Effect.sync(() => Array.from(activeCoalitions.values())),
        Schedule.spaced(Duration.seconds(10))
      ).pipe(
        Stream.tapError((error) => Effect.log(`Coalition monitoring error: ${error}`))
      )

    return {
      analyzeProjectRequirements,
      findComplementaryAgents,
      assessCoalitionViability,
      proposeCoalition,
      negotiateTerms,
      formalizeAgreement,
      broadcastOpportunity,
      monitorCoalitions
    }
  })
)

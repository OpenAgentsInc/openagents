/**
 * FAV-03 (#9113): Apple FM advisory capacity inside Full Auto.
 *
 * Apple FM participates at capacity in exactly the role its standing authority
 * admits (docs/sol/2026-07-20-apple-fm-router-to-full-agent-system-plan.md):
 * "A local model can recommend a route or produce an advisory result.
 * Deterministic policy must make the route decision. Existing host services
 * must perform all actions." It must NOT be given action authority.
 *
 * This module makes that boundary structural. Apple FM produces a typed route
 * RECOMMENDATION and typed advisory ANALYSIS — both carry a `advisory: true`
 * literal and no decision/action field. The route DECISION is a pure function
 * of the owner-ordered policy and the shared lane gate, computed WITHOUT the
 * recommendation as an input, so no recommendation can ever change what runs.
 * Recommendation and decision are recorded as distinct facts.
 */
import { Schema } from "effect"

import { type FullAutoRoutingCandidate } from "./full-auto-registry.ts"
import {
  validateFullAutoRoutingPolicy,
  type FullAutoRoutingLaneGate,
  type FullAutoRoutingPolicyRefusalReason,
} from "./full-auto-routing.ts"

export const FULL_AUTO_ADVISORY_SOURCE = "apple_fm" as const
export const FULL_AUTO_ROUTE_RECOMMENDATION_SCHEMA =
  "openagents.desktop.full_auto_route_recommendation.v1" as const
export const FULL_AUTO_ADVISORY_ANALYSIS_SCHEMA =
  "openagents.desktop.full_auto_advisory_analysis.v1" as const
export const FULL_AUTO_ADVISORY_OBSERVATION_LIMIT = 16

const LaneRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))
const BoundedText = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400))

/**
 * An on-device Apple FM route recommendation over the owner-ordered
 * candidates. `advisory: true` is a structural literal — a recommendation can
 * never be decoded as anything but advisory, and it carries no chosen/decision
 * field. It informs; it does not decide.
 */
export const FullAutoRouteRecommendationSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_ROUTE_RECOMMENDATION_SCHEMA),
  source: Schema.Literal(FULL_AUTO_ADVISORY_SOURCE),
  advisory: Schema.Literal(true),
  recommendedLane: LaneRef,
  rationale: BoundedText,
  at: Schema.String,
})
export type FullAutoRouteRecommendation = typeof FullAutoRouteRecommendationSchema.Type
export const decodeFullAutoRouteRecommendation = Schema.decodeUnknownSync(
  FullAutoRouteRecommendationSchema,
)

/**
 * A bounded, read-only Apple FM analysis of run progress (stall smell,
 * objective-drift flags). `advisory: true` is structural. It is evidence for
 * the owner, never a verdict — it flips no typed state.
 */
export const FullAutoAdvisoryAnalysisSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_ADVISORY_ANALYSIS_SCHEMA),
  source: Schema.Literal(FULL_AUTO_ADVISORY_SOURCE),
  advisory: Schema.Literal(true),
  observations: Schema.Array(BoundedText).check(
    Schema.isMaxLength(FULL_AUTO_ADVISORY_OBSERVATION_LIMIT),
  ),
  at: Schema.String,
})
export type FullAutoAdvisoryAnalysis = typeof FullAutoAdvisoryAnalysisSchema.Type
export const decodeFullAutoAdvisoryAnalysis = Schema.decodeUnknownSync(
  FullAutoAdvisoryAnalysisSchema,
)

export type FullAutoRouteDecision = Readonly<{
  chosenLane: string | null
  refusalReason: FullAutoRoutingPolicyRefusalReason | null
}>

/**
 * The deterministic route decision: the first candidate of the owner-ordered
 * policy, admitted only if the whole policy validates fail-closed. It takes
 * NO recommendation argument — the decision cannot depend on Apple FM.
 */
export const decideFullAutoRoute = (
  policy: ReadonlyArray<FullAutoRoutingCandidate>,
  laneGate: FullAutoRoutingLaneGate,
): FullAutoRouteDecision => {
  const validation = validateFullAutoRoutingPolicy(policy, laneGate)
  if (!validation.ok) return { chosenLane: null, refusalReason: validation.reason }
  // A validated policy has all candidates ready; the owner order decides.
  return { chosenLane: validation.policy[0]!.lane, refusalReason: null }
}

export type FullAutoAdvisedRoute = Readonly<{
  decision: FullAutoRouteDecision
  recommendation: FullAutoRouteRecommendation | null
  /**
   * Whether the deterministic decision HAPPENED to match Apple FM's
   * recommendation. This is a report, not a cause: `decision` is computed by
   * {@link decideFullAutoRoute} without the recommendation, so this flag can
   * never change what runs.
   */
  recommendationMatchedDecision: boolean
}>

/**
 * Record an Apple FM recommendation ALONGSIDE the deterministic decision. The
 * decision is computed independently; the recommendation is attached for the
 * report and for the owner. The two are distinct facts.
 */
export const adviseFullAutoRoute = (
  policy: ReadonlyArray<FullAutoRoutingCandidate>,
  laneGate: FullAutoRoutingLaneGate,
  recommendation: FullAutoRouteRecommendation | null,
): FullAutoAdvisedRoute => {
  const decision = decideFullAutoRoute(policy, laneGate)
  return {
    decision,
    recommendation,
    recommendationMatchedDecision:
      recommendation !== null &&
      decision.chosenLane !== null &&
      recommendation.recommendedLane === decision.chosenLane,
  }
}

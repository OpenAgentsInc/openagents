import { Schema as S } from "effect";

import { ArtifactRef, RouteDecisionRef } from "./artifact.js";
import { ContextManifestRef } from "./context.js";
import {
  TurnCostClass,
  TurnDataDestination,
  TurnProviderCandidate,
  TurnProviderRef,
} from "./provider.js";
import { MAX_OWNER_BOUND_CANDIDATES, TurnRequestRef, TurnTaskClass, TurnTimestamp } from "./turn.js";

/**
 * AFS-00 frozen route contract.
 *
 * This module freezes the split between a `RouteRecommendation` and a
 * `RouteDecision`. A model result can contain a recommendation. It can never
 * contain an admitted decision. The host derives the only decision from owner
 * policy, candidate order, capability, account readiness, data destination,
 * cost class, placement, privacy, and task needs. A recommendation can never
 * add a candidate to the owner-bound set.
 *
 * Compatibility rules are the shared AFS-00 rules recorded in `turn.ts`.
 */
export const ROUTE_RECOMMENDATION_SCHEMA_LITERAL = "openagents.agent_route_recommendation.v1" as const;
export const ROUTE_DECISION_SCHEMA_LITERAL = "openagents.agent_route_decision.v1" as const;
export const OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL = "openagents.agent_owner_bound_candidate_set.v1" as const;

/** The reason a lane produced an advisory recommendation. */
export const RouteReasonCode = S.Literals([
  "local_answer_sufficient",
  "needs_delegation",
  "needs_external_tools",
  "needs_large_context",
  "needs_source_control_action",
  "low_confidence",
  "explicit_provider_request",
]);
export type RouteReasonCode = typeof RouteReasonCode.Type;

/**
 * An advisory route recommendation. Only a host can act on it. Its confidence
 * is a bounded 0..1 signal. The candidate must already be in the owner-bound
 * set for the host to consider it.
 */
export const RouteRecommendation = S.Struct({
  schema: S.Literal(ROUTE_RECOMMENDATION_SCHEMA_LITERAL),
  candidate: TurnProviderCandidate,
  taskClass: TurnTaskClass,
  reasonCode: RouteReasonCode,
  confidence: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
});
export type RouteRecommendation = typeof RouteRecommendation.Type;

/**
 * The owner-bound ordered candidate set. Policy selects only within this set.
 * The set binds an immutable released policy artifact. A recommendation cannot
 * extend it.
 */
export const OwnerBoundCandidateSet = S.Struct({
  schema: S.Literal(OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL),
  ordered: S.Array(TurnProviderRef).check(S.isMinLength(1), S.isMaxLength(MAX_OWNER_BOUND_CANDIDATES)),
  policyArtifactRef: ArtifactRef,
});
export type OwnerBoundCandidateSet = typeof OwnerBoundCandidateSet.Type;

/**
 * A turn disclosure. It discloses the effective data destination and cost class
 * for the selected lane. A remote provider fallback needs its own disclosure.
 */
export const TurnDisclosure = S.Struct({
  dataDestination: TurnDataDestination,
  costClass: TurnCostClass,
  localOnly: S.Boolean,
  providerRef: S.optionalKey(TurnProviderRef),
});
export type TurnDisclosure = typeof TurnDisclosure.Type;

/** How the host reached a route decision. */
export const RouteDecisionReason = S.Literals([
  "admitted_first_candidate",
  "recommendation_admitted",
  "recommendation_rejected_fell_through",
  "forced_explicit_provider",
  "no_candidate_fail_closed",
]);
export type RouteDecisionReason = typeof RouteDecisionReason.Type;

/** A skipped or refused lane with its reason code. */
export const RouteLaneDisposition = S.Struct({
  providerRef: TurnProviderRef,
  candidate: TurnProviderCandidate,
  disposition: S.Literals(["skipped", "refused"]),
  reason: S.Literals([
    "not_admitted",
    "no_capability",
    "account_not_ready",
    "placement_unavailable",
    "privacy_blocked",
    "cost_blocked",
    "resource_not_ready",
  ]),
});
export type RouteLaneDisposition = typeof RouteLaneDisposition.Type;

/**
 * The admitted route decision. The host records the selected and effective
 * lane, the admitted candidate set, the policy artifact, the context manifest,
 * the disclosure, the decision reason, and every refused or skipped lane. A
 * fail-closed decision has no selected lane.
 */
export const RouteDecision = S.Union([
  S.Struct({
    schema: S.Literal(ROUTE_DECISION_SCHEMA_LITERAL),
    outcome: S.Literal("admitted"),
    routeDecisionRef: RouteDecisionRef,
    requestRef: TurnRequestRef,
    selected: TurnProviderRef,
    effective: TurnProviderRef,
    admittedCandidateSet: S.Array(TurnProviderRef).check(
      S.isMinLength(1),
      S.isMaxLength(MAX_OWNER_BOUND_CANDIDATES),
    ),
    policyArtifactRef: ArtifactRef,
    contextManifestRef: ContextManifestRef,
    disclosure: TurnDisclosure,
    decisionReason: RouteDecisionReason,
    dispositions: S.Array(RouteLaneDisposition).check(S.isMaxLength(MAX_OWNER_BOUND_CANDIDATES)),
    decidedAt: TurnTimestamp,
  }),
  S.Struct({
    schema: S.Literal(ROUTE_DECISION_SCHEMA_LITERAL),
    outcome: S.Literal("closed"),
    routeDecisionRef: RouteDecisionRef,
    requestRef: TurnRequestRef,
    policyArtifactRef: ArtifactRef,
    contextManifestRef: ContextManifestRef,
    decisionReason: S.Literal("no_candidate_fail_closed"),
    dispositions: S.Array(RouteLaneDisposition).check(S.isMaxLength(MAX_OWNER_BOUND_CANDIDATES)),
    decidedAt: TurnTimestamp,
  }),
]);
export type RouteDecision = typeof RouteDecision.Type;

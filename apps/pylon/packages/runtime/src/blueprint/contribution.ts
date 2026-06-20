import { Effect, Schema as S } from "effect";
import {
  BlueprintProgramFamily,
  BlueprintProgramRiskClass,
  isBlueprintProjectionPrivateDataSafe,
} from "./contracts.js";

export const ProbeBlueprintContributionKind = S.Literals([
  "signature_contribution",
  "developer_package_contribution",
]);
export type ProbeBlueprintContributionKind = typeof ProbeBlueprintContributionKind.Type;

export const ProbeBlueprintContributionStatus = S.Literals([
  "draft",
  "submitted",
  "in_review",
  "needs_changes",
  "rejected",
  "approved_for_release_gate",
  "promoted",
  "archived",
]);
export type ProbeBlueprintContributionStatus = typeof ProbeBlueprintContributionStatus.Type;

export const ProbeBlueprintContributionReviewStatus = S.Literals([
  "not_requested",
  "pending",
  "changes_requested",
  "approved",
  "rejected",
]);
export type ProbeBlueprintContributionReviewStatus = typeof ProbeBlueprintContributionReviewStatus.Type;

export const ProbeBlueprintContributionCapabilityFamily = S.Literals([
  "agent_tool",
  "backend_projection_adapter",
  "context_package",
  "outcome_template",
  "program_signature",
  "retrieval_package",
  "route_policy",
  "tool_package",
  "ui_binding",
  "workroom_template",
]);
export type ProbeBlueprintContributionCapabilityFamily =
  typeof ProbeBlueprintContributionCapabilityFamily.Type;

export const ProbeBlueprintContributionAuthority = S.Struct({
  canChangePublicClaims: S.Boolean,
  canCreateSite: S.Boolean,
  canDeploy: S.Boolean,
  canDispatchRuntime: S.Boolean,
  canExecute: S.Boolean,
  canMutateRepository: S.Boolean,
  canPostPublicly: S.Boolean,
  canSendEmail: S.Boolean,
  canSpend: S.Boolean,
  deniedEffectRefs: S.Array(S.String),
});
export type ProbeBlueprintContributionAuthority = typeof ProbeBlueprintContributionAuthority.Type;

export const PROBE_BLUEPRINT_CONTRIBUTION_NO_AUTHORITY: ProbeBlueprintContributionAuthority = {
  canChangePublicClaims: false,
  canCreateSite: false,
  canDeploy: false,
  canDispatchRuntime: false,
  canExecute: false,
  canMutateRepository: false,
  canPostPublicly: false,
  canSendEmail: false,
  canSpend: false,
  deniedEffectRefs: [
    "effect.execute",
    "effect.dispatch_runtime",
    "effect.deploy",
    "effect.spend",
    "effect.send_email",
    "effect.mutate_repository",
    "effect.post_publicly",
    "effect.create_site",
    "effect.change_public_claims",
  ],
};

export const ProbeBlueprintContributionDraft = S.Struct({
  authority: ProbeBlueprintContributionAuthority,
  backendProjectionAdapterRefs: S.Array(S.String),
  capabilityFamily: ProbeBlueprintContributionCapabilityFamily,
  capabilitySummaryRef: S.String,
  contentRedacted: S.Literal(true),
  contextPackageRefs: S.Array(S.String),
  contributionKind: ProbeBlueprintContributionKind,
  contributorRefs: S.Array(S.String),
  dogfoodScopeRef: S.NullOr(S.String),
  fixtureRefs: S.Array(S.String),
  id: S.String,
  intendedProgramFamily: BlueprintProgramFamily,
  noProductionRuntimeAuthority: S.Literal(true),
  outcomeTemplateRefs: S.Array(S.String),
  paymentAttributionRefs: S.Array(S.String),
  promotionRef: S.NullOr(S.String),
  proposedModuleVersionRefs: S.Array(S.String),
  proposedProgramSignatureRefs: S.Array(S.String),
  proposedProgramTypeRefs: S.Array(S.String),
  rejectionRef: S.NullOr(S.String),
  releaseGateRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  reviewStatus: ProbeBlueprintContributionReviewStatus,
  riskClass: BlueprintProgramRiskClass,
  selfPromotionAttempt: S.Boolean,
  sourceRefs: S.Array(S.String),
  status: ProbeBlueprintContributionStatus,
  toolPackageRefs: S.Array(S.String),
  uiBindingRefs: S.Array(S.String),
});
export type ProbeBlueprintContributionDraft = typeof ProbeBlueprintContributionDraft.Type;

export const ProbeBlueprintContributionRuntimeEligibility = S.Struct({
  candidateRuntimeAllowed: S.Boolean,
  productionRuntimeAllowed: S.Boolean,
  reasonRefs: S.Array(S.String),
});
export type ProbeBlueprintContributionRuntimeEligibility =
  typeof ProbeBlueprintContributionRuntimeEligibility.Type;

export class ProbeBlueprintContributionUnsafe extends S.TaggedErrorClass<ProbeBlueprintContributionUnsafe>()(
  "ProbeBlueprintContributionUnsafe",
  {
    path: S.String,
    reason: S.String,
  },
) {}

const RAW_TIMESTAMP_PATTERN = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/;

export function probeBlueprintContributionHasRuntimeAuthority(
  contribution: ProbeBlueprintContributionDraft,
): boolean {
  return (
    contribution.authority.canChangePublicClaims ||
    contribution.authority.canCreateSite ||
    contribution.authority.canDeploy ||
    contribution.authority.canDispatchRuntime ||
    contribution.authority.canExecute ||
    contribution.authority.canMutateRepository ||
    contribution.authority.canPostPublicly ||
    contribution.authority.canSendEmail ||
    contribution.authority.canSpend
  );
}

export function probeBlueprintContributionTargetRefs(
  contribution: ProbeBlueprintContributionDraft,
): ReadonlyArray<string> {
  return uniqueStrings([
    ...contribution.backendProjectionAdapterRefs,
    ...contribution.contextPackageRefs,
    ...contribution.outcomeTemplateRefs,
    ...contribution.proposedModuleVersionRefs,
    ...contribution.proposedProgramSignatureRefs,
    ...contribution.proposedProgramTypeRefs,
    ...contribution.toolPackageRefs,
    ...contribution.uiBindingRefs,
  ]);
}

export function probeBlueprintContributionCanEnterReleaseGate(
  contribution: ProbeBlueprintContributionDraft,
): boolean {
  return (
    !probeBlueprintContributionHasRuntimeAuthority(contribution) &&
    !contribution.selfPromotionAttempt &&
    contribution.status === "approved_for_release_gate" &&
    contribution.reviewStatus === "approved" &&
    contribution.rejectionRef === null &&
    contribution.promotionRef === null &&
    contribution.fixtureRefs.length > 0 &&
    contribution.releaseGateRefs.length > 0 &&
    probeBlueprintContributionTargetRefs(contribution).length > 0
  );
}

export function probeBlueprintContributionBlockerRefs(
  contribution: ProbeBlueprintContributionDraft,
): ReadonlyArray<string> {
  return uniqueStrings([
    ...(probeBlueprintContributionHasRuntimeAuthority(contribution)
      ? ["blocker.probe_blueprint_contribution.runtime_authority_present"]
      : []),
    ...(contribution.selfPromotionAttempt
      ? ["blocker.probe_blueprint_contribution.self_promotion_attempt"]
      : []),
    ...(contribution.reviewStatus !== "approved"
      ? ["blocker.probe_blueprint_contribution.review_not_approved"]
      : []),
    ...(contribution.status !== "approved_for_release_gate"
      ? ["blocker.probe_blueprint_contribution.not_release_gate_ready"]
      : []),
    ...(contribution.fixtureRefs.length === 0 ? ["blocker.probe_blueprint_contribution.fixture_refs_missing"] : []),
    ...(contribution.releaseGateRefs.length === 0
      ? ["blocker.probe_blueprint_contribution.release_gate_refs_missing"]
      : []),
    ...(probeBlueprintContributionTargetRefs(contribution).length === 0
      ? ["blocker.probe_blueprint_contribution.target_ref_missing"]
      : []),
    ...(contribution.rejectionRef !== null ? ["blocker.probe_blueprint_contribution.rejected"] : []),
    ...(contribution.promotionRef !== null ? ["blocker.probe_blueprint_contribution.already_promoted"] : []),
  ]);
}

export function probeBlueprintContributionRuntimeEligibility(
  contribution: ProbeBlueprintContributionDraft,
  options: { readonly assignmentAllowsCandidate: boolean },
): ProbeBlueprintContributionRuntimeEligibility {
  const safeForCandidate =
    options.assignmentAllowsCandidate &&
    !probeBlueprintContributionHasRuntimeAuthority(contribution) &&
    !contribution.selfPromotionAttempt &&
    contribution.status !== "rejected" &&
    contribution.status !== "archived" &&
    contribution.rejectionRef === null;
  const safeForProduction =
    !probeBlueprintContributionHasRuntimeAuthority(contribution) &&
    !contribution.selfPromotionAttempt &&
    contribution.status === "promoted" &&
    contribution.promotionRef !== null &&
    contribution.rejectionRef === null;

  return {
    candidateRuntimeAllowed: safeForCandidate,
    productionRuntimeAllowed: safeForProduction,
    reasonRefs: uniqueStrings([
      ...(safeForCandidate ? ["reason.probe_blueprint_contribution.candidate_dogfood_allowed"] : []),
      ...(safeForProduction ? ["reason.probe_blueprint_contribution.promoted_runtime_allowed"] : []),
      ...(!safeForCandidate && !safeForProduction
        ? ["reason.probe_blueprint_contribution.runtime_blocked_until_release_gate"]
        : []),
    ]),
  };
}

export function validateProbeBlueprintContributionDraft(
  contribution: ProbeBlueprintContributionDraft,
): Effect.Effect<ProbeBlueprintContributionDraft, ProbeBlueprintContributionUnsafe> {
  return Effect.gen(function* () {
    if (probeBlueprintContributionHasRuntimeAuthority(contribution)) {
      return yield* Effect.fail(
        new ProbeBlueprintContributionUnsafe({
          path: "authority",
          reason: "Probe Blueprint contributions cannot carry runtime authority",
        }),
      );
    }

    if (contribution.selfPromotionAttempt) {
      return yield* Effect.fail(
        new ProbeBlueprintContributionUnsafe({
          path: "selfPromotionAttempt",
          reason: "Probe Blueprint contributions cannot self-promote",
        }),
      );
    }

    const serialized = JSON.stringify(contribution);

    if (!isBlueprintProjectionPrivateDataSafe(contribution) || RAW_TIMESTAMP_PATTERN.test(serialized)) {
      return yield* Effect.fail(
        new ProbeBlueprintContributionUnsafe({
          path: "contribution",
          reason: "Probe Blueprint contribution contains private-data-shaped material",
        }),
      );
    }

    return contribution;
  });
}

function uniqueStrings(values: ReadonlyArray<string>): Array<string> {
  return [...new Set(values.filter((value) => value.trim() !== ""))].sort();
}

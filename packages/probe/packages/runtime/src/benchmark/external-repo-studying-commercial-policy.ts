import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Commercial policy preflight for repo-studying packages.
 *
 * This closes the explicit metering/pricing/payout policy gap for
 * autopilot.repo_study_packets.v1 and
 * autopilot.external_repo_studying_pilot.v1 without making either product green.
 * The preflight accepts refs only: usage-subject refs, package-policy refs,
 * pricing-policy refs, payout-policy refs, and settlement-gate refs.
 *
 * Hard rules:
 *  - sourceBoundary = "public_refs_only"; no customer repo content, raw task
 *    text, raw invoices, wallet material, or settlement payloads are admitted.
 *  - Inert by construction: marketplacePackageAllowed, payoutEligible,
 *    settlementReady, and effectsApplied are always false.
 *  - It can only say whether a paid package WOULD be policy-ready when the
 *    default-off commercial gate is armed and every required ref is present.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_COMMERCIAL_POLICY_SCHEMA_REF =
  "openagents.external_repo_study_commercial_policy.v0" as const;

export const ExternalRepoStudyCommercialPolicyFlagName =
  "EXTERNAL_REPO_STUDY_COMMERCIAL_POLICY_ENABLED" as const;

export const OpenAgentsExternalRepoStudyCommercialPolicyGateState = S.Literals([
  "inert_disabled",
  "armed_blocked",
  "armed_ready",
]);
export type OpenAgentsExternalRepoStudyCommercialPolicyGateState =
  typeof OpenAgentsExternalRepoStudyCommercialPolicyGateState.Type;

export const OpenAgentsExternalRepoStudyCommercialPolicyState = S.Literals([
  "policy_ready_held",
  "blocked",
]);
export type OpenAgentsExternalRepoStudyCommercialPolicyState =
  typeof OpenAgentsExternalRepoStudyCommercialPolicyState.Type;

export interface ExternalRepoStudyCommercialPolicyRequest {
  readonly customerRef: string;
  readonly repo: string;
  readonly studyPacketRef: string;
  readonly validationRef: string;
  readonly usageSubjectRef?: string;
  readonly meteringPolicyRef?: string;
  readonly packagePolicyRef?: string;
  readonly pricingPolicyRef?: string;
  readonly entitlementPolicyRef?: string;
  readonly payoutPolicyRef?: string;
  readonly settlementGateRef?: string;
  readonly refundDisputePolicyRef?: string;
  readonly reviewerRef?: string;
}

export const OpenAgentsExternalRepoStudyCommercialPolicyGate = S.Struct({
  blockedReasonRefs: S.Array(S.String),
  effectsApplied: S.Literal(false),
  flagName: S.Literal(ExternalRepoStudyCommercialPolicyFlagName),
  ownerSignoffPresent: S.Boolean,
  state: OpenAgentsExternalRepoStudyCommercialPolicyGateState,
});
export type OpenAgentsExternalRepoStudyCommercialPolicyGate =
  typeof OpenAgentsExternalRepoStudyCommercialPolicyGate.Type;

export const OpenAgentsExternalRepoStudyCommercialPolicyPreflight = S.Struct({
  blockerRefs: S.Array(S.String),
  commercialGate: OpenAgentsExternalRepoStudyCommercialPolicyGate,
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  effectsApplied: S.Literal(false),
  entitlementPolicyPresent: S.Boolean,
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  meteringPolicyPresent: S.Boolean,
  packagePolicyPresent: S.Boolean,
  payoutEligible: S.Literal(false),
  payoutPolicyPresent: S.Boolean,
  policyHash: S.String,
  policyRef: S.String,
  pricingPolicyPresent: S.Boolean,
  refundDisputePolicyPresent: S.Boolean,
  repo: S.String,
  safeCopy: S.String,
  schemaRef: S.Literal(OPENAGENTS_EXTERNAL_REPO_STUDY_COMMERCIAL_POLICY_SCHEMA_REF),
  settlementGatePresent: S.Boolean,
  settlementReady: S.Literal(false),
  sourceBoundary: S.Literal("public_refs_only"),
  state: OpenAgentsExternalRepoStudyCommercialPolicyState,
  studyPacketRef: S.String,
  unsafeCopyRefs: S.Array(S.String),
  usageSubjectPresent: S.Boolean,
  validationRef: S.String,
  wouldAllowPaidPackageWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyCommercialPolicyPreflight =
  typeof OpenAgentsExternalRepoStudyCommercialPolicyPreflight.Type;

export interface BuildOpenAgentsExternalRepoStudyCommercialPolicyPreflightInput {
  readonly commercialPolicyFlagArmed?: boolean;
  readonly generatedAt?: string;
  readonly ownerSignoffPresent?: boolean;
  readonly request: ExternalRepoStudyCommercialPolicyRequest;
}

export function buildOpenAgentsExternalRepoStudyCommercialPolicyPreflight(
  input: BuildOpenAgentsExternalRepoStudyCommercialPolicyPreflightInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyCommercialPolicyPreflight,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const request = input.request;
    yield* requireNonEmpty(request.customerRef, "externalRepoStudyCommercialPolicy.customerRef");
    yield* requireNonEmpty(request.repo, "externalRepoStudyCommercialPolicy.repo");
    yield* requireNonEmpty(request.studyPacketRef, "externalRepoStudyCommercialPolicy.studyPacketRef");
    yield* requireNonEmpty(request.validationRef, "externalRepoStudyCommercialPolicy.validationRef");

    const usageSubjectPresent = present(request.usageSubjectRef);
    const meteringPolicyPresent = present(request.meteringPolicyRef);
    const packagePolicyPresent = present(request.packagePolicyRef);
    const pricingPolicyPresent = present(request.pricingPolicyRef);
    const entitlementPolicyPresent = present(request.entitlementPolicyRef);
    const payoutPolicyPresent = present(request.payoutPolicyRef);
    const settlementGatePresent = present(request.settlementGateRef);
    const refundDisputePolicyPresent = present(request.refundDisputePolicyRef);

    const policyPassed =
      usageSubjectPresent &&
      meteringPolicyPresent &&
      packagePolicyPresent &&
      pricingPolicyPresent &&
      entitlementPolicyPresent &&
      payoutPolicyPresent &&
      settlementGatePresent &&
      refundDisputePolicyPresent;

    const blockerRefs = buildCommercialBlockerRefs({
      entitlementPolicyPresent,
      meteringPolicyPresent,
      packagePolicyPresent,
      payoutPolicyPresent,
      pricingPolicyPresent,
      refundDisputePolicyPresent,
      settlementGatePresent,
      usageSubjectPresent,
    });

    const commercialGate = buildCommercialGate({
      commercialPolicyFlagArmed: input.commercialPolicyFlagArmed ?? false,
      ownerSignoffPresent: input.ownerSignoffPresent ?? false,
      policyPassed,
    });

    const wouldAllowPaidPackageWhenArmed =
      policyPassed && commercialGate.state === "armed_ready";

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_commercial_policy_hash";

    const evidenceRefs = [
      request.customerRef,
      request.studyPacketRef,
      request.validationRef,
      request.usageSubjectRef,
      request.meteringPolicyRef,
      request.packagePolicyRef,
      request.pricingPolicyRef,
      request.entitlementPolicyRef,
      request.payoutPolicyRef,
      request.settlementGateRef,
      request.refundDisputePolicyRef,
      request.reviewerRef,
      "docs/promises/2026-06-17-repo-studying-product-promise-gate-review.md",
    ].filter((ref): ref is string => present(ref));

    const base: OpenAgentsExternalRepoStudyCommercialPolicyPreflight = {
      blockerRefs,
      commercialGate,
      customerPublicClaimAllowed: false,
      customerRef: request.customerRef,
      effectsApplied: false,
      entitlementPolicyPresent,
      evidenceRefs,
      generatedAt,
      marketplacePackageAllowed: false,
      meteringPolicyPresent,
      packagePolicyPresent,
      payoutEligible: false,
      payoutPolicyPresent,
      policyHash: "sha256:pending",
      policyRef: "external_repo_study_commercial_policy.pending",
      pricingPolicyPresent,
      refundDisputePolicyPresent,
      repo: request.repo,
      safeCopy:
        "Repo-studying commercial policy was evaluated from public-safe refs only: usage subject, metering, package, pricing, entitlement, payout, settlement, and refund/dispute policy refs. The gate is held inert; no package is listed, no customer claim is made, no payout is eligible, no settlement is ready, and no money moves.",
      schemaRef: OPENAGENTS_EXTERNAL_REPO_STUDY_COMMERCIAL_POLICY_SCHEMA_REF,
      settlementGatePresent,
      settlementReady: false,
      sourceBoundary: "public_refs_only",
      state: policyPassed ? "policy_ready_held" : "blocked",
      studyPacketRef: request.studyPacketRef,
      unsafeCopyRefs: [
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.repo_studying_paid_package_live",
        "blocked_claim.machine_studying_payout_eligible",
        "blocked_claim.repo_studying_settlement_ready",
      ],
      usageSubjectPresent,
      validationRef: request.validationRef,
      wouldAllowPaidPackageWhenArmed,
    };

    const policyHash = openAgentsExternalRepoStudyCommercialPolicyHash(base);
    return yield* decodeOpenAgentsExternalRepoStudyCommercialPolicyPreflight({
      ...base,
      policyHash,
      policyRef: `external_repo_study_commercial_policy.${slugRepo(request.repo)}.${shortHash(policyHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyCommercialPolicyPreflight(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyCommercialPolicyPreflight,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "externalRepoStudyCommercialPolicy");
    const preflight = yield* S.decodeUnknownEffect(OpenAgentsExternalRepoStudyCommercialPolicyPreflight)(value).pipe(
      Effect.mapError((error) =>
        new ProbeBenchmarkContractError({
          path: "externalRepoStudyCommercialPolicy",
          reason: String(error),
        }),
      ),
    );
    yield* validateCommercialPolicy(preflight);
    return preflight;
  });
}

export function openAgentsExternalRepoStudyCommercialPolicyHash(
  preflight: OpenAgentsExternalRepoStudyCommercialPolicyPreflight,
): string {
  const {
    generatedAt: _generatedAt,
    policyHash: _policyHash,
    policyRef: _policyRef,
    ...stable
  } = preflight;
  return sha256Ref(stableJson(stable));
}

function buildCommercialGate(input: {
  readonly commercialPolicyFlagArmed: boolean;
  readonly ownerSignoffPresent: boolean;
  readonly policyPassed: boolean;
}): OpenAgentsExternalRepoStudyCommercialPolicyGate {
  const blockedReasonRefs: string[] = [];
  if (!input.commercialPolicyFlagArmed) {
    blockedReasonRefs.push("blocker.external_repo_study_commercial_policy.flag_disabled");
  }
  if (!input.ownerSignoffPresent) {
    blockedReasonRefs.push("blocker.external_repo_study_commercial_policy.owner_signoff_missing");
  }
  if (!input.policyPassed) {
    blockedReasonRefs.push("blocker.external_repo_study_commercial_policy.policy_refs_incomplete");
  }

  return {
    blockedReasonRefs,
    effectsApplied: false,
    flagName: ExternalRepoStudyCommercialPolicyFlagName,
    ownerSignoffPresent: input.ownerSignoffPresent,
    state: !input.commercialPolicyFlagArmed
      ? "inert_disabled"
      : input.policyPassed && input.ownerSignoffPresent
        ? "armed_ready"
        : "armed_blocked",
  };
}

function buildCommercialBlockerRefs(input: {
  readonly entitlementPolicyPresent: boolean;
  readonly meteringPolicyPresent: boolean;
  readonly packagePolicyPresent: boolean;
  readonly payoutPolicyPresent: boolean;
  readonly pricingPolicyPresent: boolean;
  readonly refundDisputePolicyPresent: boolean;
  readonly settlementGatePresent: boolean;
  readonly usageSubjectPresent: boolean;
}): ReadonlyArray<string> {
  const blockers: string[] = [];
  if (!input.usageSubjectPresent) blockers.push("blocker.external_repo_study_commercial_policy.usage_subject_missing");
  if (!input.meteringPolicyPresent) blockers.push("blocker.external_repo_study_commercial_policy.metering_policy_missing");
  if (!input.packagePolicyPresent) blockers.push("blocker.external_repo_study_commercial_policy.package_policy_missing");
  if (!input.pricingPolicyPresent) blockers.push("blocker.external_repo_study_commercial_policy.pricing_policy_missing");
  if (!input.entitlementPolicyPresent) blockers.push("blocker.external_repo_study_commercial_policy.entitlement_policy_missing");
  if (!input.payoutPolicyPresent) blockers.push("blocker.external_repo_study_commercial_policy.payout_policy_missing");
  if (!input.settlementGatePresent) blockers.push("blocker.external_repo_study_commercial_policy.settlement_gate_missing");
  if (!input.refundDisputePolicyPresent) blockers.push("blocker.external_repo_study_commercial_policy.refund_dispute_policy_missing");
  return blockers;
}

function validateCommercialPolicy(
  preflight: OpenAgentsExternalRepoStudyCommercialPolicyPreflight,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(preflight.policyRef, "externalRepoStudyCommercialPolicy.policyRef");
    yield* requireSha256(preflight.policyHash, "externalRepoStudyCommercialPolicy.policyHash");

    if (
      preflight.customerPublicClaimAllowed !== false ||
      preflight.marketplacePackageAllowed !== false ||
      preflight.payoutEligible !== false ||
      preflight.settlementReady !== false ||
      preflight.effectsApplied !== false
    ) {
      return yield* commercialPolicyError(
        "externalRepoStudyCommercialPolicy.claimGates",
        "commercial policy preflight must remain inert and must not grant customer, marketplace, payout, settlement, or money-movement authority",
      );
    }

    if (
      preflight.wouldAllowPaidPackageWhenArmed &&
      preflight.commercialGate.state !== "armed_ready"
    ) {
      return yield* commercialPolicyError(
        "externalRepoStudyCommercialPolicy.wouldAllowPaidPackageWhenArmed",
        "paid-package readiness requires the armed_ready commercial gate",
      );
    }

    if (
      preflight.policyHash !==
      openAgentsExternalRepoStudyCommercialPolicyHash(preflight)
    ) {
      return yield* commercialPolicyError(
        "externalRepoStudyCommercialPolicy.policyHash",
        "must match deterministic commercial policy hash",
      );
    }
  });
}

function present(value: string | undefined): value is string {
  return (value ?? "").trim().length > 0;
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0 ? commercialPolicyError(path, "must be non-empty") : Effect.void;
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:") ? Effect.void : commercialPolicyError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function commercialPolicyError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

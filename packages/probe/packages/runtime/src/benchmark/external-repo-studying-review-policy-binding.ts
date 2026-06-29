import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  isPublishedExternalRepoStudyPrivacyPolicyRef,
  type OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
} from "./external-repo-studying-privacy-policy-registry";
import {
  buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  type ExternalRepoStudyPrivacyReviewRequest,
} from "./external-repo-studying-privacy-review";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Privacy-review <-> published-policy BINDING for the external-repo-studying
 * pilot.
 *
 * This module is the smallest genuine piece of the missing privacy POLICY
 * control surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_privacy_policy_missing): it
 * closes the seam between the privacy-review preflight and the published policy
 * registry.
 *
 * Today the privacy-review preflight's DPA / retention gates are only STRING
 * PRESENCE checks: any non-empty `dataProcessingAgreementRef` /
 * `retentionPolicyRef` satisfies them. The policy registry already publishes the
 * canonical policy versions and exposes
 * `isPublishedExternalRepoStudyPrivacyPolicyRef`, but nothing forced the review
 * to consume a KNOWN published policy. So a forged or stale string could pass the
 * review's DPA / retention gates even though it pointed at no published policy.
 *
 * This composer removes that forgeability: it derives the review's
 * `dataProcessingAgreementRef` and `retentionPolicyRef` FROM a policy ref that is
 * a KNOWN published version in the supplied registry. The review preflight is
 * then built from the derived refs. When the policy ref is unknown / forged /
 * empty, NO refs are injected, so the review blocks on
 * `data_processing_agreement_missing` + `retention_policy_missing` instead of
 * trusting arbitrary strings. The caller cannot inject its own DPA / retention
 * refs (they are Omitted from the request type).
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/counts/enums ONLY. sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: `reviewCleared` and `effectsApplied` are ALWAYS false
 *    (inherited from the nested review preflight and asserted here). The binding
 *    decides only WHETHER the review's DPA / retention gates are genuinely backed
 *    by a published policy; it never clears a review, ingests, stores bytes,
 *    delivers a packet, sends, settles, or spends.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires a real, human/legal review against
 * a real customer study, durable access-controlled storage that enforces the
 * declared retention window, and an owner-signed armed clearance with a
 * dereferenceable closeout receipt per proof.claim_upgrade_receipts.v1 — all
 * owner-gated and out of scope for the pure binding built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_REVIEW_POLICY_BINDING_SCHEMA_REF =
  "openagents.external_repo_study_review_policy_binding.v0" as const;

export const OpenAgentsExternalRepoStudyReviewPolicyBindingState = S.Literals([
  // The review's DPA / retention refs are derived from a known published policy.
  "bound_held",
  // The policy ref is unknown / forged / empty; no DPA / retention ref bound.
  "unbound",
]);
export type OpenAgentsExternalRepoStudyReviewPolicyBindingState =
  typeof OpenAgentsExternalRepoStudyReviewPolicyBindingState.Type;

export const OpenAgentsExternalRepoStudyReviewPolicyBinding = S.Struct({
  bindingHash: S.String,
  bindingRef: S.String,
  bound: S.Boolean,
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  // The DPA ref derived from the published policy, or null when unbound.
  dataProcessingAgreementRef: S.NullOr(S.String),
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  mismatchRefs: S.Array(S.String),
  payoutEligible: S.Literal(false),
  policyPublished: S.Boolean,
  policyRef: S.String,
  policyRegistryRef: S.String,
  // The matched published version's content digest, recorded as evidence.
  policyTermsDigest: S.NullOr(S.String),
  repo: S.String,
  // The retention-policy ref derived from the published policy, or null.
  retentionPolicyRef: S.NullOr(S.String),
  // The full review preflight built from the (only-if-bound) derived refs.
  reviewPreflight: OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  reviewCleared: S.Literal(false),
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_REVIEW_POLICY_BINDING_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyReviewPolicyBindingState,
  unsafeCopyRefs: S.Array(S.String),
  wouldClearWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyReviewPolicyBinding =
  typeof OpenAgentsExternalRepoStudyReviewPolicyBinding.Type;

export interface BuildOpenAgentsExternalRepoStudyReviewPolicyBindingInput {
  readonly generatedAt?: string;
  /** The published policy registry the policy ref is checked against. */
  readonly policyRegistry: OpenAgentsExternalRepoStudyPrivacyPolicyRegistry;
  /**
   * The policy ref the study claims governs it. It must match a KNOWN published
   * version in the registry; otherwise no DPA / retention ref is derived.
   */
  readonly policyRef: string;
  /**
   * The privacy-review request, MINUS its dataProcessingAgreementRef and
   * retentionPolicyRef. The binding derives those from the published policy;
   * callers cannot inject their own.
   */
  readonly reviewRequest: Omit<
    ExternalRepoStudyPrivacyReviewRequest,
    "dataProcessingAgreementRef" | "retentionPolicyRef"
  >;
  /**
   * Whether the review flag is armed. Forwarded to the nested review preflight.
   * Even when armed, no real effect is applied. Default false.
   */
  readonly reviewFlagArmed?: boolean;
  /**
   * Whether a privacy reviewer sign-off is recorded for an armed review.
   * Forwarded to the nested review preflight. Default false.
   */
  readonly reviewerSignoffPresent?: boolean;
}

export function buildOpenAgentsExternalRepoStudyReviewPolicyBinding(
  input: BuildOpenAgentsExternalRepoStudyReviewPolicyBindingInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyReviewPolicyBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const registry = input.policyRegistry;
    const req = input.reviewRequest;
    const policyRef = input.policyRef;

    yield* requireNonEmpty(
      req.repo,
      "externalRepoStudyReviewPolicyBinding.repo",
    );
    yield* requireNonEmpty(
      req.customerRef,
      "externalRepoStudyReviewPolicyBinding.customerRef",
    );

    if (req.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyReviewPolicyBinding.repo",
        "review policy binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    const policyPublished = isPublishedExternalRepoStudyPrivacyPolicyRef(
      registry,
      policyRef,
    );
    const matchedVersion = policyPublished
      ? (registry.publishedVersions.find(
          (version) => version.policyRef === policyRef,
        ) ?? null)
      : null;

    const bound = policyPublished && matchedVersion !== null;

    // The DPA + retention refs are derived ONLY from a published policy. The
    // caller never supplies them directly, so the review's DPA / retention gates
    // can only be satisfied by a known published policy.
    const dataProcessingAgreementRef = bound ? matchedVersion!.policyRef : null;
    const retentionPolicyRef = bound ? matchedVersion!.policyRef : null;

    const reviewPreflight =
      yield* buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        request: {
          ...req,
          dataProcessingAgreementRef: dataProcessingAgreementRef ?? undefined,
          retentionPolicyRef: retentionPolicyRef ?? undefined,
        },
        reviewerSignoffPresent: input.reviewerSignoffPresent ?? false,
        reviewFlagArmed: input.reviewFlagArmed ?? false,
      });

    const mismatchRefs = policyPublished
      ? []
      : [
          "blocker.external_repo_study_review_policy_binding.policy_ref_not_published",
        ];

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_review_policy_binding_hash";

    const evidenceRefs = [
      req.customerRef,
      registry.registryRef,
      reviewPreflight.preflightRef,
      ...(bound ? [matchedVersion!.policyRef, matchedVersion!.termsDigest] : []),
      "docs/legal/external-repo-studying-privacy-policy.v0.md",
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyReviewPolicyBinding = {
      bindingHash: "sha256:pending",
      bindingRef: "external_repo_study_review_policy_binding.pending",
      bound,
      customerPublicClaimAllowed: false,
      customerRef: req.customerRef,
      dataProcessingAgreementRef,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      marketplacePackageAllowed: false,
      mismatchRefs,
      payoutEligible: false,
      policyPublished,
      policyRef,
      policyRegistryRef: registry.registryRef,
      policyTermsDigest: bound ? matchedVersion!.termsDigest : null,
      repo: req.repo,
      retentionPolicyRef,
      reviewPreflight,
      reviewCleared: false,
      safeCopy:
        "Review policy binding cross-checked a privacy-review request against the published policy registry using refs and counts only. It derives the review's DPA and retention refs from a known published policy version; an unknown, forged, or empty policy ref binds no refs, so the review blocks on a missing DPA and retention policy. The binding is held inert; no customer data is processed, no review is cleared, no ingestion is authorized, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef: OPENAGENTS_EXTERNAL_REPO_STUDY_REVIEW_POLICY_BINDING_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: bound ? "bound_held" : "unbound",
      unsafeCopyRefs: [
        "blocked_claim.review_dpa_forgeable_string",
        "blocked_claim.customer_data_privacy_cleared_live",
        "blocked_claim.customer_repo_data_processing_authorized",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      wouldClearWhenArmed: reviewPreflight.wouldClearWhenArmed,
    };

    const bindingHash =
      openAgentsExternalRepoStudyReviewPolicyBindingHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyReviewPolicyBinding({
      ...base,
      bindingHash,
      bindingRef: `external_repo_study_review_policy_binding.${slugRepo(req.repo)}.${shortHash(bindingHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyReviewPolicyBinding(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyReviewPolicyBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyReviewPolicyBinding",
    );
    const binding = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyReviewPolicyBinding,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyReviewPolicyBinding",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyReviewPolicyBinding(binding);
    return binding;
  });
}

export function openAgentsExternalRepoStudyReviewPolicyBindingHash(
  binding: OpenAgentsExternalRepoStudyReviewPolicyBinding,
): string {
  const {
    bindingHash: _bindingHash,
    bindingRef: _bindingRef,
    generatedAt: _generatedAt,
    ...stable
  } = binding;
  return sha256Ref(stableJson(stable));
}

function validateExternalRepoStudyReviewPolicyBinding(
  binding: OpenAgentsExternalRepoStudyReviewPolicyBinding,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      binding.repo,
      "externalRepoStudyReviewPolicyBinding.repo",
    );
    yield* requireNonEmpty(
      binding.customerRef,
      "externalRepoStudyReviewPolicyBinding.customerRef",
    );
    yield* requireNonEmpty(
      binding.bindingRef,
      "externalRepoStudyReviewPolicyBinding.bindingRef",
    );
    yield* requireSha256(
      binding.bindingHash,
      "externalRepoStudyReviewPolicyBinding.bindingHash",
    );

    if (binding.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyReviewPolicyBinding.repo",
        "review policy binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    if (
      binding.customerPublicClaimAllowed !== false ||
      binding.marketplacePackageAllowed !== false ||
      binding.payoutEligible !== false ||
      binding.reviewCleared !== false ||
      binding.effectsApplied !== false
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewPolicyBinding.claimGates",
        "review policy binding must not clear a review or grant customer, marketplace, payout, or settlement claims",
      );
    }

    // The nested review preflight is the source of truth for inertness; assert
    // it never escalates a claim through the binding.
    if (
      binding.reviewPreflight.reviewCleared !== false ||
      binding.reviewPreflight.effectsApplied !== false
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewPolicyBinding.reviewPreflight",
        "nested review preflight must remain inert",
      );
    }

    if (binding.bound !== (binding.state === "bound_held")) {
      return yield* bindingError(
        "externalRepoStudyReviewPolicyBinding.bound",
        "bound flag must agree with the binding state",
      );
    }

    if (binding.state === "bound_held") {
      if (
        !binding.policyPublished ||
        binding.dataProcessingAgreementRef === null ||
        binding.retentionPolicyRef === null ||
        binding.policyTermsDigest === null ||
        !binding.reviewPreflight.dataProcessingAgreementPresent ||
        !binding.reviewPreflight.retentionPolicyPresent
      ) {
        return yield* bindingError(
          "externalRepoStudyReviewPolicyBinding.state",
          "bound_held requires a published policy, derived DPA + retention refs, a matched terms digest, and the review preflight to record them present",
        );
      }
      // The derived refs must point at the matched published policy.
      if (
        binding.dataProcessingAgreementRef !== binding.policyRef ||
        binding.retentionPolicyRef !== binding.policyRef
      ) {
        return yield* bindingError(
          "externalRepoStudyReviewPolicyBinding.derivedRefs",
          "derived DPA + retention refs must reference the bound published policy",
        );
      }
    }

    if (binding.state === "unbound") {
      if (
        binding.dataProcessingAgreementRef !== null ||
        binding.retentionPolicyRef !== null ||
        binding.policyTermsDigest !== null
      ) {
        return yield* bindingError(
          "externalRepoStudyReviewPolicyBinding.derivedRefs",
          "an unbound binding must not derive DPA / retention refs or a terms digest",
        );
      }
      // No forged string can have slipped through, because the binding controls
      // the DPA + retention refs: an unbound review cannot present them.
      if (
        binding.reviewPreflight.dataProcessingAgreementPresent ||
        binding.reviewPreflight.retentionPolicyPresent
      ) {
        return yield* bindingError(
          "externalRepoStudyReviewPolicyBinding.reviewPreflight",
          "an unbound binding must not leave the review DPA / retention gates satisfied",
        );
      }
    }

    if (
      binding.wouldClearWhenArmed &&
      binding.reviewPreflight.clearanceGate.state !== "armed_ready"
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewPolicyBinding.wouldClearWhenArmed",
        "would-clear-when-armed requires the nested review's armed gate to be ready",
      );
    }

    if (
      binding.bindingHash !==
      openAgentsExternalRepoStudyReviewPolicyBindingHash(binding)
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewPolicyBinding.bindingHash",
        "must match the deterministic binding hash",
      );
    }
  });
}

function requireNonEmpty(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? bindingError(path, "must be non-empty")
    : Effect.void;
}

function requireSha256(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:")
    ? Effect.void
    : bindingError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function bindingError(
  path: string,
  reason: string,
): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

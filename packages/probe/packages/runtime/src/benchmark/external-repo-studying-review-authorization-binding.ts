import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  isActiveCustomerAuthorizationRef,
  type OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
} from "./external-repo-studying-customer-authorization-registry";
import {
  buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  type ExternalRepoStudyPrivacyReviewRequest,
} from "./external-repo-studying-privacy-review";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Privacy-review <-> customer-AUTHORIZATION BINDING for the external-repo-studying
 * pilot.
 *
 * This module is the smallest genuine piece of the missing privacy POLICY control
 * surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_privacy_policy_missing): it
 * closes the LAST forgeable-string seam in the privacy-review preflight — the
 * lawful-basis `customerAuthorizationRef` — and is the exact next step the
 * customer-authorization registry update named.
 *
 * Section 3 ("Lawful basis and authorization") of the published privacy policy
 * (docs/legal/external-repo-studying-privacy-policy.v0.md) states OpenAgents
 * processes a customer's external repo ONLY with the customer's recorded
 * authorization, and Section 6 says the customer may WITHDRAW it. The
 * customer-authorization registry already exposes
 * `isActiveCustomerAuthorizationRef(...)` to verify a ref matches a KNOWN, ACTIVE
 * authorization for an exact (customerRef, repo). But nothing yet forced the
 * privacy-review preflight to consume only such a ref: the review preflight's
 * `customerAuthorizationPresent` gate was still a plain string-presence check, so
 * a forged, stale, or WITHDRAWN ref passed.
 *
 * This composer removes that forgeability, mirroring the review<->policy binding.
 * It derives the review's `customerAuthorizationRef` FROM a candidate ref that
 * `isActiveCustomerAuthorizationRef` verifies as a registry-known ACTIVE
 * authorization covering the SAME customer + repo, then builds the review
 * preflight from it. An unknown / withdrawn / mismatched / empty ref binds NO
 * ref, so the review blocks on
 * `blocker.external_repo_study_privacy_review.customer_authorization_missing`
 * instead of trusting a string. The caller cannot inject its own authorization
 * ref (it is Omitted from the request type).
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/counts/enums ONLY. sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: `reviewCleared` and `effectsApplied` are ALWAYS false
 *    (inherited from the nested review preflight and asserted here). The binding
 *    decides only WHETHER the review's authorization gate is genuinely backed by
 *    an active recorded authorization; it never clears a review, ingests, stores
 *    bytes, delivers a packet, sends, settles, or spends.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires a real, human/legal review against
 * a real customer study, durable access-controlled storage, real revocation
 * enforcement, and an owner-signed armed clearance with a dereferenceable closeout
 * receipt per proof.claim_upgrade_receipts.v1 — all owner-gated and out of scope
 * for the pure binding built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_REVIEW_AUTHORIZATION_BINDING_SCHEMA_REF =
  "openagents.external_repo_study_review_authorization_binding.v0" as const;

export const OpenAgentsExternalRepoStudyReviewAuthorizationBindingState =
  S.Literals([
    // The review's authorization ref is derived from a known active authorization.
    "bound_held",
    // The candidate ref is unknown / withdrawn / mismatched / empty; none bound.
    "unbound",
  ]);
export type OpenAgentsExternalRepoStudyReviewAuthorizationBindingState =
  typeof OpenAgentsExternalRepoStudyReviewAuthorizationBindingState.Type;

export const OpenAgentsExternalRepoStudyReviewAuthorizationBinding = S.Struct({
  // Whether the candidate ref verified as a known ACTIVE authorization.
  authorizationActive: S.Boolean,
  // The candidate authorization ref supplied by the caller, recorded verbatim.
  authorizationCandidateRef: S.String,
  // The authorization ref derived from the active authorization, or null.
  authorizationRef: S.NullOr(S.String),
  authorizationRegistryRef: S.String,
  bindingHash: S.String,
  bindingRef: S.String,
  bound: S.Boolean,
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  mismatchRefs: S.Array(S.String),
  payoutEligible: S.Literal(false),
  repo: S.String,
  // The full review preflight built from the (only-if-bound) derived ref.
  reviewPreflight: OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  reviewCleared: S.Literal(false),
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_REVIEW_AUTHORIZATION_BINDING_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyReviewAuthorizationBindingState,
  unsafeCopyRefs: S.Array(S.String),
  wouldClearWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyReviewAuthorizationBinding =
  typeof OpenAgentsExternalRepoStudyReviewAuthorizationBinding.Type;

export interface BuildOpenAgentsExternalRepoStudyReviewAuthorizationBindingInput {
  /** The customer-authorization registry the candidate ref is checked against. */
  readonly authorizationRegistry: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry;
  /**
   * The authorization ref the study claims is its lawful basis. It must match a
   * KNOWN, ACTIVE authorization in the registry for the request's customer+repo;
   * otherwise no authorization ref is derived.
   */
  readonly authorizationCandidateRef: string;
  readonly generatedAt?: string;
  /**
   * The privacy-review request, MINUS its customerAuthorizationRef. The binding
   * derives that from the registry; callers cannot inject their own.
   */
  readonly reviewRequest: Omit<
    ExternalRepoStudyPrivacyReviewRequest,
    "customerAuthorizationRef"
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

export function buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding(
  input: BuildOpenAgentsExternalRepoStudyReviewAuthorizationBindingInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyReviewAuthorizationBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const registry = input.authorizationRegistry;
    const req = input.reviewRequest;
    const candidateRef = input.authorizationCandidateRef;

    yield* requireNonEmpty(
      req.repo,
      "externalRepoStudyReviewAuthorizationBinding.repo",
    );
    yield* requireNonEmpty(
      req.customerRef,
      "externalRepoStudyReviewAuthorizationBinding.customerRef",
    );

    if (req.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyReviewAuthorizationBinding.repo",
        "review authorization binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    const authorizationActive = isActiveCustomerAuthorizationRef(
      registry,
      candidateRef,
      { customerRef: req.customerRef, repo: req.repo },
    );

    const matched = authorizationActive
      ? (registry.recordedAuthorizations.find(
          (authorization) => authorization.authorizationRef === candidateRef,
        ) ?? null)
      : null;

    const bound = authorizationActive && matched !== null;

    // The authorization ref is derived ONLY from a known active authorization.
    // The caller never supplies it directly, so the review's authorization gate
    // can only be satisfied by a registry-verified active authorization.
    const authorizationRef = bound ? matched!.authorizationRef : null;

    const reviewPreflight =
      yield* buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        request: {
          ...req,
          customerAuthorizationRef: authorizationRef ?? undefined,
        },
        reviewerSignoffPresent: input.reviewerSignoffPresent ?? false,
        reviewFlagArmed: input.reviewFlagArmed ?? false,
      });

    const mismatchRefs = authorizationActive
      ? []
      : [
          "blocker.external_repo_study_review_authorization_binding.authorization_not_active",
        ];

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_review_authorization_binding_hash";

    const evidenceRefs = [
      req.customerRef,
      registry.registryRef,
      reviewPreflight.preflightRef,
      ...(bound
        ? [matched!.authorizationRef, matched!.authorizationDigest]
        : []),
      "docs/legal/external-repo-studying-privacy-policy.v0.md",
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyReviewAuthorizationBinding = {
      authorizationActive,
      authorizationCandidateRef: candidateRef,
      authorizationRef,
      authorizationRegistryRef: registry.registryRef,
      bindingHash: "sha256:pending",
      bindingRef: "external_repo_study_review_authorization_binding.pending",
      bound,
      customerPublicClaimAllowed: false,
      customerRef: req.customerRef,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      marketplacePackageAllowed: false,
      mismatchRefs,
      payoutEligible: false,
      repo: req.repo,
      reviewPreflight,
      reviewCleared: false,
      safeCopy:
        "Review authorization binding cross-checked a privacy-review request against the customer-authorization registry using refs and counts only. It derives the review's customer-authorization ref from a known active authorization for the same customer and repo; an unknown, withdrawn, mismatched, or empty ref binds nothing, so the review blocks on a missing customer authorization. The binding is held inert; no customer data is processed, no review is cleared, no ingestion is authorized, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef:
        OPENAGENTS_EXTERNAL_REPO_STUDY_REVIEW_AUTHORIZATION_BINDING_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: bound ? "bound_held" : "unbound",
      unsafeCopyRefs: [
        "blocked_claim.review_authorization_forgeable_string",
        "blocked_claim.customer_data_privacy_cleared_live",
        "blocked_claim.customer_repo_data_processing_authorized",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      wouldClearWhenArmed: reviewPreflight.wouldClearWhenArmed,
    };

    const bindingHash =
      openAgentsExternalRepoStudyReviewAuthorizationBindingHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyReviewAuthorizationBinding({
      ...base,
      bindingHash,
      bindingRef: `external_repo_study_review_authorization_binding.${slugRepo(req.repo)}.${shortHash(bindingHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyReviewAuthorizationBinding(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyReviewAuthorizationBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyReviewAuthorizationBinding",
    );
    const binding = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyReviewAuthorizationBinding,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyReviewAuthorizationBinding",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyReviewAuthorizationBinding(binding);
    return binding;
  });
}

export function openAgentsExternalRepoStudyReviewAuthorizationBindingHash(
  binding: OpenAgentsExternalRepoStudyReviewAuthorizationBinding,
): string {
  const {
    bindingHash: _bindingHash,
    bindingRef: _bindingRef,
    generatedAt: _generatedAt,
    ...stable
  } = binding;
  return sha256Ref(stableJson(stable));
}

function validateExternalRepoStudyReviewAuthorizationBinding(
  binding: OpenAgentsExternalRepoStudyReviewAuthorizationBinding,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      binding.repo,
      "externalRepoStudyReviewAuthorizationBinding.repo",
    );
    yield* requireNonEmpty(
      binding.customerRef,
      "externalRepoStudyReviewAuthorizationBinding.customerRef",
    );
    yield* requireNonEmpty(
      binding.bindingRef,
      "externalRepoStudyReviewAuthorizationBinding.bindingRef",
    );
    yield* requireSha256(
      binding.bindingHash,
      "externalRepoStudyReviewAuthorizationBinding.bindingHash",
    );

    if (binding.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyReviewAuthorizationBinding.repo",
        "review authorization binding target must be an external (non-OpenAgents) pilot repo",
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
        "externalRepoStudyReviewAuthorizationBinding.claimGates",
        "review authorization binding must not clear a review or grant customer, marketplace, payout, or settlement claims",
      );
    }

    // The nested review preflight is the source of truth for inertness; assert
    // it never escalates a claim through the binding.
    if (
      binding.reviewPreflight.reviewCleared !== false ||
      binding.reviewPreflight.effectsApplied !== false
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewAuthorizationBinding.reviewPreflight",
        "nested review preflight must remain inert",
      );
    }

    if (binding.bound !== (binding.state === "bound_held")) {
      return yield* bindingError(
        "externalRepoStudyReviewAuthorizationBinding.bound",
        "bound flag must agree with the binding state",
      );
    }

    if (binding.state === "bound_held") {
      if (
        !binding.authorizationActive ||
        binding.authorizationRef === null ||
        !binding.reviewPreflight.customerAuthorizationPresent
      ) {
        return yield* bindingError(
          "externalRepoStudyReviewAuthorizationBinding.state",
          "bound_held requires an active authorization, a derived authorization ref, and the review preflight to record it present",
        );
      }
      // The derived ref must be exactly the candidate ref that verified.
      if (binding.authorizationRef !== binding.authorizationCandidateRef) {
        return yield* bindingError(
          "externalRepoStudyReviewAuthorizationBinding.derivedRef",
          "derived authorization ref must reference the verified candidate authorization",
        );
      }
    }

    if (binding.state === "unbound") {
      if (binding.authorizationRef !== null) {
        return yield* bindingError(
          "externalRepoStudyReviewAuthorizationBinding.derivedRef",
          "an unbound binding must not derive an authorization ref",
        );
      }
      // No forged string can have slipped through, because the binding controls
      // the authorization ref: an unbound review cannot present it.
      if (binding.reviewPreflight.customerAuthorizationPresent) {
        return yield* bindingError(
          "externalRepoStudyReviewAuthorizationBinding.reviewPreflight",
          "an unbound binding must not leave the review authorization gate satisfied",
        );
      }
    }

    if (
      binding.wouldClearWhenArmed &&
      binding.reviewPreflight.clearanceGate.state !== "armed_ready"
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewAuthorizationBinding.wouldClearWhenArmed",
        "would-clear-when-armed requires the nested review's armed gate to be ready",
      );
    }

    if (
      binding.bindingHash !==
      openAgentsExternalRepoStudyReviewAuthorizationBindingHash(binding)
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewAuthorizationBinding.bindingHash",
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

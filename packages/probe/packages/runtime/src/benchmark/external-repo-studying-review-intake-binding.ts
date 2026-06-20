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
 * Combined privacy-review INTAKE binding for the external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing privacy POLICY
 * control surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_privacy_policy_missing): it
 * composes the two existing sibling review bindings into ONE step that closes
 * ALL THREE forgeable-string seams of the privacy-review preflight at the same
 * time.
 *
 * The privacy-review preflight has three refs-only gates: a DPA ref, a
 * retention-policy ref (both governed by the published policy registry), and a
 * lawful-basis `customerAuthorizationRef` (governed by the customer-authorization
 * registry). The existing `review<->policy` binding derives ONLY the DPA +
 * retention refs (and still lets a caller pass an arbitrary
 * customerAuthorizationRef), and the existing `review<->authorization` binding
 * derives ONLY the customerAuthorizationRef (and still lets a caller pass
 * arbitrary DPA / retention refs). So a single caller composing them by hand
 * could STILL forge whichever ref the chosen binding left open. Nothing yet
 * derives ALL THREE refs in one place, leaving no forgeable input on the review
 * at all — exactly the residual seam the upload<->intake binding closed for the
 * self-serve upload preflight's two gates.
 *
 * This composer removes that residual forgeability: it takes the review request
 * with all three refs (`dataProcessingAgreementRef`, `retentionPolicyRef`,
 * `customerAuthorizationRef`) omitted from its input type, derives the DPA +
 * retention refs FROM a policy ref the registry verifies as a KNOWN published
 * version, derives the customerAuthorizationRef FROM a candidate the
 * authorization registry verifies as a KNOWN, ACTIVE authorization for the SAME
 * customer + repo, and only then builds the review preflight. The review reaches
 * `review_ready_held` only when BOTH sources are genuinely backed; if either is
 * missing / forged / withdrawn / mismatched, NO ref is injected for that gate,
 * so the review blocks on the corresponding missing-ref blocker instead of
 * trusting an arbitrary string.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/counts/enums ONLY. sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: `reviewCleared` and `effectsApplied` are ALWAYS false
 *    (inherited from the nested review preflight and asserted here). The binding
 *    decides only WHETHER all three of the review's gates are genuinely backed by
 *    a published policy and an active authorization; it never clears a review,
 *    ingests, stores bytes, delivers a packet, sends, settles, or spends.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires a real, human/legal review against
 * a real customer study, durable access-controlled storage that enforces the
 * declared retention window and real revocation, and an owner-signed armed
 * clearance with a dereferenceable closeout receipt per
 * proof.claim_upgrade_receipts.v1 — all owner-gated and out of scope for the pure
 * binding built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_REVIEW_INTAKE_BINDING_SCHEMA_REF =
  "openagents.external_repo_study_review_intake_binding.v0" as const;

export const OpenAgentsExternalRepoStudyReviewIntakeBindingState = S.Literals([
  // ALL THREE review gates are genuinely backed: a known published policy
  // (DPA + retention) AND a registry-known active authorization for the same
  // customer + repo.
  "bound_held",
  // At least one source is not genuinely backed; that gate binds no ref.
  "unbound",
]);
export type OpenAgentsExternalRepoStudyReviewIntakeBindingState =
  typeof OpenAgentsExternalRepoStudyReviewIntakeBindingState.Type;

export const OpenAgentsExternalRepoStudyReviewIntakeBinding = S.Struct({
  // Whether the candidate authorization ref verified as a known ACTIVE grant.
  authorizationActive: S.Boolean,
  // The candidate authorization ref the caller supplied for verification.
  authorizationCandidateRef: S.String,
  authorizationRegistryRef: S.String,
  bindingHash: S.String,
  bindingRef: S.String,
  bound: S.Boolean,
  // The lawful-basis ref derived from the active authorization, or null.
  customerAuthorizationRef: S.NullOr(S.String),
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
    OPENAGENTS_EXTERNAL_REPO_STUDY_REVIEW_INTAKE_BINDING_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyReviewIntakeBindingState,
  unsafeCopyRefs: S.Array(S.String),
  wouldClearWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyReviewIntakeBinding =
  typeof OpenAgentsExternalRepoStudyReviewIntakeBinding.Type;

export interface BuildOpenAgentsExternalRepoStudyReviewIntakeBindingInput {
  /** The customer-authorization registry the candidate ref is checked against. */
  readonly authorizationRegistry: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry;
  /**
   * The authorization ref the study claims is its lawful basis. It must match a
   * KNOWN, ACTIVE authorization in the registry for the request's customer+repo;
   * otherwise no authorization ref is derived.
   */
  readonly authorizationCandidateRef: string;
  readonly generatedAt?: string;
  /** The published policy registry the policy ref is checked against. */
  readonly policyRegistry: OpenAgentsExternalRepoStudyPrivacyPolicyRegistry;
  /**
   * The policy ref the study claims governs it. It must match a KNOWN published
   * version in the registry; otherwise no DPA / retention ref is derived.
   */
  readonly policyRef: string;
  /**
   * The privacy-review request, MINUS its dataProcessingAgreementRef,
   * retentionPolicyRef, and customerAuthorizationRef. The binding derives ALL
   * THREE; callers cannot inject any of their own.
   */
  readonly reviewRequest: Omit<
    ExternalRepoStudyPrivacyReviewRequest,
    "dataProcessingAgreementRef" | "retentionPolicyRef" | "customerAuthorizationRef"
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

export function buildOpenAgentsExternalRepoStudyReviewIntakeBinding(
  input: BuildOpenAgentsExternalRepoStudyReviewIntakeBindingInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyReviewIntakeBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const policyRegistry = input.policyRegistry;
    const authorizationRegistry = input.authorizationRegistry;
    const req = input.reviewRequest;
    const policyRef = input.policyRef;
    const candidateRef = input.authorizationCandidateRef;

    yield* requireNonEmpty(req.repo, "externalRepoStudyReviewIntakeBinding.repo");
    yield* requireNonEmpty(
      req.customerRef,
      "externalRepoStudyReviewIntakeBinding.customerRef",
    );

    if (req.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyReviewIntakeBinding.repo",
        "review intake binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    // --- Policy gate: derive DPA + retention refs from a published policy. ---
    const policyPublished = isPublishedExternalRepoStudyPrivacyPolicyRef(
      policyRegistry,
      policyRef,
    );
    const matchedVersion = policyPublished
      ? (policyRegistry.publishedVersions.find(
          (version) => version.policyRef === policyRef,
        ) ?? null)
      : null;
    const policyBound = policyPublished && matchedVersion !== null;
    const dataProcessingAgreementRef = policyBound
      ? matchedVersion!.policyRef
      : null;
    const retentionPolicyRef = policyBound ? matchedVersion!.policyRef : null;

    // --- Authorization gate: derive the lawful-basis ref from an active grant. ---
    const authorizationActive = isActiveCustomerAuthorizationRef(
      authorizationRegistry,
      candidateRef,
      { customerRef: req.customerRef, repo: req.repo },
    );
    const matchedAuthorization = authorizationActive
      ? (authorizationRegistry.recordedAuthorizations.find(
          (authorization) => authorization.authorizationRef === candidateRef,
        ) ?? null)
      : null;
    const authorizationBound = authorizationActive && matchedAuthorization !== null;
    const customerAuthorizationRef = authorizationBound
      ? matchedAuthorization!.authorizationRef
      : null;

    // The whole review is bound only when BOTH sources are genuinely backed.
    const bound = policyBound && authorizationBound;

    // The review preflight is ALWAYS built from the derived refs (or none). The
    // caller supplies none of the three refs directly, so each gate can only be
    // satisfied by a published policy / registry-verified active authorization.
    const reviewPreflight =
      yield* buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        request: {
          ...req,
          dataProcessingAgreementRef: dataProcessingAgreementRef ?? undefined,
          retentionPolicyRef: retentionPolicyRef ?? undefined,
          customerAuthorizationRef: customerAuthorizationRef ?? undefined,
        },
        reviewerSignoffPresent: input.reviewerSignoffPresent ?? false,
        reviewFlagArmed: input.reviewFlagArmed ?? false,
      });

    const mismatchRefs = buildMismatchRefs({
      authorizationActive,
      policyPublished,
    });

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_review_intake_binding_hash";

    const evidenceRefs = [
      req.customerRef,
      policyRegistry.registryRef,
      authorizationRegistry.registryRef,
      reviewPreflight.preflightRef,
      ...(policyBound
        ? [matchedVersion!.policyRef, matchedVersion!.termsDigest]
        : []),
      ...(authorizationBound
        ? [
            matchedAuthorization!.authorizationRef,
            matchedAuthorization!.authorizationDigest,
          ]
        : []),
      "docs/legal/external-repo-studying-privacy-policy.v0.md",
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyReviewIntakeBinding = {
      authorizationActive,
      authorizationCandidateRef: candidateRef,
      authorizationRegistryRef: authorizationRegistry.registryRef,
      bindingHash: "sha256:pending",
      bindingRef: "external_repo_study_review_intake_binding.pending",
      bound,
      customerAuthorizationRef,
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
      policyRegistryRef: policyRegistry.registryRef,
      policyTermsDigest: policyBound ? matchedVersion!.termsDigest : null,
      repo: req.repo,
      retentionPolicyRef,
      reviewPreflight,
      reviewCleared: false,
      safeCopy:
        "Review intake binding cross-checked a privacy-review request against BOTH the published policy registry and the customer-authorization registry using refs and counts only. It derives the review's DPA and retention refs from a known published policy version and its lawful-basis customer-authorization ref from a known active authorization for the same customer and repo; an unknown, forged, withdrawn, mismatched, or empty source binds no ref for that gate, so the review blocks on the corresponding missing-ref blocker. The binding is held inert; no customer data is processed, no review is cleared, no ingestion is authorized, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef: OPENAGENTS_EXTERNAL_REPO_STUDY_REVIEW_INTAKE_BINDING_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: bound ? "bound_held" : "unbound",
      unsafeCopyRefs: [
        "blocked_claim.review_dpa_forgeable_string",
        "blocked_claim.review_authorization_forgeable_string",
        "blocked_claim.customer_data_privacy_cleared_live",
        "blocked_claim.customer_repo_data_processing_authorized",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      wouldClearWhenArmed: reviewPreflight.wouldClearWhenArmed,
    };

    const bindingHash =
      openAgentsExternalRepoStudyReviewIntakeBindingHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyReviewIntakeBinding({
      ...base,
      bindingHash,
      bindingRef: `external_repo_study_review_intake_binding.${slugRepo(req.repo)}.${shortHash(bindingHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyReviewIntakeBinding(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyReviewIntakeBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyReviewIntakeBinding",
    );
    const binding = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyReviewIntakeBinding,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyReviewIntakeBinding",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyReviewIntakeBinding(binding);
    return binding;
  });
}

export function openAgentsExternalRepoStudyReviewIntakeBindingHash(
  binding: OpenAgentsExternalRepoStudyReviewIntakeBinding,
): string {
  const {
    bindingHash: _bindingHash,
    bindingRef: _bindingRef,
    generatedAt: _generatedAt,
    ...stable
  } = binding;
  return sha256Ref(stableJson(stable));
}

function buildMismatchRefs(input: {
  readonly authorizationActive: boolean;
  readonly policyPublished: boolean;
}): ReadonlyArray<string> {
  const mismatches: string[] = [];
  if (!input.policyPublished) {
    mismatches.push(
      "blocker.external_repo_study_review_intake_binding.policy_ref_not_published",
    );
  }
  if (!input.authorizationActive) {
    mismatches.push(
      "blocker.external_repo_study_review_intake_binding.authorization_not_active",
    );
  }
  return mismatches;
}

function validateExternalRepoStudyReviewIntakeBinding(
  binding: OpenAgentsExternalRepoStudyReviewIntakeBinding,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      binding.repo,
      "externalRepoStudyReviewIntakeBinding.repo",
    );
    yield* requireNonEmpty(
      binding.customerRef,
      "externalRepoStudyReviewIntakeBinding.customerRef",
    );
    yield* requireNonEmpty(
      binding.bindingRef,
      "externalRepoStudyReviewIntakeBinding.bindingRef",
    );
    yield* requireSha256(
      binding.bindingHash,
      "externalRepoStudyReviewIntakeBinding.bindingHash",
    );

    if (binding.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyReviewIntakeBinding.repo",
        "review intake binding target must be an external (non-OpenAgents) pilot repo",
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
        "externalRepoStudyReviewIntakeBinding.claimGates",
        "review intake binding must not clear a review or grant customer, marketplace, payout, or settlement claims",
      );
    }

    // The nested review preflight is the source of truth for inertness; assert
    // it never escalates a claim through the binding.
    if (
      binding.reviewPreflight.reviewCleared !== false ||
      binding.reviewPreflight.effectsApplied !== false
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewIntakeBinding.reviewPreflight",
        "nested review preflight must remain inert",
      );
    }

    const allGatesBacked = binding.policyPublished && binding.authorizationActive;

    if (binding.state === "bound_held") {
      if (
        !allGatesBacked ||
        binding.dataProcessingAgreementRef === null ||
        binding.retentionPolicyRef === null ||
        binding.policyTermsDigest === null ||
        binding.customerAuthorizationRef === null ||
        binding.customerAuthorizationRef !== binding.authorizationCandidateRef ||
        !binding.reviewPreflight.dataProcessingAgreementPresent ||
        !binding.reviewPreflight.retentionPolicyPresent ||
        !binding.reviewPreflight.customerAuthorizationPresent
      ) {
        return yield* bindingError(
          "externalRepoStudyReviewIntakeBinding.state",
          "bound_held requires a published policy with derived DPA + retention refs and a matched terms digest, a registry-verified active authorization with a derived customerAuthorizationRef equal to the candidate, and the review preflight to record all three present",
        );
      }
      // The derived policy refs must point at the matched published policy.
      if (
        binding.dataProcessingAgreementRef !== binding.policyRef ||
        binding.retentionPolicyRef !== binding.policyRef
      ) {
        return yield* bindingError(
          "externalRepoStudyReviewIntakeBinding.derivedRefs",
          "derived DPA + retention refs must reference the bound published policy",
        );
      }
    }

    // Each gate independently controls its own refs: a source that is not
    // genuinely backed must derive no ref and must not leave the review's gate
    // satisfied.
    if (!binding.policyPublished) {
      if (
        binding.dataProcessingAgreementRef !== null ||
        binding.retentionPolicyRef !== null ||
        binding.policyTermsDigest !== null
      ) {
        return yield* bindingError(
          "externalRepoStudyReviewIntakeBinding.derivedRefs",
          "an unpublished policy must not derive DPA / retention refs or a terms digest",
        );
      }
      if (
        binding.reviewPreflight.dataProcessingAgreementPresent ||
        binding.reviewPreflight.retentionPolicyPresent
      ) {
        return yield* bindingError(
          "externalRepoStudyReviewIntakeBinding.reviewPreflight.policy",
          "an unpublished policy must not leave the review DPA / retention gates satisfied",
        );
      }
    }
    if (!binding.authorizationActive) {
      if (binding.customerAuthorizationRef !== null) {
        return yield* bindingError(
          "externalRepoStudyReviewIntakeBinding.customerAuthorizationRef",
          "an inactive / unknown authorization must not derive a customerAuthorizationRef",
        );
      }
      if (binding.reviewPreflight.customerAuthorizationPresent) {
        return yield* bindingError(
          "externalRepoStudyReviewIntakeBinding.reviewPreflight.authorization",
          "an inactive / unknown authorization must not leave the review authorization gate satisfied",
        );
      }
    }

    if (binding.state === "unbound" && allGatesBacked) {
      return yield* bindingError(
        "externalRepoStudyReviewIntakeBinding.state",
        "an unbound binding must have at least one source not genuinely backed",
      );
    }

    if (binding.bound !== (binding.state === "bound_held")) {
      return yield* bindingError(
        "externalRepoStudyReviewIntakeBinding.bound",
        "bound flag must agree with the binding state",
      );
    }

    if (binding.bound !== allGatesBacked) {
      return yield* bindingError(
        "externalRepoStudyReviewIntakeBinding.bound",
        "bound flag must agree with both sources being genuinely backed",
      );
    }

    if (
      binding.wouldClearWhenArmed &&
      binding.reviewPreflight.clearanceGate.state !== "armed_ready"
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewIntakeBinding.wouldClearWhenArmed",
        "would-clear-when-armed requires the nested review's armed gate to be ready",
      );
    }

    if (
      binding.bindingHash !==
      openAgentsExternalRepoStudyReviewIntakeBindingHash(binding)
    ) {
      return yield* bindingError(
        "externalRepoStudyReviewIntakeBinding.bindingHash",
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

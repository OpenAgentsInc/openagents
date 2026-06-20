import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { type OpenAgentsExternalRepoStudyPrivacyReviewPreflight } from "./external-repo-studying-privacy-review";
import {
  buildOpenAgentsExternalRepoStudySelfServeUploadPreflight,
  OpenAgentsExternalRepoStudySelfServeUploadPreflight,
  type ExternalRepoStudySelfServeUploadRequest,
} from "./external-repo-studying-self-serve-upload";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Upload <-> privacy-review BINDING for the external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing self-serve upload
 * control surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_self_serve_upload_missing):
 * it closes the seam between the two sibling preflights.
 *
 * Today the self-serve upload preflight's privacy-review gate is only a STRING
 * PRESENCE check: any non-empty `privacyReviewRef` satisfies it. The privacy
 * review preflight already derives a real `privacyReviewRef` ONLY when a review
 * would clear, but nothing forces the upload to consume THAT ref. So a forged
 * or stale string could pass the upload's privacy gate.
 *
 * This composer removes that forgeability: it derives the upload's
 * `privacyReviewRef` FROM a privacy-review preflight that (a) covers the SAME
 * customer + repo and (b) is in `review_ready_held` (so it derived a non-null
 * ref). The upload preflight is then built from the derived ref. When the
 * review is blocked or mismatched, NO ref is injected, so the upload blocks on
 * `privacy_review_missing` instead of trusting an arbitrary string.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/digests/counts ONLY. sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: `intakeAdmitted`, `ingested`, and `effectsApplied`
 *    are ALWAYS false (inherited from the nested upload preflight and asserted
 *    here). The binding decides only WHETHER the upload's privacy gate is
 *    genuinely backed by a cleared review; it never ingests, stores bytes,
 *    delivers a packet, sends, settles, or spends.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires a real customer-data privacy
 * review backing an armed clearance, real malware/secret-scan execution, real
 * durable-storage controls, an armed ingestion against a real customer repo with
 * a dereferenceable closeout receipt, marketplace metering, pricing, payout
 * eligibility, settlement, and owner sign-off per proof.claim_upgrade_receipts.v1
 * — all owner-gated and out of scope for the pure binding built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_UPLOAD_PRIVACY_BINDING_SCHEMA_REF =
  "openagents.external_repo_study_upload_privacy_binding.v0" as const;

export const OpenAgentsExternalRepoStudyUploadPrivacyBindingState = S.Literals([
  // Upload privacy gate is genuinely backed by a cleared, matching review.
  "bound_held",
  // Review is blocked or does not match the upload's customer/repo; no ref bound.
  "unbound",
]);
export type OpenAgentsExternalRepoStudyUploadPrivacyBindingState =
  typeof OpenAgentsExternalRepoStudyUploadPrivacyBindingState.Type;

export const OpenAgentsExternalRepoStudyUploadPrivacyBinding = S.Struct({
  bindingHash: S.String,
  bindingRef: S.String,
  bound: S.Boolean,
  customerMatches: S.Boolean,
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  ingested: S.Literal(false),
  intakeAdmitted: S.Literal(false),
  marketplacePackageAllowed: S.Literal(false),
  mismatchRefs: S.Array(S.String),
  payoutEligible: S.Literal(false),
  privacyReviewCleared: S.Boolean,
  privacyReviewPreflightRef: S.String,
  // The bound ref derived from the cleared review, or null when unbound.
  privacyReviewRef: S.NullOr(S.String),
  repo: S.String,
  repoMatches: S.Boolean,
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_UPLOAD_PRIVACY_BINDING_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyUploadPrivacyBindingState,
  unsafeCopyRefs: S.Array(S.String),
  // The full upload preflight built from the (only-if-bound) derived ref.
  uploadPreflight: OpenAgentsExternalRepoStudySelfServeUploadPreflight,
  wouldIngestWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyUploadPrivacyBinding =
  typeof OpenAgentsExternalRepoStudyUploadPrivacyBinding.Type;

export interface BuildOpenAgentsExternalRepoStudyUploadPrivacyBindingInput {
  readonly generatedAt?: string;
  /**
   * Whether an owner sign-off (per proof.claim_upgrade_receipts.v1) is recorded
   * for an armed upload. Forwarded to the nested upload preflight. Default false.
   */
  readonly ownerSignoffPresent?: boolean;
  /** A privacy-review preflight verdict for the same customer + repo. */
  readonly privacyReviewPreflight: OpenAgentsExternalRepoStudyPrivacyReviewPreflight;
  /**
   * Whether the upload flag is armed. Forwarded to the nested upload preflight.
   * Even when armed, no real effect is applied. Default false.
   */
  readonly uploadFlagArmed?: boolean;
  /**
   * The self-serve upload request, MINUS its privacyReviewRef. The binding
   * derives the ref from the cleared review; callers cannot inject their own.
   */
  readonly uploadRequest: Omit<
    ExternalRepoStudySelfServeUploadRequest,
    "privacyReviewRef"
  >;
}

export function buildOpenAgentsExternalRepoStudyUploadPrivacyBinding(
  input: BuildOpenAgentsExternalRepoStudyUploadPrivacyBindingInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyUploadPrivacyBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const review = input.privacyReviewPreflight;
    const req = input.uploadRequest;

    yield* requireNonEmpty(
      req.repo,
      "externalRepoStudyUploadPrivacyBinding.repo",
    );
    yield* requireNonEmpty(
      req.customerRef,
      "externalRepoStudyUploadPrivacyBinding.customerRef",
    );

    if (req.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyUploadPrivacyBinding.repo",
        "upload privacy binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    const customerMatches =
      req.customerRef.trim().length > 0 &&
      req.customerRef === review.customerRef;
    const repoMatches = req.repo.trim().length > 0 && req.repo === review.repo;
    const privacyReviewCleared =
      review.state === "review_ready_held" && review.privacyReviewRef !== null;

    const bound = customerMatches && repoMatches && privacyReviewCleared;
    const boundPrivacyReviewRef = bound ? review.privacyReviewRef : null;

    // The upload preflight is ALWAYS built from the derived ref (or none). The
    // caller never supplies a privacyReviewRef directly, so the upload's privacy
    // gate can only be satisfied by a cleared, matching review.
    const uploadPreflight =
      yield* buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        ownerSignoffPresent: input.ownerSignoffPresent ?? false,
        request: {
          ...req,
          privacyReviewRef: boundPrivacyReviewRef ?? undefined,
        },
        uploadFlagArmed: input.uploadFlagArmed ?? false,
      });

    const mismatchRefs = buildMismatchRefs({
      customerMatches,
      privacyReviewCleared,
      repoMatches,
    });

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_upload_privacy_binding_hash";

    const evidenceRefs = [
      req.customerRef,
      req.uploaderRef,
      review.preflightRef,
      uploadPreflight.preflightRef,
      ...(boundPrivacyReviewRef ? [boundPrivacyReviewRef] : []),
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyUploadPrivacyBinding = {
      bindingHash: "sha256:pending",
      bindingRef: "external_repo_study_upload_privacy_binding.pending",
      bound,
      customerMatches,
      customerPublicClaimAllowed: false,
      customerRef: req.customerRef,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      ingested: false,
      intakeAdmitted: false,
      marketplacePackageAllowed: false,
      mismatchRefs,
      payoutEligible: false,
      privacyReviewCleared,
      privacyReviewPreflightRef: review.preflightRef,
      privacyReviewRef: boundPrivacyReviewRef,
      repo: req.repo,
      repoMatches,
      safeCopy:
        "Upload privacy binding cross-checked a self-serve upload request against a privacy-review preflight using refs and counts only. It derives the upload's privacy-review ref from a cleared, customer/repo-matched review; a blocked or mismatched review binds no ref, so the upload blocks on a missing privacy review. The binding is held inert; no repo content is uploaded, stored, or ingested, no review is cleared, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef: OPENAGENTS_EXTERNAL_REPO_STUDY_UPLOAD_PRIVACY_BINDING_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: bound ? "bound_held" : "unbound",
      unsafeCopyRefs: [
        "blocked_claim.upload_privacy_review_forgeable_string",
        "blocked_claim.customer_repo_upload_live",
        "blocked_claim.self_serve_customer_repo_ingestion_live",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      uploadPreflight,
      wouldIngestWhenArmed: uploadPreflight.wouldIngestWhenArmed,
    };

    const bindingHash =
      openAgentsExternalRepoStudyUploadPrivacyBindingHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyUploadPrivacyBinding({
      ...base,
      bindingHash,
      bindingRef: `external_repo_study_upload_privacy_binding.${slugRepo(req.repo)}.${shortHash(bindingHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyUploadPrivacyBinding(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyUploadPrivacyBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyUploadPrivacyBinding",
    );
    const binding = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyUploadPrivacyBinding,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyUploadPrivacyBinding",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyUploadPrivacyBinding(binding);
    return binding;
  });
}

export function openAgentsExternalRepoStudyUploadPrivacyBindingHash(
  binding: OpenAgentsExternalRepoStudyUploadPrivacyBinding,
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
  readonly customerMatches: boolean;
  readonly privacyReviewCleared: boolean;
  readonly repoMatches: boolean;
}): ReadonlyArray<string> {
  const mismatches: string[] = [];
  if (!input.customerMatches) {
    mismatches.push(
      "blocker.external_repo_study_upload_privacy_binding.customer_ref_mismatch",
    );
  }
  if (!input.repoMatches) {
    mismatches.push(
      "blocker.external_repo_study_upload_privacy_binding.repo_mismatch",
    );
  }
  if (!input.privacyReviewCleared) {
    mismatches.push(
      "blocker.external_repo_study_upload_privacy_binding.privacy_review_not_cleared",
    );
  }
  return mismatches;
}

function validateExternalRepoStudyUploadPrivacyBinding(
  binding: OpenAgentsExternalRepoStudyUploadPrivacyBinding,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      binding.repo,
      "externalRepoStudyUploadPrivacyBinding.repo",
    );
    yield* requireNonEmpty(
      binding.customerRef,
      "externalRepoStudyUploadPrivacyBinding.customerRef",
    );
    yield* requireNonEmpty(
      binding.bindingRef,
      "externalRepoStudyUploadPrivacyBinding.bindingRef",
    );
    yield* requireSha256(
      binding.bindingHash,
      "externalRepoStudyUploadPrivacyBinding.bindingHash",
    );

    if (binding.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyUploadPrivacyBinding.repo",
        "upload privacy binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    if (
      binding.customerPublicClaimAllowed !== false ||
      binding.marketplacePackageAllowed !== false ||
      binding.payoutEligible !== false ||
      binding.intakeAdmitted !== false ||
      binding.ingested !== false ||
      binding.effectsApplied !== false
    ) {
      return yield* bindingError(
        "externalRepoStudyUploadPrivacyBinding.claimGates",
        "upload privacy binding must not grant ingestion, customer, marketplace, payout, or settlement claims",
      );
    }

    // The nested upload preflight is the source of truth for inertness; assert
    // it never escalates a claim through the binding.
    if (
      binding.uploadPreflight.intakeAdmitted !== false ||
      binding.uploadPreflight.ingested !== false ||
      binding.uploadPreflight.effectsApplied !== false
    ) {
      return yield* bindingError(
        "externalRepoStudyUploadPrivacyBinding.uploadPreflight",
        "nested upload preflight must remain inert",
      );
    }

    if (binding.state === "bound_held") {
      if (
        !binding.customerMatches ||
        !binding.repoMatches ||
        !binding.privacyReviewCleared ||
        binding.privacyReviewRef === null ||
        !binding.uploadPreflight.privacyReviewPresent
      ) {
        return yield* bindingError(
          "externalRepoStudyUploadPrivacyBinding.state",
          "bound_held requires a matching customer + repo, a cleared review, a derived privacyReviewRef, and the upload preflight to record it present",
        );
      }
    }

    if (binding.state === "unbound" && binding.privacyReviewRef !== null) {
      return yield* bindingError(
        "externalRepoStudyUploadPrivacyBinding.privacyReviewRef",
        "an unbound binding must not derive a privacyReviewRef",
      );
    }

    // When unbound, the upload's privacy gate must not be satisfied: no forged
    // string can have slipped through, because the binding controls the ref.
    if (binding.state === "unbound" && binding.uploadPreflight.privacyReviewPresent) {
      return yield* bindingError(
        "externalRepoStudyUploadPrivacyBinding.uploadPreflight.privacyReviewPresent",
        "an unbound binding must not leave the upload privacy gate satisfied",
      );
    }

    if (binding.bound !== (binding.state === "bound_held")) {
      return yield* bindingError(
        "externalRepoStudyUploadPrivacyBinding.bound",
        "bound flag must agree with the binding state",
      );
    }

    if (
      binding.bindingHash !==
      openAgentsExternalRepoStudyUploadPrivacyBindingHash(binding)
    ) {
      return yield* bindingError(
        "externalRepoStudyUploadPrivacyBinding.bindingHash",
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

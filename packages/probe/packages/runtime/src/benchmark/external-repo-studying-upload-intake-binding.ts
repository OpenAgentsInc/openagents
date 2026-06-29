import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { type OpenAgentsExternalRepoStudyPrivacyReviewPreflight } from "./external-repo-studying-privacy-review";
import {
  isCleanScanAttestationRef,
  type OpenAgentsExternalRepoStudyScanAttestationRegistry,
} from "./external-repo-studying-scan-attestation-registry";
import {
  buildOpenAgentsExternalRepoStudySelfServeUploadPreflight,
  OpenAgentsExternalRepoStudySelfServeUploadPreflight,
  type ExternalRepoStudySelfServeUploadRequest,
} from "./external-repo-studying-self-serve-upload";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Combined upload INTAKE binding for the external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing self-serve upload
 * control surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_self_serve_upload_missing):
 * it composes the two existing sibling bindings into ONE step that closes BOTH
 * forgeable-string seams of the self-serve upload preflight at the same time.
 *
 * The upload preflight has two refs-only gates: a privacy-review ref and a
 * clean-scan attestation ref. The existing `upload<->privacy` binding derives
 * ONLY the privacyReviewRef (and still lets a caller pass an arbitrary
 * scanAttestationRef), and the existing `scan<->upload` binding derives ONLY the
 * scanAttestationRef (and still lets a caller pass an arbitrary
 * privacyReviewRef). So a single caller composing them by hand could STILL forge
 * whichever ref the chosen binding left open. Nothing yet derives BOTH refs in
 * one place, leaving no forgeable input on the upload at all.
 *
 * This composer removes that residual forgeability: it takes the upload request
 * with BOTH `privacyReviewRef` and `scanAttestationRef` omitted from its input
 * type, derives the privacyReviewRef FROM a privacy-review preflight that covers
 * the SAME customer + repo and is in `review_ready_held`, derives the
 * scanAttestationRef FROM a candidate the scan-attestation registry verifies as a
 * KNOWN, CLEAN attestation for the SAME customer + repo + upload manifest, and
 * only then builds the upload preflight. The upload reaches `intake_ready_held`
 * only when BOTH refs are genuinely bound; if either is missing or mismatched,
 * NO ref is injected for that gate, so the upload blocks on the corresponding
 * missing-ref blocker instead of trusting an arbitrary string.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/digests/counts ONLY. sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: `intakeAdmitted`, `ingested`, and `effectsApplied`
 *    are ALWAYS false (inherited from the nested upload preflight and asserted
 *    here). The binding decides only WHETHER both of the upload's gates are
 *    genuinely backed by a cleared review and a registry-known clean scan; it
 *    never runs a scan, clears a review, ingests, stores bytes, delivers a
 *    packet, sends, settles, or spends.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires real malware/secret-scan
 * EXECUTION (the registry only mirrors a verdict), a real customer-data privacy
 * review backing an armed clearance, real durable, access-controlled upload
 * storage + signed-URL intake, an armed ingestion against a real customer repo
 * with a dereferenceable closeout receipt, marketplace metering, pricing, payout
 * eligibility, settlement, and owner sign-off per proof.claim_upgrade_receipts.v1
 * — all owner-gated and out of scope for the pure binding built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_UPLOAD_INTAKE_BINDING_SCHEMA_REF =
  "openagents.external_repo_study_upload_intake_binding.v0" as const;

export const OpenAgentsExternalRepoStudyUploadIntakeBindingState = S.Literals([
  // BOTH upload gates are genuinely backed: a cleared, matching review AND a
  // registry-known clean attestation for the same customer + repo + manifest.
  "bound_held",
  // At least one gate is not genuinely backed; that gate binds no ref.
  "unbound",
]);
export type OpenAgentsExternalRepoStudyUploadIntakeBindingState =
  typeof OpenAgentsExternalRepoStudyUploadIntakeBindingState.Type;

export const OpenAgentsExternalRepoStudyUploadIntakeBinding = S.Struct({
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
  // The bound privacy-review ref derived from the cleared review, or null.
  privacyReviewRef: S.NullOr(S.String),
  repo: S.String,
  repoMatches: S.Boolean,
  safeCopy: S.String,
  // The candidate clean-scan ref the caller supplied for verification (echoed).
  scanAttestationCandidateRef: S.String,
  // The bound clean-scan ref derived from the verified attestation, or null.
  scanAttestationRef: S.NullOr(S.String),
  scanAttestationRegistryRef: S.String,
  scanAttestationVerified: S.Boolean,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_UPLOAD_INTAKE_BINDING_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyUploadIntakeBindingState,
  unsafeCopyRefs: S.Array(S.String),
  uploadManifestDigest: S.String,
  // The full upload preflight built from the (only-if-bound) derived refs.
  uploadPreflight: OpenAgentsExternalRepoStudySelfServeUploadPreflight,
  wouldIngestWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyUploadIntakeBinding =
  typeof OpenAgentsExternalRepoStudyUploadIntakeBinding.Type;

export interface BuildOpenAgentsExternalRepoStudyUploadIntakeBindingInput {
  readonly generatedAt?: string;
  /**
   * Whether an owner sign-off (per proof.claim_upgrade_receipts.v1) is recorded
   * for an armed upload. Forwarded to the nested upload preflight. Default false.
   */
  readonly ownerSignoffPresent?: boolean;
  /** A privacy-review preflight verdict for the same customer + repo. */
  readonly privacyReviewPreflight: OpenAgentsExternalRepoStudyPrivacyReviewPreflight;
  /** The candidate clean-scan attestation ref to verify against the registry. */
  readonly scanAttestationCandidateRef: string;
  /** The recorded scan-attestation registry to verify the candidate against. */
  readonly scanAttestationRegistry: OpenAgentsExternalRepoStudyScanAttestationRegistry;
  /**
   * Whether the upload flag is armed. Forwarded to the nested upload preflight.
   * Even when armed, no real effect is applied. Default false.
   */
  readonly uploadFlagArmed?: boolean;
  /**
   * The self-serve upload request, MINUS both its privacyReviewRef and its
   * scanAttestationRef. The binding derives BOTH refs; callers cannot inject
   * either of their own.
   */
  readonly uploadRequest: Omit<
    ExternalRepoStudySelfServeUploadRequest,
    "privacyReviewRef" | "scanAttestationRef"
  >;
}

export function buildOpenAgentsExternalRepoStudyUploadIntakeBinding(
  input: BuildOpenAgentsExternalRepoStudyUploadIntakeBindingInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyUploadIntakeBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const review = input.privacyReviewPreflight;
    const registry = input.scanAttestationRegistry;
    const req = input.uploadRequest;
    const candidateRef = input.scanAttestationCandidateRef;

    yield* requireNonEmpty(req.repo, "externalRepoStudyUploadIntakeBinding.repo");
    yield* requireNonEmpty(
      req.customerRef,
      "externalRepoStudyUploadIntakeBinding.customerRef",
    );
    yield* requireNonEmpty(
      candidateRef,
      "externalRepoStudyUploadIntakeBinding.scanAttestationCandidateRef",
    );
    yield* requireSha256(
      req.uploadManifestDigest,
      "externalRepoStudyUploadIntakeBinding.uploadManifestDigest",
    );

    if (req.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.repo",
        "upload intake binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    // --- Privacy-review gate: derive privacyReviewRef from a cleared review. ---
    const customerMatches =
      req.customerRef.trim().length > 0 &&
      req.customerRef === review.customerRef;
    const repoMatches = req.repo.trim().length > 0 && req.repo === review.repo;
    const privacyReviewCleared =
      review.state === "review_ready_held" && review.privacyReviewRef !== null;
    const privacyBound = customerMatches && repoMatches && privacyReviewCleared;
    const boundPrivacyReviewRef = privacyBound ? review.privacyReviewRef : null;

    // --- Clean-scan gate: derive scanAttestationRef from a verified clean scan. ---
    const scanAttestationVerified = isCleanScanAttestationRef(
      registry,
      candidateRef,
      {
        customerRef: req.customerRef,
        repo: req.repo,
        uploadManifestDigest: req.uploadManifestDigest,
      },
    );
    const boundScanAttestationRef = scanAttestationVerified ? candidateRef : null;

    // The whole upload is bound only when BOTH gates are genuinely backed.
    const bound = privacyBound && scanAttestationVerified;

    // The upload preflight is ALWAYS built from the derived refs (or none). The
    // caller supplies neither ref directly, so each gate can only be satisfied by
    // a cleared review / registry-verified clean attestation respectively.
    const uploadPreflight =
      yield* buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        ownerSignoffPresent: input.ownerSignoffPresent ?? false,
        request: {
          ...req,
          privacyReviewRef: boundPrivacyReviewRef ?? undefined,
          scanAttestationRef: boundScanAttestationRef ?? undefined,
        },
        uploadFlagArmed: input.uploadFlagArmed ?? false,
      });

    const mismatchRefs = buildMismatchRefs({
      customerMatches,
      privacyReviewCleared,
      repoMatches,
      scanAttestationVerified,
    });

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_upload_intake_binding_hash";

    const evidenceRefs = [
      req.customerRef,
      req.uploaderRef,
      review.preflightRef,
      registry.registryRef,
      uploadPreflight.preflightRef,
      ...(boundPrivacyReviewRef ? [boundPrivacyReviewRef] : []),
      ...(boundScanAttestationRef ? [boundScanAttestationRef] : []),
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyUploadIntakeBinding = {
      bindingHash: "sha256:pending",
      bindingRef: "external_repo_study_upload_intake_binding.pending",
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
        "Upload intake binding cross-checked a self-serve upload request against BOTH a privacy-review preflight and a scan-attestation registry using refs, digests, and counts only. It derives the upload's privacy-review ref from a cleared, customer/repo-matched review and its clean-scan ref from a registry-known clean attestation covering the same customer, repo, and upload manifest digest; a blocked, mismatched, unknown, stale, or non-clean source binds no ref for that gate, so the upload blocks on the corresponding missing-ref blocker. The binding is held inert; no scan is run, no review is cleared, no repo content is uploaded, stored, or ingested, and no customer, marketplace, payout, or settlement claim is made.",
      scanAttestationCandidateRef: candidateRef,
      scanAttestationRef: boundScanAttestationRef,
      scanAttestationRegistryRef: registry.registryRef,
      scanAttestationVerified,
      schemaRef: OPENAGENTS_EXTERNAL_REPO_STUDY_UPLOAD_INTAKE_BINDING_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: bound ? "bound_held" : "unbound",
      unsafeCopyRefs: [
        "blocked_claim.upload_privacy_review_forgeable_string",
        "blocked_claim.upload_clean_scan_forgeable_string",
        "blocked_claim.customer_repo_upload_live",
        "blocked_claim.self_serve_customer_repo_ingestion_live",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      uploadManifestDigest: req.uploadManifestDigest,
      uploadPreflight,
      wouldIngestWhenArmed: uploadPreflight.wouldIngestWhenArmed,
    };

    const bindingHash = openAgentsExternalRepoStudyUploadIntakeBindingHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyUploadIntakeBinding({
      ...base,
      bindingHash,
      bindingRef: `external_repo_study_upload_intake_binding.${slugRepo(req.repo)}.${shortHash(bindingHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyUploadIntakeBinding(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyUploadIntakeBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyUploadIntakeBinding",
    );
    const binding = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyUploadIntakeBinding,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyUploadIntakeBinding",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyUploadIntakeBinding(binding);
    return binding;
  });
}

export function openAgentsExternalRepoStudyUploadIntakeBindingHash(
  binding: OpenAgentsExternalRepoStudyUploadIntakeBinding,
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
  readonly scanAttestationVerified: boolean;
}): ReadonlyArray<string> {
  const mismatches: string[] = [];
  if (!input.customerMatches) {
    mismatches.push(
      "blocker.external_repo_study_upload_intake_binding.customer_ref_mismatch",
    );
  }
  if (!input.repoMatches) {
    mismatches.push(
      "blocker.external_repo_study_upload_intake_binding.repo_mismatch",
    );
  }
  if (!input.privacyReviewCleared) {
    mismatches.push(
      "blocker.external_repo_study_upload_intake_binding.privacy_review_not_cleared",
    );
  }
  if (!input.scanAttestationVerified) {
    mismatches.push(
      "blocker.external_repo_study_upload_intake_binding.clean_scan_attestation_not_verified",
    );
  }
  return mismatches;
}

function validateExternalRepoStudyUploadIntakeBinding(
  binding: OpenAgentsExternalRepoStudyUploadIntakeBinding,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      binding.repo,
      "externalRepoStudyUploadIntakeBinding.repo",
    );
    yield* requireNonEmpty(
      binding.customerRef,
      "externalRepoStudyUploadIntakeBinding.customerRef",
    );
    yield* requireNonEmpty(
      binding.bindingRef,
      "externalRepoStudyUploadIntakeBinding.bindingRef",
    );
    yield* requireSha256(
      binding.bindingHash,
      "externalRepoStudyUploadIntakeBinding.bindingHash",
    );
    yield* requireSha256(
      binding.uploadManifestDigest,
      "externalRepoStudyUploadIntakeBinding.uploadManifestDigest",
    );

    if (binding.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.repo",
        "upload intake binding target must be an external (non-OpenAgents) pilot repo",
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
        "externalRepoStudyUploadIntakeBinding.claimGates",
        "upload intake binding must not grant ingestion, customer, marketplace, payout, or settlement claims",
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
        "externalRepoStudyUploadIntakeBinding.uploadPreflight",
        "nested upload preflight must remain inert",
      );
    }

    const allGatesBacked =
      binding.customerMatches &&
      binding.repoMatches &&
      binding.privacyReviewCleared &&
      binding.scanAttestationVerified;

    if (binding.state === "bound_held") {
      if (
        !allGatesBacked ||
        binding.privacyReviewRef === null ||
        binding.scanAttestationRef === null ||
        binding.scanAttestationRef !== binding.scanAttestationCandidateRef ||
        !binding.uploadPreflight.privacyReviewPresent ||
        !binding.uploadPreflight.scanAttestationPresent
      ) {
        return yield* bindingError(
          "externalRepoStudyUploadIntakeBinding.state",
          "bound_held requires a matching customer + repo, a cleared review with a derived privacyReviewRef, a registry-verified clean attestation with a derived scanAttestationRef equal to the candidate, and the upload preflight to record both present",
        );
      }
    }

    // Each gate independently controls its own ref: a gate that is not genuinely
    // backed must derive no ref and must not leave the upload's gate satisfied.
    const privacyGateBacked =
      binding.customerMatches &&
      binding.repoMatches &&
      binding.privacyReviewCleared;
    if (!privacyGateBacked && binding.privacyReviewRef !== null) {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.privacyReviewRef",
        "a non-cleared/mismatched review must not derive a privacyReviewRef",
      );
    }
    if (!privacyGateBacked && binding.uploadPreflight.privacyReviewPresent) {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.uploadPreflight.privacyReviewPresent",
        "a non-cleared/mismatched review must not leave the upload privacy gate satisfied",
      );
    }
    if (!binding.scanAttestationVerified && binding.scanAttestationRef !== null) {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.scanAttestationRef",
        "an unverified attestation must not derive a scanAttestationRef",
      );
    }
    if (
      !binding.scanAttestationVerified &&
      binding.uploadPreflight.scanAttestationPresent
    ) {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.uploadPreflight.scanAttestationPresent",
        "an unverified attestation must not leave the upload clean-scan gate satisfied",
      );
    }

    if (binding.state === "unbound" && allGatesBacked) {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.state",
        "an unbound binding must have at least one gate not genuinely backed",
      );
    }

    if (binding.bound !== (binding.state === "bound_held")) {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.bound",
        "bound flag must agree with the binding state",
      );
    }

    if (binding.bound !== allGatesBacked) {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.bound",
        "bound flag must agree with both gates being genuinely backed",
      );
    }

    if (
      binding.bindingHash !==
      openAgentsExternalRepoStudyUploadIntakeBindingHash(binding)
    ) {
      return yield* bindingError(
        "externalRepoStudyUploadIntakeBinding.bindingHash",
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

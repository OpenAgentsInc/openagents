import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
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
 * Scan-attestation <-> upload BINDING for the external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing self-serve upload
 * control surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_self_serve_upload_missing):
 * it closes the LAST forgeable-string seam in the self-serve upload preflight —
 * the malware/secret clean-scan gate.
 *
 * Today the self-serve upload preflight's `scanAttestationPresent` is only a
 * STRING PRESENCE check: any non-empty `scanAttestationRef` satisfies it,
 * including a forged or stale string, or a clean scan of a DIFFERENT upload
 * manifest. The scan-attestation registry already exposes
 * `isCleanScanAttestationRef(...)` to verify a ref matches a KNOWN, CLEAN
 * attestation for an exact (customer, repo, manifest), but nothing yet forced
 * the upload to consume only such a ref.
 *
 * This composer removes that forgeability exactly as the upload<->privacy
 * binding did for the privacy-review ref: it derives the upload's
 * `scanAttestationRef` FROM a candidate ref that the registry verifies covers
 * the SAME customer + repo + upload manifest digest with a CLEAN verdict. The
 * upload preflight is then built from the derived ref. When the candidate ref
 * is unknown, stale, non-clean, or covers a different manifest, NO ref is
 * injected, so the upload blocks on `clean_scan_attestation_missing` instead of
 * trusting an arbitrary string.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/digests/counts ONLY. sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: `intakeAdmitted`, `ingested`, and `effectsApplied`
 *    are ALWAYS false (inherited from the nested upload preflight and asserted
 *    here). The binding decides only WHETHER the upload's clean-scan gate is
 *    genuinely backed by a registry-known clean attestation; it never runs a
 *    scan, ingests, stores bytes, delivers a packet, sends, settles, or spends.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires real malware/secret-scan
 * EXECUTION (the registry only mirrors a verdict), a real customer-data privacy
 * review, real durable-storage controls, an armed ingestion against a real
 * customer repo with a dereferenceable closeout receipt, marketplace metering,
 * pricing, payout eligibility, settlement, and owner sign-off per
 * proof.claim_upgrade_receipts.v1 — all owner-gated and out of scope for the
 * pure binding built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_SCAN_UPLOAD_BINDING_SCHEMA_REF =
  "openagents.external_repo_study_scan_upload_binding.v0" as const;

export const OpenAgentsExternalRepoStudyScanUploadBindingState = S.Literals([
  // Upload clean-scan gate is genuinely backed by a registry-known clean
  // attestation covering the same customer + repo + upload manifest.
  "bound_held",
  // Candidate ref is unknown/stale/non-clean or covers a different manifest; no
  // ref bound.
  "unbound",
]);
export type OpenAgentsExternalRepoStudyScanUploadBindingState =
  typeof OpenAgentsExternalRepoStudyScanUploadBindingState.Type;

export const OpenAgentsExternalRepoStudyScanUploadBinding = S.Struct({
  bindingHash: S.String,
  bindingRef: S.String,
  bound: S.Boolean,
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
  repo: S.String,
  safeCopy: S.String,
  scanAttestationRegistryRef: S.String,
  // The candidate ref the caller supplied for verification (echoed for audit).
  scanAttestationCandidateRef: S.String,
  // Whether the registry verified the candidate as a clean attestation for the
  // exact customer + repo + manifest.
  scanAttestationVerified: S.Boolean,
  // The bound ref derived from the verified attestation, or null when unbound.
  scanAttestationRef: S.NullOr(S.String),
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_SCAN_UPLOAD_BINDING_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyScanUploadBindingState,
  unsafeCopyRefs: S.Array(S.String),
  uploadManifestDigest: S.String,
  // The full upload preflight built from the (only-if-bound) derived ref.
  uploadPreflight: OpenAgentsExternalRepoStudySelfServeUploadPreflight,
  wouldIngestWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyScanUploadBinding =
  typeof OpenAgentsExternalRepoStudyScanUploadBinding.Type;

export interface BuildOpenAgentsExternalRepoStudyScanUploadBindingInput {
  readonly generatedAt?: string;
  /**
   * Whether an owner sign-off (per proof.claim_upgrade_receipts.v1) is recorded
   * for an armed upload. Forwarded to the nested upload preflight. Default false.
   */
  readonly ownerSignoffPresent?: boolean;
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
   * The self-serve upload request, MINUS its scanAttestationRef. The binding
   * derives the ref from the verified attestation; callers cannot inject their
   * own.
   */
  readonly uploadRequest: Omit<
    ExternalRepoStudySelfServeUploadRequest,
    "scanAttestationRef"
  >;
}

export function buildOpenAgentsExternalRepoStudyScanUploadBinding(
  input: BuildOpenAgentsExternalRepoStudyScanUploadBindingInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyScanUploadBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const registry = input.scanAttestationRegistry;
    const req = input.uploadRequest;
    const candidateRef = input.scanAttestationCandidateRef;

    yield* requireNonEmpty(
      req.repo,
      "externalRepoStudyScanUploadBinding.repo",
    );
    yield* requireNonEmpty(
      req.customerRef,
      "externalRepoStudyScanUploadBinding.customerRef",
    );
    yield* requireNonEmpty(
      candidateRef,
      "externalRepoStudyScanUploadBinding.scanAttestationCandidateRef",
    );
    yield* requireSha256(
      req.uploadManifestDigest,
      "externalRepoStudyScanUploadBinding.uploadManifestDigest",
    );

    if (req.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyScanUploadBinding.repo",
        "scan upload binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    const scanAttestationVerified = isCleanScanAttestationRef(
      registry,
      candidateRef,
      {
        customerRef: req.customerRef,
        repo: req.repo,
        uploadManifestDigest: req.uploadManifestDigest,
      },
    );

    const bound = scanAttestationVerified;
    const boundScanAttestationRef = bound ? candidateRef : null;

    // The upload preflight is ALWAYS built from the derived ref (or none). The
    // caller never supplies a scanAttestationRef directly, so the upload's
    // clean-scan gate can only be satisfied by a registry-verified attestation.
    const uploadPreflight =
      yield* buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        ownerSignoffPresent: input.ownerSignoffPresent ?? false,
        request: {
          ...req,
          scanAttestationRef: boundScanAttestationRef ?? undefined,
        },
        uploadFlagArmed: input.uploadFlagArmed ?? false,
      });

    const mismatchRefs = scanAttestationVerified
      ? []
      : [
          "blocker.external_repo_study_scan_upload_binding.clean_scan_attestation_not_verified",
        ];

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_scan_upload_binding_hash";

    const evidenceRefs = [
      req.customerRef,
      req.uploaderRef,
      registry.registryRef,
      uploadPreflight.preflightRef,
      ...(boundScanAttestationRef ? [boundScanAttestationRef] : []),
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyScanUploadBinding = {
      bindingHash: "sha256:pending",
      bindingRef: "external_repo_study_scan_upload_binding.pending",
      bound,
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
      repo: req.repo,
      safeCopy:
        "Scan upload binding cross-checked a self-serve upload request against a scan-attestation registry using refs, digests, and counts only. It derives the upload's clean-scan ref from a registry-known clean attestation covering the same customer, repo, and upload manifest digest; an unknown, stale, non-clean, or mismatched attestation binds no ref, so the upload blocks on a missing clean scan. The binding is held inert; no scan is run, no repo content is uploaded, stored, or ingested, and no customer, marketplace, payout, or settlement claim is made.",
      scanAttestationCandidateRef: candidateRef,
      scanAttestationRef: boundScanAttestationRef,
      scanAttestationRegistryRef: registry.registryRef,
      scanAttestationVerified,
      schemaRef: OPENAGENTS_EXTERNAL_REPO_STUDY_SCAN_UPLOAD_BINDING_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: bound ? "bound_held" : "unbound",
      unsafeCopyRefs: [
        "blocked_claim.upload_clean_scan_forgeable_string",
        "blocked_claim.customer_repo_scan_executed_live",
        "blocked_claim.customer_repo_upload_live",
        "blocked_claim.self_serve_customer_repo_ingestion_live",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      uploadManifestDigest: req.uploadManifestDigest,
      uploadPreflight,
      wouldIngestWhenArmed: uploadPreflight.wouldIngestWhenArmed,
    };

    const bindingHash = openAgentsExternalRepoStudyScanUploadBindingHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyScanUploadBinding({
      ...base,
      bindingHash,
      bindingRef: `external_repo_study_scan_upload_binding.${slugRepo(req.repo)}.${shortHash(bindingHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyScanUploadBinding(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyScanUploadBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyScanUploadBinding",
    );
    const binding = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyScanUploadBinding,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyScanUploadBinding",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyScanUploadBinding(binding);
    return binding;
  });
}

export function openAgentsExternalRepoStudyScanUploadBindingHash(
  binding: OpenAgentsExternalRepoStudyScanUploadBinding,
): string {
  const {
    bindingHash: _bindingHash,
    bindingRef: _bindingRef,
    generatedAt: _generatedAt,
    ...stable
  } = binding;
  return sha256Ref(stableJson(stable));
}

function validateExternalRepoStudyScanUploadBinding(
  binding: OpenAgentsExternalRepoStudyScanUploadBinding,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      binding.repo,
      "externalRepoStudyScanUploadBinding.repo",
    );
    yield* requireNonEmpty(
      binding.customerRef,
      "externalRepoStudyScanUploadBinding.customerRef",
    );
    yield* requireNonEmpty(
      binding.bindingRef,
      "externalRepoStudyScanUploadBinding.bindingRef",
    );
    yield* requireSha256(
      binding.bindingHash,
      "externalRepoStudyScanUploadBinding.bindingHash",
    );
    yield* requireSha256(
      binding.uploadManifestDigest,
      "externalRepoStudyScanUploadBinding.uploadManifestDigest",
    );

    if (binding.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyScanUploadBinding.repo",
        "scan upload binding target must be an external (non-OpenAgents) pilot repo",
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
        "externalRepoStudyScanUploadBinding.claimGates",
        "scan upload binding must not grant ingestion, customer, marketplace, payout, or settlement claims",
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
        "externalRepoStudyScanUploadBinding.uploadPreflight",
        "nested upload preflight must remain inert",
      );
    }

    if (binding.state === "bound_held") {
      if (
        !binding.scanAttestationVerified ||
        binding.scanAttestationRef === null ||
        binding.scanAttestationRef !== binding.scanAttestationCandidateRef ||
        !binding.uploadPreflight.scanAttestationPresent
      ) {
        return yield* bindingError(
          "externalRepoStudyScanUploadBinding.state",
          "bound_held requires a registry-verified clean attestation, a derived scanAttestationRef equal to the candidate, and the upload preflight to record it present",
        );
      }
    }

    if (binding.state === "unbound" && binding.scanAttestationRef !== null) {
      return yield* bindingError(
        "externalRepoStudyScanUploadBinding.scanAttestationRef",
        "an unbound binding must not derive a scanAttestationRef",
      );
    }

    // When unbound, the upload's clean-scan gate must not be satisfied: no
    // forged string can have slipped through, because the binding controls the
    // ref.
    if (
      binding.state === "unbound" &&
      binding.uploadPreflight.scanAttestationPresent
    ) {
      return yield* bindingError(
        "externalRepoStudyScanUploadBinding.uploadPreflight.scanAttestationPresent",
        "an unbound binding must not leave the upload clean-scan gate satisfied",
      );
    }

    if (binding.bound !== (binding.state === "bound_held")) {
      return yield* bindingError(
        "externalRepoStudyScanUploadBinding.bound",
        "bound flag must agree with the binding state",
      );
    }

    if (binding.bound !== binding.scanAttestationVerified) {
      return yield* bindingError(
        "externalRepoStudyScanUploadBinding.scanAttestationVerified",
        "bound flag must agree with the registry verification result",
      );
    }

    if (
      binding.bindingHash !==
      openAgentsExternalRepoStudyScanUploadBindingHash(binding)
    ) {
      return yield* bindingError(
        "externalRepoStudyScanUploadBinding.bindingHash",
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

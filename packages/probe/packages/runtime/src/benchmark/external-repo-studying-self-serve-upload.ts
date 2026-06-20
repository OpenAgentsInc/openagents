import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Self-serve upload intake preflight for the external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing self-serve upload
 * control surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_self_serve_upload_missing):
 * it decides whether a customer/contributor's SELF-SERVE upload of an external
 * (non-OpenAgents) repo WOULD be admitted for ingestion into the pilot, based on
 * refs, digests, and counts ONLY.
 *
 * It does NOT ingest, store, fetch, or unpack any customer repo content. It
 * computes an inert preflight VERDICT over an upload REQUEST that is itself
 * expressed only as references (a manifest digest, a clean-scan attestation ref,
 * a privacy-review ref) plus bounded counts (declared byte size, file count).
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/digests/counts ONLY. No raw file content, repository tree, file paths
 *    beyond the external repo slug, archive bytes, or uploader PII ever crosses
 *    this boundary. sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: `intakeAdmitted` and `ingested` are ALWAYS false.
 *    The module computes WHETHER an upload WOULD be ingested when the upload flag
 *    is armed and every gate passes, but it never ingests a real upload, stores
 *    bytes, delivers a packet, marks anything claimable, sends, settles, or
 *    spends. effectsApplied is always false.
 *  - Flag-gated default-OFF: the upload flag
 *    (EXTERNAL_REPO_STUDY_SELF_SERVE_UPLOAD_ENABLED) defaults disabled. With no
 *    flag the intake gate resolves inert_disabled.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires real customer-data privacy
 * review, real malware/secret-scan execution (not just an attestation ref), real
 * durable-storage controls, an armed ingestion against a real customer repo with
 * a dereferenceable closeout receipt, marketplace metering, pricing, payout
 * eligibility, settlement, and owner sign-off per proof.claim_upgrade_receipts.v1
 * — all owner-gated and out of scope for the pure preflight built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_SELF_SERVE_UPLOAD_PREFLIGHT_SCHEMA_REF =
  "openagents.external_repo_study_self_serve_upload_preflight.v0" as const;

export const ExternalRepoStudySelfServeUploadFlagName =
  "EXTERNAL_REPO_STUDY_SELF_SERVE_UPLOAD_ENABLED" as const;

/** Conservative refs-only preflight caps. These bound the DECLARED request; no
 *  bytes are read. Real ingestion would re-verify against the actual upload. */
export const SELF_SERVE_UPLOAD_MAX_DECLARED_BYTES = 512 * 1024 * 1024; // 512 MiB
export const SELF_SERVE_UPLOAD_MAX_FILE_COUNT = 100_000;

export const OpenAgentsExternalRepoStudySelfServeUploadGateState = S.Literals([
  // No flag, default posture: preflight evaluated, held inert.
  "inert_disabled",
  // Flag armed but a required gate (preflight/owner) is unmet.
  "armed_blocked",
  // Flag armed and every gate passed; ingestion permitted by policy, but this
  // module STILL applies no real effect (effectsApplied is always false).
  "armed_ready",
]);
export type OpenAgentsExternalRepoStudySelfServeUploadGateState =
  typeof OpenAgentsExternalRepoStudySelfServeUploadGateState.Type;

export const OpenAgentsExternalRepoStudySelfServeUploadState = S.Literals([
  "intake_ready_held",
  "blocked",
]);
export type OpenAgentsExternalRepoStudySelfServeUploadState =
  typeof OpenAgentsExternalRepoStudySelfServeUploadState.Type;

/**
 * A self-serve upload request, expressed as refs/digests/counts only. No raw
 * file content, archive bytes, repository tree, or uploader PII.
 */
export interface ExternalRepoStudySelfServeUploadRequest {
  /** Stable customer ref the upload is for, e.g. customer.<id>.v0 */
  readonly customerRef: string;
  /** Stable uploader/contributor ref, e.g. contributor.pylon.<id>.v0 */
  readonly uploaderRef: string;
  /** External (non-OpenAgents) repo slug, e.g. ExampleCorp/widget-service */
  readonly repo: string;
  /** sha256 digest of the client-computed upload manifest (refs only). */
  readonly uploadManifestDigest: string;
  /** Declared total byte size of the upload (bounded; no bytes are read). */
  readonly declaredByteSize: number;
  /** Declared file count of the upload (bounded; no tree is read). */
  readonly fileCount: number;
  /**
   * Ref to a CLEAN secret/malware scan attestation for the upload. The scan
   * itself is performed elsewhere; this preflight only checks the ref exists.
   */
  readonly scanAttestationRef?: string;
  /**
   * Ref to the recorded customer-data privacy review for this upload. Ties this
   * preflight to the privacy_policy blocker; here it is only a presence check.
   */
  readonly privacyReviewRef?: string;
  /** Whether the uploader accepted the pilot upload terms (refs-only, no-leak,
   *  inert). Default false. */
  readonly uploaderTermsAccepted?: boolean;
}

export const OpenAgentsExternalRepoStudySelfServeUploadGate = S.Struct({
  blockedReasonRefs: S.Array(S.String),
  effectsApplied: S.Literal(false),
  flagName: S.Literal(ExternalRepoStudySelfServeUploadFlagName),
  ownerSignoffPresent: S.Boolean,
  state: OpenAgentsExternalRepoStudySelfServeUploadGateState,
});
export type OpenAgentsExternalRepoStudySelfServeUploadGate =
  typeof OpenAgentsExternalRepoStudySelfServeUploadGate.Type;

export const OpenAgentsExternalRepoStudySelfServeUploadPreflight = S.Struct({
  blockerRefs: S.Array(S.String),
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  declaredByteSize: S.Number,
  declaredByteSizeWithinCap: S.Boolean,
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  fileCount: S.Number,
  fileCountWithinCap: S.Boolean,
  generatedAt: S.String,
  ingested: S.Literal(false),
  intakeAdmitted: S.Literal(false),
  intakeGate: OpenAgentsExternalRepoStudySelfServeUploadGate,
  marketplacePackageAllowed: S.Literal(false),
  payoutEligible: S.Literal(false),
  preflightHash: S.String,
  preflightRef: S.String,
  privacyReviewPresent: S.Boolean,
  repo: S.String,
  safeCopy: S.String,
  scanAttestationPresent: S.Boolean,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_SELF_SERVE_UPLOAD_PREFLIGHT_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudySelfServeUploadState,
  unsafeCopyRefs: S.Array(S.String),
  uploaderRef: S.String,
  uploaderTermsAccepted: S.Boolean,
  uploadManifestDigest: S.String,
  wouldIngestWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudySelfServeUploadPreflight =
  typeof OpenAgentsExternalRepoStudySelfServeUploadPreflight.Type;

export interface BuildOpenAgentsExternalRepoStudySelfServeUploadPreflightInput {
  readonly generatedAt?: string;
  /**
   * Whether an owner sign-off (per proof.claim_upgrade_receipts.v1) is recorded
   * for an armed upload. Default false.
   */
  readonly ownerSignoffPresent?: boolean;
  readonly request: ExternalRepoStudySelfServeUploadRequest;
  /**
   * Whether the upload flag is armed. Default false. Even when armed, this
   * module applies no real effect (intakeAdmitted/ingested/effectsApplied stay
   * false).
   */
  readonly uploadFlagArmed?: boolean;
}

export function buildOpenAgentsExternalRepoStudySelfServeUploadPreflight(
  input: BuildOpenAgentsExternalRepoStudySelfServeUploadPreflightInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudySelfServeUploadPreflight,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const request = input.request;
    const repo = request.repo;
    yield* requireNonEmpty(repo, "externalRepoStudySelfServeUpload.repo");
    yield* requireNonEmpty(
      request.customerRef,
      "externalRepoStudySelfServeUpload.customerRef",
    );
    yield* requireNonEmpty(
      request.uploaderRef,
      "externalRepoStudySelfServeUpload.uploaderRef",
    );
    yield* requireSha256(
      request.uploadManifestDigest,
      "externalRepoStudySelfServeUpload.uploadManifestDigest",
    );

    if (repo === "OpenAgentsInc/openagents") {
      return yield* uploadError(
        "externalRepoStudySelfServeUpload.repo",
        "self-serve upload target must be an external (non-OpenAgents) pilot repo",
      );
    }

    const declaredByteSize = request.declaredByteSize;
    const fileCount = request.fileCount;
    const declaredByteSizeWithinCap =
      Number.isInteger(declaredByteSize) &&
      declaredByteSize > 0 &&
      declaredByteSize <= SELF_SERVE_UPLOAD_MAX_DECLARED_BYTES;
    const fileCountWithinCap =
      Number.isInteger(fileCount) &&
      fileCount > 0 &&
      fileCount <= SELF_SERVE_UPLOAD_MAX_FILE_COUNT;
    const scanAttestationPresent =
      (request.scanAttestationRef ?? "").trim().length > 0;
    const privacyReviewPresent =
      (request.privacyReviewRef ?? "").trim().length > 0;
    const uploaderTermsAccepted = request.uploaderTermsAccepted ?? false;

    const preflightPassed =
      declaredByteSizeWithinCap &&
      fileCountWithinCap &&
      scanAttestationPresent &&
      privacyReviewPresent &&
      uploaderTermsAccepted;

    const blockerRefs = buildUploadBlockerRefs({
      declaredByteSizeWithinCap,
      fileCountWithinCap,
      privacyReviewPresent,
      scanAttestationPresent,
      uploaderTermsAccepted,
    });

    const intakeGate = buildUploadGate({
      ownerSignoffPresent: input.ownerSignoffPresent ?? false,
      preflightPassed,
      uploadFlagArmed: input.uploadFlagArmed ?? false,
    });

    const wouldIngestWhenArmed =
      preflightPassed && intakeGate.state === "armed_ready";

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_self_serve_upload_preflight_hash";

    const evidenceRefs = [
      request.uploadManifestDigest,
      request.customerRef,
      request.uploaderRef,
      ...(scanAttestationPresent && request.scanAttestationRef
        ? [request.scanAttestationRef]
        : []),
      ...(privacyReviewPresent && request.privacyReviewRef
        ? [request.privacyReviewRef]
        : []),
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudySelfServeUploadPreflight = {
      blockerRefs,
      customerPublicClaimAllowed: false,
      customerRef: request.customerRef,
      declaredByteSize,
      declaredByteSizeWithinCap,
      effectsApplied: false,
      evidenceRefs,
      fileCount,
      fileCountWithinCap,
      generatedAt,
      ingested: false,
      intakeAdmitted: false,
      intakeGate,
      marketplacePackageAllowed: false,
      payoutEligible: false,
      preflightHash: "sha256:pending",
      preflightRef: "external_repo_study_self_serve_upload_preflight.pending",
      privacyReviewPresent,
      repo,
      safeCopy:
        "Self-serve upload preflight evaluated an external-repo upload request from refs, digests, and counts only (manifest digest, clean-scan ref, privacy-review ref, declared size and file count). The preflight is held inert; no repo content is uploaded, stored, or ingested, no packet is delivered or claimable, and no customer, marketplace, payout, or settlement claim is made.",
      scanAttestationPresent,
      schemaRef:
        OPENAGENTS_EXTERNAL_REPO_STUDY_SELF_SERVE_UPLOAD_PREFLIGHT_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: preflightPassed ? "intake_ready_held" : "blocked",
      unsafeCopyRefs: [
        "blocked_claim.customer_repo_upload_live",
        "blocked_claim.self_serve_customer_repo_ingestion_live",
        "blocked_claim.uploaded_repo_studied_and_delivered",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      uploaderRef: request.uploaderRef,
      uploaderTermsAccepted,
      uploadManifestDigest: request.uploadManifestDigest,
      wouldIngestWhenArmed,
    };

    const preflightHash =
      openAgentsExternalRepoStudySelfServeUploadPreflightHash(base);

    return yield* decodeOpenAgentsExternalRepoStudySelfServeUploadPreflight({
      ...base,
      preflightHash,
      preflightRef: `external_repo_study_self_serve_upload_preflight.${slugRepo(repo)}.${shortHash(preflightHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudySelfServeUploadPreflight(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudySelfServeUploadPreflight,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudySelfServeUpload",
    );
    const preflight = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudySelfServeUploadPreflight,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudySelfServeUpload",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudySelfServeUploadPreflight(preflight);
    return preflight;
  });
}

export function openAgentsExternalRepoStudySelfServeUploadPreflightHash(
  preflight: OpenAgentsExternalRepoStudySelfServeUploadPreflight,
): string {
  const {
    preflightHash: _preflightHash,
    preflightRef: _preflightRef,
    generatedAt: _generatedAt,
    ...stable
  } = preflight;
  return sha256Ref(stableJson(stable));
}

function buildUploadGate(input: {
  readonly ownerSignoffPresent: boolean;
  readonly preflightPassed: boolean;
  readonly uploadFlagArmed: boolean;
}): OpenAgentsExternalRepoStudySelfServeUploadGate {
  if (!input.uploadFlagArmed) {
    return {
      blockedReasonRefs: [],
      effectsApplied: false,
      flagName: ExternalRepoStudySelfServeUploadFlagName,
      ownerSignoffPresent: input.ownerSignoffPresent,
      state: "inert_disabled",
    };
  }

  const blockedReasonRefs: string[] = [];
  if (!input.preflightPassed) {
    blockedReasonRefs.push("upload.blocked.preflight_not_passed");
  }
  if (!input.ownerSignoffPresent) {
    blockedReasonRefs.push("upload.blocked.owner_signoff_missing");
  }

  return {
    blockedReasonRefs,
    effectsApplied: false,
    flagName: ExternalRepoStudySelfServeUploadFlagName,
    ownerSignoffPresent: input.ownerSignoffPresent,
    state: blockedReasonRefs.length === 0 ? "armed_ready" : "armed_blocked",
  };
}

function buildUploadBlockerRefs(input: {
  readonly declaredByteSizeWithinCap: boolean;
  readonly fileCountWithinCap: boolean;
  readonly privacyReviewPresent: boolean;
  readonly scanAttestationPresent: boolean;
  readonly uploaderTermsAccepted: boolean;
}): ReadonlyArray<string> {
  const blockers: string[] = [];
  if (!input.declaredByteSizeWithinCap) {
    blockers.push(
      "blocker.external_repo_study_self_serve_upload.declared_byte_size_out_of_bounds",
    );
  }
  if (!input.fileCountWithinCap) {
    blockers.push(
      "blocker.external_repo_study_self_serve_upload.file_count_out_of_bounds",
    );
  }
  if (!input.scanAttestationPresent) {
    blockers.push(
      "blocker.external_repo_study_self_serve_upload.clean_scan_attestation_missing",
    );
  }
  if (!input.privacyReviewPresent) {
    blockers.push(
      "blocker.external_repo_study_self_serve_upload.privacy_review_missing",
    );
  }
  if (!input.uploaderTermsAccepted) {
    blockers.push(
      "blocker.external_repo_study_self_serve_upload.uploader_terms_not_accepted",
    );
  }
  return blockers;
}

function validateExternalRepoStudySelfServeUploadPreflight(
  preflight: OpenAgentsExternalRepoStudySelfServeUploadPreflight,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      preflight.repo,
      "externalRepoStudySelfServeUpload.repo",
    );
    yield* requireNonEmpty(
      preflight.customerRef,
      "externalRepoStudySelfServeUpload.customerRef",
    );
    yield* requireNonEmpty(
      preflight.uploaderRef,
      "externalRepoStudySelfServeUpload.uploaderRef",
    );
    yield* requireNonEmpty(
      preflight.preflightRef,
      "externalRepoStudySelfServeUpload.preflightRef",
    );
    yield* requireSha256(
      preflight.preflightHash,
      "externalRepoStudySelfServeUpload.preflightHash",
    );
    yield* requireSha256(
      preflight.uploadManifestDigest,
      "externalRepoStudySelfServeUpload.uploadManifestDigest",
    );

    if (preflight.repo === "OpenAgentsInc/openagents") {
      return yield* uploadError(
        "externalRepoStudySelfServeUpload.repo",
        "self-serve upload target must be an external (non-OpenAgents) pilot repo",
      );
    }

    if (
      preflight.customerPublicClaimAllowed !== false ||
      preflight.marketplacePackageAllowed !== false ||
      preflight.payoutEligible !== false ||
      preflight.intakeAdmitted !== false ||
      preflight.ingested !== false ||
      preflight.effectsApplied !== false
    ) {
      return yield* uploadError(
        "externalRepoStudySelfServeUpload.claimGates",
        "self-serve upload preflight must not grant ingestion, customer, marketplace, payout, or settlement claims",
      );
    }

    if (preflight.intakeGate.effectsApplied !== false) {
      return yield* uploadError(
        "externalRepoStudySelfServeUpload.intakeGate.effectsApplied",
        "intake gate must never apply a real effect",
      );
    }

    if (preflight.state === "intake_ready_held") {
      if (
        !preflight.declaredByteSizeWithinCap ||
        !preflight.fileCountWithinCap ||
        !preflight.scanAttestationPresent ||
        !preflight.privacyReviewPresent ||
        !preflight.uploaderTermsAccepted
      ) {
        return yield* uploadError(
          "externalRepoStudySelfServeUpload.state",
          "intake_ready_held requires every preflight check (size, file count, clean scan, privacy review, uploader terms) to pass",
        );
      }
    }

    if (
      preflight.wouldIngestWhenArmed &&
      preflight.intakeGate.state !== "armed_ready"
    ) {
      return yield* uploadError(
        "externalRepoStudySelfServeUpload.wouldIngestWhenArmed",
        "an upload can only be marked would-ingest-when-armed once the armed gate is ready",
      );
    }

    if (
      preflight.preflightHash !==
      openAgentsExternalRepoStudySelfServeUploadPreflightHash(preflight)
    ) {
      return yield* uploadError(
        "externalRepoStudySelfServeUpload.preflightHash",
        "must match the deterministic preflight hash",
      );
    }
  });
}

function requireNonEmpty(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? uploadError(path, "must be non-empty")
    : Effect.void;
}

function requireSha256(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:")
    ? Effect.void
    : uploadError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function uploadError(
  path: string,
  reason: string,
): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

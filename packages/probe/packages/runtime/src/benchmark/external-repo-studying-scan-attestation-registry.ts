import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Clean-scan ATTESTATION registry for the external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing self-serve upload
 * control surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_self_serve_upload_missing): it
 * closes the forgeable-string seam in the upload preflight's CLEAN-SCAN gate.
 *
 * Today the self-serve upload preflight's `scanAttestationPresent` is only a
 * STRING PRESENCE check: any non-empty `scanAttestationRef` satisfies it,
 * including a forged or stale string that points at no real scan. This is the
 * exact seam the privacy-policy registry + the upload<->privacy binding already
 * closed for the privacy-review ref; this module closes it for the malware/secret
 * scan attestation.
 *
 * The registry records ISSUED scan attestations as refs/digests/enums/counts
 * only, each bound to a SPECIFIC (customerRef, repo, uploadManifestDigest), and
 * exposes `isCleanScanAttestationRef(...)` so a verifier can require an upload's
 * scan ref to match a KNOWN, CLEAN attestation covering THAT exact upload
 * manifest — not merely be a non-empty string.
 *
 * It does NOT run a scan, read repo bytes, or process customer data. Recording
 * an attestation here only mirrors a scan VERDICT that was produced elsewhere.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/digests/enums/counts ONLY. No raw file content, archive bytes,
 *    repository tree, or scanner output ever crosses this boundary.
 *    sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: recording an attestation grants no ingestion and
 *    applies no effect. effectsApplied is always false.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires real malware/secret-scan
 * EXECUTION (this registry only mirrors a verdict), real durable-storage
 * controls, an armed ingestion against a real customer upload with a
 * dereferenceable closeout receipt, and owner sign-off per
 * proof.claim_upgrade_receipts.v1 — all owner-gated and out of scope for the
 * registry built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_SCAN_ATTESTATION_REGISTRY_SCHEMA_REF =
  "openagents.external_repo_study_scan_attestation_registry.v0" as const;

export const OpenAgentsExternalRepoStudyScanVerdict = S.Literals([
  "clean",
  "findings",
]);
export type OpenAgentsExternalRepoStudyScanVerdict =
  typeof OpenAgentsExternalRepoStudyScanVerdict.Type;

/**
 * A scan attestation recorded in the registry, expressed as refs/digests/enums/
 * counts only. The deterministic `attestationDigest` is sha256 over the bound
 * fields, so a change to any of them changes the digest and the derived ref.
 */
export const OpenAgentsExternalRepoStudyScanAttestation = S.Struct({
  attestationDigest: S.String,
  attestationRef: S.String,
  customerRef: S.String,
  findingsCount: S.Number,
  repo: S.String,
  scannerRef: S.String,
  uploadManifestDigest: S.String,
  verdict: OpenAgentsExternalRepoStudyScanVerdict,
});
export type OpenAgentsExternalRepoStudyScanAttestation =
  typeof OpenAgentsExternalRepoStudyScanAttestation.Type;

export const OpenAgentsExternalRepoStudyScanAttestationRegistry = S.Struct({
  customerPublicClaimAllowed: S.Literal(false),
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  payoutEligible: S.Literal(false),
  recordedAttestations: S.Array(OpenAgentsExternalRepoStudyScanAttestation),
  registryHash: S.String,
  registryRef: S.String,
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_SCAN_ATTESTATION_REGISTRY_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  unsafeCopyRefs: S.Array(S.String),
});
export type OpenAgentsExternalRepoStudyScanAttestationRegistry =
  typeof OpenAgentsExternalRepoStudyScanAttestationRegistry.Type;

/**
 * The bound fields of a scan attestation, supplied by the caller. The registry
 * derives `attestationDigest` and `attestationRef` from these deterministically.
 */
export interface ExternalRepoStudyScanAttestationInput {
  readonly customerRef: string;
  readonly repo: string;
  /** sha256 digest of the upload manifest this scan covered (refs only). */
  readonly uploadManifestDigest: string;
  /** Stable ref of the scanner that produced this verdict. */
  readonly scannerRef: string;
  readonly verdict: OpenAgentsExternalRepoStudyScanVerdict;
  /** Number of findings (0 for a clean verdict). */
  readonly findingsCount: number;
}

export interface BuildOpenAgentsExternalRepoStudyScanAttestationRegistryInput {
  readonly attestations?: ReadonlyArray<ExternalRepoStudyScanAttestationInput>;
  readonly generatedAt?: string;
}

/**
 * The fields a scan attestation digest binds. Recomputing this from a verifier's
 * own view of the upload manifest is how forgery is detected.
 */
function scanAttestationDigestSource(
  attestation: ExternalRepoStudyScanAttestationInput,
): {
  readonly customerRef: string;
  readonly findingsCount: number;
  readonly repo: string;
  readonly scannerRef: string;
  readonly uploadManifestDigest: string;
  readonly verdict: OpenAgentsExternalRepoStudyScanVerdict;
} {
  return {
    customerRef: attestation.customerRef,
    findingsCount: attestation.findingsCount,
    repo: attestation.repo,
    scannerRef: attestation.scannerRef,
    uploadManifestDigest: attestation.uploadManifestDigest,
    verdict: attestation.verdict,
  };
}

/**
 * Deterministic content digest of a scan attestation's bound fields. The
 * registry pins each attestation with this digest so a recorded verdict cannot
 * silently drift from the customer/repo/manifest it actually covered.
 */
export function externalRepoStudyScanAttestationDigest(
  attestation: ExternalRepoStudyScanAttestationInput,
): string {
  return sha256Ref(stableJson(scanAttestationDigestSource(attestation)));
}

export function buildOpenAgentsExternalRepoStudyScanAttestationRegistry(
  input: BuildOpenAgentsExternalRepoStudyScanAttestationRegistryInput = {},
): Effect.Effect<
  OpenAgentsExternalRepoStudyScanAttestationRegistry,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const inputs = input.attestations ?? [];

    const recordedAttestations: OpenAgentsExternalRepoStudyScanAttestation[] =
      [];
    for (const attestation of inputs) {
      yield* requireNonEmpty(
        attestation.customerRef,
        "externalRepoStudyScanAttestationRegistry.attestation.customerRef",
      );
      yield* requireNonEmpty(
        attestation.repo,
        "externalRepoStudyScanAttestationRegistry.attestation.repo",
      );
      yield* requireNonEmpty(
        attestation.scannerRef,
        "externalRepoStudyScanAttestationRegistry.attestation.scannerRef",
      );
      yield* requireSha256(
        attestation.uploadManifestDigest,
        "externalRepoStudyScanAttestationRegistry.attestation.uploadManifestDigest",
      );

      if (attestation.repo === "OpenAgentsInc/openagents") {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.repo",
          "scan attestation target must be an external (non-OpenAgents) pilot repo",
        );
      }

      if (
        !Number.isInteger(attestation.findingsCount) ||
        attestation.findingsCount < 0
      ) {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.findingsCount",
          "findingsCount must be a non-negative integer",
        );
      }

      if (attestation.verdict === "clean" && attestation.findingsCount !== 0) {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.findingsCount",
          "a clean verdict must record zero findings",
        );
      }

      if (attestation.verdict === "findings" && attestation.findingsCount <= 0) {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.findingsCount",
          "a findings verdict must record at least one finding",
        );
      }

      const attestationDigest =
        externalRepoStudyScanAttestationDigest(attestation);
      recordedAttestations.push({
        attestationDigest,
        attestationRef: `scan_attestation.${slugRepo(attestation.repo)}.${shortHash(attestationDigest)}.v0`,
        customerRef: attestation.customerRef,
        findingsCount: attestation.findingsCount,
        repo: attestation.repo,
        scannerRef: attestation.scannerRef,
        uploadManifestDigest: attestation.uploadManifestDigest,
        verdict: attestation.verdict,
      });
    }

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_scan_attestation_registry_hash";

    const evidenceRefs = [
      ...recordedAttestations.map((attestation) => attestation.attestationRef),
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyScanAttestationRegistry = {
      customerPublicClaimAllowed: false,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      marketplacePackageAllowed: false,
      payoutEligible: false,
      recordedAttestations,
      registryHash: "sha256:pending",
      registryRef: "external_repo_study_scan_attestation_registry.pending",
      safeCopy:
        "Scan attestation registry records issued malware/secret-scan verdicts for external-repo uploads as refs, digests, enums, and counts only, each bound to a specific customer, repo, and upload manifest digest. Recording a verdict runs no scan, reads no repo bytes, and grants no ingestion; the pilot stays inert and gated, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef:
        OPENAGENTS_EXTERNAL_REPO_STUDY_SCAN_ATTESTATION_REGISTRY_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      unsafeCopyRefs: [
        "blocked_claim.upload_clean_scan_forgeable_string",
        "blocked_claim.customer_repo_scan_executed_live",
        "blocked_claim.customer_repo_upload_live",
        "blocked_claim.self_serve_customer_repo_ingestion_live",
        "blocked_claim.machine_studying_payout_eligible",
      ],
    };

    const registryHash =
      openAgentsExternalRepoStudyScanAttestationRegistryHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyScanAttestationRegistry({
      ...base,
      registryHash,
      registryRef: `external_repo_study_scan_attestation_registry.${shortHash(registryHash)}`,
    });
  });
}

/**
 * Whether `ref` references a KNOWN, CLEAN scan attestation covering THE SAME
 * customer + repo + upload manifest digest. Closes the forgeable-string seam in
 * the upload preflight's clean-scan gate: an upload's scan ref must match a
 * recorded clean attestation for that exact manifest, not just be non-empty.
 *
 * A non-clean verdict, an unknown/empty ref, or any customer/repo/manifest
 * mismatch returns false.
 */
export function isCleanScanAttestationRef(
  registry: OpenAgentsExternalRepoStudyScanAttestationRegistry,
  ref: string | undefined,
  match: {
    readonly customerRef: string;
    readonly repo: string;
    readonly uploadManifestDigest: string;
  },
): boolean {
  if (ref === undefined || ref.trim().length === 0) {
    return false;
  }
  if (
    match.customerRef.trim().length === 0 ||
    match.repo.trim().length === 0 ||
    match.uploadManifestDigest.trim().length === 0
  ) {
    return false;
  }
  return registry.recordedAttestations.some(
    (attestation) =>
      attestation.attestationRef === ref &&
      attestation.verdict === "clean" &&
      attestation.customerRef === match.customerRef &&
      attestation.repo === match.repo &&
      attestation.uploadManifestDigest === match.uploadManifestDigest,
  );
}

export function decodeOpenAgentsExternalRepoStudyScanAttestationRegistry(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyScanAttestationRegistry,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyScanAttestationRegistry",
    );
    const registry = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyScanAttestationRegistry,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyScanAttestationRegistry",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyScanAttestationRegistry(registry);
    return registry;
  });
}

export function openAgentsExternalRepoStudyScanAttestationRegistryHash(
  registry: OpenAgentsExternalRepoStudyScanAttestationRegistry,
): string {
  const {
    registryHash: _registryHash,
    registryRef: _registryRef,
    generatedAt: _generatedAt,
    ...stable
  } = registry;
  return sha256Ref(stableJson(stable));
}

function validateExternalRepoStudyScanAttestationRegistry(
  registry: OpenAgentsExternalRepoStudyScanAttestationRegistry,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      registry.registryRef,
      "externalRepoStudyScanAttestationRegistry.registryRef",
    );
    yield* requireSha256(
      registry.registryHash,
      "externalRepoStudyScanAttestationRegistry.registryHash",
    );

    if (
      registry.customerPublicClaimAllowed !== false ||
      registry.marketplacePackageAllowed !== false ||
      registry.payoutEligible !== false ||
      registry.effectsApplied !== false
    ) {
      return yield* registryError(
        "externalRepoStudyScanAttestationRegistry.claimGates",
        "scan attestation registry must not grant ingestion or customer, marketplace, payout, or settlement claims",
      );
    }

    const seenRefs = new Set<string>();
    for (const attestation of registry.recordedAttestations) {
      yield* requireNonEmpty(
        attestation.attestationRef,
        "externalRepoStudyScanAttestationRegistry.attestation.attestationRef",
      );
      yield* requireNonEmpty(
        attestation.customerRef,
        "externalRepoStudyScanAttestationRegistry.attestation.customerRef",
      );
      yield* requireNonEmpty(
        attestation.repo,
        "externalRepoStudyScanAttestationRegistry.attestation.repo",
      );
      yield* requireNonEmpty(
        attestation.scannerRef,
        "externalRepoStudyScanAttestationRegistry.attestation.scannerRef",
      );
      yield* requireSha256(
        attestation.uploadManifestDigest,
        "externalRepoStudyScanAttestationRegistry.attestation.uploadManifestDigest",
      );
      yield* requireSha256(
        attestation.attestationDigest,
        "externalRepoStudyScanAttestationRegistry.attestation.attestationDigest",
      );

      if (attestation.repo === "OpenAgentsInc/openagents") {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.repo",
          "scan attestation target must be an external (non-OpenAgents) pilot repo",
        );
      }

      if (seenRefs.has(attestation.attestationRef)) {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.attestationRef",
          "recorded attestation refs must be unique",
        );
      }
      seenRefs.add(attestation.attestationRef);

      if (
        !Number.isInteger(attestation.findingsCount) ||
        attestation.findingsCount < 0
      ) {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.findingsCount",
          "findingsCount must be a non-negative integer",
        );
      }

      if (attestation.verdict === "clean" && attestation.findingsCount !== 0) {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.findingsCount",
          "a clean verdict must record zero findings",
        );
      }

      if (attestation.verdict === "findings" && attestation.findingsCount <= 0) {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.findingsCount",
          "a findings verdict must record at least one finding",
        );
      }

      const recomputedDigest = externalRepoStudyScanAttestationDigest(
        attestation,
      );
      if (attestation.attestationDigest !== recomputedDigest) {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.attestationDigest",
          "attestationDigest must match the deterministic digest of the bound fields",
        );
      }

      const expectedRef = `scan_attestation.${slugRepo(attestation.repo)}.${shortHash(attestation.attestationDigest)}.v0`;
      if (attestation.attestationRef !== expectedRef) {
        return yield* registryError(
          "externalRepoStudyScanAttestationRegistry.attestation.attestationRef",
          "attestationRef must be derived from the repo slug and attestation digest",
        );
      }
    }

    if (
      registry.registryHash !==
      openAgentsExternalRepoStudyScanAttestationRegistryHash(registry)
    ) {
      return yield* registryError(
        "externalRepoStudyScanAttestationRegistry.registryHash",
        "must match the deterministic registry hash",
      );
    }
  });
}

function requireNonEmpty(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? registryError(path, "must be non-empty")
    : Effect.void;
}

function requireSha256(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:")
    ? Effect.void
    : registryError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function registryError(
  path: string,
  reason: string,
): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

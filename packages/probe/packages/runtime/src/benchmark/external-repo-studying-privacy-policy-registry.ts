import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  PRIVACY_REVIEW_MAX_RETENTION_DAYS,
  PrivacyReviewAllowedPiiCategory,
} from "./external-repo-studying-privacy-review";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Published customer-data privacy policy REGISTRY for the external-repo-studying
 * pilot.
 *
 * This module is the smallest genuine piece of the missing privacy POLICY for
 * autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_privacy_policy_missing). The
 * sibling privacy-review preflight already checks that a study's
 * `dataProcessingAgreementRef` / `retentionPolicyRef` are PRESENT, but until now
 * those refs pointed at nothing — any non-empty string passed, and there was no
 * published, canonical policy text for a customer (or a reviewer) to read.
 *
 * This module publishes the canonical policy as a deterministic, content-hashed,
 * versioned reference. The human-readable policy text lives at
 * docs/legal/external-repo-studying-privacy-policy.v0.md; this registry is its
 * machine-readable mirror plus a `termsDigest` over the structured terms. It
 * also exposes a checker so a privacy-review request's policy ref can be matched
 * against a KNOWN published version rather than trusting an arbitrary string —
 * the same forgeable-string seam the upload<->privacy binding closes elsewhere.
 *
 * The published caps mirror what the privacy-review preflight actually enforces
 * (PRIVACY_REVIEW_MAX_RETENTION_DAYS and the closed PII-category set), so the
 * published policy stays in lockstep with the code. A test asserts they match.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/counts/enums ONLY. sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: publishing policy text grants no clearance and
 *    applies no effect. effectsApplied is always false.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires a real, human/legal review
 * against a real customer study, durable access-controlled storage that enforces
 * the declared retention window, and an owner-signed armed clearance with a
 * dereferenceable closeout receipt per proof.claim_upgrade_receipts.v1 — all
 * owner-gated and out of scope for the published registry built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_PRIVACY_POLICY_REGISTRY_SCHEMA_REF =
  "openagents.external_repo_study_privacy_policy_registry.v0" as const;

/** The canonical published policy ref a study's DPA / retention ref should
 *  reference. The human-readable text lives at the documentPath below. */
export const EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF =
  "policy.external_repo_study_privacy.v0" as const;

export const EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_PATH =
  "docs/legal/external-repo-studying-privacy-policy.v0.md" as const;

/**
 * Content digest (`sha256:…`) of the canonical human-readable policy document at
 * EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_PATH. This PINS the exact legal
 * text a customer reads to the machine-readable registry: if the document drifts
 * from this digest (or vice versa), the registry no longer matches the published
 * policy. A test reads the on-disk document, recomputes the digest with
 * `externalRepoStudyPrivacyPolicyDocumentDigest`, and asserts it equals this
 * value, so CI catches any doc<->registry drift.
 */
export const EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_DIGEST =
  "sha256:84235952c90ebd4cab57f98a241fffe6654d3a50d1ebf49bbe67e3b1f3b5d510" as const;

/**
 * Deterministic content digest of a privacy-policy document's raw text. The
 * registry pins the canonical document with this digest so the human-readable
 * legal text cannot silently drift from the machine-readable published terms.
 */
export function externalRepoStudyPrivacyPolicyDocumentDigest(
  documentText: string,
): string {
  return sha256Ref(documentText);
}

/**
 * The structured terms of a published policy version, expressed as refs, counts,
 * and enums only. The deterministic `termsDigest` is sha256 over this object, so
 * a change to the published terms changes the digest.
 */
export const OpenAgentsExternalRepoStudyPrivacyPolicyTerms = S.Struct({
  allowedPiiCategories: S.Array(PrivacyReviewAllowedPiiCategory),
  customerAuthorizationRequired: S.Literal(true),
  dataProcessingAgreementRequired: S.Literal(true),
  inertByDefault: S.Literal(true),
  maxRetentionDays: S.Number,
  sourceBoundary: S.Literal("customer_refs_withheld"),
});
export type OpenAgentsExternalRepoStudyPrivacyPolicyTerms =
  typeof OpenAgentsExternalRepoStudyPrivacyPolicyTerms.Type;

export const OpenAgentsExternalRepoStudyPrivacyPolicyVersion = S.Struct({
  documentDigest: S.String,
  documentPath: S.String,
  effectiveDate: S.String,
  policyRef: S.String,
  status: S.Literal("published_inert"),
  terms: OpenAgentsExternalRepoStudyPrivacyPolicyTerms,
  termsDigest: S.String,
  version: S.String,
});
export type OpenAgentsExternalRepoStudyPrivacyPolicyVersion =
  typeof OpenAgentsExternalRepoStudyPrivacyPolicyVersion.Type;

export const OpenAgentsExternalRepoStudyPrivacyPolicyRegistry = S.Struct({
  customerPublicClaimAllowed: S.Literal(false),
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  payoutEligible: S.Literal(false),
  publishedVersions: S.Array(OpenAgentsExternalRepoStudyPrivacyPolicyVersion),
  registryHash: S.String,
  registryRef: S.String,
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_PRIVACY_POLICY_REGISTRY_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  unsafeCopyRefs: S.Array(S.String),
});
export type OpenAgentsExternalRepoStudyPrivacyPolicyRegistry =
  typeof OpenAgentsExternalRepoStudyPrivacyPolicyRegistry.Type;

export interface BuildOpenAgentsExternalRepoStudyPrivacyPolicyRegistryInput {
  readonly generatedAt?: string;
}

/**
 * The published policy terms for v0. These mirror the caps the privacy-review
 * preflight enforces (PRIVACY_REVIEW_MAX_RETENTION_DAYS + the closed PII set).
 */
const PRIVACY_POLICY_V0_TERMS: OpenAgentsExternalRepoStudyPrivacyPolicyTerms = {
  allowedPiiCategories: [
    "none",
    "contributor_handle",
    "commit_author_email",
    "code_comment_text",
  ],
  customerAuthorizationRequired: true,
  dataProcessingAgreementRequired: true,
  inertByDefault: true,
  maxRetentionDays: PRIVACY_REVIEW_MAX_RETENTION_DAYS,
  sourceBoundary: "customer_refs_withheld",
};

export function buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry(
  input: BuildOpenAgentsExternalRepoStudyPrivacyPolicyRegistryInput = {},
): Effect.Effect<
  OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const v0: OpenAgentsExternalRepoStudyPrivacyPolicyVersion = {
      documentDigest: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_DIGEST,
      documentPath: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_PATH,
      effectiveDate: "2026-06-20",
      policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
      status: "published_inert",
      terms: PRIVACY_POLICY_V0_TERMS,
      termsDigest: sha256Ref(stableJson(PRIVACY_POLICY_V0_TERMS)),
      version: "v0",
    };

    const publishedVersions = [v0];

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_privacy_policy_registry_hash";

    const evidenceRefs = [
      ...publishedVersions.map((version) => version.policyRef),
      ...publishedVersions.map((version) => version.documentPath),
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyPrivacyPolicyRegistry = {
      customerPublicClaimAllowed: false,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      marketplacePackageAllowed: false,
      payoutEligible: false,
      publishedVersions,
      registryHash: "sha256:pending",
      registryRef: "external_repo_study_privacy_policy_registry.pending",
      safeCopy:
        "Privacy policy registry publishes the canonical, content-hashed external-repo-studying customer-data privacy policy versions as refs, counts, and enums only. Publishing policy text grants no clearance and ingests no repository; the pilot stays inert and gated, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef:
        OPENAGENTS_EXTERNAL_REPO_STUDY_PRIVACY_POLICY_REGISTRY_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      unsafeCopyRefs: [
        "blocked_claim.privacy_policy_grants_clearance",
        "blocked_claim.customer_repo_data_processing_authorized",
        "blocked_claim.privacy_review_grants_ingestion",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
    };

    const registryHash =
      openAgentsExternalRepoStudyPrivacyPolicyRegistryHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({
      ...base,
      registryHash,
      registryRef: `external_repo_study_privacy_policy_registry.${shortHash(registryHash)}`,
    });
  });
}

/**
 * Whether `ref` references a KNOWN published policy version. Closes the
 * forgeable-string seam: a study's policy ref must match a published version,
 * not just be a non-empty string.
 */
export function isPublishedExternalRepoStudyPrivacyPolicyRef(
  registry: OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
  ref: string | undefined,
): boolean {
  if (ref === undefined || ref.trim().length === 0) {
    return false;
  }
  return registry.publishedVersions.some(
    (version) => version.policyRef === ref,
  );
}

/**
 * Whether `documentText` is the EXACT canonical text published for `ref`. Closes
 * the document-drift seam: a study or reviewer can verify the policy document
 * actually served matches the content the registry pinned, rather than trusting
 * that `documentPath` still points at the published text. Returns false for an
 * unknown ref or any content whose digest differs from the published one.
 */
export function isMatchingPublishedExternalRepoStudyPrivacyPolicyDocument(
  registry: OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
  ref: string | undefined,
  documentText: string,
): boolean {
  if (ref === undefined || ref.trim().length === 0) {
    return false;
  }
  const digest = externalRepoStudyPrivacyPolicyDocumentDigest(documentText);
  return registry.publishedVersions.some(
    (version) =>
      version.policyRef === ref && version.documentDigest === digest,
  );
}

export function decodeOpenAgentsExternalRepoStudyPrivacyPolicyRegistry(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyPrivacyPolicyRegistry",
    );
    const registry = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyPrivacyPolicyRegistry",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyPrivacyPolicyRegistry(registry);
    return registry;
  });
}

export function openAgentsExternalRepoStudyPrivacyPolicyRegistryHash(
  registry: OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
): string {
  const {
    registryHash: _registryHash,
    registryRef: _registryRef,
    generatedAt: _generatedAt,
    ...stable
  } = registry;
  return sha256Ref(stableJson(stable));
}

function validateExternalRepoStudyPrivacyPolicyRegistry(
  registry: OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      registry.registryRef,
      "externalRepoStudyPrivacyPolicyRegistry.registryRef",
    );
    yield* requireSha256(
      registry.registryHash,
      "externalRepoStudyPrivacyPolicyRegistry.registryHash",
    );

    if (registry.publishedVersions.length === 0) {
      return yield* registryError(
        "externalRepoStudyPrivacyPolicyRegistry.publishedVersions",
        "at least one published policy version is required",
      );
    }

    if (
      registry.customerPublicClaimAllowed !== false ||
      registry.marketplacePackageAllowed !== false ||
      registry.payoutEligible !== false ||
      registry.effectsApplied !== false
    ) {
      return yield* registryError(
        "externalRepoStudyPrivacyPolicyRegistry.claimGates",
        "policy registry must not grant clearance or customer, marketplace, payout, or settlement claims",
      );
    }

    const seen = new Set<string>();
    for (const version of registry.publishedVersions) {
      yield* requireNonEmpty(
        version.policyRef,
        "externalRepoStudyPrivacyPolicyRegistry.version.policyRef",
      );
      yield* requireNonEmpty(
        version.documentPath,
        "externalRepoStudyPrivacyPolicyRegistry.version.documentPath",
      );
      yield* requireSha256(
        version.termsDigest,
        "externalRepoStudyPrivacyPolicyRegistry.version.termsDigest",
      );
      yield* requireSha256(
        version.documentDigest,
        "externalRepoStudyPrivacyPolicyRegistry.version.documentDigest",
      );

      if (seen.has(version.policyRef)) {
        return yield* registryError(
          "externalRepoStudyPrivacyPolicyRegistry.version.policyRef",
          "published policy refs must be unique",
        );
      }
      seen.add(version.policyRef);

      if (version.terms.maxRetentionDays <= 0) {
        return yield* registryError(
          "externalRepoStudyPrivacyPolicyRegistry.version.terms.maxRetentionDays",
          "published retention cap must be positive",
        );
      }

      if (version.terms.allowedPiiCategories.length === 0) {
        return yield* registryError(
          "externalRepoStudyPrivacyPolicyRegistry.version.terms.allowedPiiCategories",
          "published policy must declare at least one allowed PII category",
        );
      }

      const recomputed = sha256Ref(stableJson(version.terms));
      if (version.termsDigest !== recomputed) {
        return yield* registryError(
          "externalRepoStudyPrivacyPolicyRegistry.version.termsDigest",
          "termsDigest must match the deterministic digest of the published terms",
        );
      }
    }

    if (
      registry.registryHash !==
      openAgentsExternalRepoStudyPrivacyPolicyRegistryHash(registry)
    ) {
      return yield* registryError(
        "externalRepoStudyPrivacyPolicyRegistry.registryHash",
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

function registryError(
  path: string,
  reason: string,
): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

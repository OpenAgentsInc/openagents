import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Customer-AUTHORIZATION registry for the external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing privacy POLICY control
 * surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_privacy_policy_missing): it
 * closes the forgeable-string seam on the LAWFUL-BASIS anchor, the
 * `customerAuthorizationRef`.
 *
 * Section 3 ("Lawful basis and authorization") of the published privacy policy
 * (docs/legal/external-repo-studying-privacy-policy.v0.md) states OpenAgents
 * processes a customer's external repository ONLY with the customer's recorded
 * authorization (`customerAuthorizationRef`). Section 6 ("Data subject rights")
 * states a customer may WITHDRAW that authorization, which halts further study.
 * Until now, however, every control-surface gate that consumes
 * `customerAuthorizationRef` (the privacy-review, self-serve upload, and
 * data-subject-request preflights) checked only that the ref was a non-empty
 * STRING: any string passed, including a forged or stale one, and nothing modeled
 * withdrawal. A policy that requires (and lets a customer revoke) authorization
 * but cannot tell a real authorization from any string is incomplete.
 *
 * This registry closes that gap, mirroring the scan-attestation and privacy-policy
 * registries. It records ISSUED customer authorizations as refs/enums/dates only,
 * each bound to a SPECIFIC (customerRef, repo) and pinned with a deterministic
 * `authorizationDigest`, and exposes `isActiveCustomerAuthorizationRef(...)` so a
 * verifier can require an authorization ref to match a KNOWN, ACTIVE authorization
 * for THAT exact customer + repo — not merely be a non-empty string. A WITHDRAWN
 * authorization is recorded but never verifies, modeling Section 6 revocation.
 *
 * It does NOT obtain consent, process customer data, or read repo bytes.
 * Recording an authorization here only mirrors a customer grant captured
 * elsewhere; it grants no ingestion.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/enums/dates ONLY. No raw repo content, PII value, signed-document body,
 *    or customer identity ever crosses this boundary.
 *    sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: recording an authorization grants no ingestion and
 *    applies no effect. effectsApplied is always false.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires a real, human/legal review against
 * a real customer study, durable access-controlled storage, real revocation
 * enforcement, and an owner-signed armed clearance with a dereferenceable closeout
 * receipt per proof.claim_upgrade_receipts.v1 — all owner-gated and out of scope
 * for the registry built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_CUSTOMER_AUTHORIZATION_REGISTRY_SCHEMA_REF =
  "openagents.external_repo_study_customer_authorization_registry.v0" as const;

/**
 * The only scope this pilot's authorizations may grant: studying the named
 * external repo. Any other scope is rejected at record time.
 */
export const OpenAgentsExternalRepoStudyAuthorizationScope = S.Literals([
  "external_repo_study",
]);
export type OpenAgentsExternalRepoStudyAuthorizationScope =
  typeof OpenAgentsExternalRepoStudyAuthorizationScope.Type;

/**
 * Authorization status. `active` authorizations are the lawful basis for a study;
 * `withdrawn` records a Section-6 revocation and never verifies as active.
 */
export const OpenAgentsExternalRepoStudyAuthorizationStatus = S.Literals([
  "active",
  "withdrawn",
]);
export type OpenAgentsExternalRepoStudyAuthorizationStatus =
  typeof OpenAgentsExternalRepoStudyAuthorizationStatus.Type;

/**
 * A customer authorization recorded in the registry, expressed as refs/enums/
 * dates only. The deterministic `authorizationDigest` is sha256 over the bound
 * fields, so a change to any of them changes the digest and the derived ref.
 */
export const OpenAgentsExternalRepoStudyCustomerAuthorization = S.Struct({
  authorizationDigest: S.String,
  authorizationRef: S.String,
  customerRef: S.String,
  effectiveDate: S.String,
  grantRef: S.String,
  repo: S.String,
  scope: OpenAgentsExternalRepoStudyAuthorizationScope,
  status: OpenAgentsExternalRepoStudyAuthorizationStatus,
});
export type OpenAgentsExternalRepoStudyCustomerAuthorization =
  typeof OpenAgentsExternalRepoStudyCustomerAuthorization.Type;

export const OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry = S.Struct({
  customerPublicClaimAllowed: S.Literal(false),
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  payoutEligible: S.Literal(false),
  recordedAuthorizations: S.Array(
    OpenAgentsExternalRepoStudyCustomerAuthorization,
  ),
  registryHash: S.String,
  registryRef: S.String,
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_CUSTOMER_AUTHORIZATION_REGISTRY_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  unsafeCopyRefs: S.Array(S.String),
});
export type OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry =
  typeof OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry.Type;

/**
 * The bound fields of a customer authorization, supplied by the caller. The
 * registry derives `authorizationDigest` and `authorizationRef` from these
 * deterministically.
 */
export interface ExternalRepoStudyCustomerAuthorizationInput {
  readonly customerRef: string;
  readonly repo: string;
  /** Stable ref of the customer's signed authorization grant (refs only). */
  readonly grantRef: string;
  readonly scope: OpenAgentsExternalRepoStudyAuthorizationScope;
  readonly status: OpenAgentsExternalRepoStudyAuthorizationStatus;
  /** ISO date the authorization (or its withdrawal) took effect. */
  readonly effectiveDate: string;
}

export interface BuildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistryInput {
  readonly authorizations?: ReadonlyArray<ExternalRepoStudyCustomerAuthorizationInput>;
  readonly generatedAt?: string;
}

/**
 * The fields a customer-authorization digest binds. Recomputing this from a
 * verifier's own view is how forgery and drift are detected.
 */
function customerAuthorizationDigestSource(
  authorization: ExternalRepoStudyCustomerAuthorizationInput,
): {
  readonly customerRef: string;
  readonly effectiveDate: string;
  readonly grantRef: string;
  readonly repo: string;
  readonly scope: OpenAgentsExternalRepoStudyAuthorizationScope;
  readonly status: OpenAgentsExternalRepoStudyAuthorizationStatus;
} {
  return {
    customerRef: authorization.customerRef,
    effectiveDate: authorization.effectiveDate,
    grantRef: authorization.grantRef,
    repo: authorization.repo,
    scope: authorization.scope,
    status: authorization.status,
  };
}

/**
 * Deterministic content digest of a customer authorization's bound fields. The
 * registry pins each authorization with this digest so a recorded authorization
 * cannot silently drift from the customer/repo/grant/status it actually covered.
 */
export function externalRepoStudyCustomerAuthorizationDigest(
  authorization: ExternalRepoStudyCustomerAuthorizationInput,
): string {
  return sha256Ref(stableJson(customerAuthorizationDigestSource(authorization)));
}

export function buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry(
  input: BuildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistryInput = {},
): Effect.Effect<
  OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const inputs = input.authorizations ?? [];

    const recordedAuthorizations: OpenAgentsExternalRepoStudyCustomerAuthorization[] =
      [];
    for (const authorization of inputs) {
      yield* requireNonEmpty(
        authorization.customerRef,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.customerRef",
      );
      yield* requireNonEmpty(
        authorization.repo,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.repo",
      );
      yield* requireNonEmpty(
        authorization.grantRef,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.grantRef",
      );
      yield* requireNonEmpty(
        authorization.effectiveDate,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.effectiveDate",
      );

      if (authorization.repo === "OpenAgentsInc/openagents") {
        return yield* registryError(
          "externalRepoStudyCustomerAuthorizationRegistry.authorization.repo",
          "authorization target must be an external (non-OpenAgents) pilot repo",
        );
      }

      const authorizationDigest =
        externalRepoStudyCustomerAuthorizationDigest(authorization);
      recordedAuthorizations.push({
        authorizationDigest,
        authorizationRef: `customer_authorization.${slugRepo(authorization.repo)}.${shortHash(authorizationDigest)}.v0`,
        customerRef: authorization.customerRef,
        effectiveDate: authorization.effectiveDate,
        grantRef: authorization.grantRef,
        repo: authorization.repo,
        scope: authorization.scope,
        status: authorization.status,
      });
    }

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_customer_authorization_registry_hash";

    const evidenceRefs = [
      ...recordedAuthorizations.map(
        (authorization) => authorization.authorizationRef,
      ),
      "docs/legal/external-repo-studying-privacy-policy.v0.md",
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry = {
      customerPublicClaimAllowed: false,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      marketplacePackageAllowed: false,
      payoutEligible: false,
      recordedAuthorizations,
      registryHash: "sha256:pending",
      registryRef: "external_repo_study_customer_authorization_registry.pending",
      safeCopy:
        "Customer authorization registry records issued lawful-basis grants for external-repo studies as refs, enums, and dates only, each bound to a specific customer and repo. Recording (or withdrawing) an authorization obtains no consent, processes no customer data, reads no repo bytes, and grants no ingestion; the pilot stays inert and gated, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef:
        OPENAGENTS_EXTERNAL_REPO_STUDY_CUSTOMER_AUTHORIZATION_REGISTRY_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      unsafeCopyRefs: [
        "blocked_claim.customer_authorization_forgeable_string",
        "blocked_claim.customer_repo_data_processing_authorized",
        "blocked_claim.privacy_review_grants_ingestion",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
    };

    const registryHash =
      openAgentsExternalRepoStudyCustomerAuthorizationRegistryHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
      ...base,
      registryHash,
      registryRef: `external_repo_study_customer_authorization_registry.${shortHash(registryHash)}`,
    });
  });
}

/**
 * Whether `ref` references a KNOWN, ACTIVE customer authorization covering THE
 * SAME customer + repo. Closes the forgeable-string seam on the lawful-basis
 * anchor: a study's `customerAuthorizationRef` must match a recorded ACTIVE
 * authorization for that exact customer + repo, not just be non-empty.
 *
 * A WITHDRAWN authorization (Section 6 revocation), an unknown/empty ref, or any
 * customer/repo mismatch returns false.
 */
export function isActiveCustomerAuthorizationRef(
  registry: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
  ref: string | undefined,
  match: {
    readonly customerRef: string;
    readonly repo: string;
  },
): boolean {
  if (ref === undefined || ref.trim().length === 0) {
    return false;
  }
  if (match.customerRef.trim().length === 0 || match.repo.trim().length === 0) {
    return false;
  }
  return registry.recordedAuthorizations.some(
    (authorization) =>
      authorization.authorizationRef === ref &&
      authorization.status === "active" &&
      authorization.scope === "external_repo_study" &&
      authorization.customerRef === match.customerRef &&
      authorization.repo === match.repo,
  );
}

export function decodeOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyCustomerAuthorizationRegistry",
    );
    const registry = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyCustomerAuthorizationRegistry",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyCustomerAuthorizationRegistry(registry);
    return registry;
  });
}

export function openAgentsExternalRepoStudyCustomerAuthorizationRegistryHash(
  registry: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
): string {
  const {
    registryHash: _registryHash,
    registryRef: _registryRef,
    generatedAt: _generatedAt,
    ...stable
  } = registry;
  return sha256Ref(stableJson(stable));
}

function validateExternalRepoStudyCustomerAuthorizationRegistry(
  registry: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      registry.registryRef,
      "externalRepoStudyCustomerAuthorizationRegistry.registryRef",
    );
    yield* requireSha256(
      registry.registryHash,
      "externalRepoStudyCustomerAuthorizationRegistry.registryHash",
    );

    if (
      registry.customerPublicClaimAllowed !== false ||
      registry.marketplacePackageAllowed !== false ||
      registry.payoutEligible !== false ||
      registry.effectsApplied !== false
    ) {
      return yield* registryError(
        "externalRepoStudyCustomerAuthorizationRegistry.claimGates",
        "authorization registry must not grant ingestion or customer, marketplace, payout, or settlement claims",
      );
    }

    const seenRefs = new Set<string>();
    for (const authorization of registry.recordedAuthorizations) {
      yield* requireNonEmpty(
        authorization.authorizationRef,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.authorizationRef",
      );
      yield* requireNonEmpty(
        authorization.customerRef,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.customerRef",
      );
      yield* requireNonEmpty(
        authorization.repo,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.repo",
      );
      yield* requireNonEmpty(
        authorization.grantRef,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.grantRef",
      );
      yield* requireNonEmpty(
        authorization.effectiveDate,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.effectiveDate",
      );
      yield* requireSha256(
        authorization.authorizationDigest,
        "externalRepoStudyCustomerAuthorizationRegistry.authorization.authorizationDigest",
      );

      if (authorization.repo === "OpenAgentsInc/openagents") {
        return yield* registryError(
          "externalRepoStudyCustomerAuthorizationRegistry.authorization.repo",
          "authorization target must be an external (non-OpenAgents) pilot repo",
        );
      }

      if (seenRefs.has(authorization.authorizationRef)) {
        return yield* registryError(
          "externalRepoStudyCustomerAuthorizationRegistry.authorization.authorizationRef",
          "recorded authorization refs must be unique",
        );
      }
      seenRefs.add(authorization.authorizationRef);

      const recomputedDigest =
        externalRepoStudyCustomerAuthorizationDigest(authorization);
      if (authorization.authorizationDigest !== recomputedDigest) {
        return yield* registryError(
          "externalRepoStudyCustomerAuthorizationRegistry.authorization.authorizationDigest",
          "authorizationDigest must match the deterministic digest of the bound fields",
        );
      }

      const expectedRef = `customer_authorization.${slugRepo(authorization.repo)}.${shortHash(authorization.authorizationDigest)}.v0`;
      if (authorization.authorizationRef !== expectedRef) {
        return yield* registryError(
          "externalRepoStudyCustomerAuthorizationRegistry.authorization.authorizationRef",
          "authorizationRef must be derived from the repo slug and authorization digest",
        );
      }
    }

    if (
      registry.registryHash !==
      openAgentsExternalRepoStudyCustomerAuthorizationRegistryHash(registry)
    ) {
      return yield* registryError(
        "externalRepoStudyCustomerAuthorizationRegistry.registryHash",
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
